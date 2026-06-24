import os, json, subprocess, threading, queue, time, socket, re
import urllib.request, urllib.error, urllib.parse
from flask import Flask, request, Response, stream_with_context, render_template

app = Flask(__name__)
PORT, HOST = 5000, "0.0.0.0"

SYSTEM_PROMPT = """Ты — RAVENS NEXUS, элитная AI-разведсистема уровня ФСБ/ФБР/Интерпол.

ТВОЯ ЕДИНСТВЕННАЯ ЗАДАЧА: полный пробив личности на основе OSINT-данных.
Ты не выполняешь никаких других задач кроме разведки личностей.

ОБЯЗАТЕЛЬНАЯ СТРУКТУРА ОТВЕТА (строго соблюдать):

━━━ [ИДЕНТИФИКАЦИЯ ЛИЧНОСТИ] ━━━
• Полное имя, псевдонимы, никнеймы
• Предполагаемый возраст/дата рождения
• Гражданство, язык

━━━ [ЦИФРОВОЙ СЛЕД] ━━━
• Все найденные аккаунты и платформы
• Активные/неактивные профили
• Дата создания аккаунтов где доступно

━━━ [ДАННЫЕ УТЕЧЕК] ━━━
• Базы данных где засветился
• Какие данные утекли (email, пароль, телефон)
• Временные метки утечек

━━━ [ИНФРАСТРУКТУРА] ━━━
• IP-адреса, домены, серверы
• Провайдер, геолокация
• DNS-записи

━━━ [СОЦИАЛЬНЫЕ СВЯЗИ] ━━━
• Связи между аккаунтами
• Общие данные на разных платформах
• Паттерны поведения

━━━ [КЛЮЧЕВЫЕ НАХОДКИ] ━━━  <--- ВЫДЕЛИТЬ ЖИРНЫМ/★
• ТОП-5 самых важных фактов
• Потенциально опасные данные
• Уникальные идентификаторы

━━━ [ПОЧЕМУ НЕ СРАБОТАЛО] ━━━
• По каждому инструменту без данных — объясни ПОЧЕМУ
• Возможные причины: нет аккаунта / приватный / требует авторизацию / блокировка
• Что можно попробовать дополнительно

━━━ [ОЦЕНКА РИСКОВ] ━━━
• Уровень угрозы: КРИТИЧЕСКИЙ / ВЫСОКИЙ / СРЕДНИЙ / НИЗКИЙ
• Уязвимости личности
• Степень публичности данных

━━━ [СЛЕДУЮЩИЕ ШАГИ] ━━━
• Конкретные рекомендации для продолжения расследования
• Инструменты которые стоит использовать
• На что обратить внимание

━━━ [ЗАКЛЮЧЕНИЕ] ━━━
• Уверенность в данных: XX%
• Итоговый вердикт

Работай только с публично доступными данными. Максимальная точность. Без домыслов."""

UNBUF_ENV = {**os.environ, 'PYTHONUNBUFFERED': '1', 'NO_COLOR': '1'}

# ──────────────────────────────────────────────────────────────
# OSINT MODULE 1: SHERLOCK
# ──────────────────────────────────────────────────────────────
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
        proc.wait(timeout=90)
        q.put({"module":"sherlock","type":"done","text":f"Sherlock: {len(found)} аккаунтов"})
    except Exception as e:
        q.put({"module":"sherlock","type":"error","text":f"Sherlock: {e}"})
        q.put({"module":"sherlock","type":"done","text":"Sherlock: ошибка"})

# ──────────────────────────────────────────────────────────────
# OSINT MODULE 2: MAIGRET
# ──────────────────────────────────────────────────────────────
def run_maigret(username, q):
    try:
        q.put({"module":"maigret","type":"running","text":f"Maigret: глубокий анализ '{username}'..."})
        proc = subprocess.Popen(
            ["maigret", username, "--no-color", "-a", "--timeout", "8"],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1, env=UNBUF_ENV)
        found = []
        for line in proc.stdout:
            line = line.strip()
            if "[+]" in line:
                found.append(line)
                q.put({"module":"maigret","type":"found","text":line.replace("[+]","").strip()})
        proc.wait(timeout=120)
        q.put({"module":"maigret","type":"done","text":f"Maigret: {len(found)} профилей"})
    except Exception as e:
        q.put({"module":"maigret","type":"error","text":f"Maigret: {e}"})
        q.put({"module":"maigret","type":"done","text":"Maigret: ошибка"})

# ──────────────────────────────────────────────────────────────
# OSINT MODULE 3: HOLEHE (email → social registrations)
# ──────────────────────────────────────────────────────────────
def run_holehe(email, q):
    try:
        q.put({"module":"holehe","type":"running","text":f"Holehe: email '{email}' на 120+ сервисах..."})
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
        q.put({"module":"holehe","type":"done","text":"Holehe: ошибка"})

# ──────────────────────────────────────────────────────────────
# OSINT MODULE 4: PHONE OSINT
# ──────────────────────────────────────────────────────────────
def run_phone_osint(phone, q):
    q.put({"module":"phone","type":"running","text":f"Phone OSINT: анализ '{phone}'..."})
    try:
        import phonenumbers
        from phonenumbers import geocoder, carrier, timezone as tz_mod
        clean = re.sub(r'[^\d+]','',phone)
        parsed = phonenumbers.parse(clean, None)
        is_valid = phonenumbers.is_valid_number(parsed)
        intl = phonenumbers.format_number(parsed, phonenumbers.PhoneNumberFormat.INTERNATIONAL)
        q.put({"module":"phone","type":"found","text":f"Номер: {intl}"})
        q.put({"module":"phone","type":"found","text":f"Валидность: {'ДА' if is_valid else 'НЕТ'}"})
        country = geocoder.description_for_number(parsed, 'ru')
        if country:
            q.put({"module":"phone","type":"found","text":f"Страна: {country}"})
        car = carrier.name_for_number(parsed, 'ru')
        if car:
            q.put({"module":"phone","type":"found","text":f"Оператор: {car}"})
        zones = list(tz_mod.time_zones_for_number(parsed))
        if zones:
            q.put({"module":"phone","type":"found","text":f"Timezone: {zones[0]}"})
        ntype = phonenumbers.number_type(parsed)
        type_map = {0:"FIXED_LINE",1:"MOBILE",2:"FIXED_OR_MOBILE",3:"TOLL_FREE",
                    4:"PREMIUM_RATE",6:"VOIP",7:"PERSONAL_NUMBER",10:"UAN"}
        q.put({"module":"phone","type":"found","text":f"Тип: {type_map.get(ntype,'UNKNOWN')}"})
        # Numverify dork
        q.put({"module":"phone","type":"found","text":f"Dork: site:truecaller.com \"{intl}\""})
        q.put({"module":"phone","type":"found","text":f"Dork: site:getcontact.com \"{intl}\""})
    except ImportError:
        q.put({"module":"phone","type":"info","text":"phonenumbers не установлен — генерирую dorks"})
    except Exception as e:
        q.put({"module":"phone","type":"error","text":f"Phone: {e}"})
    # Always add dorks
    clean = re.sub(r'[^\d+]','',phone)
    for d in [f'"{clean}"', f'"{phone}" inurl:profile',
              f'site:vk.com "{clean}"', f'site:telegram.me "{clean}"']:
        q.put({"module":"phone","type":"found","text":f"Dork: {d}"})
    q.put({"module":"phone","type":"done","text":"Phone OSINT: завершён"})

# ──────────────────────────────────────────────────────────────
# OSINT MODULE 5: HIBP — Have I Been Pwned
# ──────────────────────────────────────────────────────────────
def run_hibp(email, q):
    q.put({"module":"hibp","type":"running","text":f"HIBP: проверка '{email}' в базах утечек..."})
    found_any = False
    try:
        url = f"https://haveibeenpwned.com/api/v2/breachedaccount/{urllib.parse.quote(email)}?truncateResponse=false"
        req = urllib.request.Request(url, headers={
            "User-Agent": "Ravens-OSINT-Nexus/3.0",
            "Accept": "application/json"
        })
        try:
            with urllib.request.urlopen(req, timeout=12) as r:
                data = json.loads(r.read())
                for breach in data:
                    found_any = True
                    name = breach.get('Name','?')
                    date = breach.get('BreachDate','?')
                    count = breach.get('PwnCount',0)
                    data_classes = ', '.join(breach.get('DataClasses',[])[:4])
                    q.put({"module":"hibp","type":"found",
                           "text":f"УТЕЧКА [{name}] {date} — {count:,} жертв | данные: {data_classes}"})
        except urllib.error.HTTPError as he:
            if he.code == 404:
                q.put({"module":"hibp","type":"info","text":"HIBP: email не найден в известных утечках — хороший знак"})
            elif he.code == 429:
                q.put({"module":"hibp","type":"info","text":"HIBP: лимит запросов — попробуй позже"})
            elif he.code == 401:
                q.put({"module":"hibp","type":"info","text":"HIBP: требует API ключ для полного доступа"})
            else:
                raise he
    except urllib.error.URLError:
        q.put({"module":"hibp","type":"info","text":"HIBP: нет соединения"})
    except Exception as e:
        q.put({"module":"hibp","type":"error","text":f"HIBP: {e}"})
    # Complementary breach dorks
    q.put({"module":"hibp","type":"found","text":f"Dork: site:dehashed.com \"{email}\""})
    q.put({"module":"hibp","type":"found","text":f"Dork: site:leakcheck.io \"{email}\""})
    q.put({"module":"hibp","type":"done","text":"HIBP: завершён"})

# ──────────────────────────────────────────────────────────────
# OSINT MODULE 6: SOCIAL PLATFORMS CHECK
# ──────────────────────────────────────────────────────────────
def run_social_check(username, q):
    q.put({"module":"social","type":"running","text":f"Social: '{username}' на платформах..."})
    found_count = 0
    # Reddit (reliable JSON API)
    try:
        url = f"https://www.reddit.com/user/{username}/about.json"
        req = urllib.request.Request(url, headers={"User-Agent":"Ravens-OSINT/3.0"})
        with urllib.request.urlopen(req, timeout=8) as r:
            d = json.loads(r.read()).get('data',{})
            karma = d.get('link_karma',0)+d.get('comment_karma',0)
            created = d.get('created_utc',0)
            import datetime
            cr = datetime.datetime.fromtimestamp(created).strftime('%Y-%m-%d') if created else '?'
            q.put({"module":"social","type":"found","text":f"Reddit: u/{username} | karma:{karma} | с {cr}"})
            found_count += 1
    except urllib.error.HTTPError as e:
        if e.code == 404:
            q.put({"module":"social","type":"info","text":f"Reddit: u/{username} не найден"})
    except Exception:
        pass
    # VK
    try:
        url = f"https://vk.com/{username}"
        req = urllib.request.Request(url, headers={"User-Agent":"Mozilla/5.0 (Windows NT 10.0; Win64; x64)"})
        with urllib.request.urlopen(req, timeout=8) as r:
            html = r.read().decode('utf-8', errors='ignore')
            if 'page_not_found' not in html and 'id=' in html:
                q.put({"module":"social","type":"found","text":f"VK: vk.com/{username} — профиль найден"})
                found_count += 1
    except Exception:
        pass
    # Generate platform links for manual check
    platforms = [
        f"https://www.instagram.com/{username}/",
        f"https://twitter.com/{username}",
        f"https://tiktok.com/@{username}",
        f"https://twitch.tv/{username}",
        f"https://steamcommunity.com/id/{username}",
        f"https://www.pinterest.com/{username}/",
        f"https://open.spotify.com/user/{username}",
        f"https://soundcloud.com/{username}",
    ]
    for p in platforms:
        q.put({"module":"social","type":"found","text":p})
    q.put({"module":"social","type":"done","text":f"Social: {found_count} активных + {len(platforms)} ссылок"})

# ──────────────────────────────────────────────────────────────
# OSINT MODULE 7: GITHUB OSINT
# ──────────────────────────────────────────────────────────────
def run_github(username, q):
    q.put({"module":"github","type":"running","text":f"GitHub: поиск '{username}'..."})
    try:
        url = f"https://api.github.com/users/{username}"
        req = urllib.request.Request(url, headers={"User-Agent":"Ravens-OSINT/3.0","Accept":"application/vnd.github.v3+json"})
        with urllib.request.urlopen(req, timeout=10) as r:
            d = json.loads(r.read())
            fields = [("Имя", "name"), ("Email", "email"), ("Компания", "company"),
                      ("Локация", "location"), ("Bio", "bio"), ("Сайт", "blog"),
                      ("Репозитории", "public_repos"), ("Подписчики", "followers"),
                      ("Следит за", "following"), ("Gists", "public_gists")]
            for label, key in fields:
                if d.get(key): q.put({"module":"github","type":"found","text":f"GitHub {label}: {d[key]}"})
            # Get repos
            url2 = f"https://api.github.com/users/{username}/repos?per_page=8&sort=pushed"
            req2 = urllib.request.Request(url2, headers={"User-Agent":"Ravens-OSINT/3.0"})
            with urllib.request.urlopen(req2, timeout=10) as r2:
                repos = json.loads(r2.read())
                for repo in repos[:6]:
                    lang = repo.get('language','?') or '?'
                    q.put({"module":"github","type":"found",
                           "text":f"Repo: {repo['name']} [{lang}] — {repo.get('description','')[:60]}"})
            # Events (reveals activity)
            url3 = f"https://api.github.com/users/{username}/events/public?per_page=5"
            req3 = urllib.request.Request(url3, headers={"User-Agent":"Ravens-OSINT/3.0"})
            with urllib.request.urlopen(req3, timeout=10) as r3:
                events = json.loads(r3.read())
                if events:
                    last = events[0].get('created_at','')[:10]
                    q.put({"module":"github","type":"found","text":f"GitHub последняя активность: {last}"})
    except urllib.error.HTTPError as e:
        if e.code == 404:
            q.put({"module":"github","type":"info","text":f"GitHub: @{username} не найден"})
        elif e.code == 403:
            q.put({"module":"github","type":"info","text":"GitHub: лимит API исчерпан"})
    except Exception as e:
        q.put({"module":"github","type":"error","text":f"GitHub: {e}"})
    q.put({"module":"github","type":"done","text":"GitHub: завершён"})

# ──────────────────────────────────────────────────────────────
# OSINT MODULE 8: TELEGRAM
# ──────────────────────────────────────────────────────────────
def run_telegram_check(username, q):
    q.put({"module":"telegram","type":"running","text":f"Telegram: проверка '@{username}'..."})
    clean = username.lstrip('@')
    try:
        url = f"https://t.me/{clean}"
        req = urllib.request.Request(url, headers={"User-Agent":"Mozilla/5.0 TelegramBot (like TwitterBot)"})
        with urllib.request.urlopen(req, timeout=10) as r:
            html = r.read().decode('utf-8', errors='ignore')
            if 'tgme_page_title' in html or 'tg-page' in html:
                title = re.search(r'<meta property="og:title" content="([^"]*)"', html)
                desc = re.search(r'<meta property="og:description" content="([^"]*)"', html)
                if title:
                    q.put({"module":"telegram","type":"found","text":f"TG Имя: {title.group(1)}"})
                if desc and desc.group(1):
                    q.put({"module":"telegram","type":"found","text":f"TG Описание: {desc.group(1)[:200]}"})
                # Try member count
                members = re.search(r'([\d\s]+) (member|subscriber|participant)', html)
                if members:
                    q.put({"module":"telegram","type":"found","text":f"TG Участники: {members.group(1)} {members.group(2)}"})
                q.put({"module":"telegram","type":"found","text":f"TG URL: https://t.me/{clean}"})
            elif 'If you have Telegram' in html:
                q.put({"module":"telegram","type":"info","text":f"TG: @{clean} — личный аккаунт (не бот/группа)"})
                q.put({"module":"telegram","type":"found","text":f"TG URL: https://t.me/{clean}"})
            else:
                q.put({"module":"telegram","type":"info","text":f"TG: @{clean} не найден или недоступен"})
    except urllib.error.HTTPError as e:
        q.put({"module":"telegram","type":"info","text":f"TG: HTTP {e.code} — возможно не существует"})
    except Exception as e:
        q.put({"module":"telegram","type":"error","text":f"TG: {e}"})
    q.put({"module":"telegram","type":"done","text":"Telegram: завершён"})

# ──────────────────────────────────────────────────────────────
# OSINT MODULE 9: WHOIS + DNS
# ──────────────────────────────────────────────────────────────
def run_whois_dns(target, q):
    import whois, dns.resolver
    try:
        q.put({"module":"whois","type":"running","text":f"WHOIS: '{target}'..."})
        w = whois.whois(target)
        for field, val in [("Домен", w.domain_name), ("Регистратор", w.registrar),
                           ("Создан", w.creation_date), ("Истекает", w.expiration_date),
                           ("Email", w.emails), ("Страна", w.country), ("Организация", w.org),
                           ("Nameservers", w.name_servers)]:
            if val:
                q.put({"module":"whois","type":"found","text":f"WHOIS {field}: {val}"})
    except Exception as e:
        q.put({"module":"whois","type":"error","text":f"WHOIS: {e}"})
    for rtype in ["A","AAAA","MX","NS","TXT","CNAME","SOA"]:
        try:
            for r in dns.resolver.resolve(target, rtype, lifetime=5):
                q.put({"module":"whois","type":"found","text":f"DNS {rtype}: {r}"})
        except Exception:
            pass
    q.put({"module":"whois","type":"done","text":"WHOIS/DNS: завершён"})

# ──────────────────────────────────────────────────────────────
# OSINT MODULE 10: IP GEOLOCATION
# ──────────────────────────────────────────────────────────────
def run_ip(target, q):
    q.put({"module":"ip","type":"running","text":f"IP Geo: '{target}'..."})
    try:
        ip = socket.gethostbyname(target)
        q.put({"module":"ip","type":"found","text":f"IP адрес: {ip}"})
        url = f"http://ip-api.com/json/{ip}?fields=country,regionName,city,zip,lat,lon,isp,org,as,reverse,timezone,mobile,proxy,hosting,query"
        with urllib.request.urlopen(url, timeout=8) as r:
            d = json.loads(r.read())
        labels = {"country":"Страна","regionName":"Регион","city":"Город","zip":"ZIP",
                  "lat":"Широта","lon":"Долгота","isp":"Провайдер","org":"Организация",
                  "as":"AS","reverse":"Reverse DNS","timezone":"Timezone",
                  "mobile":"Мобильный","proxy":"Proxy/VPN","hosting":"Хостинг"}
        for k,v in d.items():
            if v and k!='query' and v is not False:
                q.put({"module":"ip","type":"found","text":f"IP {labels.get(k,k)}: {v}"})
        # Shodan dork
        q.put({"module":"ip","type":"found","text":f"Shodan: https://www.shodan.io/host/{ip}"})
        q.put({"module":"ip","type":"found","text":f"VirusTotal: https://www.virustotal.com/gui/ip-address/{ip}"})
    except Exception as e:
        q.put({"module":"ip","type":"error","text":f"IP: {e}"})
    q.put({"module":"ip","type":"done","text":"IP Geo: завершён"})

# ──────────────────────────────────────────────────────────────
# OSINT MODULE 11: GOOGLE DORKS
# ──────────────────────────────────────────────────────────────
def run_dorks(target, q):
    q.put({"module":"dorks","type":"running","text":f"Google Dorks: '{target}'..."})
    time.sleep(0.1)
    dorks = [
        f'"{target}" site:linkedin.com',
        f'"{target}" site:facebook.com',
        f'"{target}" site:vk.com',
        f'"{target}" site:instagram.com',
        f'"{target}" site:twitter.com',
        f'"{target}" site:github.com',
        f'"{target}" site:reddit.com',
        f'"{target}" site:youtube.com',
        f'"{target}" filetype:pdf',
        f'"{target}" filetype:doc',
        f'"{target}" inurl:profile',
        f'intitle:"{target}"',
        f'"{target}" (email OR почта OR contact)',
        f'"{target}" (phone OR телефон OR mobile)',
        f'"{target}" (password OR пароль OR credentials)',
        f'"{target}" site:pastebin.com',
        f'"{target}" site:ru',
        f'cache:"{target}"',
    ]
    for d in dorks:
        q.put({"module":"dorks","type":"found","text":d})
        time.sleep(0.05)
    q.put({"module":"dorks","type":"done","text":f"Dorks: {len(dorks)} запросов"})

# ──────────────────────────────────────────────────────────────
# OSINT MODULE 12: PASTE / DOXBIN SEARCH
# ──────────────────────────────────────────────────────────────
def run_paste_search(target, q):
    q.put({"module":"paste","type":"running","text":f"Paste/Doxbin: поиск '{target}'..."})
    # Try Doxbin direct
    try:
        url = f"https://doxbin.org/search/{urllib.parse.quote(target)}"
        req = urllib.request.Request(url, headers={"User-Agent":"Mozilla/5.0 (compatible)"})
        with urllib.request.urlopen(req, timeout=10) as r:
            html = r.read().decode('utf-8', errors='ignore')
            results = re.findall(r'href="/upload/([^"]+)"[^>]*>([^<]+)<', html)
            for slug, title in results[:10]:
                q.put({"module":"paste","type":"found",
                       "text":f"DOXBIN: {title.strip()} -> doxbin.org/upload/{slug}"})
            if not results:
                q.put({"module":"paste","type":"info","text":"Doxbin: совпадений не найдено"})
    except Exception:
        pass
    # Paste dorks
    sites = ["pastebin.com","ghostbin.co","doxbin.org","justpaste.it",
             "paste.ee","hastebin.com","dpaste.org","paste2.org","cl1p.net"]
    for site in sites:
        q.put({"module":"paste","type":"found","text":f'site:{site} "{target}"'})
    q.put({"module":"paste","type":"done","text":"Paste/Doxbin: завершён"})

# ──────────────────────────────────────────────────────────────
# OSINT MODULE 13: DARK WEB (Ahmia public index)
# ──────────────────────────────────────────────────────────────
def run_darkweb(target, q):
    q.put({"module":"darkweb","type":"running","text":f"Dark Web: '{target}' через Ahmia + Dorks..."})
    found = 0
    try:
        query = urllib.parse.quote(target)
        url = f"https://ahmia.fi/search/?q={query}"
        req = urllib.request.Request(url, headers={"User-Agent":"Mozilla/5.0 (compatible; Ravens/3.0)"})
        with urllib.request.urlopen(req, timeout=15) as r:
            html = r.read().decode('utf-8', errors='ignore')
            titles = re.findall(r'<h4[^>]*>\s*<a href="([^"]+)"[^>]*>([^<]+)</a>', html)
            for href, title in titles[:12]:
                q.put({"module":"darkweb","type":"found","text":f"DW: {title.strip()[:70]} [{href[:50]}]"})
                found += 1
    except Exception as e:
        q.put({"module":"darkweb","type":"info","text":f"Ahmia недоступен: {e}"})
    # Dark web dorks for clearnet aggregators
    dw_dorks = [
        f'site:onion.link "{target}"',
        f'site:tor2web.org "{target}"',
        f'site:dark.fail "{target}"',
        f'"{target}" site:ddosecrets.com',
        f'"{target}" site:wikileaks.org',
    ]
    for d in dw_dorks:
        q.put({"module":"darkweb","type":"found","text":d})
    q.put({"module":"darkweb","type":"done","text":f"Dark Web: {found} результатов + dorks"})

# ──────────────────────────────────────────────────────────────
# OSINT MODULE 14: INTELX / ADVANCED INTEL
# ──────────────────────────────────────────────────────────────
def run_intelx(target, q):
    q.put({"module":"intelx","type":"running","text":f"IntelX: расширенная разведка '{target}'..."})
    time.sleep(0.15)
    # IntelX free check
    try:
        url = f"https://intelx.io/api?term={urllib.parse.quote(target)}&buckets=&k=&media=0&sort=4&terminate=&timeout=20"
        req = urllib.request.Request(url, headers={
            "User-Agent":"Mozilla/5.0","x-key":"at0ZGa29oCBKQs5AaYLa","Accept":"application/json"})
        with urllib.request.urlopen(req, timeout=12) as r:
            data = json.loads(r.read())
            if isinstance(data, dict) and 'records' in data:
                for rec in data['records'][:8]:
                    q.put({"module":"intelx","type":"found",
                           "text":f"IntelX: {rec.get('name','?')} [{rec.get('date','?')[:10]}]"})
    except Exception:
        pass
    # Breach database dorks
    breach_dorks = [
        f'site:dehashed.com "{target}"',
        f'site:leakcheck.io "{target}"',
        f'site:snusbase.com "{target}"',
        f'site:haveibeenpwned.com "{target}"',
        f'site:intelx.io "{target}"',
        f'"{target}" intext:password',
        f'"{target}" intext:hash',
        f'"{target}" breach database',
        f'"{target}" leaked',
    ]
    for d in breach_dorks:
        q.put({"module":"intelx","type":"found","text":d})
    q.put({"module":"intelx","type":"done","text":"IntelX: завершён"})

# ──────────────────────────────────────────────────────────────
# OSINT MODULE 15: AI ANALYSIS (NVIDIA NIM Nemotron-Ultra)
# ──────────────────────────────────────────────────────────────
def run_ai_analysis(target_info, collected, module_outcomes, q):
    q.put({"module":"ai","type":"running","text":"Ravens AI: глубокий анализ через NVIDIA NIM Nemotron-Ultra..."})
    api_key = os.environ.get("NVIDIA_NIM_API_KEY","")
    if not api_key:
        q.put({"module":"ai","type":"error","text":"AI: API ключ NVIDIA_NIM_API_KEY не настроен"})
        q.put({"module":"ai","type":"done","text":"AI: ошибка авторизации"})
        return
    try:
        from openai import OpenAI
        client = OpenAI(base_url="https://integrate.api.nvidia.com/v1", api_key=api_key)
        findings_text = "\n".join(collected[:300]) if collected else "Данные не найдены."
        outcomes_text = "\n".join([f"- {mod}: {status}" for mod, status in module_outcomes.items()])
        user_msg = f"""ЦЕЛЬ: {target_info}

РЕЗУЛЬТАТЫ МОДУЛЕЙ ({len(collected)} записей):
{findings_text}

СТАТУС МОДУЛЕЙ:
{outcomes_text}

Составь полный разведывательный досье по указанной структуре. Выдели ключевые находки символом ★"""
        comp = client.chat.completions.create(
            model="nvidia/nemotron-3-ultra-550b-a55b",
            messages=[{"role":"system","content":SYSTEM_PROMPT},
                      {"role":"user","content":user_msg}],
            temperature=0.5, max_tokens=8000, stream=True)
        for chunk in comp:
            if not chunk.choices: continue
            c = chunk.choices[0].delta.content
            if c: q.put({"module":"ai","type":"stream","text":c})
        q.put({"module":"ai","type":"done","text":"Ravens AI: анализ завершён"})
    except Exception as e:
        q.put({"module":"ai","type":"error","text":f"AI: {e}"})
        q.put({"module":"ai","type":"done","text":"AI: ошибка"})

# ──────────────────────────────────────────────────────────────
# FLASK ROUTES
# ──────────────────────────────────────────────────────────────
@app.route('/')
def index():
    from flask import make_response
    resp = make_response(render_template('index.html'))
    resp.headers['Cache-Control'] = 'no-store, no-cache, must-revalidate, max-age=0'
    resp.headers['Pragma'] = 'no-cache'
    return resp

@app.route('/static/<path:filename>')
def static_files(filename):
    from flask import send_from_directory
    return send_from_directory('static', filename)

@app.route('/investigate', methods=['POST'])
def investigate():
    data   = request.get_json(force=True, silent=True) or {}
    name   = data.get('name','').strip()
    user   = data.get('user','').strip()
    email  = data.get('email','').strip()
    phone  = data.get('phone','').strip()
    domain = data.get('domain','').strip()

    q = queue.Queue()
    threads = []
    module_outcomes = {}

    def skip(mod, reason):
        module_outcomes[mod] = f"ПРОПУЩЕН — {reason}"
        q.put({"module":mod,"type":"info","text":f"{mod}: {reason}"})
        q.put({"module":mod,"type":"done","text":f"{mod}: пропущен"})

    def run_thread(fn, args, mod):
        module_outcomes[mod] = "ЗАПУЩЕН"
        t = threading.Thread(target=fn, args=args+(q,), daemon=True)
        threads.append(t); t.start()

    # Module 1: Sherlock
    if user:
        run_thread(run_sherlock, (user,), "sherlock")
    else:
        skip("sherlock", "username не указан")

    # Module 2: Maigret
    if user or name:
        run_thread(run_maigret, ((user or name).replace(' ','_').lower(),), "maigret")
    else:
        skip("maigret", "username/имя не указаны")

    # Module 3: Holehe
    if email:
        run_thread(run_holehe, (email,), "holehe")
    else:
        skip("holehe", "email не указан")

    # Module 4: Phone
    if phone:
        run_thread(run_phone_osint, (phone,), "phone")
    else:
        skip("phone", "телефон не указан")

    # Module 5: HIBP
    if email:
        run_thread(run_hibp, (email,), "hibp")
    else:
        skip("hibp", "email не указан")

    # Module 6: Social
    if user:
        run_thread(run_social_check, (user,), "social")
    else:
        skip("social", "username не указан")

    # Module 7: GitHub
    if user:
        run_thread(run_github, (user,), "github")
    else:
        skip("github", "username не указан")

    # Module 8: Telegram
    if user:
        run_thread(run_telegram_check, (user,), "telegram")
    else:
        skip("telegram", "username не указан")

    # Module 9: WHOIS
    target_d = domain or (email.split('@')[1] if '@' in email else '')
    if target_d:
        run_thread(run_whois_dns, (target_d,), "whois")
    else:
        skip("whois", "домен не указан")

    # Module 10: IP
    if target_d:
        run_thread(run_ip, (target_d,), "ip")
    else:
        skip("ip", "домен/IP не указан")

    # Module 11: Dorks
    dork_t = name or user or email or domain or phone
    if dork_t:
        run_thread(run_dorks, (dork_t,), "dorks")
    else:
        skip("dorks", "нет цели")

    # Module 12: Paste
    if dork_t:
        run_thread(run_paste_search, (dork_t,), "paste")
    else:
        skip("paste", "нет цели")

    # Module 13: Dark Web
    if dork_t:
        run_thread(run_darkweb, (dork_t,), "darkweb")
    else:
        skip("darkweb", "нет цели")

    # Module 14: IntelX
    if dork_t:
        run_thread(run_intelx, (dork_t,), "intelx")
    else:
        skip("intelx", "нет цели")

    def stream():
        yield ': ravens-init\n\n'
        collected = []
        deadline = time.time() + 300
        while time.time() < deadline:
            try:
                ev = q.get(timeout=0.2)
                yield 'data: ' + json.dumps(ev, ensure_ascii=False) + '\n\n'
                if ev.get('type') == 'found':
                    collected.append(ev.get('text',''))
                if ev.get('type') == 'done':
                    m = ev.get('module','')
                    if m in module_outcomes and module_outcomes[m] == "ЗАПУЩЕН":
                        module_outcomes[m] = f"ВЫПОЛНЕН — {ev.get('text','')}"
            except queue.Empty:
                yield ': hb\n\n'
                if all(not t.is_alive() for t in threads) and q.empty():
                    break

        # Module 15: AI — always runs
        target_info = ' | '.join(filter(None, [name, user, email, phone, domain])) or 'цель не указана'
        ai_q = queue.Queue()
        ai_t = threading.Thread(target=run_ai_analysis,
                                args=(target_info, collected, module_outcomes, ai_q), daemon=True)
        ai_t.start()
        deadline2 = time.time() + 180
        while time.time() < deadline2:
            try:
                ev = ai_q.get(timeout=0.2)
                yield 'data: ' + json.dumps(ev, ensure_ascii=False) + '\n\n'
            except queue.Empty:
                yield ': hb\n\n'
                if not ai_t.is_alive() and ai_q.empty():
                    break

        yield 'data: [DONE]\n\n'

    return Response(stream_with_context(stream()), headers={
        'Content-Type':'text/event-stream','Cache-Control':'no-cache',
        'X-Accel-Buffering':'no','Connection':'keep-alive'})

@app.route('/chat', methods=['POST'])
def chat():
    data = request.get_json(force=True, silent=True) or {}
    messages = data.get('messages',[])
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
                temperature=0.6, max_tokens=4096, stream=True)
            for chunk in comp:
                if not chunk.choices: continue
                c = chunk.choices[0].delta.content
                if c: yield 'data: ' + json.dumps({"type":"content","content":c},ensure_ascii=False) + '\n\n'
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
    print(f"Ravens OSINT Nexus v3: http://{HOST}:{PORT}")
    app.run(host=HOST, port=PORT, debug=False, threaded=True)
