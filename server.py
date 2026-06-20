import os, json, subprocess, threading, queue, time, socket
from flask import Flask, request, Response, stream_with_context, render_template

app = Flask(__name__)
PORT, HOST = 5000, "0.0.0.0"

SYSTEM_PROMPT = """You are Ravens — an elite OSINT AI analyst powered by NVIDIA NIM Nemotron-Ultra.
Analyze all data found by automated OSINT tools and deliver a comprehensive intelligence report.

Structure your report as follows:
1. EXECUTIVE SUMMARY — key findings in 3-5 sentences
2. IDENTITY PROFILE — name, aliases, known accounts
3. DIGITAL FOOTPRINT — platforms, usernames, emails found
4. NETWORK CONNECTIONS — links between platforms and identities
5. GEOLOCATION & INFRASTRUCTURE — IPs, domains, servers
6. CREDIBILITY SCORE — HIGH/MEDIUM/LOW with justification
7. FURTHER INVESTIGATION — next recommended steps
8. RISK ASSESSMENT — potential exposure level

Always work only with publicly available information. Respond in Russian."""

UNBUF_ENV = {**os.environ, 'PYTHONUNBUFFERED': '1', 'NO_COLOR': '1'}

def run_sherlock(username, q):
    try:
        q.put({"module":"sherlock","type":"running","text":f"Sherlock: поиск '{username}' на 400+ сайтах..."})
        proc = subprocess.Popen(
            ["sherlock", username, "--timeout", "8", "--print-found", "--no-color"],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1, env=UNBUF_ENV)
        found = []
        for line in proc.stdout:
            line = line.strip()
            if line.startswith("[+]"):
                site = line.replace("[+]","").strip()
                found.append(site)
                q.put({"module":"sherlock","type":"found","text":site})
            elif line.startswith("[*]") and "Searching" not in line:
                q.put({"module":"sherlock","type":"info","text":line[3:].strip()})
        proc.wait(timeout=90)
        q.put({"module":"sherlock","type":"done","text":f"Sherlock: найдено {len(found)} аккаунтов"})
    except Exception as e:
        q.put({"module":"sherlock","type":"error","text":f"Sherlock: {e}"})
        q.put({"module":"sherlock","type":"done","text":"Sherlock: завершён с ошибкой"})

def run_maigret(username, q):
    try:
        q.put({"module":"maigret","type":"running","text":f"Maigret: анализ профилей '{username}'..."})
        proc = subprocess.Popen(
            ["maigret", username, "--no-color", "-a", "--timeout", "8"],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1, env=UNBUF_ENV)
        found = []
        for line in proc.stdout:
            line = line.strip()
            if "[+]" in line:
                found.append(line)
                q.put({"module":"maigret","type":"found","text":line.replace("[+]","").strip()})
            elif line and not line.startswith("[") and len(line) > 5:
                q.put({"module":"maigret","type":"info","text":line})
        proc.wait(timeout=120)
        q.put({"module":"maigret","type":"done","text":f"Maigret: {len(found)} профилей"})
    except Exception as e:
        q.put({"module":"maigret","type":"error","text":f"Maigret: {e}"})
        q.put({"module":"maigret","type":"done","text":"Maigret: завершён с ошибкой"})

def run_holehe(email, q):
    try:
        q.put({"module":"holehe","type":"running","text":f"Holehe: проверка '{email}' на 120+ сервисах..."})
        proc = subprocess.Popen(
            ["holehe", email, "--no-color"],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1, env=UNBUF_ENV)
        found = []
        for line in proc.stdout:
            line = line.strip()
            if "[+]" in line:
                found.append(line)
                q.put({"module":"holehe","type":"found","text":line.replace("[+]","").strip()})
        proc.wait(timeout=90)
        q.put({"module":"holehe","type":"done","text":f"Holehe: зарегистрирован на {len(found)} сервисах"})
    except Exception as e:
        q.put({"module":"holehe","type":"error","text":f"Holehe: {e}"})
        q.put({"module":"holehe","type":"done","text":"Holehe: завершён с ошибкой"})

def run_whois_dns(target, q):
    import whois, dns.resolver
    results = []
    try:
        q.put({"module":"whois","type":"running","text":f"WHOIS: запрос по '{target}'..."})
        w = whois.whois(target)
        for field, val in [("Домен", w.domain_name), ("Регистратор", w.registrar),
                           ("Создан", w.creation_date), ("Истекает", w.expiration_date),
                           ("Email", w.emails), ("Страна", w.country), ("Организация", w.org)]:
            if val:
                txt = f"{field}: {val}"
                results.append(txt)
                q.put({"module":"whois","type":"found","text":txt})
        q.put({"module":"whois","type":"done","text":f"WHOIS: {len(results)} записей"})
    except Exception as e:
        q.put({"module":"whois","type":"error","text":f"WHOIS: {e}"})
        q.put({"module":"whois","type":"done","text":"WHOIS: нет данных"})

    dns_found = 0
    try:
        q.put({"module":"dns","type":"running","text":f"DNS: резолюция '{target}'..."})
        for rtype in ["A","AAAA","MX","NS","TXT","CNAME"]:
            try:
                for r in dns.resolver.resolve(target, rtype, lifetime=5):
                    txt = f"{rtype}: {r}"
                    dns_found += 1
                    q.put({"module":"dns","type":"found","text":txt})
            except Exception:
                pass
        q.put({"module":"dns","type":"done","text":f"DNS: {dns_found} записей"})
    except Exception as e:
        q.put({"module":"dns","type":"error","text":f"DNS: {e}"})
        q.put({"module":"dns","type":"done","text":"DNS: нет данных"})

def run_ip(target, q):
    try:
        q.put({"module":"ip","type":"running","text":f"IP Lookup: '{target}'..."})
        ip = socket.gethostbyname(target)
        q.put({"module":"ip","type":"found","text":f"IP адрес: {ip}"})
        import urllib.request
        try:
            with urllib.request.urlopen(
                f"http://ip-api.com/json/{ip}?fields=country,regionName,city,isp,org,as,reverse,timezone,mobile,proxy,hosting",
                timeout=8) as r:
                data = json.loads(r.read())
            labels = {"country":"Страна","regionName":"Регион","city":"Город",
                      "isp":"Провайдер","org":"Организация","as":"AS","reverse":"Reverse DNS",
                      "timezone":"Timezone","mobile":"Mobile","proxy":"Proxy","hosting":"Hosting"}
            for k, v in data.items():
                if v and v is not False:
                    q.put({"module":"ip","type":"found","text":f"{labels.get(k,k)}: {v}"})
        except Exception:
            pass
        q.put({"module":"ip","type":"done","text":f"IP Geo: {ip}"})
    except Exception as e:
        q.put({"module":"ip","type":"error","text":f"IP: {e}"})
        q.put({"module":"ip","type":"done","text":"IP: нет данных"})

def run_dorks(target, q):
    q.put({"module":"dorks","type":"running","text":f"Google Dorks для '{target}'..."})
    time.sleep(0.2)
    dorks = [
        f'site:linkedin.com "{target}"',
        f'site:facebook.com "{target}"',
        f'site:twitter.com "{target}"',
        f'site:instagram.com "{target}"',
        f'site:vk.com "{target}"',
        f'site:github.com "{target}"',
        f'site:reddit.com "{target}"',
        f'"{target}" filetype:pdf',
        f'"{target}" inurl:profile',
        f'intitle:"{target}" site:ru',
        f'"{target}" (email OR contact OR phone)',
        f'"{target}" (password OR leak OR breach)',
        f'"{target}" site:pastebin.com',
        f'"{target}" site:haveibeenpwned.com',
    ]
    for d in dorks:
        q.put({"module":"dorks","type":"found","text":d})
        time.sleep(0.08)
    q.put({"module":"dorks","type":"done","text":f"Dorks: {len(dorks)} запросов сгенерировано"})

def run_ai_analysis(target, collected, q):
    q.put({"module":"ai","type":"running","text":"Ravens AI: анализирую данные через NVIDIA NIM Nemotron-Ultra..."})
    api_key = os.environ.get("NVIDIA_NIM_API_KEY","")
    if not api_key:
        q.put({"module":"ai","type":"error","text":"AI: API ключ NVIDIA_NIM_API_KEY не настроен"})
        q.put({"module":"ai","type":"done","text":"AI: ошибка авторизации"})
        return
    try:
        from openai import OpenAI
        client = OpenAI(base_url="https://integrate.api.nvidia.com/v1", api_key=api_key)
        findings_text = "\n".join(collected[:200]) if collected else "Данные OSINT не найдены — выполни общий анализ угроз по цели."
        msgs = [
            {"role":"system","content":SYSTEM_PROMPT},
            {"role":"user","content":f"ЦЕЛЬ РАССЛЕДОВАНИЯ: {target}\n\nНАЙДЕННЫЕ ДАННЫЕ OSINT ({len(collected)} записей):\n{findings_text}\n\nСоставь полный разведывательный отчёт."}
        ]
        comp = client.chat.completions.create(
            model="nvidia/nemotron-3-ultra-550b-a55b",
            messages=msgs,
            temperature=0.6,
            max_tokens=6000,
            stream=True
        )
        for chunk in comp:
            if not chunk.choices:
                continue
            c = chunk.choices[0].delta.content
            if c:
                q.put({"module":"ai","type":"stream","text":c})
        q.put({"module":"ai","type":"done","text":"Ravens AI: анализ завершён"})
    except Exception as e:
        q.put({"module":"ai","type":"error","text":f"AI: {e}"})
        q.put({"module":"ai","type":"done","text":"AI: завершён с ошибкой"})

@app.route('/')
def index():
    from flask import make_response
    resp = make_response(render_template('index.html'))
    resp.headers['Cache-Control'] = 'no-store, no-cache, must-revalidate, max-age=0'
    resp.headers['Pragma'] = 'no-cache'
    return resp

@app.route('/investigate', methods=['POST'])
def investigate():
    data   = request.get_json(force=True, silent=True) or {}
    name   = data.get('name','').strip()
    user   = data.get('user','').strip()
    email  = data.get('email','').strip()
    domain = data.get('domain','').strip()

    q = queue.Queue()
    threads = []

    # Sherlock
    if user:
        t = threading.Thread(target=run_sherlock, args=(user, q), daemon=True)
        threads.append(t); t.start()
    else:
        q.put({"module":"sherlock","type":"info","text":"Sherlock: username не указан — пропуск"})
        q.put({"module":"sherlock","type":"done","text":"Sherlock: пропущен"})

    # Maigret
    if user or name:
        slug = (user or name).replace(' ','_').lower()
        t = threading.Thread(target=run_maigret, args=(slug, q), daemon=True)
        threads.append(t); t.start()
    else:
        q.put({"module":"maigret","type":"info","text":"Maigret: нет имени/username — пропуск"})
        q.put({"module":"maigret","type":"done","text":"Maigret: пропущен"})

    # Holehe
    if email:
        t = threading.Thread(target=run_holehe, args=(email, q), daemon=True)
        threads.append(t); t.start()
    else:
        q.put({"module":"holehe","type":"info","text":"Holehe: email не указан — пропуск"})
        q.put({"module":"holehe","type":"done","text":"Holehe: пропущен"})

    # WHOIS + DNS
    target_d = domain or (email.split('@')[1] if '@' in email else '')
    if target_d:
        t = threading.Thread(target=run_whois_dns, args=(target_d, q), daemon=True)
        threads.append(t); t.start()
    else:
        q.put({"module":"whois","type":"info","text":"WHOIS: домен не указан — пропуск"})
        q.put({"module":"whois","type":"done","text":"WHOIS: пропущен"})
        q.put({"module":"dns","type":"done","text":"DNS: пропущен"})

    # IP Geo
    if target_d:
        t2 = threading.Thread(target=run_ip, args=(target_d, q), daemon=True)
        threads.append(t2); t2.start()
    else:
        q.put({"module":"ip","type":"info","text":"IP: домен не указан — пропуск"})
        q.put({"module":"ip","type":"done","text":"IP: пропущен"})

    # Google Dorks — always run
    dork_t = name or user or email or domain
    if dork_t:
        t = threading.Thread(target=run_dorks, args=(dork_t, q), daemon=True)
        threads.append(t); t.start()
    else:
        q.put({"module":"dorks","type":"done","text":"Dorks: нет цели"})

    def stream():
        yield ': ravens-init\n\n'
        collected = []
        deadline = time.time() + 240
        while time.time() < deadline:
            try:
                ev = q.get(timeout=0.25)
                yield 'data: ' + json.dumps(ev, ensure_ascii=False) + '\n\n'
                if ev.get('type') == 'found':
                    collected.append(ev.get('text',''))
            except queue.Empty:
                yield ': hb\n\n'
                if all(not t.is_alive() for t in threads) and q.empty():
                    break

        # AI always runs
        ai_q = queue.Queue()
        summary = ' | '.join(filter(None, [name, user, email, domain]))
        ai_t = threading.Thread(target=run_ai_analysis, args=(summary or 'неизвестная цель', collected, ai_q), daemon=True)
        ai_t.start()
        deadline2 = time.time() + 180
        while time.time() < deadline2:
            try:
                ev = ai_q.get(timeout=0.25)
                yield 'data: ' + json.dumps(ev, ensure_ascii=False) + '\n\n'
            except queue.Empty:
                yield ': hb\n\n'
                if not ai_t.is_alive() and ai_q.empty():
                    break

        yield 'data: [DONE]\n\n'

    return Response(stream_with_context(stream()), headers={
        'Content-Type': 'text/event-stream',
        'Cache-Control': 'no-cache',
        'X-Accel-Buffering': 'no',
        'Connection': 'keep-alive'
    })

@app.route('/chat', methods=['POST'])
def chat():
    data = request.get_json(force=True, silent=True) or {}
    messages = data.get('messages', [])
    api_key = os.environ.get('NVIDIA_NIM_API_KEY','')

    from openai import OpenAI
    client = OpenAI(base_url="https://integrate.api.nvidia.com/v1", api_key=api_key)

    def generate():
        yield ': ravens-chat\n\n'
        try:
            if not api_key:
                yield 'data: ' + json.dumps({"type":"content","content":"[ОШИБКА] API ключ не настроен."}) + '\n\n'
                yield 'data: [DONE]\n\n'; return
            comp = client.chat.completions.create(
                model="nvidia/nemotron-3-ultra-550b-a55b",
                messages=[{"role":"system","content":SYSTEM_PROMPT}]+messages,
                temperature=0.8, top_p=0.95, max_tokens=4096, stream=True)
            for chunk in comp:
                if not chunk.choices: continue
                c = chunk.choices[0].delta.content
                if c: yield 'data: ' + json.dumps({"type":"content","content":c}, ensure_ascii=False) + '\n\n'
            yield 'data: [DONE]\n\n'
        except Exception as e:
            yield 'data: ' + json.dumps({"type":"content","content":f"[ОШИБКА] {e}"}) + '\n\n'
            yield 'data: [DONE]\n\n'

    return Response(stream_with_context(generate()), headers={
        'Content-Type':'text/event-stream','Cache-Control':'no-cache',
        'X-Accel-Buffering':'no','Connection':'keep-alive'})

@app.route('/favicon.ico')
def favicon():
    return Response(status=204)

if __name__ == '__main__':
    print(f"Ravens OSINT: http://{HOST}:{PORT}")
    app.run(host=HOST, port=PORT, debug=False, threaded=True)
