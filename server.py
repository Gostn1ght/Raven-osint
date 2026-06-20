import os, json, subprocess, threading, queue, time, socket
from flask import Flask, request, Response, stream_with_context
import whois
import dns.resolver

app = Flask(__name__)
PORT, HOST = 5000, "0.0.0.0"

SYSTEM_PROMPT = """Ты — эксперт по OSINT (разведке на основе открытых источников). Ты анализируешь данные, найденные автоматическими инструментами, и даёшь детальный разбор на русском языке.

При анализе указывай:
1. Краткое резюме находок
2. Ключевые аккаунты и профили
3. Связи между платформами
4. Оценку достоверности (высокая/средняя/низкая)
5. Рекомендации для дальнейшего расследования
6. Возможные псевдонимы и альтернативные идентификаторы

Всегда работай только с публично доступной информацией. Соблюдай законодательство."""

# ── OSINT MODULES ─────────────────────────────────────────────────────────────

def run_sherlock(username, q):
    try:
        q.put({"module":"sherlock","type":"running","text":f"🔍 Sherlock: поиск '{username}' на 400+ сайтах..."})
        proc = subprocess.Popen(
            ["sherlock", username, "--timeout", "8", "--print-found", "--no-color"],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1)
        found = []
        for line in proc.stdout:
            line = line.strip()
            if line.startswith("[+]"):
                site = line.replace("[+]","").strip()
                found.append(site)
                q.put({"module":"sherlock","type":"found","text":f"✅ {site}"})
            elif line.startswith("[*]"):
                q.put({"module":"sherlock","type":"info","text":f"   {line[3:].strip()}"})
        proc.wait(timeout=60)
        q.put({"module":"sherlock","type":"done","text":f"Sherlock: найдено {len(found)} аккаунтов"})
    except Exception as e:
        q.put({"module":"sherlock","type":"error","text":f"❌ Sherlock: {e}"})

def run_maigret(username, q):
    try:
        q.put({"module":"maigret","type":"running","text":f"🕵️ Maigret: поиск профилей '{username}'..."})
        proc = subprocess.Popen(
            ["maigret", username, "--no-color", "-a", "--timeout", "8"],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1)
        found = []
        for line in proc.stdout:
            line = line.strip()
            if "[+]" in line:
                found.append(line)
                q.put({"module":"maigret","type":"found","text":f"✅ {line}"})
        proc.wait(timeout=90)
        q.put({"module":"maigret","type":"done","text":f"Maigret: {len(found)} профилей"})
    except Exception as e:
        q.put({"module":"maigret","type":"error","text":f"❌ Maigret: {e}"})

def run_holehe(email, q):
    try:
        q.put({"module":"holehe","type":"running","text":f"📧 Holehe: проверка '{email}'..."})
        proc = subprocess.Popen(
            ["holehe", email, "--no-color"],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1)
        found = []
        for line in proc.stdout:
            line = line.strip()
            if "[+]" in line:
                found.append(line)
                q.put({"module":"holehe","type":"found","text":f"✅ {line}"})
            elif "[-]" in line:
                q.put({"module":"holehe","type":"info","text":f"   {line}"})
        proc.wait(timeout=60)
        q.put({"module":"holehe","type":"done","text":f"Holehe: {len(found)} сервисов"})
    except Exception as e:
        q.put({"module":"holehe","type":"error","text":f"❌ Holehe: {e}"})

def run_whois_dns(target, q):
    results = []
    try:
        q.put({"module":"whois","type":"running","text":f"🌐 WHOIS: запрос по '{target}'..."})
        w = whois.whois(target)
        for field, val in [("Домен", w.domain_name), ("Регистратор", w.registrar),
                           ("Создан", w.creation_date), ("Email", w.emails), ("Страна", w.country)]:
            if val:
                txt = f"{field}: {val}"
                results.append(txt)
                q.put({"module":"whois","type":"found","text":f"✅ {txt}"})
        q.put({"module":"whois","type":"done","text":f"WHOIS: {len(results)} записей"})
    except Exception as e:
        q.put({"module":"whois","type":"error","text":f"❌ WHOIS: {e}"})

    try:
        q.put({"module":"dns","type":"running","text":f"🔌 DNS: резолюция '{target}'..."})
        dns_found = 0
        for rtype in ["A","MX","NS","TXT"]:
            try:
                for r in dns.resolver.resolve(target, rtype, lifetime=5):
                    txt = f"{rtype}: {r}"
                    results.append(txt)
                    dns_found += 1
                    q.put({"module":"dns","type":"found","text":f"✅ {txt}"})
            except Exception:
                pass
        q.put({"module":"dns","type":"done","text":f"DNS: {dns_found} записей"})
    except Exception as e:
        q.put({"module":"dns","type":"error","text":f"❌ DNS: {e}"})

def run_ip(target, q):
    try:
        q.put({"module":"ip","type":"running","text":f"📡 IP Lookup: '{target}'..."})
        ip = socket.gethostbyname(target)
        q.put({"module":"ip","type":"found","text":f"✅ IP: {ip}"})
        import urllib.request
        with urllib.request.urlopen(f"http://ip-api.com/json/{ip}?fields=country,regionName,city,isp,org,as,reverse", timeout=8) as r:
            data = json.loads(r.read())
        for k, v in data.items():
            if v:
                q.put({"module":"ip","type":"found","text":f"✅ {k}: {v}"})
        q.put({"module":"ip","type":"done","text":f"IP: {ip}"})
    except Exception as e:
        q.put({"module":"ip","type":"error","text":f"❌ IP: {e}"})

def run_dorks(target, q):
    q.put({"module":"dorks","type":"running","text":f"🔎 Google Dorks для '{target}'..."})
    dorks = [
        f'site:linkedin.com "{target}"', f'site:facebook.com "{target}"',
        f'site:twitter.com "{target}"', f'site:instagram.com "{target}"',
        f'"{target}" filetype:pdf', f'"{target}" site:github.com',
        f'"{target}" inurl:profile', f'intitle:"{target}"',
        f'"{target}" (email OR contact)', f'"{target}" (phone OR телефон)',
    ]
    for d in dorks:
        q.put({"module":"dorks","type":"found","text":f"✅ {d}"})
        time.sleep(0.05)
    q.put({"module":"dorks","type":"done","text":f"Dorks: {len(dorks)} запросов"})

def run_ai_analysis(target, collected, q):
    q.put({"module":"ai","type":"running","text":"🧠 AI: анализирую данные через NVIDIA NIM..."})
    api_key = os.environ.get("NVIDIA_NIM_API_KEY","")
    if not api_key:
        q.put({"module":"ai","type":"error","text":"❌ AI: API ключ не настроен"})
        return
    try:
        from openai import OpenAI
        client = OpenAI(base_url="https://integrate.api.nvidia.com/v1", api_key=api_key)
        findings = "\n".join(collected[:150])
        msgs = [
            {"role":"system","content":SYSTEM_PROMPT},
            {"role":"user","content":f"Цель: {target}\n\nНайденные данные:\n{findings}\n\nДай полный анализ."}
        ]
        comp = client.chat.completions.create(
            model="nvidia/nemotron-3-ultra-550b-a55b", messages=msgs,
            temperature=0.7, max_tokens=4096, stream=True)
        for chunk in comp:
            if not chunk.choices: continue
            c = chunk.choices[0].delta.content
            if c: q.put({"module":"ai","type":"stream","text":c})
        q.put({"module":"ai","type":"done","text":"✅ AI анализ завершён"})
    except Exception as e:
        q.put({"module":"ai","type":"error","text":f"❌ AI: {e}"})

# ── HTML ───────────────────────────────────────────────────────────────────────

HTML = """<!DOCTYPE html>
<html lang="ru">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>OSINT NEXUS — AI Investigator</title>
<style>
:root{
  --bg:#030609; --s1:#060d14; --s2:#0a1520;
  --border:#0d2035; --border2:#123050;
  --green:#00ff88; --green2:#00cc66;
  --blue:#00aaff; --purple:#aa55ff;
  --orange:#ff8c00; --red:#ff3333;
  --yellow:#ffdd00; --text:#b0cce0; --textd:#3a6080;
  --nvidia:#76b947;
}
*{box-sizing:border-box;margin:0;padding:0}
html,body{height:100%;overflow:hidden;font-family:'Segoe UI',monospace;background:var(--bg);color:var(--text)}
body::after{content:'';position:fixed;inset:0;pointer-events:none;z-index:9999;
  background:repeating-linear-gradient(0deg,transparent,transparent 2px,rgba(0,255,136,.012) 2px,rgba(0,255,136,.012) 4px)}

#topbar{position:fixed;top:0;left:0;right:0;height:48px;z-index:100;
  background:rgba(3,6,9,.95);border-bottom:1px solid var(--border2);
  display:flex;align-items:center;padding:0 1.2rem;gap:1rem;backdrop-filter:blur(10px)}
.logo{display:flex;align-items:center;gap:.6rem;font-size:.95rem;font-weight:700;color:#e0f0ff;letter-spacing:.05em}
.logo .badge{background:var(--nvidia);color:#000;font-size:.6rem;font-weight:900;padding:2px 6px;border-radius:3px}
.pill{display:flex;align-items:center;gap:.4rem;background:var(--s1);border:1px solid var(--border2);
  border-radius:20px;padding:2px 10px 2px 7px;font-size:.7rem;color:var(--textd)}
.pill .dot{width:6px;height:6px;border-radius:50%;background:var(--green);box-shadow:0 0 8px var(--green)}
.right-bar{margin-left:auto;display:flex;align-items:center;gap:.6rem}
.hbtn{background:none;border:1px solid var(--border2);color:var(--textd);border-radius:6px;
  padding:3px 10px;font-size:.7rem;cursor:pointer;transition:all .15s}
.hbtn:hover{border-color:var(--green);color:var(--green)}

#app{position:fixed;inset:0;top:48px;display:flex}

/* LEFT */
#left{width:290px;flex-shrink:0;display:flex;flex-direction:column;
  background:rgba(6,13,20,.97);border-right:1px solid var(--border);z-index:10;overflow:hidden}
#lscroll{flex:1;overflow-y:auto;padding:.9rem}
#lscroll::-webkit-scrollbar{width:3px}
#lscroll::-webkit-scrollbar-thumb{background:var(--border2);border-radius:10px}
.slabel{font-size:.62rem;font-weight:700;color:var(--textd);text-transform:uppercase;
  letter-spacing:.12em;margin-bottom:.45rem;margin-top:.9rem}
.slabel:first-child{margin-top:0}
.field{margin-bottom:.55rem}
.field label{font-size:.68rem;color:var(--textd);display:block;margin-bottom:.2rem}
.field input{width:100%;background:var(--s2);border:1px solid var(--border2);border-radius:6px;
  color:var(--text);font-size:.8rem;padding:.45rem .7rem;outline:none;font-family:monospace;transition:border-color .15s}
.field input:focus{border-color:var(--green);box-shadow:0 0 0 2px rgba(0,255,136,.07)}
.field input::placeholder{color:var(--textd)}
.mods{display:flex;flex-direction:column;gap:.28rem}
.mod{display:flex;align-items:center;gap:.55rem;padding:.38rem .5rem;border-radius:6px;
  border:1px solid var(--border);cursor:pointer;transition:all .15s;user-select:none}
.mod:hover{border-color:var(--border2);background:var(--s2)}
.mod.active{border-color:rgba(0,204,102,.3);background:rgba(0,255,136,.03)}
.mi{width:20px;text-align:center;font-size:.85rem}
.ml{flex:1;font-size:.73rem;color:var(--text)}
.ms{width:7px;height:7px;border-radius:50%;background:var(--border2);transition:all .3s;flex-shrink:0}
.mod.active .ms{background:var(--green);box-shadow:0 0 5px var(--green)}
.mod.running .ms{background:var(--yellow);box-shadow:0 0 8px var(--yellow);animation:blink 0.8s infinite}
.mod.found .ms{background:var(--green);box-shadow:0 0 10px var(--green)}
.mod.error .ms{background:var(--red);box-shadow:0 0 6px var(--red)}
@keyframes blink{0%,100%{opacity:1}50%{opacity:.3}}

#runBtn{width:100%;padding:.7rem;font-size:.85rem;font-weight:700;
  background:linear-gradient(135deg,#002618,#004d2e);
  border:1px solid var(--green2);border-radius:8px;color:var(--green);
  cursor:pointer;transition:all .2s;letter-spacing:.05em;margin-top:.7rem;
  text-shadow:0 0 10px rgba(0,255,136,.4)}
#runBtn:hover:not(:disabled){background:linear-gradient(135deg,#003d24,#00693e);
  box-shadow:0 0 20px rgba(0,255,136,.2)}
#runBtn:disabled{opacity:.35;cursor:not-allowed}
#runBtn.scanning{animation:scan 1.5s infinite}
@keyframes scan{0%,100%{box-shadow:0 0 8px rgba(0,255,136,.15)}50%{box-shadow:0 0 25px rgba(0,255,136,.5)}}
.stats{display:grid;grid-template-columns:1fr 1fr;gap:.35rem;margin-top:.7rem}
.stat{background:var(--s2);border:1px solid var(--border);border-radius:6px;padding:.45rem .5rem;text-align:center}
.sv{font-size:1.1rem;font-weight:700;color:var(--green);font-family:monospace}
.sk{font-size:.6rem;color:var(--textd);text-transform:uppercase;letter-spacing:.08em}

/* CENTER */
#center{flex:1;position:relative;overflow:hidden;background:var(--bg)}
#net-canvas{position:absolute;inset:0;display:block;cursor:grab}
#net-canvas:active{cursor:grabbing}
#hud{position:absolute;top:1rem;left:50%;transform:translateX(-50%);
  text-align:center;pointer-events:none;z-index:5}
#target-display{font-size:1rem;font-weight:700;color:var(--green);
  text-shadow:0 0 20px var(--green),0 0 40px rgba(0,255,136,.3);
  letter-spacing:.12em;font-family:monospace}
#target-sub{font-size:.63rem;color:var(--textd);letter-spacing:.15em;text-transform:uppercase;margin-top:.2rem}
#progress-bar{position:absolute;bottom:0;left:0;right:0;height:2px;background:var(--border);z-index:10}
#progress-fill{height:100%;width:0;background:linear-gradient(90deg,var(--green),var(--blue));transition:width .5s;box-shadow:0 0 6px var(--green)}

/* RIGHT */
#right{width:350px;flex-shrink:0;display:flex;flex-direction:column;
  background:rgba(6,13,20,.97);border-left:1px solid var(--border);z-index:10}
#tabs{display:flex;border-bottom:1px solid var(--border);flex-shrink:0}
.tab{flex:1;padding:.55rem;font-size:.68rem;font-weight:700;color:var(--textd);
  cursor:pointer;text-align:center;letter-spacing:.08em;text-transform:uppercase;
  border-bottom:2px solid transparent;transition:all .15s}
.tab:hover{color:var(--text)}
.tab.active{color:var(--green);border-bottom-color:var(--green)}
.panel{flex:1;overflow-y:auto;display:none;flex-direction:column}
.panel.show{display:flex}
.panel::-webkit-scrollbar{width:3px}
.panel::-webkit-scrollbar-thumb{background:var(--border2)}

#log-out{flex:1;padding:.7rem;font-family:'Courier New',monospace;font-size:.7rem;
  line-height:1.65;overflow-y:auto}
#log-out::-webkit-scrollbar{width:3px}
#log-out::-webkit-scrollbar-thumb{background:var(--border2)}
.ll{margin-bottom:.12rem;animation:fi .2s}
@keyframes fi{from{opacity:0;transform:translateX(5px)}to{opacity:1}}
.ll.running{color:var(--yellow)}.ll.found{color:var(--green)}.ll.info{color:var(--textd)}
.ll.error{color:var(--red)}.ll.done{color:var(--blue)}.ll.stream{color:#b0d8ff}
.lts{color:var(--textd);margin-right:.4rem;font-size:.62rem}

#res-content{padding:.7rem;overflow-y:auto;flex:1}
#res-content::-webkit-scrollbar{width:3px}
#res-content::-webkit-scrollbar-thumb{background:var(--border2)}
.rsec{margin-bottom:.9rem}
.rsec h4{font-size:.68rem;font-weight:700;color:var(--green);text-transform:uppercase;
  letter-spacing:.1em;margin-bottom:.35rem}
.ritem{background:var(--s2);border:1px solid var(--border);border-radius:5px;
  padding:.35rem .55rem;font-size:.7rem;color:var(--text);margin-bottom:.2rem;
  word-break:break-all;font-family:monospace}
.ritem.hi{border-color:var(--green2);color:var(--green)}

#chat-msgs{flex:1;overflow-y:auto;padding:.7rem;display:flex;flex-direction:column;gap:.7rem}
#chat-msgs::-webkit-scrollbar{width:3px}
#chat-msgs::-webkit-scrollbar-thumb{background:var(--border2)}
.cm{max-width:92%;padding:.5rem .7rem;border-radius:8px;font-size:.76rem;
  line-height:1.65;white-space:pre-wrap;word-break:break-word}
.cm.user{align-self:flex-end;background:#0a2545;border:1px solid #1a4575;color:#c8e4ff}
.cm.assistant{align-self:flex-start;background:var(--s2);border:1px solid var(--border2);color:var(--text)}
#cinput-row{display:flex;gap:.4rem;padding:.55rem;border-top:1px solid var(--border);flex-shrink:0}
#cinput{flex:1;background:var(--s2);border:1px solid var(--border2);border-radius:6px;
  color:var(--text);font-size:.76rem;padding:.4rem .6rem;outline:none;font-family:inherit;resize:none}
#cinput:focus{border-color:var(--green)}
#csend{background:var(--green2);color:#000;border:none;border-radius:6px;
  padding:.4rem .75rem;font-size:.75rem;font-weight:700;cursor:pointer}
#csend:hover{background:var(--green)}
</style>
</head>
<body>

<div id="topbar">
  <div class="logo"><span>⬡</span> OSINT NEXUS <span class="badge">NVIDIA NIM</span></div>
  <div class="pill"><span class="dot"></span>nemotron-ultra-550b</div>
  <div class="right-bar"><button class="hbtn" onclick="clearAll()">🗑 Сброс</button></div>
</div>

<div id="app">
  <div id="left">
    <div id="lscroll">
      <div class="slabel">🎯 Цель расследования</div>
      <div class="field"><label>ИМЯ / ПСЕВДОНИМ</label><input id="i-name" placeholder="Иван Петров / darkwolf99"></div>
      <div class="field"><label>USERNAME</label><input id="i-user" placeholder="username / @handle"></div>
      <div class="field"><label>EMAIL</label><input id="i-email" type="email" placeholder="target@email.com"></div>
      <div class="field"><label>ДОМЕН / IP</label><input id="i-domain" placeholder="example.com / 8.8.8.8"></div>

      <div class="slabel">⚡ Модули</div>
      <div class="mods">
        <div class="mod active" data-mod="sherlock"><span class="mi">🔍</span><span class="ml">Sherlock — 400+ сайтов</span><span class="ms"></span></div>
        <div class="mod active" data-mod="maigret"><span class="mi">🕵️</span><span class="ml">Maigret — профили</span><span class="ms"></span></div>
        <div class="mod active" data-mod="holehe"><span class="mi">📧</span><span class="ml">Holehe — email→соцсети</span><span class="ms"></span></div>
        <div class="mod active" data-mod="whois"><span class="mi">🌐</span><span class="ml">WHOIS / DNS</span><span class="ms"></span></div>
        <div class="mod active" data-mod="ip"><span class="mi">📡</span><span class="ml">IP Геолокация</span><span class="ms"></span></div>
        <div class="mod active" data-mod="dorks"><span class="mi">🔎</span><span class="ml">Google Dorks</span><span class="ms"></span></div>
        <div class="mod active" data-mod="ai"><span class="mi">🧠</span><span class="ml">AI Анализ (NIM)</span><span class="ms"></span></div>
      </div>
      <button id="runBtn" onclick="startInvestigation()">▶ НАЧАТЬ РАССЛЕДОВАНИЕ</button>
      <div class="stats">
        <div class="stat"><div class="sv" id="s-found">0</div><div class="sk">Найдено</div></div>
        <div class="stat"><div class="sv" id="s-mods">0/7</div><div class="sk">Модулей</div></div>
        <div class="stat"><div class="sv" id="s-time">0s</div><div class="sk">Время</div></div>
        <div class="stat"><div class="sv" id="s-score" style="color:var(--yellow)">—</div><div class="sk">Уровень</div></div>
      </div>
    </div>
  </div>

  <div id="center">
    <canvas id="net-canvas"></canvas>
    <div id="hud">
      <div id="target-display">OSINT NEXUS</div>
      <div id="target-sub">Введите данные и запустите расследование</div>
    </div>
    <div id="progress-bar"><div id="progress-fill"></div></div>
  </div>

  <div id="right">
    <div id="tabs">
      <div class="tab active" onclick="switchTab('log')">📟 Консоль</div>
      <div class="tab" onclick="switchTab('res')">📊 Данные</div>
      <div class="tab" onclick="switchTab('chat')">🧠 AI Чат</div>
    </div>
    <div class="panel show" id="panel-log"><div id="log-out"><div class="ll info"><span class="lts">[INIT]</span>Система готова.</div></div></div>
    <div class="panel" id="panel-res"><div id="res-content"><div style="color:var(--textd);font-size:.73rem;padding:.5rem">Данные появятся после расследования...</div></div></div>
    <div class="panel" id="panel-chat">
      <div id="chat-msgs"><div class="cm assistant">Привет! Я AI-аналитик на базе NVIDIA NIM Nemotron. Спроси что угодно по OSINT или запусти расследование — я проанализирую результаты.</div></div>
      <div id="cinput-row">
        <textarea id="cinput" rows="2" placeholder="Задай вопрос..."></textarea>
        <button id="csend" onclick="sendChat()">→</button>
      </div>
    </div>
  </div>
</div>

<script>
// =======================================================
// CANVAS 2D — PSEUDO-3D NETWORK VISUALIZATION
// =======================================================
const cv = document.getElementById('net-canvas');
const cx = cv.getContext('2d');

const NODES = [
  {id:'target',  label:'TARGET',     icon:'[T]', col:'#00aaff', x3:0,   y3:0,   z3:0,   r:22},
  {id:'sherlock',label:'SHERLOCK',   icon:'[S]', col:'#00ff88', x3:120, y3:-60, z3:40,  r:16},
  {id:'maigret', label:'MAIGRET',    icon:'[M]', col:'#00dd77', x3:-110,y3:-80, z3:-30, r:16},
  {id:'holehe',  label:'HOLEHE',     icon:'[@]', col:'#aa55ff', x3:100, y3:80,  z3:50,  r:16},
  {id:'whois',   label:'WHOIS/DNS',  icon:'[W]', col:'#00aaff', x3:-120,y3:70,  z3:-20, r:16},
  {id:'ip',      label:'IP GEO',     icon:'[I]', col:'#ff8c00', x3:50,  y3:130, z3:-60, r:16},
  {id:'dorks',   label:'DORKS',      icon:'[D]', col:'#ffdd00', x3:-60, y3:-130,z3:60,  r:16},
  {id:'ai',      label:'AI NIM',     icon:'[A]', col:'#76b947', x3:0,   y3:0,   z3:160, r:20},
];

const EDGES = [
  [0,1],[0,2],[0,3],[0,4],[0,5],[0,6],[0,7],
  [1,2],[2,3],[3,4],[4,5],[5,6],[6,1],[7,1],[7,3],[7,5]
];

// State per node
const NS = {};
NODES.forEach(n => NS[n.id] = {state:'idle', pulse:0});

// Particles
const parts = [];

// Camera rotation
let rotY = 0.3, rotX = -0.2, rotZ = 0;
let autoRot = true;
let isDrag = false, prevX = 0, prevY = 0;
let zoom = 1;

// Project 3D -> 2D with perspective
function project(x3, y3, z3) {
  // Rotate Y
  let x1 = x3*Math.cos(rotY) + z3*Math.sin(rotY);
  let y1 = y3;
  let z1 = -x3*Math.sin(rotY) + z3*Math.cos(rotY);
  // Rotate X
  let x2 = x1;
  let y2 = y1*Math.cos(rotX) - z1*Math.sin(rotX);
  let z2 = y1*Math.sin(rotX) + z1*Math.cos(rotX);

  const fov = 600;
  const dz = fov + z2;
  const scale = (fov / dz) * zoom;
  const cx2 = cv.width/2;
  const cy2 = cv.height/2;
  return {x: cx2 + x2*scale, y: cy2 + y2*scale, z: z2, scale};
}

function hexToRgb(hex) {
  const r=parseInt(hex.slice(1,3),16), g=parseInt(hex.slice(3,5),16), b=parseInt(hex.slice(5,7),16);
  return `${r},${g},${b}`;
}

function drawNode(n, proj) {
  const {x,y,scale} = proj;
  const r = n.r * scale;
  const st = NS[n.id].state;
  const col = n.col;
  const rgb = hexToRgb(col);
  const t = Date.now()/1000;

  // Glow
  let glowR = r * 1.8, glowA = 0.12;
  if(st==='running') { glowR = r*(2+0.5*Math.sin(t*8)); glowA = 0.35+0.2*Math.sin(t*6); }
  else if(st==='found') { glowR = r*(1.8+0.2*Math.sin(t*3)); glowA = 0.25+0.1*Math.sin(t*3); }
  else if(st==='error') { glowA = 0.3; }

  const grd = cx.createRadialGradient(x,y,0,x,y,glowR);
  const glowCol = st==='error' ? '255,51,51' : rgb;
  grd.addColorStop(0, `rgba(${glowCol},${glowA})`);
  grd.addColorStop(1, `rgba(${glowCol},0)`);
  cx.beginPath(); cx.arc(x,y,glowR,0,Math.PI*2);
  cx.fillStyle = grd; cx.fill();

  // Box (rotated square)
  const boxR = r * 0.95;
  const angle = t * (st==='running' ? 2 : 0.4);
  cx.save();
  cx.translate(x,y); cx.rotate(angle);
  // Fill
  cx.beginPath();
  cx.rect(-boxR,-boxR,boxR*2,boxR*2);
  let fillCol = st==='running' ? `rgba(${rgb},0.15)` : st==='found' ? `rgba(${rgb},0.1)` : `rgba(${rgb},0.04)`;
  cx.fillStyle = fillCol; cx.fill();
  // Border
  const borderAlpha = st==='running' ? 0.9 : st==='found' ? 0.8 : 0.3;
  cx.strokeStyle = `rgba(${rgb},${borderAlpha})`;
  cx.lineWidth = st==='running'||st==='found' ? 1.5 : 0.8;
  cx.stroke();
  // Inner smaller box
  cx.beginPath();
  cx.rect(-boxR*0.55,-boxR*0.55,boxR*1.1,boxR*1.1);
  cx.strokeStyle = `rgba(${rgb},${borderAlpha*0.4})`;
  cx.lineWidth = 0.5; cx.stroke();
  cx.restore();

  // Icon
  cx.font = `${r*0.85}px serif`;
  cx.textAlign='center'; cx.textBaseline='middle';
  cx.fillText(n.icon, x, y);

  // Label
  cx.font = `bold ${Math.max(9,r*0.55)}px monospace`;
  cx.fillStyle = st==='found' ? col : st==='running' ? '#ffdd00' : 'rgba(160,200,220,0.7)';
  cx.textAlign = 'center'; cx.textBaseline = 'top';
  if(st==='found'||st==='running') {
    cx.shadowColor = col; cx.shadowBlur = 8;
  }
  cx.fillText(n.label, x, y + r + 4*scale);
  cx.shadowBlur = 0;
}

function drawEdge(a, b, pA, pB) {
  const aActive = NS[a.id].state==='running'||NS[a.id].state==='found';
  const bActive = NS[b.id].state==='running'||NS[b.id].state==='found';
  const active = aActive || bActive;
  const t = Date.now()/1000;

  const alpha = active ? (0.4+0.2*Math.sin(t*4)) : 0.08;
  const col = active ? (NS[b.id].state!=='idle' ? b.col : a.col) : '#0a2535';

  cx.beginPath();
  cx.moveTo(pA.x, pA.y);
  cx.lineTo(pB.x, pB.y);
  cx.strokeStyle = col.startsWith('#') ? col+'88' : col;
  cx.lineWidth = active ? 1 : 0.5;
  cx.setLineDash(active ? [] : [4,6]);
  cx.stroke();
  cx.setLineDash([]);
}

function spawnParticle(fromNode, toNode) {
  parts.push({
    from: fromNode, to: toNode,
    t: 0, speed: 0.006+Math.random()*0.006,
    col: toNode.col
  });
}

function drawParticles() {
  for(let i=parts.length-1;i>=0;i--) {
    const p=parts[i];
    p.t += p.speed;
    if(p.t>=1){parts.splice(i,1);continue;}
    const pA=project(p.from.x3,p.from.y3,p.from.z3);
    const pB=project(p.to.x3,p.to.y3,p.to.z3);
    const px=pA.x+(pB.x-pA.x)*p.t;
    const py=pA.y+(pB.y-pA.y)*p.t;
    const rgb=hexToRgb(p.col);
    const alpha=1-p.t*0.8;
    cx.beginPath();
    cx.arc(px,py,2.5,0,Math.PI*2);
    cx.fillStyle=`rgba(${rgb},${alpha})`;
    cx.fill();
    // tail
    cx.beginPath();
    const t2=Math.max(0,p.t-0.05);
    const px2=pA.x+(pB.x-pA.x)*t2, py2=pA.y+(pB.y-pA.y)*t2;
    cx.moveTo(px,py); cx.lineTo(px2,py2);
    cx.strokeStyle=`rgba(${rgb},${alpha*0.4})`;
    cx.lineWidth=1.5; cx.stroke();
  }
}

// Starfield
const stars=[];
for(let i=0;i<200;i++) stars.push({x:(Math.random()-0.5)*800,y:(Math.random()-0.5)*800,z:(Math.random()-0.5)*400,s:Math.random()*1.5});
function drawStars(){
  stars.forEach(s=>{
    const p=project(s.x,s.y,s.z);
    if(p.x<0||p.x>cv.width||p.y<0||p.y>cv.height) return;
    cx.beginPath(); cx.arc(p.x,p.y,s.s*p.scale*0.3,0,Math.PI*2);
    cx.fillStyle=`rgba(30,70,100,${0.4*p.scale})`; cx.fill();
  });
}

let lastParticle=0;
function render(){
  requestAnimationFrame(render);
  const W=cv.width, H=cv.height;

  // Auto-rotate
  if(autoRot) rotY+=0.004;

  // Clear
  cx.clearRect(0,0,W,H);

  // Background gradient
  const bg=cx.createRadialGradient(W/2,H/2,0,W/2,H/2,Math.max(W,H)/1.5);
  bg.addColorStop(0,'rgba(5,12,20,1)');
  bg.addColorStop(1,'rgba(2,5,8,1)');
  cx.fillStyle=bg; cx.fillRect(0,0,W,H);

  drawStars();

  // Project all nodes
  const projs=NODES.map(n=>project(n.x3,n.y3,n.z3));

  // Sort by Z for painter's algorithm
  const order=[...NODES.keys()].sort((a,b)=>projs[a].z-projs[b].z);

  // Draw edges first
  EDGES.forEach(([ai,bi])=>{
    drawEdge(NODES[ai],NODES[bi],projs[ai],projs[bi]);
  });

  // Spawn particles on active edges
  const now=Date.now();
  if(now-lastParticle>120){
    EDGES.forEach(([ai,bi])=>{
      const a=NODES[ai],b=NODES[bi];
      if((NS[a.id].state==='running'||NS[a.id].state==='found')&&
         (NS[b.id].state==='running'||NS[b.id].state==='found')){
        if(Math.random()<0.4) spawnParticle(a,b);
        if(Math.random()<0.2) spawnParticle(b,a);
      }
    });
    lastParticle=now;
  }

  drawParticles();

  // Draw nodes in depth order
  order.forEach(i=>drawNode(NODES[i],projs[i]));

  // Center scan ring animation
  if(Object.values(NS).some(s=>s.state==='running')){
    const cp=projs[0];
    const t2=Date.now()/1000;
    for(let ri=0;ri<3;ri++){
      const prog=(t2*0.6+ri*0.33)%1;
      const rad=30+prog*120;
      const alpha=(1-prog)*0.3;
      cx.beginPath(); cx.arc(cp.x,cp.y,rad,0,Math.PI*2);
      cx.strokeStyle=`rgba(0,170,255,${alpha})`;
      cx.lineWidth=1; cx.stroke();
    }
  }
}

function resize(){
  const c=document.getElementById('center');
  cv.width=c.clientWidth; cv.height=c.clientHeight;
}
window.addEventListener('resize',resize); resize(); render();

// Mouse interaction
cv.addEventListener('mousedown',e=>{isDrag=true;autoRot=false;prevX=e.clientX;prevY=e.clientY});
window.addEventListener('mouseup',()=>{isDrag=false;setTimeout(()=>autoRot=true,4000)});
window.addEventListener('mousemove',e=>{
  if(!isDrag)return;
  rotY+=(e.clientX-prevX)*0.008;
  rotX+=(e.clientY-prevY)*0.008;
  prevX=e.clientX;prevY=e.clientY;
});
cv.addEventListener('wheel',e=>{zoom=Math.max(0.4,Math.min(2.5,zoom-e.deltaY*0.001))});

// Touch support
cv.addEventListener('touchstart',e=>{isDrag=true;autoRot=false;prevX=e.touches[0].clientX;prevY=e.touches[0].clientY},{passive:true});
cv.addEventListener('touchend',()=>{isDrag=false;setTimeout(()=>autoRot=true,4000)});
cv.addEventListener('touchmove',e=>{
  if(!isDrag)return;
  rotY+=(e.touches[0].clientX-prevX)*0.008;
  rotX+=(e.touches[0].clientY-prevY)*0.008;
  prevX=e.touches[0].clientX;prevY=e.touches[0].clientY;
},{passive:true});

// =======================================================
// UI STATE
// =======================================================
let foundCount=0, doneModules=0, startTime=0, timerInt=null;
let chatHist=[];

function setNodeState(id,state){
  if(NS[id]) NS[id].state=state;
  const mod=document.querySelector(`[data-mod="${id}"]`);
  if(mod) mod.className='mod active '+state;
}

function addLog(type,text){
  const el=document.getElementById('log-out');
  const d=document.createElement('div');
  d.className='ll '+type;
  const now=new Date();
  const ts=`${now.getHours().toString().padStart(2,'0')}:${now.getMinutes().toString().padStart(2,'0')}:${now.getSeconds().toString().padStart(2,'0')}`;
  d.innerHTML=`<span class="lts">[${ts}]</span>${escH(text)}`;
  el.appendChild(d);
  el.scrollTop=el.scrollHeight;
}

function addResult(mod,text,hi=false){
  let sec=document.getElementById('rs-'+mod);
  if(!sec){
    const rc=document.getElementById('res-content');
    if(rc.children.length===1&&rc.firstChild.tagName!=='DIV') rc.innerHTML='';
    const icons={sherlock:'🔍',maigret:'🕵️',holehe:'📧',whois:'🌐',dns:'🔌',ip:'📡',dorks:'🔎',ai:'🧠'};
    const wrap=document.createElement('div'); wrap.className='rsec';
    wrap.innerHTML=`<h4>${icons[mod]||'•'} ${mod.toUpperCase()}</h4>`;
    sec=document.createElement('div'); sec.id='rs-'+mod;
    wrap.appendChild(sec); rc.appendChild(wrap);
  }
  const item=document.createElement('div');
  item.className='ritem'+(hi?' hi':'');
  item.textContent=text; sec.appendChild(item);
}

function escH(t){return String(t).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;')}

function switchTab(t){
  document.querySelectorAll('.tab').forEach((el,i)=>el.classList.toggle('active',['log','res','chat'][i]===t));
  document.querySelectorAll('.panel').forEach((el,i)=>el.classList.toggle('show',['log','res','chat'][i]===t));
}

function clearAll(){
  foundCount=0;doneModules=0;
  document.getElementById('log-out').innerHTML='<div class="ll info"><span class="lts">[RESET]</span>Сброс выполнен.</div>';
  document.getElementById('res-content').innerHTML='<div style="color:var(--textd);font-size:.73rem;padding:.5rem">Данные появятся после расследования...</div>';
  document.getElementById('target-display').textContent='OSINT NEXUS';
  document.getElementById('target-sub').textContent='Введите данные и запустите расследование';
  document.getElementById('progress-fill').style.width='0';
  document.getElementById('s-found').textContent='0';
  document.getElementById('s-mods').textContent='0/7';
  document.getElementById('s-time').textContent='0s';
  document.getElementById('s-score').textContent='—';
  clearInterval(timerInt);
  NODES.forEach(n=>NS[n.id].state='idle');
  document.getElementById('runBtn').disabled=false;
  document.getElementById('runBtn').classList.remove('scanning');
  document.getElementById('runBtn').textContent='▶ НАЧАТЬ РАССЛЕДОВАНИЕ';
}

async function startInvestigation(){
  const name=document.getElementById('i-name').value.trim();
  const user=document.getElementById('i-user').value.trim();
  const email=document.getElementById('i-email').value.trim();
  const domain=document.getElementById('i-domain').value.trim();
  if(!name&&!user&&!email&&!domain){addLog('error','❌ Введите хотя бы одно поле!');return;}

  clearAll();
  const target=[name,user,email,domain].filter(Boolean).join(' / ');
  document.getElementById('target-display').textContent=target.toUpperCase().substring(0,40);
  document.getElementById('target-sub').textContent='🔴 РАССЛЕДОВАНИЕ АКТИВНО';
  document.getElementById('runBtn').disabled=true;
  document.getElementById('runBtn').classList.add('scanning');
  document.getElementById('runBtn').textContent='⏳ СКАНИРОВАНИЕ...';
  setNodeState('target','found');

  startTime=Date.now();
  timerInt=setInterval(()=>{document.getElementById('s-time').textContent=Math.round((Date.now()-startTime)/1000)+'s';},500);

  switchTab('log');
  const resp=await fetch('/investigate',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({name,user,email,domain})});
  const reader=resp.body.getReader(), dec=new TextDecoder();
  let buf='';

  while(true){
    const {done,value}=await reader.read();
    if(done)break;
    buf+=dec.decode(value,{stream:true});
    const lines=buf.split('\n'); buf=lines.pop();
    for(const line of lines){
      if(!line.startsWith('data: '))continue;
      const raw=line.slice(6);
      if(raw==='[DONE]')continue;
      try{handleEvent(JSON.parse(raw));}catch(e){}
    }
  }

  clearInterval(timerInt);
  const elapsed=Math.round((Date.now()-startTime)/1000);
  document.getElementById('target-sub').textContent=`✅ Завершено за ${elapsed}s`;
  document.getElementById('runBtn').disabled=false;
  document.getElementById('runBtn').classList.remove('scanning');
  document.getElementById('runBtn').textContent='▶ НОВОЕ РАССЛЕДОВАНИЕ';
  const score=foundCount>20?'HIGH':foundCount>5?'MED':'LOW';
  const scoreEl=document.getElementById('s-score');
  scoreEl.textContent=score;
  scoreEl.style.color=foundCount>20?'var(--red)':foundCount>5?'var(--yellow)':'var(--green)';
  addLog('done',`✅ Завершено. Найдено: ${foundCount} записей за ${elapsed}s.`);
  switchTab('res');
}

function handleEvent(ev){
  const {module:mod,type,text}=ev;
  if(type==='running'){setNodeState(mod,'running');addLog('running',text);return;}
  if(type==='done'){setNodeState(mod,'found');doneModules++;document.getElementById('s-mods').textContent=doneModules+'/7';
    document.getElementById('progress-fill').style.width=(doneModules/7*100)+'%';addLog('done',text);return;}
  if(type==='error'){setNodeState(mod,'error');addLog('error',text);return;}
  if(type==='info'){addLog('info',text);return;}
  if(type==='found'){
    foundCount++;document.getElementById('s-found').textContent=foundCount;
    addLog('found',text);addResult(mod,text,true);return;}
  if(type==='stream'){
    let el=document.getElementById('ai-stream');
    if(!el){
      const rc=document.getElementById('res-content');
      const sec=document.createElement('div');sec.className='rsec';
      sec.innerHTML='<h4>🧠 AI АНАЛИЗ</h4>';
      el=document.createElement('div');el.id='ai-stream';
      el.style.cssText='font-size:.72rem;line-height:1.7;color:#b0d8ff;white-space:pre-wrap;padding:.3rem;';
      sec.appendChild(el);rc.appendChild(sec);switchTab('res');
    }
    el.textContent+=text; el.scrollIntoView({block:'end'});
  }
}

// =======================================================
// AI CHAT
// =======================================================
document.getElementById('cinput').addEventListener('keydown',e=>{
  if(e.key==='Enter'&&!e.shiftKey){e.preventDefault();sendChat();}
});

async function sendChat(){
  const inp=document.getElementById('cinput');
  const text=inp.value.trim(); if(!text)return;
  inp.value='';
  const msgs=document.getElementById('chat-msgs');
  const ud=document.createElement('div'); ud.className='cm user'; ud.textContent=text;
  msgs.appendChild(ud); chatHist.push({role:'user',content:text}); msgs.scrollTop=msgs.scrollHeight;
  const ad=document.createElement('div'); ad.className='cm assistant'; ad.textContent='...';
  msgs.appendChild(ad); msgs.scrollTop=msgs.scrollHeight;
  const resp=await fetch('/chat',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({messages:chatHist})});
  const reader=resp.body.getReader(), dec=new TextDecoder();
  let buf='',ans=''; ad.textContent='';
  while(true){
    const {done,value}=await reader.read(); if(done)break;
    buf+=dec.decode(value,{stream:true});
    const lines=buf.split('\n'); buf=lines.pop();
    for(const l of lines){
      if(!l.startsWith('data: '))continue;
      const raw=l.slice(6); if(raw==='[DONE]')break;
      try{const o=JSON.parse(raw);if(o.type==='content'){ans+=o.content;ad.textContent=ans;msgs.scrollTop=msgs.scrollHeight;}}catch(e){}
    }
  }
  chatHist.push({role:'assistant',content:ans});
}
</script>
</body>
</html>"""

# ── FLASK ROUTES ───────────────────────────────────────────────────────────────

@app.route('/')
def index():
    from flask import make_response, render_template
    resp = make_response(render_template('index.html'))
    resp.headers['Cache-Control'] = 'no-store, no-cache, must-revalidate, max-age=0'
    resp.headers['Pragma'] = 'no-cache'
    return resp

@app.route('/investigate', methods=['POST'])
def investigate():
    data = request.get_json(force=True, silent=True) or {}
    name   = data.get('name','').strip()
    user   = data.get('user','').strip()
    email  = data.get('email','').strip()
    domain = data.get('domain','').strip()

    q = queue.Queue()
    threads = []

    if user:
        for fn, arg in [(run_sherlock, user), (run_maigret, user)]:
            t = threading.Thread(target=fn, args=(arg, q), daemon=True)
            threads.append(t); t.start()
    elif name:
        slug = name.replace(' ','_').lower()
        t = threading.Thread(target=run_maigret, args=(slug, q), daemon=True)
        threads.append(t); t.start()
    else:
        q.put({"module":"sherlock","type":"info","text":"⏭ Sherlock/Maigret: username не указан"})
        q.put({"module":"maigret","type":"done","text":"Maigret: пропуск"})

    if email:
        t = threading.Thread(target=run_holehe, args=(email, q), daemon=True)
        threads.append(t); t.start()
    else:
        q.put({"module":"holehe","type":"info","text":"⏭ Holehe: email не указан"})

    target_d = domain or (email.split('@')[1] if '@' in email else '')
    if target_d:
        t = threading.Thread(target=run_whois_dns, args=(target_d, q), daemon=True)
        threads.append(t); t.start()
        t2 = threading.Thread(target=run_ip, args=(target_d, q), daemon=True)
        threads.append(t2); t2.start()
    else:
        q.put({"module":"whois","type":"info","text":"⏭ WHOIS: домен не указан"})
        q.put({"module":"ip","type":"info","text":"⏭ IP: домен не указан"})

    dork_t = name or user or email or domain
    if dork_t:
        t = threading.Thread(target=run_dorks, args=(dork_t, q), daemon=True)
        threads.append(t); t.start()

    def stream():
        yield ': heartbeat\n\n'
        collected = []
        deadline = time.time() + 180
        while time.time() < deadline:
            try:
                ev = q.get(timeout=0.3)
                yield 'data: ' + json.dumps(ev) + '\n\n'
                if ev.get('type') == 'found':
                    collected.append(ev.get('text',''))
            except queue.Empty:
                if all(not t.is_alive() for t in threads) and q.empty():
                    break

        # AI analysis
        ai_q = queue.Queue()
        summary = ' | '.join(filter(None,[name,user,email,domain]))
        ai_t = threading.Thread(target=run_ai_analysis, args=(summary, collected, ai_q), daemon=True)
        ai_t.start()
        deadline2 = time.time() + 120
        while time.time() < deadline2:
            try:
                ev = ai_q.get(timeout=0.3)
                yield 'data: ' + json.dumps(ev) + '\n\n'
            except queue.Empty:
                if not ai_t.is_alive() and ai_q.empty():
                    break

        yield 'data: [DONE]\n\n'

    return Response(stream_with_context(stream()), headers={
        'Content-Type':'text/event-stream','Cache-Control':'no-cache',
        'X-Accel-Buffering':'no','Connection':'keep-alive'})

@app.route('/chat', methods=['POST'])
def chat():
    data = request.get_json(force=True, silent=True) or {}
    messages = data.get('messages', [])
    api_key = os.environ.get('NVIDIA_NIM_API_KEY','')

    from openai import OpenAI
    client = OpenAI(base_url="https://integrate.api.nvidia.com/v1", api_key=api_key)

    def generate():
        yield ': heartbeat\n\n'
        try:
            if not api_key:
                yield 'data: ' + json.dumps({"type":"error","content":"API ключ не настроен."}) + '\n\n'
                yield 'data: [DONE]\n\n'; return
            comp = client.chat.completions.create(
                model="nvidia/nemotron-3-ultra-550b-a55b",
                messages=[{"role":"system","content":SYSTEM_PROMPT}]+messages,
                temperature=1, top_p=0.95, max_tokens=8192,
                extra_body={"chat_template_kwargs":{"enable_thinking":True},"reasoning_budget":8192},
                stream=True)
            for chunk in comp:
                if not chunk.choices: continue
                r = getattr(chunk.choices[0].delta,'reasoning_content',None)
                if r: yield 'data: ' + json.dumps({"type":"thinking","content":r}) + '\n\n'
                c = chunk.choices[0].delta.content
                if c: yield 'data: ' + json.dumps({"type":"content","content":c}) + '\n\n'
            yield 'data: [DONE]\n\n'
        except Exception as e:
            yield 'data: ' + json.dumps({"type":"error","content":str(e)}) + '\n\n'
            yield 'data: [DONE]\n\n'

    return Response(stream_with_context(generate()), headers={
        'Content-Type':'text/event-stream','Cache-Control':'no-cache',
        'X-Accel-Buffering':'no','Connection':'keep-alive'})

if __name__ == '__main__':
    print(f"🚀 OSINT NEXUS: http://{HOST}:{PORT}")
    app.run(host=HOST, port=PORT, debug=False, threaded=True)
