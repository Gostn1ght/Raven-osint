use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsintEvent {
    pub module: String,
    #[serde(rename = "type")]
    pub kind: String, // "running" | "found" | "info" | "error" | "done"
    pub text: String,
}

impl OsintEvent {
    pub fn new(module: &str, kind: &str, text: impl Into<String>) -> Self {
        Self {
            module: module.to_string(),
            kind: kind.to_string(),
            text: text.into(),
        }
    }
}

fn client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(12))
        .user_agent("Ravens-OSINT/3.0")
        .danger_accept_invalid_certs(false)
        .build()
        .unwrap_or_default()
}

// ── USERNAME SEARCH (социальные платформы) ──────────────────────────────────
pub async fn run_social_check(username: &str) -> Vec<OsintEvent> {
    let mut events = vec![OsintEvent::new("social", "running",
        format!("Social: '{}' на платформах...", username))];

    let c = client();

    // Reddit
    let reddit_url = format!("https://www.reddit.com/user/{}/about.json", username);
    match c.get(&reddit_url).send().await {
        Ok(r) if r.status().is_success() => {
            if let Ok(json) = r.json::<serde_json::Value>().await {
                let data = &json["data"];
                let karma = data["link_karma"].as_i64().unwrap_or(0)
                    + data["comment_karma"].as_i64().unwrap_or(0);
                events.push(OsintEvent::new("social", "found",
                    format!("Reddit: u/{} | karma: {}", username, karma)));
            }
        }
        _ => {}
    }

    // GitHub
    let gh_url = format!("https://api.github.com/users/{}", username);
    match c.get(&gh_url).header("Accept","application/vnd.github.v3+json").send().await {
        Ok(r) if r.status().is_success() => {
            if let Ok(json) = r.json::<serde_json::Value>().await {
                let name = json["name"].as_str().unwrap_or("");
                let repos = json["public_repos"].as_i64().unwrap_or(0);
                let followers = json["followers"].as_i64().unwrap_or(0);
                events.push(OsintEvent::new("social", "found",
                    format!("GitHub: {} | repos: {} | followers: {}", name, repos, followers)));
                if let Some(loc) = json["location"].as_str() {
                    if !loc.is_empty() {
                        events.push(OsintEvent::new("social", "found",
                            format!("GitHub location: {}", loc)));
                    }
                }
                if let Some(email) = json["email"].as_str() {
                    if !email.is_empty() {
                        events.push(OsintEvent::new("social", "found",
                            format!("GitHub email: {}", email)));
                    }
                }
            }
        }
        _ => {}
    }

    // Telegram
    let tg_url = format!("https://t.me/{}", username.trim_start_matches('@'));
    match c.get(&tg_url).send().await {
        Ok(r) if r.status().is_success() => {
            let html = r.text().await.unwrap_or_default();
            if !html.contains("tgme_page_extra") || html.contains("tgme_page_description") {
                let title = extract_meta(&html, "og:title");
                let desc = extract_meta(&html, "og:description");
                if !title.is_empty() {
                    events.push(OsintEvent::new("social", "found",
                        format!("Telegram: {} | {}", title, desc)));
                }
            }
        }
        _ => {}
    }

    // Платформы — просто генерируем ссылки для проверки
    let platforms = [
        ("Instagram", format!("https://instagram.com/{}/", username)),
        ("Twitter/X", format!("https://twitter.com/{}", username)),
        ("TikTok", format!("https://tiktok.com/@{}", username)),
        ("Twitch", format!("https://twitch.tv/{}", username)),
        ("Steam", format!("https://steamcommunity.com/id/{}", username)),
        ("YouTube", format!("https://youtube.com/@{}", username)),
        ("Pinterest", format!("https://pinterest.com/{}/", username)),
        ("SoundCloud", format!("https://soundcloud.com/{}", username)),
        ("Spotify", format!("https://open.spotify.com/user/{}", username)),
        ("VK", format!("https://vk.com/{}", username)),
    ];
    for (name, url) in &platforms {
        events.push(OsintEvent::new("social", "found",
            format!("{}: {}", name, url)));
    }

    events.push(OsintEvent::new("social", "done",
        format!("Social: завершён для '{}'", username)));
    events
}

// ── EMAIL / HIBP ─────────────────────────────────────────────────────────────
pub async fn run_hibp(email: &str) -> Vec<OsintEvent> {
    let mut events = vec![OsintEvent::new("hibp", "running",
        format!("HIBP: проверка '{}' в базах утечек...", email))];

    let c = client();
    let url = format!(
        "https://haveibeenpwned.com/api/v2/breachedaccount/{}?truncateResponse=false",
        urlencoding(email)
    );

    match c.get(&url)
        .header("User-Agent", "Ravens-OSINT/3.0")
        .header("Accept", "application/json")
        .send().await
    {
        Ok(r) => {
            match r.status().as_u16() {
                200 => {
                    if let Ok(breaches) = r.json::<serde_json::Value>().await {
                        if let Some(arr) = breaches.as_array() {
                            for b in arr {
                                let name = b["Name"].as_str().unwrap_or("?");
                                let date = b["BreachDate"].as_str().unwrap_or("?");
                                let count = b["PwnCount"].as_i64().unwrap_or(0);
                                let empty_dc: Vec<serde_json::Value> = vec![];
                                let classes: Vec<&str> = b["DataClasses"]
                                    .as_array().unwrap_or(&empty_dc)
                                    .iter().take(4)
                                    .filter_map(|v| v.as_str())
                                    .collect();
                                events.push(OsintEvent::new("hibp", "found",
                                    format!("★ УТЕЧКА [{}] {} — {} жертв | {}",
                                        name, date, fmt_num(count), classes.join(", "))));
                            }
                        }
                    }
                }
                404 => events.push(OsintEvent::new("hibp", "info",
                    "HIBP: не найден в утечках")),
                429 => events.push(OsintEvent::new("hibp", "info",
                    "HIBP: лимит запросов (нужен API-ключ)")),
                401 => events.push(OsintEvent::new("hibp", "info",
                    "HIBP: требуется API-ключ")),
                code => events.push(OsintEvent::new("hibp", "info",
                    format!("HIBP: HTTP {}", code))),
            }
        }
        Err(e) => events.push(OsintEvent::new("hibp", "error",
            format!("HIBP: {}", e))),
    }

    // Dorks для ручной проверки
    for dork in [
        format!("site:dehashed.com \"{}\"", email),
        format!("site:leakcheck.io \"{}\"", email),
        format!("site:snusbase.com \"{}\"", email),
        format!("site:breachdirectory.org \"{}\"", email),
    ] {
        events.push(OsintEvent::new("hibp", "found", format!("Dork: {}", dork)));
    }

    events.push(OsintEvent::new("hibp", "done", "HIBP: завершён"));
    events
}

// ── IP / GEO ──────────────────────────────────────────────────────────────────
pub async fn run_ip(target: &str) -> Vec<OsintEvent> {
    let mut events = vec![OsintEvent::new("ip", "running",
        format!("IP Geo: '{}'...", target))];

    let c = client();

    // Резолвим хост -> IP
    let ip = {
        use std::net::ToSocketAddrs;
        let addr = format!("{}:80", target);
        match addr.to_socket_addrs() {
            Ok(mut iter) => iter.next()
                .map(|a| a.ip().to_string())
                .unwrap_or_else(|| target.to_string()),
            Err(_) => target.to_string(),
        }
    };

    events.push(OsintEvent::new("ip", "found", format!("IP: {}", ip)));

    let url = format!(
        "http://ip-api.com/json/{}?fields=status,country,regionName,city,lat,lon,isp,org,as,reverse,timezone,proxy,hosting,query",
        ip
    );

    match c.get(&url).send().await {
        Ok(r) if r.status().is_success() => {
            if let Ok(d) = r.json::<serde_json::Value>().await {
                let labels = [
                    ("country", "Страна"), ("regionName", "Регион"),
                    ("city", "Город"), ("isp", "ISP"), ("org", "Org"),
                    ("as", "AS"), ("reverse", "rDNS"), ("timezone", "TZ"),
                    ("proxy", "Proxy/VPN"), ("hosting", "Хостинг"),
                ];
                for (key, label) in &labels {
                    if let Some(v) = d[key].as_str() {
                        if !v.is_empty() && v != "false" {
                            events.push(OsintEvent::new("ip", "found",
                                format!("IP {}: {}", label, v)));
                        }
                    } else if let Some(v) = d[key].as_bool() {
                        if v {
                            events.push(OsintEvent::new("ip", "found",
                                format!("IP {}: ДА", label)));
                        }
                    }
                }
                // GEO PIN
                if let (Some(lat), Some(lon)) = (d["lat"].as_f64(), d["lon"].as_f64()) {
                    let city = d["city"].as_str().unwrap_or(target);
                    let country = d["country"].as_str().unwrap_or("");
                    events.push(OsintEvent::new("ip", "found",
                        format!("★ GEO_PIN:{},{}|{}, {}|{}", lat, lon, city, country, target)));
                }
                events.push(OsintEvent::new("ip", "found",
                    format!("Shodan: https://www.shodan.io/host/{}", ip)));
                events.push(OsintEvent::new("ip", "found",
                    format!("VirusTotal: https://www.virustotal.com/gui/ip-address/{}", ip)));
            }
        }
        Err(e) => events.push(OsintEvent::new("ip", "error", format!("IP: {}", e))),
        _ => events.push(OsintEvent::new("ip", "error", "IP: неверный ответ")),
    }

    events.push(OsintEvent::new("ip", "done", "IP Geo: завершён"));
    events
}

// ── WHOIS / DNS ───────────────────────────────────────────────────────────────
pub async fn run_whois(target: &str) -> Vec<OsintEvent> {
    let mut events = vec![OsintEvent::new("whois", "running",
        format!("WHOIS/DNS: '{}'...", target))];

    let c = client();

    // Используем rdap.org как публичный WHOIS API
    let rdap_url = format!("https://rdap.org/domain/{}", target);
    match c.get(&rdap_url).send().await {
        Ok(r) if r.status().is_success() => {
            if let Ok(d) = r.json::<serde_json::Value>().await {
                if let Some(name) = d["ldhName"].as_str() {
                    events.push(OsintEvent::new("whois", "found",
                        format!("Домен: {}", name)));
                }
                // Registrar
                if let Some(entities) = d["entities"].as_array() {
                    for entity in entities {
                        let empty_arr: Vec<serde_json::Value> = vec![];
                        let roles: Vec<&str> = entity["roles"].as_array()
                            .unwrap_or(&empty_arr)
                            .iter().filter_map(|v| v.as_str()).collect();
                        if roles.contains(&"registrar") {
                            if let Some(fn_name) = entity["vcardArray"][1].as_array()
                                .and_then(|arr| arr.iter().find(|v| v[0] == "fn"))
                                .and_then(|v| v[3].as_str())
                            {
                                events.push(OsintEvent::new("whois", "found",
                                    format!("Регистратор: {}", fn_name)));
                            }
                        }
                    }
                }
                // Dates
                if let Some(events_arr) = d["events"].as_array() {
                    for ev in events_arr {
                        let action = ev["eventAction"].as_str().unwrap_or("");
                        let date = ev["eventDate"].as_str().unwrap_or("");
                        match action {
                            "registration" => events.push(OsintEvent::new("whois", "found",
                                format!("Создан: {}", &date[..10.min(date.len())]))),
                            "expiration" => events.push(OsintEvent::new("whois", "found",
                                format!("Истекает: {}", &date[..10.min(date.len())]))),
                            _ => {}
                        }
                    }
                }
                // Nameservers
                if let Some(ns_arr) = d["nameservers"].as_array() {
                    for ns in ns_arr.iter().take(4) {
                        if let Some(ns_name) = ns["ldhName"].as_str() {
                            events.push(OsintEvent::new("whois", "found",
                                format!("NS: {}", ns_name)));
                        }
                    }
                }
            }
        }
        _ => {
            events.push(OsintEvent::new("whois", "info",
                "WHOIS: RDAP недоступен, попробуйте whois вручную"));
        }
    }

    // DNS через Cloudflare DoH
    let dns_types = ["A", "AAAA", "MX", "NS", "TXT"];
    for rtype in &dns_types {
        let dns_url = format!(
            "https://cloudflare-dns.com/dns-query?name={}&type={}",
            target, rtype
        );
        match c.get(&dns_url)
            .header("Accept", "application/dns-json")
            .send().await
        {
            Ok(r) if r.status().is_success() => {
                if let Ok(d) = r.json::<serde_json::Value>().await {
                    if let Some(answers) = d["Answer"].as_array() {
                        for ans in answers.iter().take(3) {
                            if let Some(data) = ans["data"].as_str() {
                                events.push(OsintEvent::new("whois", "found",
                                    format!("DNS {}: {}", rtype, data)));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    events.push(OsintEvent::new("whois", "done", "WHOIS/DNS: завершён"));
    events
}

// ── GOOGLE DORKS ──────────────────────────────────────────────────────────────
pub fn run_dorks(target: &str) -> Vec<OsintEvent> {
    let mut events = vec![OsintEvent::new("dorks", "running",
        format!("Google Dorks: '{}'...", target))];

    let dorks = vec![
        format!("\"{}\" site:linkedin.com", target),
        format!("\"{}\" site:facebook.com", target),
        format!("\"{}\" site:vk.com", target),
        format!("\"{}\" site:instagram.com", target),
        format!("\"{}\" site:twitter.com", target),
        format!("\"{}\" site:github.com", target),
        format!("\"{}\" site:reddit.com", target),
        format!("\"{}\" filetype:pdf", target),
        format!("\"{}\" filetype:doc", target),
        format!("\"{}\" inurl:profile", target),
        format!("intitle:\"{}\"", target),
        format!("\"{}\" (email OR phone OR телефон)", target),
        format!("\"{}\" (password OR пароль OR hash)", target),
        format!("\"{}\" site:pastebin.com", target),
        format!("\"{}\" site:ru", target),
        format!("\"{}\" site:youtube.com", target),
        format!("\"{}\" leaked breach", target),
        format!("\"{}\" site:t.me", target),
    ];

    for d in &dorks {
        events.push(OsintEvent::new("dorks", "found", d.clone()));
    }

    events.push(OsintEvent::new("dorks", "done",
        format!("Dorks: {} запросов сгенерировано", dorks.len())));
    events
}

// ── PASTE / DOXBIN ────────────────────────────────────────────────────────────
pub async fn run_paste(target: &str) -> Vec<OsintEvent> {
    let mut events = vec![OsintEvent::new("paste", "running",
        format!("Paste/Doxbin: '{}'...", target))];

    let c = client();

    // Doxbin search
    let url = format!("https://doxbin.org/search/{}", urlencoding(target));
    match c.get(&url).header("User-Agent", "Mozilla/5.0").send().await {
        Ok(r) if r.status().is_success() => {
            let html = r.text().await.unwrap_or_default();
            let re = regex::Regex::new(r#"href="/upload/([^"]+)"[^>]*>([^<]+)<"#).unwrap();
            for cap in re.captures_iter(&html).take(10) {
                let slug = &cap[1];
                let title = cap[2].trim();
                events.push(OsintEvent::new("paste", "found",
                    format!("★ DOXBIN: {} -> doxbin.org/upload/{}", title, slug)));
            }
        }
        _ => {}
    }

    for site in ["pastebin.com", "ghostbin.co", "justpaste.it", "paste.ee"] {
        events.push(OsintEvent::new("paste", "found",
            format!("site:{} \"{}\"", site, target)));
    }

    events.push(OsintEvent::new("paste", "done", "Paste/Doxbin: завершён"));
    events
}

// ── DARK WEB (Ahmia) ──────────────────────────────────────────────────────────
pub async fn run_darkweb(target: &str) -> Vec<OsintEvent> {
    let mut events = vec![OsintEvent::new("darkweb", "running",
        format!("Dark Web: '{}' через Ahmia...", target))];

    let c = client();
    let url = format!("https://ahmia.fi/search/?q={}", urlencoding(target));

    match c.get(&url).header("User-Agent", "Mozilla/5.0").send().await {
        Ok(r) if r.status().is_success() => {
            let html = r.text().await.unwrap_or_default();
            let re = regex::Regex::new(r#"<h4[^>]*>\s*<a href="([^"]+)"[^>]*>([^<]+)</a>"#).unwrap();
            let mut found = 0;
            for cap in re.captures_iter(&html).take(10) {
                let title = cap[2].trim();
                events.push(OsintEvent::new("darkweb", "found",
                    format!("DW: {}", &title[..title.len().min(80)])));
                found += 1;
            }
            if found == 0 {
                events.push(OsintEvent::new("darkweb", "info",
                    "Dark Web: результатов не найдено"));
            }
        }
        Err(e) => events.push(OsintEvent::new("darkweb", "info",
            format!("Ahmia: {}", e))),
        _ => {}
    }

    for d in [
        format!("site:onion.link \"{}\"", target),
        format!("site:tor2web.org \"{}\"", target),
        format!("\"{}\" site:ddosecrets.com", target),
        format!("\"{}\" site:wikileaks.org", target),
    ] {
        events.push(OsintEvent::new("darkweb", "found", d));
    }

    events.push(OsintEvent::new("darkweb", "done", "Dark Web: завершён"));
    events
}

// ── PHONE OSINT ───────────────────────────────────────────────────────────────
pub async fn run_phone(phone: &str) -> Vec<OsintEvent> {
    let mut events = vec![OsintEvent::new("phone", "running",
        format!("Phone OSINT: '{}'...", phone))];

    let clean: String = phone.chars().filter(|c| c.is_ascii_digit() || *c == '+').collect();

    // Базовый анализ номера через публичный API Veriphone
    let c = client();
    let url2 = format!("https://api.veriphone.io/v2/verify?phone={}", clean);
    match c.get(&url2).send().await {
        Ok(r) if r.status().is_success() => {
            if let Ok(d) = r.json::<serde_json::Value>().await {
                if let Some(country) = d["country"].as_str() {
                    events.push(OsintEvent::new("phone", "found",
                        format!("Страна: {}", country)));
                }
                if let Some(carrier) = d["carrier"].as_str() {
                    if !carrier.is_empty() {
                        events.push(OsintEvent::new("phone", "found",
                            format!("Оператор: {}", carrier)));
                    }
                }
                if let Some(ptype) = d["phone_type"].as_str() {
                    events.push(OsintEvent::new("phone", "found",
                        format!("Тип: {}", ptype)));
                }
            }
        }
        _ => {}
    }

    // Dorks для номера
    for d in [
        format!("\"{}\"", clean),
        format!("site:vk.com \"{}\"", clean),
        format!("site:t.me \"{}\"", clean),
        format!("site:truecaller.com \"{}\"", clean),
        format!("site:getcontact.com \"{}\"", clean),
        format!("site:eyecon.mobi \"{}\"", clean),
    ] {
        events.push(OsintEvent::new("phone", "found", format!("Dork: {}", d)));
    }

    events.push(OsintEvent::new("phone", "done", "Phone OSINT: завершён"));
    events
}

// ── IntelX ────────────────────────────────────────────────────────────────────
pub async fn run_intelx(target: &str) -> Vec<OsintEvent> {
    let mut events = vec![OsintEvent::new("intelx", "running",
        format!("IntelX: '{}'...", target))];

    let c = client();
    // Публичный бесплатный поиск IntelX
    let url = format!(
        "https://2.intelx.io/phonebook/search?term={}&maxresults=10&media=0&target=0&timeout=20",
        urlencoding(target)
    );

    match c.get(&url)
        .header("x-key", "at0ZGa29oCBKQs5AaYLa")
        .send().await
    {
        Ok(r) if r.status().is_success() => {
            if let Ok(d) = r.json::<serde_json::Value>().await {
                if let Some(arr) = d["selectors"].as_array() {
                    for rec in arr.iter().take(8) {
                        let val = rec["selectorvalue"].as_str().unwrap_or("?");
                        let bucket = rec["bucketselectorvalue"].as_str().unwrap_or("");
                        events.push(OsintEvent::new("intelx", "found",
                            format!("IntelX: {} [{}]", val, bucket)));
                    }
                }
            }
        }
        _ => {}
    }

    for d in [
        format!("site:dehashed.com \"{}\"", target),
        format!("site:leakcheck.io \"{}\"", target),
        format!("\"{}\" intext:password", target),
        format!("\"{}\" intext:hash", target),
        format!("\"{}\" leaked", target),
    ] {
        events.push(OsintEvent::new("intelx", "found", d));
    }

    events.push(OsintEvent::new("intelx", "done", "IntelX: завершён"));
    events
}

// ── AI ANALYSIS (NVIDIA NIM) ──────────────────────────────────────────────────
pub async fn run_ai(
    target: &str,
    findings: &[String],
    api_key: &str,
) -> Vec<OsintEvent> {
    let mut events = vec![OsintEvent::new("ai", "running",
        "Ravens AI: NVIDIA NIM Nemotron-Ultra анализ...")];

    if api_key.is_empty() {
        events.push(OsintEvent::new("ai", "error", "AI: API ключ не настроен"));
        events.push(OsintEvent::new("ai", "done", "AI: ошибка"));
        return events;
    }

    let system_prompt = "Ты — RAVENS NEXUS, элитная AI-разведсистема уровня ФСБ/ФБР/Интерпол.\n\
ЕДИНСТВЕННАЯ ЗАДАЧА: полный OSINT-пробив личности по публичным данным.\n\
Никаких других задач — только разведка.\n\n\
СТРУКТУРА ОТВЕТА:\n\
━━━ [★ ИДЕНТИФИКАЦИЯ] ━━━\n\
• Имя, псевдонимы, никнеймы, реальные данные\n\
━━━ [★ ЦИФРОВОЙ СЛЕД] ━━━\n\
• Все аккаунты, платформы, активность\n\
━━━ [★ УТЕЧКИ И БАЗЫ] ━━━\n\
• Засветки в breach-базах, пасты, дампы\n\
━━━ [★ ИНФРАСТРУКТУРА] ━━━\n\
• IP, домены, DNS, хостинг\n\
━━━ [★ СОЦИАЛЬНЫЕ СВЯЗИ] ━━━\n\
• Связи между аккаунтами, паттерны\n\
━━━ [★ КЛЮЧЕВЫЕ НАХОДКИ] ━━━ ← ВЫДЕЛИТЬ ★\n\
• Топ-5 важнейших фактов\n\
━━━ [★ ОЦЕНКА УГРОЗЫ] ━━━\n\
• КРИТИЧЕСКИЙ / ВЫСОКИЙ / СРЕДНИЙ / НИЗКИЙ\n\
━━━ [★ ВЕРДИКТ] ━━━\n\
• Уверенность: XX% | Итог";

    let findings_text = findings.iter().take(300).cloned().collect::<Vec<_>>().join("\n");
    let user_msg = format!(
        "ЦЕЛЬ: {}\n\nНАХОДКИ ({}):\n{}\n\nСоставь полное досье.",
        target, findings.len(), findings_text
    );

    let body = serde_json::json!({
        "model": "nvidia/llama-3.1-nemotron-ultra-253b-v1",
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_msg}
        ],
        "temperature": 0.5,
        "max_tokens": 4000,
        "stream": false
    });

    let c = Client::builder()
        .timeout(Duration::from_secs(120))
        .user_agent("Ravens-OSINT/3.0")
        .build()
        .unwrap_or_default();

    match c.post("https://integrate.api.nvidia.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send().await
    {
        Ok(r) if r.status().is_success() => {
            if let Ok(d) = r.json::<serde_json::Value>().await {
                if let Some(content) = d["choices"][0]["message"]["content"].as_str() {
                    events.push(OsintEvent::new("ai", "stream", content.to_string()));
                }
            }
        }
        Ok(r) => {
            events.push(OsintEvent::new("ai", "error",
                format!("AI: HTTP {}", r.status())));
        }
        Err(e) => {
            events.push(OsintEvent::new("ai", "error",
                format!("AI: {}", e)));
        }
    }

    events.push(OsintEvent::new("ai", "done", "Ravens AI: завершён"));
    events
}

// ── helpers ───────────────────────────────────────────────────────────────────
fn urlencoding(s: &str) -> String {
    s.chars().map(|c| {
        if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~' {
            c.to_string()
        } else {
            format!("%{:02X}", c as u32)
        }
    }).collect()
}

fn extract_meta(html: &str, property: &str) -> String {
    let pattern = format!(r#"<meta property="{}" content="([^"]*)"#, property);
    if let Ok(re) = regex::Regex::new(&pattern) {
        if let Some(cap) = re.captures(html) {
            return cap[1].to_string();
        }
    }
    String::new()
}

fn fmt_num(n: i64) -> String {
    let s = n.abs().to_string();
    let mut out = String::new();
    let chars: Vec<char> = s.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if i > 0 && (chars.len() - i) % 3 == 0 {
            out.push(' ');
        }
        out.push(*c);
    }
    if n < 0 { format!("-{}", out) } else { out }
}
