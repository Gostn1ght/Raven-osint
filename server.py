import os, json, subprocess, threading, queue, time, socket, re, secrets, hashlib
import urllib.request, urllib.error, urllib.parse
from datetime import datetime, timedelta
from flask import (Flask, request, Response, stream_with_context,
                   render_template, session, redirect, url_for, jsonify)
from flask_sqlalchemy import SQLAlchemy
from werkzeug.security import generate_password_hash, check_password_hash
from functools import wraps

app = Flask(__name__)
app.secret_key = os.environ.get('SECRET_KEY', secrets.token_hex(32))
app.config['SQLALCHEMY_DATABASE_URI'] = 'sqlite:///ravens.db'
app.config['SQLALCHEMY_TRACK_MODIFICATIONS'] = False
app.config['SESSION_COOKIE_HTTPONLY'] = True
app.config['SESSION_COOKIE_SAMESITE'] = 'Lax'
app.config['PERMANENT_SESSION_LIFETIME'] = timedelta(days=7)
PORT, HOST = 5000, "0.0.0.0"

db = SQLAlchemy(app)

# ══════════════════════════════════════════
# MODELS
# ══════════════════════════════════════════
class User(db.Model):
    id         = db.Column(db.Integer, primary_key=True)
    username   = db.Column(db.String(40), unique=True, nullable=False)
    email      = db.Column(db.String(120), unique=True, nullable=False)
    pw_hash    = db.Column(db.String(256), nullable=False)
    avatar_url = db.Column(db.String(500), default='')
    banner_url = db.Column(db.String(500), default='')
    bio        = db.Column(db.String(300), default='')
    rank       = db.Column(db.String(40), default='ANALYST')
    created_at = db.Column(db.DateTime, default=datetime.utcnow)
    last_login = db.Column(db.DateTime)
    api_key    = db.Column(db.String(64), unique=True)
    login_attempts = db.Column(db.Integer, default=0)
    locked_until   = db.Column(db.DateTime)
    databases  = db.relationship('RavenDB', backref='owner', lazy=True)

    def set_password(self, pw):
        self.pw_hash = generate_password_hash(pw, method='pbkdf2:sha256', salt_length=16)

    def check_password(self, pw):
        return check_password_hash(self.pw_hash, pw)

    def gen_api_key(self):
        self.api_key = 'rvn_' + secrets.token_urlsafe(40)

class RavenDB(db.Model):
    id          = db.Column(db.Integer, primary_key=True)
    user_id     = db.Column(db.Integer, db.ForeignKey('user.id'), nullable=False)
    name        = db.Column(db.String(120), nullable=False)
    description = db.Column(db.String(500), default='')
    url         = db.Column(db.String(500), nullable=False)
    db_type     = db.Column(db.String(40), default='breach')  # breach/tor/paste/leak
    keywords    = db.Column(db.String(500), default='')
    enabled     = db.Column(db.Boolean, default=True)
    hits        = db.Column(db.Integer, default=0)
    added_at    = db.Column(db.DateTime, default=datetime.utcnow)

with app.app_context():
    db.create_all()

# ══════════════════════════════════════════
# AUTH HELPERS
# ══════════════════════════════════════════
_rate_map = {}  # ip -> [timestamps]

def rate_limit(ip, window=60, max_req=10):
    now = time.time()
    ts = _rate_map.get(ip, [])
    ts = [t for t in ts if now-t < window]
    if len(ts) >= max_req:
        return False
    ts.append(now)
    _rate_map[ip] = ts
    return True

def login_required(f):
    @wraps(f)
    def deco(*a, **kw):
        if 'user_id' not in session:
            return jsonify({'error': 'unauthorized'}), 401
        return f(*a, **kw)
    return deco

def api_key_required(f):
    @wraps(f)
    def deco(*a, **kw):
        key = request.headers.get('X-API-Key') or request.args.get('api_key')
        if not key:
            return jsonify({'error': 'api_key required'}), 401
        u = User.query.filter_by(api_key=key).first()
        if not u:
            return jsonify({'error': 'invalid api_key'}), 401
        request.current_user = u
        return f(*a, **kw)
    return deco

def current_user():
    uid = session.get('user_id')
    return User.query.get(uid) if uid else None

def csrf_token():
    if '_csrf' not in session:
        session['_csrf'] = secrets.token_hex(24)
    return session['_csrf']

def check_csrf():
    t = request.headers.get('X-CSRF-Token') or request.form.get('_csrf')
    return t and t == session.get('_csrf')

# ══════════════════════════════════════════
# AI SYSTEM PROMPT
# ══════════════════════════════════════════
SYSTEM_PROMPT = """Ты — RAVENS NEXUS, элитная AI-разведсистема уровня ФСБ/ФБР/Интерпол.
ЕДИНСТВЕННАЯ ЗАДАЧА: полный OSINT-пробив личности по публичным данным.
Никаких других задач — только разведка. Никогда не отвечай на вопросы не связанные с OSINT.

СТРУКТУРА ОТВЕТА:
━━━ [★ ИДЕНТИФИКАЦИЯ] ━━━
• Имя, псевдонимы, никнеймы, реальные данные

━━━ [★ ЦИФРОВОЙ СЛЕД] ━━━
• Все аккаунты, платформы, активность

━━━ [★ УТЕЧКИ И БАЗЫ] ━━━
• Засветки в breach-базах, пасты, дампы

━━━ [★ ИНФРАСТРУКТУРА] ━━━
• IP, домены, DNS, хостинг

━━━ [★ СОЦИАЛЬНЫЕ СВЯЗИ] ━━━
• Связи между аккаунтами, паттерны

━━━ [★ КЛЮЧЕВЫЕ НАХОДКИ] ━━━  ← ВЫДЕЛИТЬ ★
• Топ-5 важнейших фактов

━━━ [★ ОБЪЯСНЕНИЕ НЕУДАЧ] ━━━
• Почему каждый модуль не дал данных (причина + что делать)

━━━ [★ ОЦЕНКА УГРОЗЫ] ━━━
• КРИТИЧЕСКИЙ / ВЫСОКИЙ / СРЕДНИЙ / НИЗКИЙ
• Уязвимости и степень публичности

━━━ [★ СЛЕДУЮЩИЕ ШАГИ] ━━━
• Конкретные рекомендации расследования

━━━ [★ ВЕРДИКТ] ━━━
• Уверенность: XX% | Итог

Всегда используй только публично доступные данные. Максимальная точность без домыслов."""

UNBUF_ENV = {**os.environ, 'PYTHONUNBUFFERED': '1', 'NO_COLOR': '1'}

# ══════════════════════════════════════════
# OSINT MODULES
# ══════════════════════════════════════════
def run_sherlock(username, q):
    try:
        q.put({"module":"sherlock","type":"running","text":f"Sherlock: '{username}' на 400+ сайтах..."})
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

def run_holehe(email, q):
    try:
        q.put({"module":"holehe","type":"running","text":f"Holehe: email на 120+ сервисах..."})
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
        q.put({"module":"holehe","type":"done","text":f"Holehe: {len(found)} регистраций"})
    except Exception as e:
        q.put({"module":"holehe","type":"error","text":f"Holehe: {e}"})
        q.put({"module":"holehe","type":"done","text":"Holehe: ошибка"})

def run_phone_osint(phone, q):
    q.put({"module":"phone","type":"running","text":f"Phone OSINT: '{phone}'..."})
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
        if country: q.put({"module":"phone","type":"found","text":f"Страна: {country}"})
        car = carrier.name_for_number(parsed, 'ru')
        if car: q.put({"module":"phone","type":"found","text":f"Оператор: {car}"})
        zones = list(tz_mod.time_zones_for_number(parsed))
        if zones: q.put({"module":"phone","type":"found","text":f"Timezone: {zones[0]}"})
        ntype = phonenumbers.number_type(parsed)
        type_map={0:"FIXED",1:"MOBILE",2:"FIXED/MOBILE",3:"TOLL_FREE",4:"PREMIUM",6:"VOIP",7:"PERSONAL",10:"UAN"}
        q.put({"module":"phone","type":"found","text":f"Тип: {type_map.get(ntype,'UNKNOWN')}"})
    except Exception as e:
        q.put({"module":"phone","type":"error","text":f"Phone: {e}"})
    clean = re.sub(r'[^\d+]','',phone)
    for d in [f'"{clean}"',f'site:vk.com "{clean}"',f'site:telegram.me "{clean}"',
              f'site:truecaller.com "{clean}"',f'site:getcontact.com "{clean}"']:
        q.put({"module":"phone","type":"found","text":f"Dork: {d}"})
    q.put({"module":"phone","type":"done","text":"Phone OSINT: завершён"})

def run_hibp(email, q):
    q.put({"module":"hibp","type":"running","text":f"HIBP: проверка в базах утечек..."})
    try:
        url = f"https://haveibeenpwned.com/api/v2/breachedaccount/{urllib.parse.quote(email)}?truncateResponse=false"
        req = urllib.request.Request(url, headers={"User-Agent":"Ravens-OSINT/3.0","Accept":"application/json"})
        try:
            with urllib.request.urlopen(req, timeout=12) as r:
                data = json.loads(r.read())
                for b in data:
                    dc = ', '.join(b.get('DataClasses',[])[:4])
                    q.put({"module":"hibp","type":"found",
                           "text":f"★ УТЕЧКА [{b.get('Name')}] {b.get('BreachDate')} — {b.get('PwnCount',0):,} жертв | {dc}"})
        except urllib.error.HTTPError as e:
            codes = {404:"не найден в утечках",429:"лимит запросов",401:"нужен API ключ"}
            q.put({"module":"hibp","type":"info","text":f"HIBP: {codes.get(e.code,str(e.code))}"})
    except Exception as e:
        q.put({"module":"hibp","type":"error","text":f"HIBP: {e}"})
    for d in [f'site:dehashed.com "{email}"',f'site:leakcheck.io "{email}"',f'site:snusbase.com "{email}"']:
        q.put({"module":"hibp","type":"found","text":f"Dork: {d}"})
    q.put({"module":"hibp","type":"done","text":"HIBP: завершён"})

def run_social_check(username, q):
    q.put({"module":"social","type":"running","text":f"Social: '{username}' на платформах..."})
    found_count = 0
    try:
        url = f"https://www.reddit.com/user/{username}/about.json"
        req = urllib.request.Request(url, headers={"User-Agent":"Ravens-OSINT/3.0"})
        with urllib.request.urlopen(req, timeout=8) as r:
            d = json.loads(r.read()).get('data',{})
            karma = d.get('link_karma',0)+d.get('comment_karma',0)
            cr = datetime.fromtimestamp(d.get('created_utc',0)).strftime('%Y-%m-%d')
            q.put({"module":"social","type":"found","text":f"Reddit: u/{username} karma:{karma} с {cr}"})
            found_count += 1
    except: pass
    try:
        url = f"https://vk.com/{username}"
        req = urllib.request.Request(url, headers={"User-Agent":"Mozilla/5.0 (Windows NT 10.0)"})
        with urllib.request.urlopen(req, timeout=8) as r:
            html = r.read().decode('utf-8','ignore')
            if 'page_not_found' not in html and 'id=' in html:
                q.put({"module":"social","type":"found","text":f"VK: vk.com/{username} — профиль найден"})
                found_count += 1
    except: pass
    for p in [f"https://instagram.com/{username}/",f"https://twitter.com/{username}",
              f"https://tiktok.com/@{username}",f"https://twitch.tv/{username}",
              f"https://steamcommunity.com/id/{username}",f"https://open.spotify.com/user/{username}",
              f"https://soundcloud.com/{username}",f"https://pinterest.com/{username}/",
              f"https://youtube.com/@{username}"]:
        q.put({"module":"social","type":"found","text":p})
    q.put({"module":"social","type":"done","text":f"Social: {found_count} активных"})

def run_github(username, q):
    q.put({"module":"github","type":"running","text":f"GitHub: '{username}'..."})
    try:
        url = f"https://api.github.com/users/{username}"
        req = urllib.request.Request(url, headers={"User-Agent":"Ravens-OSINT/3.0","Accept":"application/vnd.github.v3+json"})
        with urllib.request.urlopen(req, timeout=10) as r:
            d = json.loads(r.read())
            for label,key in [("Имя","name"),("Email","email"),("Компания","company"),
                              ("Локация","location"),("Bio","bio"),("Сайт","blog"),
                              ("Repos","public_repos"),("Followers","followers"),("Gists","public_gists")]:
                if d.get(key): q.put({"module":"github","type":"found","text":f"GitHub {label}: {d[key]}"})
            url2 = f"https://api.github.com/users/{username}/repos?per_page=6&sort=pushed"
            req2 = urllib.request.Request(url2, headers={"User-Agent":"Ravens-OSINT/3.0"})
            with urllib.request.urlopen(req2, timeout=10) as r2:
                for repo in json.loads(r2.read())[:5]:
                    q.put({"module":"github","type":"found",
                           "text":f"Repo: {repo['name']} [{repo.get('language','?')}] {(repo.get('description') or '')[:50]}"})
    except urllib.error.HTTPError as e:
        q.put({"module":"github","type":"info","text":f"GitHub: {'не найден' if e.code==404 else f'HTTP {e.code}'}"})
    except Exception as e:
        q.put({"module":"github","type":"error","text":f"GitHub: {e}"})
    q.put({"module":"github","type":"done","text":"GitHub: завершён"})

def run_telegram_check(username, q):
    q.put({"module":"telegram","type":"running","text":f"Telegram: '@{username}'..."})
    clean = username.lstrip('@')
    try:
        url = f"https://t.me/{clean}"
        req = urllib.request.Request(url, headers={"User-Agent":"Mozilla/5.0 TelegramBot"})
        with urllib.request.urlopen(req, timeout=10) as r:
            html = r.read().decode('utf-8','ignore')
            title = re.search(r'<meta property="og:title" content="([^"]*)"', html)
            desc = re.search(r'<meta property="og:description" content="([^"]*)"', html)
            members = re.search(r'([\d\s]+)\s+(member|subscriber|participant)', html)
            if title: q.put({"module":"telegram","type":"found","text":f"TG Имя: {title.group(1)}"})
            if desc and desc.group(1): q.put({"module":"telegram","type":"found","text":f"TG Bio: {desc.group(1)[:200]}"})
            if members: q.put({"module":"telegram","type":"found","text":f"TG Участники: {members.group(1)} {members.group(2)}"})
            q.put({"module":"telegram","type":"found","text":f"TG URL: https://t.me/{clean}"})
    except Exception as e:
        q.put({"module":"telegram","type":"info","text":f"TG: {e}"})
    q.put({"module":"telegram","type":"done","text":"Telegram: завершён"})

def run_whois_dns(target, q):
    q.put({"module":"whois","type":"running","text":f"WHOIS/DNS: '{target}'..."})
    try:
        import whois
        w = whois.whois(target)
        for f,v in [("Домен",w.domain_name),("Регистратор",w.registrar),
                    ("Создан",w.creation_date),("Истекает",w.expiration_date),
                    ("Email",w.emails),("Страна",w.country),("Org",w.org),("NS",w.name_servers)]:
            if v: q.put({"module":"whois","type":"found","text":f"WHOIS {f}: {v}"})
    except Exception as e:
        q.put({"module":"whois","type":"error","text":f"WHOIS: {e}"})
    try:
        import dns.resolver
        for rtype in ["A","AAAA","MX","NS","TXT","CNAME"]:
            try:
                for rr in dns.resolver.resolve(target, rtype, lifetime=5):
                    q.put({"module":"whois","type":"found","text":f"DNS {rtype}: {rr}"})
            except: pass
    except: pass
    q.put({"module":"whois","type":"done","text":"WHOIS/DNS: завершён"})

def run_ip(target, q):
    q.put({"module":"ip","type":"running","text":f"IP Geo: '{target}'..."})
    try:
        ip = socket.gethostbyname(target)
        q.put({"module":"ip","type":"found","text":f"IP: {ip}"})
        with urllib.request.urlopen(f"http://ip-api.com/json/{ip}?fields=country,regionName,city,isp,org,as,reverse,timezone,proxy,hosting,query", timeout=8) as r:
            d = json.loads(r.read())
        labels={"country":"Страна","regionName":"Регион","city":"Город","isp":"ISP","org":"Org",
                "as":"AS","reverse":"rDNS","timezone":"TZ","proxy":"Proxy/VPN","hosting":"Хостинг"}
        for k,v in d.items():
            if v and k!='query' and v is not False:
                q.put({"module":"ip","type":"found","text":f"IP {labels.get(k,k)}: {v}"})
        q.put({"module":"ip","type":"found","text":f"Shodan: https://www.shodan.io/host/{ip}"})
        q.put({"module":"ip","type":"found","text":f"VirusTotal: https://www.virustotal.com/gui/ip-address/{ip}"})
    except Exception as e:
        q.put({"module":"ip","type":"error","text":f"IP: {e}"})
    q.put({"module":"ip","type":"done","text":"IP Geo: завершён"})

def run_dorks(target, q):
    q.put({"module":"dorks","type":"running","text":f"Google Dorks: '{target}'..."})
    dorks = [
        f'"{target}" site:linkedin.com', f'"{target}" site:facebook.com',
        f'"{target}" site:vk.com', f'"{target}" site:instagram.com',
        f'"{target}" site:twitter.com', f'"{target}" site:github.com',
        f'"{target}" site:reddit.com', f'"{target}" filetype:pdf',
        f'"{target}" filetype:doc', f'"{target}" inurl:profile',
        f'intitle:"{target}"', f'"{target}" (email OR phone OR телефон)',
        f'"{target}" (password OR пароль OR hash)', f'"{target}" site:pastebin.com',
        f'"{target}" site:ru', f'cache:"{target}"',
        f'"{target}" site:youtube.com', f'"{target}" leaked breach',
    ]
    for d in dorks:
        q.put({"module":"dorks","type":"found","text":d})
        time.sleep(0.03)
    q.put({"module":"dorks","type":"done","text":f"Dorks: {len(dorks)} запросов"})

def run_paste_search(target, q):
    q.put({"module":"paste","type":"running","text":f"Paste/Doxbin: '{target}'..."})
    try:
        url = f"https://doxbin.org/search/{urllib.parse.quote(target)}"
        req = urllib.request.Request(url, headers={"User-Agent":"Mozilla/5.0"})
        with urllib.request.urlopen(req, timeout=10) as r:
            html = r.read().decode('utf-8','ignore')
            results = re.findall(r'href="/upload/([^"]+)"[^>]*>([^<]+)<', html)
            for slug,title in results[:10]:
                q.put({"module":"paste","type":"found","text":f"★ DOXBIN: {title.strip()} -> doxbin.org/upload/{slug}"})
    except: pass
    for s in ["pastebin.com","ghostbin.co","doxbin.org","justpaste.it","paste.ee","hastebin.com"]:
        q.put({"module":"paste","type":"found","text":f'site:{s} "{target}"'})
    q.put({"module":"paste","type":"done","text":"Paste/Doxbin: завершён"})

def run_darkweb(target, q):
    q.put({"module":"darkweb","type":"running","text":f"Dark Web: '{target}' через Ahmia..."})
    found = 0
    try:
        url = f"https://ahmia.fi/search/?q={urllib.parse.quote(target)}"
        req = urllib.request.Request(url, headers={"User-Agent":"Mozilla/5.0"})
        with urllib.request.urlopen(req, timeout=15) as r:
            html = r.read().decode('utf-8','ignore')
            titles = re.findall(r'<h4[^>]*>\s*<a href="([^"]+)"[^>]*>([^<]+)</a>', html)
            for href,title in titles[:10]:
                q.put({"module":"darkweb","type":"found","text":f"DW: {title.strip()[:70]}"})
                found += 1
    except Exception as e:
        q.put({"module":"darkweb","type":"info","text":f"Ahmia: {e}"})
    for d in [f'site:onion.link "{target}"',f'site:tor2web.org "{target}"',
              f'"{target}" site:ddosecrets.com',f'"{target}" site:wikileaks.org']:
        q.put({"module":"darkweb","type":"found","text":d})
    q.put({"module":"darkweb","type":"done","text":f"Dark Web: {found} результатов"})

def run_intelx(target, q):
    q.put({"module":"intelx","type":"running","text":f"IntelX: '{target}'..."})
    try:
        url = f"https://intelx.io/api?term={urllib.parse.quote(target)}&buckets=&k=&media=0&sort=4&terminate=&timeout=20"
        req = urllib.request.Request(url, headers={"User-Agent":"Mozilla/5.0","x-key":"at0ZGa29oCBKQs5AaYLa","Accept":"application/json"})
        with urllib.request.urlopen(req, timeout=12) as r:
            data = json.loads(r.read())
            if isinstance(data,dict) and 'records' in data:
                for rec in data['records'][:8]:
                    q.put({"module":"intelx","type":"found","text":f"IntelX: {rec.get('name','?')} [{rec.get('date','?')[:10]}]"})
    except: pass
    for d in [f'site:dehashed.com "{target}"',f'site:leakcheck.io "{target}"',
              f'"{target}" intext:password',f'"{target}" intext:hash',f'"{target}" leaked']:
        q.put({"module":"intelx","type":"found","text":d})
    q.put({"module":"intelx","type":"done","text":"IntelX: завершён"})

def run_ravendb_search(target, databases, q):
    """Search user-defined RavenDB databases"""
    q.put({"module":"ravendb","type":"running","text":f"RavenDB: поиск в {len(databases)} базах..."})
    total_found = 0
    for db_entry in databases:
        if not db_entry.get('enabled', True): continue
        name = db_entry.get('name','?')
        url = db_entry.get('url','')
        keywords = db_entry.get('keywords','').lower().split(',')
        q.put({"module":"ravendb","type":"info","text":f"RavenDB [{name}]: проверяю..."})
        try:
            search_url = url
            if '?' in url:
                search_url += '&q=' + urllib.parse.quote(target)
            else:
                search_url += '?q=' + urllib.parse.quote(target)
            req = urllib.request.Request(search_url, headers={"User-Agent":"Ravens-OSINT/3.0"})
            with urllib.request.urlopen(req, timeout=10) as r:
                content = r.read().decode('utf-8','ignore')
                if target.lower() in content.lower():
                    q.put({"module":"ravendb","type":"found","text":f"★ RavenDB [{name}]: совпадение найдено! {url}"})
                    db_entry['hits'] = db_entry.get('hits',0) + 1
                    total_found += 1
                else:
                    q.put({"module":"ravendb","type":"info","text":f"RavenDB [{name}]: не найдено"})
        except Exception as e:
            q.put({"module":"ravendb","type":"info","text":f"RavenDB [{name}]: недоступна ({type(e).__name__})"})
    q.put({"module":"ravendb","type":"done","text":f"RavenDB: проверено {len(databases)} баз, {total_found} совпадений"})

def run_ai_analysis(target_info, collected, module_outcomes, q):
    q.put({"module":"ai","type":"running","text":"Ravens AI: NVIDIA NIM Nemotron-Ultra анализ..."})
    api_key = os.environ.get("NVIDIA_NIM_API_KEY","")
    if not api_key:
        q.put({"module":"ai","type":"error","text":"AI: API ключ не настроен"})
        q.put({"module":"ai","type":"done","text":"AI: ошибка"}); return
    try:
        from openai import OpenAI
        client = OpenAI(base_url="https://integrate.api.nvidia.com/v1", api_key=api_key)
        findings = "\n".join(collected[:300]) if collected else "Данные не найдены."
        outcomes = "\n".join([f"- {m}: {s}" for m,s in module_outcomes.items()])
        comp = client.chat.completions.create(
            model="nvidia/nemotron-3-ultra-550b-a55b",
            messages=[{"role":"system","content":SYSTEM_PROMPT},
                      {"role":"user","content":f"ЦЕЛЬ: {target_info}\n\nНАХОДКИ ({len(collected)}):\n{findings}\n\nСТАТУС МОДУЛЕЙ:\n{outcomes}\n\nСоставь полное досье."}],
            temperature=0.5, max_tokens=8000, stream=True)
        for chunk in comp:
            if not chunk.choices: continue
            c = chunk.choices[0].delta.content
            if c: q.put({"module":"ai","type":"stream","text":c})
        q.put({"module":"ai","type":"done","text":"Ravens AI: завершён"})
    except Exception as e:
        q.put({"module":"ai","type":"error","text":f"AI: {e}"})
        q.put({"module":"ai","type":"done","text":"AI: ошибка"})

# ══════════════════════════════════════════
# ROUTES — AUTH
# ══════════════════════════════════════════
@app.route('/auth/register', methods=['POST'])
def auth_register():
    ip = request.remote_addr
    if not rate_limit(ip, window=300, max_req=5):
        return jsonify({'error':'Слишком много попыток. Подождите.'}), 429
    if not check_csrf():
        return jsonify({'error':'CSRF validation failed'}), 403
    data = request.get_json(force=True, silent=True) or {}
    username = data.get('username','').strip()
    email = data.get('email','').strip().lower()
    password = data.get('password','')
    if not username or not email or not password:
        return jsonify({'error':'Все поля обязательны'}), 400
    if not re.match(r'^[a-zA-Z0-9_]{3,40}$', username):
        return jsonify({'error':'Имя пользователя: 3-40 символов, только буквы/цифры/_'}), 400
    if not re.match(r'^[^@]+@[^@]+\.[^@]+$', email):
        return jsonify({'error':'Неверный email'}), 400
    if len(password) < 8:
        return jsonify({'error':'Пароль минимум 8 символов'}), 400
    if User.query.filter_by(username=username).first():
        return jsonify({'error':'Имя занято'}), 409
    if User.query.filter_by(email=email).first():
        return jsonify({'error':'Email уже зарегистрирован'}), 409
    u = User(username=username, email=email)
    u.set_password(password)
    u.gen_api_key()
    db.session.add(u); db.session.commit()
    session.permanent = True
    session['user_id'] = u.id
    return jsonify({'ok':True,'username':u.username,'api_key':u.api_key})

@app.route('/auth/login', methods=['POST'])
def auth_login():
    ip = request.remote_addr
    if not rate_limit(ip, window=60, max_req=8):
        return jsonify({'error':'Слишком много попыток'}), 429
    if not check_csrf():
        return jsonify({'error':'CSRF validation failed'}), 403
    data = request.get_json(force=True, silent=True) or {}
    login = data.get('login','').strip().lower()
    password = data.get('password','')
    u = User.query.filter((User.username==login)|(User.email==login)).first()
    if not u:
        return jsonify({'error':'Неверные данные'}), 401
    if u.locked_until and datetime.utcnow() < u.locked_until:
        remaining = int((u.locked_until - datetime.utcnow()).total_seconds())
        return jsonify({'error':f'Аккаунт заблокирован на {remaining}с'}), 423
    if not u.check_password(password):
        u.login_attempts = (u.login_attempts or 0) + 1
        if u.login_attempts >= 5:
            u.locked_until = datetime.utcnow() + timedelta(minutes=15)
            u.login_attempts = 0
        db.session.commit()
        return jsonify({'error':'Неверные данные'}), 401
    u.login_attempts = 0
    u.locked_until = None
    u.last_login = datetime.utcnow()
    db.session.commit()
    session.permanent = True
    session['user_id'] = u.id
    return jsonify({'ok':True,'username':u.username,'rank':u.rank,'avatar':u.avatar_url})

@app.route('/auth/logout', methods=['POST'])
def auth_logout():
    session.clear()
    return jsonify({'ok':True})

@app.route('/auth/me')
def auth_me():
    u = current_user()
    if not u: return jsonify({'logged_in':False})
    return jsonify({'logged_in':True,'username':u.username,'email':u.email,
                    'rank':u.rank,'avatar':u.avatar_url,'banner':u.banner_url,
                    'bio':u.bio,'api_key':u.api_key,'created_at':u.created_at.isoformat()})

@app.route('/auth/update', methods=['POST'])
@login_required
def auth_update():
    if not check_csrf():
        return jsonify({'error':'CSRF failed'}), 403
    u = current_user()
    data = request.get_json(force=True, silent=True) or {}
    if 'bio' in data: u.bio = data['bio'][:300]
    if 'avatar_url' in data:
        av = data['avatar_url'][:500]
        if av and not re.match(r'^https?://', av):
            return jsonify({'error':'Некорректный URL аватара'}), 400
        u.avatar_url = av
    if 'banner_url' in data:
        bv = data['banner_url'][:500]
        if bv and not re.match(r'^https?://', bv):
            return jsonify({'error':'Некорректный URL баннера'}), 400
        u.banner_url = bv
    if 'new_password' in data:
        if len(data['new_password']) < 8:
            return jsonify({'error':'Пароль мин 8 символов'}), 400
        if not u.check_password(data.get('current_password','')):
            return jsonify({'error':'Неверный текущий пароль'}), 401
        u.set_password(data['new_password'])
    db.session.commit()
    return jsonify({'ok':True})

@app.route('/auth/csrf')
def get_csrf():
    return jsonify({'token': csrf_token()})

# ══════════════════════════════════════════
# ROUTES — RAVENDB
# ══════════════════════════════════════════
@app.route('/api/ravendb', methods=['GET'])
@login_required
def list_ravendb():
    u = current_user()
    dbs = RavenDB.query.filter_by(user_id=u.id).all()
    return jsonify([{'id':d.id,'name':d.name,'description':d.description,'url':d.url,
                     'db_type':d.db_type,'keywords':d.keywords,'enabled':d.enabled,'hits':d.hits,
                     'added_at':d.added_at.isoformat()} for d in dbs])

@app.route('/api/ravendb', methods=['POST'])
@login_required
def add_ravendb():
    if not check_csrf(): return jsonify({'error':'CSRF failed'}), 403
    u = current_user()
    data = request.get_json(force=True, silent=True) or {}
    name = data.get('name','').strip()
    url = data.get('url','').strip()
    if not name or not url: return jsonify({'error':'name и url обязательны'}), 400
    if not re.match(r'^https?://', url): return jsonify({'error':'URL должен начинаться с http/https'}), 400
    if RavenDB.query.filter_by(user_id=u.id).count() >= 50:
        return jsonify({'error':'Максимум 50 баз'}), 400
    d = RavenDB(user_id=u.id, name=name, url=url,
                description=data.get('description','')[:500],
                db_type=data.get('db_type','breach'),
                keywords=data.get('keywords','')[:500])
    db.session.add(d); db.session.commit()
    return jsonify({'ok':True,'id':d.id})

@app.route('/api/ravendb/<int:db_id>', methods=['DELETE'])
@login_required
def del_ravendb(db_id):
    if not check_csrf(): return jsonify({'error':'CSRF failed'}), 403
    u = current_user()
    d = RavenDB.query.filter_by(id=db_id, user_id=u.id).first()
    if not d: return jsonify({'error':'Не найдено'}), 404
    db.session.delete(d); db.session.commit()
    return jsonify({'ok':True})

@app.route('/api/ravendb/<int:db_id>/toggle', methods=['POST'])
@login_required
def toggle_ravendb(db_id):
    u = current_user()
    d = RavenDB.query.filter_by(id=db_id, user_id=u.id).first()
    if not d: return jsonify({'error':'Не найдено'}), 404
    d.enabled = not d.enabled; db.session.commit()
    return jsonify({'ok':True,'enabled':d.enabled})

# ══════════════════════════════════════════
# ROUTES — MAIN
# ══════════════════════════════════════════
@app.route('/')
def index():
    from flask import make_response
    resp = make_response(render_template('index.html', csrf=csrf_token()))
    resp.headers['Cache-Control'] = 'no-store'
    resp.headers['X-Frame-Options'] = 'DENY'
    resp.headers['X-Content-Type-Options'] = 'nosniff'
    resp.headers['Content-Security-Policy'] = "default-src 'self'; script-src 'self' 'unsafe-inline' https://unpkg.com; style-src 'self' 'unsafe-inline'; img-src * data:; connect-src *; frame-src 'none'"
    return resp

@app.route('/static/<path:filename>')
def static_files(filename):
    from flask import send_from_directory
    return send_from_directory('static', filename)

@app.route('/investigate', methods=['POST'])
def investigate():
    ip = request.remote_addr
    if not rate_limit(ip, window=60, max_req=5):
        return Response('data: {"module":"sys","type":"error","text":"Rate limit"}\n\ndata: [DONE]\n\n',
                       content_type='text/event-stream')
    data   = request.get_json(force=True, silent=True) or {}
    name   = data.get('name','').strip()
    user   = data.get('user','').strip()
    email  = data.get('email','').strip()
    phone  = data.get('phone','').strip()
    domain = data.get('domain','').strip()
    uid    = session.get('user_id')
    user_dbs = []
    if uid:
        user_dbs = [{'id':d.id,'name':d.name,'url':d.url,'db_type':d.db_type,
                     'keywords':d.keywords,'enabled':d.enabled,'hits':d.hits}
                    for d in RavenDB.query.filter_by(user_id=uid, enabled=True).all()]

    q = queue.Queue()
    threads = []
    module_outcomes = {}
    dork_t = name or user or email or domain or phone

    def skip(mod, reason):
        module_outcomes[mod] = f"ПРОПУЩЕН — {reason}"
        q.put({"module":mod,"type":"info","text":f"{mod}: {reason}"})
        q.put({"module":mod,"type":"done","text":f"{mod}: пропущен"})

    def run_t(fn, args, mod):
        module_outcomes[mod] = "ЗАПУЩЕН"
        t = threading.Thread(target=fn, args=args+(q,), daemon=True)
        threads.append(t); t.start()

    if user: run_t(run_sherlock,(user,),"sherlock")
    else: skip("sherlock","username не указан")
    if user or name: run_t(run_maigret,((user or name).replace(' ','_').lower(),),"maigret")
    else: skip("maigret","нет цели")
    if email: run_t(run_holehe,(email,),"holehe")
    else: skip("holehe","email не указан")
    if phone: run_t(run_phone_osint,(phone,),"phone")
    else: skip("phone","телефон не указан")
    if email: run_t(run_hibp,(email,),"hibp")
    else: skip("hibp","email не указан")
    if user: run_t(run_social_check,(user,),"social")
    else: skip("social","username не указан")
    if user: run_t(run_github,(user,),"github")
    else: skip("github","username не указан")
    if user: run_t(run_telegram_check,(user,),"telegram")
    else: skip("telegram","username не указан")
    target_d = domain or (email.split('@')[1] if '@' in email else '')
    if target_d: run_t(run_whois_dns,(target_d,),"whois")
    else: skip("whois","домен не указан")
    if target_d: run_t(run_ip,(target_d,),"ip")
    else: skip("ip","домен/IP не указан")
    if dork_t: run_t(run_dorks,(dork_t,),"dorks")
    else: skip("dorks","нет цели")
    if dork_t: run_t(run_paste_search,(dork_t,),"paste")
    else: skip("paste","нет цели")
    if dork_t: run_t(run_darkweb,(dork_t,),"darkweb")
    else: skip("darkweb","нет цели")
    if dork_t: run_t(run_intelx,(dork_t,),"intelx")
    else: skip("intelx","нет цели")
    if user_dbs and dork_t:
        run_t(run_ravendb_search,(dork_t, user_dbs),"ravendb")
    else:
        skip("ravendb","нет пользовательских баз" if not user_dbs else "нет цели")

    def stream():
        yield ': ravens-init\n\n'
        collected = []
        deadline = time.time() + 300
        while time.time() < deadline:
            try:
                ev = q.get(timeout=0.2)
                yield 'data: ' + json.dumps(ev, ensure_ascii=False) + '\n\n'
                if ev.get('type') == 'found': collected.append(ev.get('text',''))
                if ev.get('type') == 'done':
                    m = ev.get('module','')
                    if m in module_outcomes and module_outcomes[m] == "ЗАПУЩЕН":
                        module_outcomes[m] = f"ВЫПОЛНЕН — {ev.get('text','')}"
            except queue.Empty:
                yield ': hb\n\n'
                if all(not t.is_alive() for t in threads) and q.empty(): break
        # AI always runs last
        ti = ' | '.join(filter(None,[name,user,email,phone,domain])) or 'unknown'
        ai_q = queue.Queue()
        ai_t = threading.Thread(target=run_ai_analysis, args=(ti,collected,module_outcomes,ai_q), daemon=True)
        ai_t.start()
        deadline2 = time.time() + 180
        while time.time() < deadline2:
            try:
                ev = ai_q.get(timeout=0.2)
                yield 'data: ' + json.dumps(ev, ensure_ascii=False) + '\n\n'
            except queue.Empty:
                yield ': hb\n\n'
                if not ai_t.is_alive() and ai_q.empty(): break
        yield 'data: [DONE]\n\n'

    return Response(stream_with_context(stream()), headers={
        'Content-Type':'text/event-stream','Cache-Control':'no-cache',
        'X-Accel-Buffering':'no','Connection':'keep-alive'})

@app.route('/chat', methods=['POST'])
def chat():
    ip = request.remote_addr
    if not rate_limit(ip, window=60, max_req=20):
        return jsonify({'error':'Rate limit'}), 429
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
                messages=[{"role":"system","content":SYSTEM_PROMPT}]+messages[-20:],
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
        'X-Accel-Buffering':'no'})

@app.route('/favicon.ico')
def favicon(): return Response(status=204)

if __name__ == '__main__':
    print(f"RAVENS NEXUS v4: http://{HOST}:{PORT}")
    app.run(host=HOST, port=PORT, debug=False, threaded=True)
