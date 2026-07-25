<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount, tick } from "svelte";

  // ── Tabs ─────────────────────────────────────────────────────────────────────
  let activeTab = "ravens";
  let settingsSection = "server";
  let panelTab = "console";
  let filterModule = "all";

  // ── Scan state ───────────────────────────────────────────────────────────────
  let target = "";
  let targetType = "username";
  let scanning = false;
  let events: any[] = [];
  let mapPins: any[] = [];
  let selectedModules = new Set(["social", "ip_geo", "whois"]);
  let aiText = "";

  // ── Telegram → Discord forwarder ──────────────────────────────────────────────
  // Discord self-bot (target side)
  let discordToken = "";
  let discordConnected = false;
  let discordUser = "";
  let discordChannels: any[] = [];   // [{id,name,guild}]
  let discordTarget = "";            // chosen target channel id
  let discordBusy = false;
  let discordMsg = "";
  // Telegram account (source side)
  let tgApiId = "";
  let tgApiHash = "";
  let tgPhone = "";
  let tgCode = "";
  let tgPassword = "";
  let tgStage = "disconnected";      // disconnected | code_sent | password_required | authorized
  let tgUser = "";
  let tgChannels: any[] = [];        // [{id,title,username}]
  let tgSource = "";                 // chosen source channel id
  let tgBusy = false;
  let tgMsg = "";
  // Forwarding
  let fwdActive = false;

  // ── License / server ─────────────────────────────────────────────────────────
  const DEFAULT_SERVER_URL = "http://localhost:3000";
  let serverUrl = "";
  let tokenInput = "";
  let tokenStatus = "";
  let licenseTier = "FREE";
  let requestsRemaining: number = 2;
  let sessionId = "";
  let hwid = "";
  let allowedModules: string[] = ["social", "ip_geo", "whois"];

  // ── Ban ──────────────────────────────────────────────────────────────────────
  let isBanned = false;
  let banReason = "";
  let banDate = "";

  // ── News ─────────────────────────────────────────────────────────────────────
  let news: any[] = [];
  let openedNews: any = null;
  let newsLoading = false;

  // ── UI prefs ─────────────────────────────────────────────────────────────────
  let foundCount = 0;
  let moduleCount = 0;
  let threatLevel = "MINIMAL";
  let scanTime: any = 0;
  let compactMode = false;
  let showTimestamps = true;
  let autoScroll = true;
  let consoleEl: HTMLElement;

  // ── Color schemes (accent theming over a fixed dark base) ─────────────────────
  const schemes: Record<string, any> = {
    raven:  { name: "RAVEN",  accent: "#8b5cf6", accent2: "#6366f1" },
    blood:  { name: "BLOOD",  accent: "#ef4444", accent2: "#b91c1c" },
    matrix: { name: "MATRIX", accent: "#22c55e", accent2: "#15a34a" },
    ice:    { name: "ICE",    accent: "#38bdf8", accent2: "#0ea5e9" },
    gold:   { name: "GOLD",   accent: "#f59e0b", accent2: "#d97706" },
  };
  let currentScheme = "raven";
  $: scheme = schemes[currentScheme];
  $: cssVars = scheme ? `--accent:${scheme.accent};--accent2:${scheme.accent2}` : "";

  const tiers = [
    { name: "FREE",  price: "Бесплатно", period: "", requests: 2,
      features: ["2 запроса в день", "OSINT: Username, IP, WHOIS", "Лента новостей"],
      locked: ["Карта угроз", "AI-анализ", "Dark Web", "Phone OSINT"], highlight: false },
    { name: "PRO",   price: "₽990", period: "мес", requests: 1000,
      features: ["1000 запросов в день", "Все OSINT-модули", "Карта угроз", "Dark Web", "Phone OSINT", "Paste-поиск"],
      locked: ["AI-анализ (Nemotron)", "IntelX"], highlight: true },
    { name: "ELITE", price: "₽2490", period: "мес", requests: -1,
      features: ["∞ запросов", "Все модули", "AI-анализ (NVIDIA NIM)", "IntelX", "Приоритет", "Экспорт отчётов"],
      locked: [], highlight: false },
  ];

  const availableModules: [string, any][] = [
    ["social",  { icon: "◵", label: "Social / Username", pro: false }],
    ["ip_geo",  { icon: "◈", label: "IP Geolocation",    pro: false }],
    ["whois",   { icon: "◆", label: "WHOIS / DNS",        pro: false }],
    ["hibp",    { icon: "⚠", label: "HIBP Leaks",         pro: true  }],
    ["dorks",   { icon: "⌗", label: "Google Dorks",       pro: true  }],
    ["paste",   { icon: "▤", label: "Paste / Doxbin",     pro: true  }],
    ["darkweb", { icon: "☇", label: "Dark Web",           pro: true  }],
    ["phone",   { icon: "☏", label: "Phone OSINT",        pro: true  }],
    ["intelx",  { icon: "⬡", label: "IntelX",             pro: true  }],
    ["ai",      { icon: "✷", label: "AI Analysis",        pro: true  }],
  ];

  const settingsSections = ["server", "subscriptions", "account", "appearance", "advanced"];
  const settingsLabels: Record<string, string> = {
    server: "Сервер", subscriptions: "Подписки", account: "Аккаунт",
    appearance: "Внешний вид", advanced: "Дополнительно",
  };

  // ── Helpers ──────────────────────────────────────────────────────────────────
  const normRemaining = (n: any) => (n === null || n === undefined || n >= 9e18) ? Infinity : Number(n);
  const remainingLabel = () => (requestsRemaining === Infinity ? "∞" : requestsRemaining);
  // Resolve media uploaded to the server (stored as /uploads/…) against the server origin.
  const mediaUrl = (u: string) => (u && u.startsWith("/") ? serverUrl + u : u);
  const isVideoAtt = (a: any) => (a?.mime || "").startsWith("video") || /\.(mp4|webm|mov)$/i.test(a?.url || "");
  const isImageAtt = (a: any) => (a?.mime || "").startsWith("image") || /\.(png|jpe?g|gif|webp)$/i.test(a?.url || "");
  const attName = (a: any) => a?.name || (a?.url || "").split("/").pop() || "файл";
  const fmtSize = (b: number) => !b ? "" : b < 1024 ? b + " Б" : b < 1048576 ? (b / 1024).toFixed(1) + " КБ" : (b / 1048576).toFixed(1) + " МБ";
  const attIcon = (a: any) => {
    const m = a?.mime || "", n = (a?.name || a?.url || "").toLowerCase();
    if (m.startsWith("audio")) return "🎵";
    if (/zip|rar|7z|tar|gz|compress/.test(m) || /\.(zip|rar|7z|tar|gz)$/.test(n)) return "🗜";
    if (m.includes("pdf") || n.endsWith(".pdf")) return "📕";
    return "📎";
  };

  async function queueScroll() {
    if (!autoScroll) return;
    await tick();
    if (consoleEl) consoleEl.scrollTop = consoleEl.scrollHeight;
  }

  function pushEvent(module: string, type: string, text: string) {
    events = [...events, { module, type, text, time: new Date().toLocaleTimeString() }];
    if (type === "found") foundCount++;
    queueScroll();
  }

  function ingestEvent(ev: any) {
    if (ev.type === "stream") {
      aiText = ev.text;
      pushEvent(ev.module, "found", "AI-досье сформировано — откройте вкладку AI");
      panelTab = "ai";
      return;
    }
    const m = /GEO_PIN:([\-0-9.]+),([\-0-9.]+)\|([^|]*)\|/.exec(ev.text || "");
    if (m) {
      mapPins = [...mapPins, { lat: +m[1], lon: +m[2], label: m[3] }];
      updateMapMarkers();
    }
    pushEvent(ev.module, ev.type, ev.text);
  }

  // ── HWID ban check ───────────────────────────────────────────────────────────
  let banChecking = false;
  let banCheckMsg = "";
  let banPollTimer: any = null;

  // Re-checks ban state with the server. Clears the block screen when the ban is lifted.
  async function checkBanStatus() {
    if (!hwid || !serverUrl) return;
    banChecking = true;
    banCheckMsg = "";
    try {
      const res = await fetch(`${serverUrl}/api/hwid/check`, {
        method: "POST", headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ hwid }),
      });
      const data = await res.json();
      const wasBanned = isBanned;
      isBanned = !!data.banned;
      if (isBanned) {
        banReason = data.reason || "Устройство заблокировано";
        banDate = data.date || "";
        banCheckMsg = "Блокировка всё ещё активна";
      } else {
        banReason = ""; banDate = "";
        if (wasBanned) { fetchNews(); checkHealth(); }
      }
    } catch {
      banCheckMsg = "Сервер недоступен — проверьте, что он запущен";
    }
    banChecking = false;
  }

  // While blocked, poll the server so a lifted ban clears on its own.
  $: {
    clearInterval(banPollTimer);
    if (isBanned) banPollTimer = setInterval(checkBanStatus, 15000);
  }

  // ── Lifecycle ────────────────────────────────────────────────────────────────
  onMount(async () => {
    try { hwid = await invoke("get_hwid"); } catch {}
    serverUrl = localStorage.getItem("ravens_server_url") || DEFAULT_SERVER_URL;
    localStorage.setItem("ravens_server_url", serverUrl);
    try { await invoke("set_server_url", { serverUrl }); } catch {}

    tokenInput = localStorage.getItem("ravens_token") || "";
    sessionId = localStorage.getItem("ravens_session_id") || "";
    licenseTier = localStorage.getItem("ravens_tier") || "FREE";
    const savedAllowed = localStorage.getItem("ravens_allowed_modules");
    if (savedAllowed) allowedModules = JSON.parse(savedAllowed);
    const savedRemaining = localStorage.getItem("ravens_remaining");
    if (savedRemaining) requestsRemaining = savedRemaining === "Infinity" ? Infinity : parseInt(savedRemaining);
    currentScheme = localStorage.getItem("ravens_scheme") || "raven";
    compactMode = localStorage.getItem("ravens_compact") === "true";

    await checkBanStatus();
    checkHealth();
    if (serverUrl) { fetchNews(); subscribeNewsStream(); }
  });

  // ── Health ───────────────────────────────────────────────────────────────────
  let healthModules: any = {};
  async function checkHealth() {
    if (!serverUrl) return;
    try {
      const res = await fetch(`${serverUrl}/health`);
      if (res.ok) { const data = await res.json(); healthModules = data.modules || {}; }
    } catch {}
  }

  // ── Window controls ──────────────────────────────────────────────────────────
  const winMin = () => invoke("minimize_window").catch(() => {});
  const winMax = () => invoke("maximize_window").catch(() => {});
  const winClose = () => invoke("close_window").catch(() => {});

  // ── Map (Leaflet via CDN) ────────────────────────────────────────────────────
  let mapInstance: any = null;
  let mapMarkers: any[] = [];
  function initMap() {
    if (mapInstance) return;
    // @ts-ignore
    if (typeof L === "undefined" || !document.getElementById("map-container")) return;
    // @ts-ignore
    mapInstance = L.map("map-container", { zoomControl: true, attributionControl: false }).setView([30, 10], 2);
    // @ts-ignore
    L.tileLayer("https://{s}.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}{r}.png", { maxZoom: 19 }).addTo(mapInstance);
    updateMapMarkers();
  }
  function updateMapMarkers() {
    if (!mapInstance) return;
    mapMarkers.forEach((m) => mapInstance.removeLayer(m));
    mapMarkers = [];
    // @ts-ignore
    if (typeof L === "undefined") return;
    mapPins.forEach((pin) => {
      // @ts-ignore
      const marker = L.marker([pin.lat, pin.lon]).addTo(mapInstance);
      marker.bindPopup(`<b>${pin.label}</b><br>${pin.lat}, ${pin.lon}`);
      mapMarkers.push(marker);
    });
    if (mapPins.length) mapInstance.setView([mapPins[mapPins.length - 1].lat, mapPins[mapPins.length - 1].lon], 6);
  }
  $: if (activeTab === "map" && licenseTier !== "FREE") setTimeout(initMap, 60);

  // ── News (SSE + fetch) ───────────────────────────────────────────────────────
  let newsEventSource: EventSource | null = null;
  let newsSyncTimer: any = null;
  // Re-sync the whole list rather than appending, so server-side deletions propagate.
  function scheduleNewsSync() {
    clearTimeout(newsSyncTimer);
    newsSyncTimer = setTimeout(fetchNews, 250);
  }
  function subscribeNewsStream() {
    if (newsEventSource) newsEventSource.close();
    if (!serverUrl) return;
    try {
      newsEventSource = new EventSource(`${serverUrl}/api/news/stream`);
      newsEventSource.addEventListener("news", () => scheduleNewsSync());
      newsEventSource.onopen = () => scheduleNewsSync();
      newsEventSource.onerror = () => { newsEventSource?.close(); setTimeout(subscribeNewsStream, 5000); };
    } catch {}
  }
  // Refresh the feed whenever the Ravens tab is opened.
  $: if (activeTab === "ravens" && serverUrl) fetchNews();
  async function fetchNews() {
    if (!serverUrl) return;
    newsLoading = true;
    try { const res = await fetch(`${serverUrl}/api/news`); if (res.ok) news = await res.json(); } catch {}
    newsLoading = false;
  }
  const openNewsCard = (item: any) => (openedNews = item);
  const closeNewsCard = () => (openedNews = null);

  // ── Server URL persistence ───────────────────────────────────────────────────
  async function saveServerUrl() {
    serverUrl = serverUrl.trim().replace(/\/+$/, "");
    localStorage.setItem("ravens_server_url", serverUrl);
    try { await invoke("set_server_url", { serverUrl }); } catch {}
    checkHealth(); fetchNews(); subscribeNewsStream(); checkBanStatus();
  }

  // ── Activation ───────────────────────────────────────────────────────────────
  async function activateToken() {
    if (!tokenInput) { tokenStatus = "⚠ Введите токен"; return; }
    if (!serverUrl) { tokenStatus = "⚠ Укажите URL сервера"; return; }
    tokenStatus = "⏳ Активация с проверкой подписи…";
    try {
      await invoke("set_server_url", { serverUrl });
      const res: any = await invoke("authenticate", { token: tokenInput, serverUrl });
      const b = res?.body;
      if (b?.ok) {
        licenseTier = b.tier;
        requestsRemaining = normRemaining(b.requests_remaining);
        allowedModules = b.allowed_modules || [];
        sessionId = b.session_id || "";
        tokenStatus = `✓ Активирован · ${b.tier} · ${b.expires ? "до " + b.expires : "∞"}`;
        localStorage.setItem("ravens_token", tokenInput);
        localStorage.setItem("ravens_session_id", sessionId);
        localStorage.setItem("ravens_tier", licenseTier);
        localStorage.setItem("ravens_allowed_modules", JSON.stringify(allowedModules));
        localStorage.setItem("ravens_remaining", String(requestsRemaining));
        fetchNews();
      } else {
        tokenStatus = `✗ ${res?.error || b?.error || "Ошибка активации"}`;
      }
    } catch (e) {
      tokenStatus = `✗ ${e}`;
    }
  }

  // ── Scan (server-side execution) ─────────────────────────────────────────────
  async function startScan() {
    const t = target.trim();
    if (!t || scanning) return;
    if (!sessionId || !tokenInput) {
      activeTab = "settings"; settingsSection = "server";
      tokenStatus = "⚠ Сначала активируйте токен";
      return;
    }
    scanning = true; events = []; mapPins = []; foundCount = 0; moduleCount = 0; aiText = ""; threatLevel = "MINIMAL"; panelTab = "console";
    const t0 = Date.now();
    const mods = [...selectedModules].filter((m) => m !== "ai");
    const wantAI = selectedModules.has("ai");

    for (const mod of mods) {
      if (!allowedModules.includes(mod)) { pushEvent(mod, "error", `Модуль недоступен на тарифе ${licenseTier}`); continue; }
      moduleCount++;
      pushEvent(mod, "running", `Запуск модуля ${mod}…`);
      try {
        const body: any = await invoke("osint_run", { token: tokenInput, module: mod, target: t, sessionId });
        if (!body || body.ok === false) { pushEvent(mod, "error", body?.error || "Отказано сервером"); continue; }
        requestsRemaining = normRemaining(body.requests_remaining);
        for (const ev of body.events || []) ingestEvent(ev);
      } catch (e) { pushEvent(mod, "error", `${e}`); }
    }

    if (wantAI) {
      if (!allowedModules.includes("ai")) pushEvent("ai", "error", `AI-анализ недоступен на тарифе ${licenseTier}`);
      else {
        moduleCount++;
        pushEvent("ai", "running", "AI-анализ находок…");
        const findings = events.filter((e) => e.type === "found").map((e) => e.text);
        try {
          const body: any = await invoke("osint_run", { token: tokenInput, module: "ai", target: t, sessionId, findings });
          if (body && body.ok !== false) { requestsRemaining = normRemaining(body.requests_remaining); for (const ev of body.events || []) ingestEvent(ev); }
          else pushEvent("ai", "error", body?.error || "Отказано сервером");
        } catch (e) { pushEvent("ai", "error", `${e}`); }
      }
    }

    localStorage.setItem("ravens_remaining", String(requestsRemaining));
    scanTime = ((Date.now() - t0) / 1000).toFixed(1);
    threatLevel = foundCount > 20 ? "CRITICAL" : foundCount > 10 ? "HIGH" : foundCount > 3 ? "MEDIUM" : foundCount > 0 ? "LOW" : "MINIMAL";
    scanning = false;
  }

  // ── Telegram → Discord forwarder ──────────────────────────────────────────────
  // Discord self-bot (discord.js-selfbot-v13, via a Node sidecar) is the TARGET; a real
  // Telegram user account (MTProto/grammers) is the SOURCE. All work happens in the Rust
  // backend; the UI only invokes commands and shows escaped text (no innerHTML).
  const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

  async function discordConnect() {
    if (!discordToken.trim()) { discordMsg = "Введите токен пользователя"; return; }
    discordBusy = true; discordMsg = "Подключение self-bot…";
    try {
      await invoke("discord_login", { token: discordToken.trim() });
      for (let i = 0; i < 30; i++) {           // readiness is signalled asynchronously
        const s: any = await invoke("discord_status");
        if (s.ready) { discordConnected = true; discordUser = s.user || ""; break; }
        await sleep(500);
      }
      if (discordConnected) { discordMsg = ""; await discordLoadChannels(); }
      else discordMsg = "Вошли, но клиент ещё не готов — нажмите «Обновить каналы»";
    } catch (e) { discordMsg = `${e}`; }
    discordBusy = false;
  }
  async function discordLoadChannels() {
    try { discordChannels = (await invoke("discord_list_channels")) as any[]; }
    catch (e) { discordMsg = `${e}`; }
  }
  async function discordDisconnect() {
    try { await invoke("discord_logout"); } catch {}
    discordConnected = false; discordUser = ""; discordChannels = []; discordTarget = ""; fwdActive = false;
    discordMsg = "";
  }

  async function tgSendCode() {
    if (!tgApiId.trim() || !tgApiHash.trim() || !tgPhone.trim()) { tgMsg = "Заполните api_id, api_hash и телефон"; return; }
    tgBusy = true; tgMsg = "Запрос кода…";
    try {
      const s: any = await invoke("tg_request_code", { apiId: parseInt(tgApiId, 10), apiHash: tgApiHash.trim(), phone: tgPhone.trim() });
      tgStage = s.stage; tgMsg = "Код отправлен в Telegram";
    } catch (e) { tgMsg = `${e}`; }
    tgBusy = false;
  }
  async function tgSignIn() {
    tgBusy = true; tgMsg = "Вход…";
    try {
      const s: any = await invoke("tg_sign_in", { code: tgCode.trim() });
      tgStage = s.stage;
      if (s.stage === "authorized") { tgUser = s.user || ""; tgMsg = ""; await tgLoadChannels(); }
      else if (s.stage === "password_required") tgMsg = "Введите пароль двухфакторной защиты";
    } catch (e) { tgMsg = `${e}`; }
    tgBusy = false;
  }
  async function tgCheckPassword() {
    tgBusy = true; tgMsg = "Проверка пароля…";
    try {
      const s: any = await invoke("tg_check_password", { password: tgPassword });
      tgStage = s.stage;
      if (s.stage === "authorized") { tgUser = s.user || ""; tgMsg = ""; tgPassword = ""; await tgLoadChannels(); }
    } catch (e) { tgMsg = `${e}`; }
    tgBusy = false;
  }
  async function tgLoadChannels() {
    try { tgChannels = (await invoke("tg_list_channels")) as any[]; }
    catch (e) { tgMsg = `${e}`; }
  }
  async function tgLogout() {
    try { await invoke("tg_logout"); } catch {}
    tgStage = "disconnected"; tgUser = ""; tgChannels = []; tgSource = ""; tgCode = ""; tgPassword = ""; fwdActive = false;
    tgMsg = "";
  }

  async function startForward() {
    if (!tgSource) { tgMsg = "Выберите канал Telegram-источник"; return; }
    if (!discordTarget) { tgMsg = "Выберите канал Discord на вкладке Discord"; return; }
    try { await invoke("tg_start_forward", { tgChannelId: tgSource, discordChannelId: discordTarget }); fwdActive = true; tgMsg = "Пересылка запущена"; }
    catch (e) { tgMsg = `${e}`; }
  }
  async function stopForward() {
    try { await invoke("tg_stop_forward"); } catch {}
    fwdActive = false; tgMsg = "Пересылка остановлена";
  }

  function toggleModule(mod: string) {
    if (selectedModules.has(mod)) selectedModules.delete(mod);
    else selectedModules.add(mod);
    selectedModules = new Set(selectedModules);
  }
  function setScheme(key: string) { currentScheme = key; localStorage.setItem("ravens_scheme", key); }
  function toggleCompactMode() { compactMode = !compactMode; localStorage.setItem("ravens_compact", String(compactMode)); }

  // ── Dossier export ───────────────────────────────────────────────────────────
  function generateDossier() {
    const found = events.filter((e: any) => e.type === "found");
    const categories: Record<string, string[]> = {};
    found.forEach((e: any) => { (categories[e.module] ||= []).push(e.text); });
    const esc = (s: string) => (s || "").replace(/[<>&]/g, (c) => ({ "<": "&lt;", ">": "&gt;", "&": "&amp;" }[c] as string));
    const acc = scheme.accent;
    // Interpolate the tag name so the source contains no literal style-tag token
    // (otherwise the Svelte preprocessor would try to compile it as component CSS).
    const st = "style";
    const css = `body{background:#0a0b10;color:#e5e7eb;font-family:'JetBrains Mono',monospace;padding:40px;max-width:900px;margin:auto}`
      + `h1{color:${acc};letter-spacing:3px;border-bottom:1px solid ${acc};padding-bottom:12px}`
      + `h2{color:${acc};margin-top:28px;font-size:13px;letter-spacing:1px}`
      + `.summary{background:#14151c;padding:16px;border-radius:10px;margin:20px 0;line-height:1.9}`
      + `.item{padding:6px 0;border-bottom:1px solid rgba(255,255,255,.06);font-size:12px}`
      + `.meta{color:#6b7280;font-size:10px;margin-top:30px}`
      + `.ai{white-space:pre-wrap;background:#14151c;padding:16px;border-radius:10px;line-height:1.7;font-size:12px}`;
    const html = `<!DOCTYPE html><html lang="ru"><head><meta charset="UTF-8"><title>Ravens Nexus — Dossier</title>
<${st}>${css}</${st}></head><body>
<h1>⬡ RAVENS NEXUS — DOSSIER</h1>
<div class="summary"><b>Сформировано:</b> ${new Date().toLocaleString("ru-RU")}<br><b>Цель:</b> ${esc(target) || "N/A"}<br><b>Находок:</b> ${found.length}<br><b>Модулей:</b> ${Object.keys(categories).join(", ") || "N/A"}</div>
${Object.entries(categories).map(([cat, items]) => `<h2>${cat.toUpperCase()}</h2>${items.map((i) => `<div class="item">• ${esc(i)}</div>`).join("")}`).join("")}
${aiText ? `<h2>AI-ДОСЬЕ</h2><div class="ai">${esc(aiText)}</div>` : ""}
<div class="meta">Ravens Nexus OSINT Platform · v2.8 · ${new Date().toISOString()}</div></body></html>`;
    const url = URL.createObjectURL(new Blob([html], { type: "text/html" }));
    const a = document.createElement("a");
    a.href = url; a.download = `ravens_dossier_${Date.now()}.html`; a.click();
    URL.revokeObjectURL(url);
  }

  $: filteredEvents = filterModule === "all" ? events : events.filter((e) => e.module === filterModule);
  const threatColor = (l: string) => l === "CRITICAL" ? "#ef4444" : l === "HIGH" ? "#f59e0b" : l === "MEDIUM" ? "#eab308" : l === "LOW" ? "#84cc16" : "#4ade80";
</script>

<!-- ── Ban screen ──────────────────────────────────────────────────────────── -->
{#if isBanned}
<div class="ban-screen">
  <div class="ban-card">
    <div class="ban-icon">⛔</div>
    <div class="ban-title">УСТРОЙСТВО ЗАБЛОКИРОВАНО</div>
    <div class="ban-body">
      Ваше устройство заблокировано администратором.<br>
      Причина: <strong>{banReason}</strong>
      {#if banDate}<br><span class="dim">Дата: {banDate}</span>{/if}
    </div>
    <div class="ban-actions">
      <button class="btn primary" on:click={checkBanStatus} disabled={banChecking}>
        {banChecking ? "⏳ Проверяю…" : "🔄 Проверить статус бана"}
      </button>
      <div class="ban-hint">Если администратор снял блокировку — нажми кнопку.<br>Проверка также идёт автоматически каждые 15 секунд.</div>
      {#if banCheckMsg}<div class="ban-msg">{banCheckMsg}</div>{/if}
    </div>
    <div class="hwid-chip">HWID: {hwid.substring(0, 20)}…</div>
  </div>
</div>
{/if}

<div class="app" style={cssVars} class:compact={compactMode}>
  <div class="aurora" aria-hidden="true"></div>

  <!-- Topbar -->
  <header class="header" data-tauri-drag-region>
    <div class="brand" data-tauri-drag-region>
      <span class="brand-mark">⬡</span>
      <span class="brand-name">RAVENS</span>
      <span class="brand-sub" data-tauri-drag-region>NEXUS · OSINT</span>
    </div>
    <div class="stats" data-tauri-drag-region>
      <div class="stat"><b>{foundCount}</b><span>НАЙДЕНО</span></div>
      <div class="stat"><b>{moduleCount}</b><span>МОДУЛЕЙ</span></div>
      <div class="stat"><b style="color:{threatColor(threatLevel)}">{threatLevel}</b><span>УГРОЗА</span></div>
      <div class="stat"><b>{scanTime}s</b><span>ВРЕМЯ</span></div>
      <div class="tier-badge"><span class="dot"></span>{licenseTier} · {remainingLabel()}</div>
    </div>
    <div class="win-controls">
      <button class="win-btn" on:click={winMin} title="Свернуть">‒</button>
      <button class="win-btn" on:click={winMax} title="Развернуть">▢</button>
      <button class="win-btn win-close" on:click={winClose} title="Закрыть">✕</button>
    </div>
  </header>

  <!-- Nav -->
  <nav class="nav">
    {#each [["ravens","⬡","Лента"],["scan","◎","OSINT"],["discord","❖","Discord"],["telegram","✈","Telegram"],["map","◈","Карта"],["settings","⚙","Настройки"],["about","𝓲","О системе"]] as [t, icon, label]}
      <button class="nav-btn" class:active={activeTab === t} on:click={() => (activeTab = t)}>
        <span class="nav-ico">{icon}</span>{label}
      </button>
    {/each}
  </nav>

  <main class="content">
    <!-- ── RAVENS / NEWS ─────────────────────────────────────────────────────── -->
    {#if activeTab === "ravens"}
      <div class="scroll">
        <div class="section-head">
          <div>
            <div class="section-title">Лента новостей</div>
            <div class="section-sub">Обновления платформы и оперативные сводки</div>
          </div>
          <button class="btn ghost sm" on:click={fetchNews}>↻ Обновить</button>
        </div>

        {#if newsLoading}
          <div class="empty"><div class="empty-ico">⏳</div>Загрузка…</div>
        {:else if news.length === 0}
          <div class="empty">
            <div class="empty-ico">🦅</div>
            <div>Пока нет новостей</div>
            <div class="dim sm">Подключите сервер в Настройках → Сервер</div>
          </div>
        {:else}
          <div class="news-grid">
            {#each news as item (item.id)}
              <button class="news-card" on:click={() => openNewsCard(item)}>
                {#if item.image_url || item.video_url}
                  <div class="news-media">
                    {#if item.video_url}<video src={mediaUrl(item.video_url)} muted loop autoplay playsinline></video>
                    {:else}<img src={mediaUrl(item.image_url)} alt={item.title} loading="lazy" />{/if}
                  </div>
                {/if}
                <div class="news-body">
                  <div class="news-title">{item.title}</div>
                  <div class="news-preview">{item.preview_text}</div>
                </div>
                <div class="news-foot">
                  <span class="news-author">{item.author}</span>
                  <span class="news-more">Читать →</span>
                </div>
              </button>
            {/each}
          </div>
        {/if}
      </div>

      {#if openedNews}
        <div class="overlay" on:click|self={closeNewsCard}>
          <div class="modal">
            {#if openedNews.image_url || openedNews.video_url}
              <div class="modal-media">
                {#if openedNews.video_url}<video src={mediaUrl(openedNews.video_url)} controls autoplay></video>
                {:else}<img src={mediaUrl(openedNews.image_url)} alt={openedNews.title} />{/if}
              </div>
            {/if}
            <div class="modal-body">
              <div class="modal-title">{openedNews.title}</div>
              <div class="modal-meta">{openedNews.author} · {new Date(openedNews.created_at).toLocaleDateString("ru-RU")}</div>
              <div class="modal-text">{openedNews.full_text}</div>
              {#if openedNews.attachments && openedNews.attachments.length}
                <div class="attachments">
                  {#each openedNews.attachments as att}
                    {#if isVideoAtt(att)}
                      <video class="att-media" src={mediaUrl(att.url)} controls playsinline></video>
                    {:else if isImageAtt(att)}
                      <img class="att-media" src={mediaUrl(att.url)} alt="attachment" />
                    {:else}
                      <a class="att-file" href={mediaUrl(att.url)} download={attName(att)} target="_blank" rel="noreferrer">
                        <span class="att-ico">{attIcon(att)}</span>
                        <span class="att-name">{attName(att)}</span>
                        {#if att.size}<span class="att-sz">{fmtSize(att.size)}</span>{/if}
                        <span class="att-dl">↓</span>
                      </a>
                    {/if}
                  {/each}
                </div>
              {/if}
            </div>
            <button class="modal-close" on:click={closeNewsCard}>✕</button>
          </div>
        </div>
      {/if}

    <!-- ── OSINT SCAN ────────────────────────────────────────────────────────── -->
    {:else if activeTab === "scan"}
      <div class="scan-layout">
        <aside class="scan-sidebar">
          <div class="panel">
            <div class="panel-title">Цель разведки</div>
            <input class="input" bind:value={target} placeholder="username · email · IP · домен · телефон"
              on:keydown={(e) => e.key === "Enter" && startScan()} />
            <div class="chips">
              {#each ["username","email","ip","domain","phone"] as t}
                <button class="chip" class:active={targetType === t} on:click={() => (targetType = t)}>{t}</button>
              {/each}
            </div>
          </div>

          <div class="panel grow">
            <div class="panel-title">Модули · {selectedModules.size}</div>
            <div class="modules">
              {#each availableModules as [mod, def]}
                {@const locked = !allowedModules.includes(mod)}
                {@const health = healthModules[mod]?.status}
                <label class="module" class:locked class:on={selectedModules.has(mod)}>
                  <input type="checkbox" checked={selectedModules.has(mod)} disabled={locked}
                    on:change={() => !locked && toggleModule(mod)} />
                  <span class="module-ico">{def.icon}</span>
                  <span class="module-label">{def.label}</span>
                  {#if health === "degraded"}<span class="tag warn" title="Нужен API-ключ на сервере">◐</span>{/if}
                  {#if def.pro && locked}<span class="tag">PRO</span>{/if}
                </label>
              {/each}
            </div>
          </div>

          <div class="actions">
            <button class="btn primary" on:click={startScan} disabled={scanning || !target}>
              {scanning ? "⚡ Сканирование…" : "► Начать разведку"}
            </button>
            <button class="btn ghost sm" on:click={() => { events = []; foundCount = 0; mapPins = []; aiText = ''; }}>Очистить</button>
          </div>
        </aside>

        <section class="scan-main">
          <div class="term-head">
            <span class="prompt">ravens@nexus</span><span class="dim">:~$ osint {target || "—"}</span>
            {#if scanning}<span class="rec">● LIVE</span>{/if}
            <div class="panel-tabs">
              {#if events.length}<button class="btn ghost xs" on:click={generateDossier}>📄 Досье</button>{/if}
              {#each [["console","Консоль"],["dossier","Сводка"],["ai","AI"]] as [t, label]}
                <button class="ptab" class:active={panelTab === t} on:click={() => (panelTab = t)}>{label}</button>
              {/each}
            </div>
          </div>

          {#if panelTab === "console"}
            <div class="panel term">
              {#if events.length}
                <div class="filters">
                  <button class="chip sm" class:active={filterModule === "all"} on:click={() => (filterModule = "all")}>все</button>
                  {#each [...new Set(events.map((e) => e.module))] as mod}
                    <button class="chip sm" class:active={filterModule === mod} on:click={() => (filterModule = mod)}>{mod}</button>
                  {/each}
                </div>
              {/if}
              <div class="console" bind:this={consoleEl}>
                {#if events.length === 0 && !scanning}
                  <div class="empty"><div class="empty-ico">🦅</div><div>RAVEN OSINT готов</div><div class="dim sm">Введите цель и запустите разведку</div></div>
                {/if}
                {#each filteredEvents as ev}
                  <div class="ev ev-{ev.type}">
                    {#if showTimestamps}<span class="ev-time">{ev.time}</span>{/if}
                    <span class="ev-mod">{ev.module}</span>
                    <span class="ev-ico">{ev.type === "found" ? "★" : ev.type === "error" ? "✕" : ev.type === "running" ? "▶" : ev.type === "done" ? "✓" : "·"}</span>
                    <span class="ev-text">{ev.text}</span>
                  </div>
                {/each}
                {#if scanning}<div class="ev ev-running"><span class="ev-text">▶ Выполняется разведка…</span></div>{/if}
              </div>
            </div>

          {:else if panelTab === "dossier"}
            <div class="panel scrolly">
              {#if events.length === 0}
                <div class="empty"><div class="empty-ico">📄</div>Запустите разведку</div>
              {:else}
                <div class="dossier">
                  <div class="dossier-h">Досье · {target || "N/A"}</div>
                  <div class="dim sm">Находок: {events.filter((e) => e.type === "found").length}</div>
                  {#each events.filter((e) => e.type === "found") as ev}
                    <div class="dossier-item"><span class="ev-mod">{ev.module}</span> {ev.text}</div>
                  {/each}
                </div>
              {/if}
            </div>

          {:else if panelTab === "ai"}
            <div class="panel scrolly">
              {#if aiText}
                <div class="ai-out">{aiText}</div>
              {:else}
                <div class="empty"><div class="empty-ico">✷</div><div>RAVEN AI · Nemotron-Ultra</div><div class="dim sm">Включите модуль AI (тариф ELITE) и запустите разведку</div></div>
              {/if}
            </div>
          {/if}
        </section>
      </div>

    <!-- ── MAP ───────────────────────────────────────────────────────────────── -->
    {:else if activeTab === "map"}
      {#if licenseTier === "FREE"}
        <div class="empty locked-tab">
          <div class="empty-ico">◈</div>
          <div class="section-title">Карта угроз</div>
          <div class="dim">Доступна на тарифе PRO и выше</div>
          <button class="btn primary sm" on:click={() => { settingsSection = 'subscriptions'; activeTab = 'settings'; }}>Обновить подписку</button>
        </div>
      {:else}
        <div id="map-container" class="map"></div>
      {/if}

    <!-- ── DISCORD ───────────────────────────────────────────────────────────── -->
    {:else if activeTab === "discord"}
      <div class="scroll">
        <div class="section-head">
          <div>
            <div class="section-title">Discord · приёмник</div>
            <div class="section-sub">Self-bot аккаунт, куда пересылаются посты из Telegram</div>
          </div>
        </div>

        {#if !allowedModules.includes("discord")}
          <div class="empty locked-tab">
            <div class="empty-ico">❖</div>
            <div class="section-title">Discord</div>
            <div class="dim">Доступно на тарифе PRO и выше</div>
            <button class="btn primary sm" on:click={() => { settingsSection = 'subscriptions'; activeTab = 'settings'; }}>Обновить подписку</button>
          </div>
        {:else if !discordConnected}
          <div class="panel">
            <div class="panel-title">Токен пользователя Discord</div>
            <input class="input" type="password" bind:value={discordToken} placeholder="user token (self-bot)" />
            <div style="display:flex;gap:8px;margin-top:10px">
              <button class="btn primary" on:click={discordConnect} disabled={discordBusy || !discordToken}>
                {discordBusy ? '⚡ Подключение…' : '► Подключить self-bot'}
              </button>
            </div>
            {#if discordMsg}<div class="dim sm" style="margin-top:8px">{discordMsg}</div>{/if}
          </div>
        {:else}
          <div class="panel">
            <div class="panel-title">Подключено: {discordUser || '—'}</div>
            <div style="display:flex;gap:8px;margin:6px 0 12px">
              <button class="btn ghost sm" on:click={discordLoadChannels}>↻ Обновить каналы</button>
              <button class="btn ghost sm" on:click={discordDisconnect}>Выйти</button>
            </div>
            <div class="panel-title">Целевой канал (куда слать)</div>
            <div class="chan-list">
              {#each discordChannels as ch}
                <label class="chan" class:sel={discordTarget === ch.id}>
                  <input type="radio" name="dtarget" value={ch.id} bind:group={discordTarget} />
                  <span class="chan-name">#{ch.name}</span>
                  <span class="chan-guild">{ch.guild}</span>
                </label>
              {/each}
              {#if discordChannels.length === 0}<div class="dim sm" style="padding:10px">Нет доступных текстовых каналов</div>{/if}
            </div>
            {#if discordMsg}<div class="dim sm" style="margin-top:8px">{discordMsg}</div>{/if}
          </div>
        {/if}

        <!-- Reference repository + risk disclaimer -->
        <div class="repo-card">
          <div class="repo-head">
            <span class="repo-ico">❖</span>
            <div class="repo-meta">
              <div class="repo-title">discord.js-selfbot-v13</div>
              <div class="repo-sub">aiko-chan-ai · официальный репозиторий self-bot библиотеки</div>
            </div>
            <span class="repo-badge">GitHub</span>
          </div>
          <div class="repo-desc">
            Библиотека для автоматизации пользовательских аккаунтов Discord (self-bot). Приложена как справочный материал и доказательство.
          </div>
          <a class="repo-link" href="https://github.com/aiko-chan-ai/discord.js-selfbot-v13" target="_blank" rel="noopener noreferrer">
            github.com/aiko-chan-ai/discord.js-selfbot-v13 ↗
          </a>
          <div class="repo-warn">
            ⚠ Внимание: использование self-bot нарушает Условия обслуживания Discord и может привести к блокировке аккаунта.
            Вы используете это исключительно на свой страх и риск. Администрация платформы не несёт никакой ответственности
            за последствия использования.
          </div>
        </div>
      </div>

    <!-- ── TELEGRAM ──────────────────────────────────────────────────────────── -->
    {:else if activeTab === "telegram"}
      <div class="scroll">
        <div class="section-head">
          <div>
            <div class="section-title">Telegram · источник</div>
            <div class="section-sub">Аккаунт MTProto · выбор канала и пересылка постов в Discord</div>
          </div>
        </div>

        {#if !allowedModules.includes("telegram")}
          <div class="empty locked-tab">
            <div class="empty-ico">✈</div>
            <div class="section-title">Telegram</div>
            <div class="dim">Доступно на тарифе PRO и выше</div>
            <button class="btn primary sm" on:click={() => { settingsSection = 'subscriptions'; activeTab = 'settings'; }}>Обновить подписку</button>
          </div>
        {:else if tgStage !== 'authorized'}
          <div class="panel">
            <div class="panel-title">Подключение аккаунта (MTProto)</div>
            <input class="input" bind:value={tgApiId} placeholder="api_id (my.telegram.org)" style="margin-bottom:8px" />
            <input class="input" type="password" bind:value={tgApiHash} placeholder="api_hash" style="margin-bottom:8px" />
            <input class="input" bind:value={tgPhone} placeholder="+телефон, напр. +79991234567" />
            <div style="display:flex;gap:8px;margin-top:10px">
              <button class="btn primary" on:click={tgSendCode} disabled={tgBusy}>{tgBusy ? '⚡…' : '► Отправить код'}</button>
            </div>

            {#if tgStage === 'code_sent' || tgStage === 'password_required'}
              <div class="panel-title" style="margin-top:14px">Код из приложения Telegram</div>
              <input class="input" bind:value={tgCode} placeholder="код подтверждения" />
              <div style="margin-top:8px"><button class="btn primary" on:click={tgSignIn} disabled={tgBusy}>Войти</button></div>
            {/if}
            {#if tgStage === 'password_required'}
              <div class="panel-title" style="margin-top:14px">Пароль 2FA</div>
              <input class="input" type="password" bind:value={tgPassword} placeholder="пароль двухфакторной защиты" />
              <div style="margin-top:8px"><button class="btn primary" on:click={tgCheckPassword} disabled={tgBusy}>Подтвердить</button></div>
            {/if}
            {#if tgMsg}<div class="dim sm" style="margin-top:8px">{tgMsg}</div>{/if}
          </div>
        {:else}
          <div class="panel">
            <div class="panel-title">Аккаунт: {tgUser || '—'}</div>
            <div style="display:flex;gap:8px;margin:6px 0 12px">
              <button class="btn ghost sm" on:click={tgLoadChannels}>↻ Обновить каналы</button>
              <button class="btn ghost sm" on:click={tgLogout}>Выйти</button>
            </div>
            <div class="panel-title">Канал-источник</div>
            <div class="chan-list">
              {#each tgChannels as ch}
                <label class="chan" class:sel={tgSource === ch.id}>
                  <input type="radio" name="tgsource" value={ch.id} bind:group={tgSource} />
                  <span class="chan-name">{ch.title}</span>
                  {#if ch.username}<span class="chan-guild">@{ch.username}</span>{/if}
                </label>
              {/each}
              {#if tgChannels.length === 0}<div class="dim sm" style="padding:10px">Каналы не найдены</div>{/if}
            </div>
          </div>

          <div class="panel" style="margin-top:12px">
            <div class="panel-title">Пересылка Telegram → Discord</div>
            <div class="fwd-row">
              <span class="fwd-chip">TG: {tgSource ? (tgChannels.find((c) => c.id === tgSource)?.title || tgSource) : '— не выбран'}</span>
              <span class="fwd-arrow">→</span>
              <span class="fwd-chip">DS: {discordTarget ? ('#' + (discordChannels.find((c) => c.id === discordTarget)?.name || discordTarget)) : '— выберите на вкладке Discord'}</span>
            </div>
            <div style="display:flex;gap:8px;align-items:center;margin-top:10px">
              {#if !fwdActive}
                <button class="btn primary" on:click={startForward} disabled={!tgSource || !discordTarget}>► Запустить пересылку</button>
              {:else}
                <button class="btn" on:click={stopForward}>■ Остановить</button>
                <span class="rec">● LIVE</span>
              {/if}
            </div>
            <div class="dim sm" style="margin-top:8px">В v1 пересылается текст постов (медиа — в следующей версии).</div>
            {#if tgMsg}<div class="dim sm" style="margin-top:4px">{tgMsg}</div>{/if}
          </div>
        {/if}
      </div>

    <!-- ── SETTINGS ──────────────────────────────────────────────────────────── -->
    {:else if activeTab === "settings"}
      <div class="scroll">
        <div class="settings">
          <div class="settings-nav">
            {#each settingsSections as sec}
              <button class="snav" class:active={settingsSection === sec} on:click={() => (settingsSection = sec)}>{settingsLabels[sec]}</button>
            {/each}
          </div>

          <div class="settings-body">
            {#if settingsSection === "server"}
              <div class="section-title">Сервер лицензий</div>
              <div class="field">
                <label class="lbl">URL сервера</label>
                <div class="row">
                  <input class="input grow" bind:value={serverUrl} placeholder="http://localhost:3000" />
                  <button class="btn ghost sm" on:click={saveServerUrl}>Сохранить</button>
                </div>
              </div>
              <div class="field">
                <label class="lbl">Токен доступа (ключ аккаунта)</label>
                <input class="input" bind:value={tokenInput} placeholder="RVN-XXXXXXXXXXXXXXXX" />
                <div class="hint">Выдаётся администратором. Привязывается к вашему HWID.</div>
              </div>
              <button class="btn primary" on:click={activateToken}>Активировать</button>
              {#if tokenStatus}
                <div class="token-status" class:ok={tokenStatus.startsWith("✓")} class:err={tokenStatus.startsWith("✗")} class:pending={tokenStatus.startsWith("⏳")}>{tokenStatus}</div>
              {/if}
              <div class="divider"></div>
              <div class="kv"><span>Тариф</span><b>{licenseTier}</b></div>
              <div class="kv"><span>Запросов осталось</span><b>{remainingLabel()}</b></div>
              <div class="kv"><span>Сессия</span><b class="mono">{sessionId ? sessionId.substring(0, 8) + "…" : "—"}</b></div>
              <div class="field" style="margin-top:14px">
                <label class="lbl">Device ID (HWID)</label>
                <div class="hwid-box mono">{hwid || "—"}</div>
              </div>

            {:else if settingsSection === "subscriptions"}
              <div class="section-title">Подписки</div>
              <div class="subs">
                {#each tiers as tier}
                  <div class="sub" class:highlight={tier.highlight}>
                    {#if tier.highlight}<div class="sub-ribbon">ПОПУЛЯРНЫЙ</div>{/if}
                    <div class="sub-name">{tier.name}</div>
                    <div class="sub-price">{tier.price}{#if tier.period}<span>/{tier.period}</span>{/if}</div>
                    <div class="dim sm">{tier.requests === -1 ? "∞ запросов" : `${tier.requests} запросов/день`}</div>
                    <ul class="sub-feats">
                      {#each tier.features as f}<li class="ok">{f}</li>{/each}
                      {#each tier.locked as f}<li class="no">{f}</li>{/each}
                    </ul>
                    {#if licenseTier === tier.name}
                      <button class="btn ghost sm" disabled>✓ Текущий</button>
                    {:else}
                      <button class="btn primary sm" on:click={() => (settingsSection = "server")}>Активировать токен</button>
                    {/if}
                  </div>
                {/each}
              </div>

            {:else if settingsSection === "account"}
              <div class="section-title">Аккаунт</div>
              <div class="hint" style="margin-bottom:14px">
                Все API-ключи (NVIDIA NIM, IntelX, HIBP) хранятся <b>на сервере</b> и никогда не попадают на клиент.
                Обратитесь к администратору для настройки платных источников.
              </div>
              <div class="kv"><span>HWID</span><b class="mono">{hwid ? hwid.substring(0, 16) + "…" : "—"}</b></div>
              <div class="kv"><span>Тариф</span><b>{licenseTier}</b></div>

            {:else if settingsSection === "appearance"}
              <div class="section-title">Внешний вид</div>
              <div class="field">
                <label class="lbl">Цветовая схема</label>
                <div class="swatches">
                  {#each Object.entries(schemes) as [key, s]}
                    <button class="swatch" class:active={currentScheme === key} on:click={() => setScheme(key)}>
                      <span class="swatch-dot" style="background:linear-gradient(135deg,{s.accent},{s.accent2})"></span>
                      <span class="sm">{s.name}</span>
                    </button>
                  {/each}
                </div>
              </div>
              <div class="field">
                <label class="lbl">Параметры интерфейса</label>
                <label class="toggle"><input type="checkbox" checked={compactMode} on:change={toggleCompactMode} /> Компактный режим</label>
                <label class="toggle"><input type="checkbox" bind:checked={showTimestamps} /> Метки времени в консоли</label>
                <label class="toggle"><input type="checkbox" bind:checked={autoScroll} /> Авто-прокрутка консоли</label>
              </div>

            {:else if settingsSection === "advanced"}
              <div class="section-title">Дополнительно</div>
              <div class="field">
                <label class="lbl">Данные сессии</label>
                <div class="row">
                  <button class="btn ghost sm" on:click={() => { events = []; mapPins = []; foundCount = 0; }}>Очистить консоль</button>
                  <button class="btn ghost sm" on:click={() => { localStorage.clear(); location.reload(); }}>Сбросить настройки</button>
                </div>
              </div>
              <div class="kv"><span>Версия клиента</span><b>2.8.0</b></div>
              <div class="kv"><span>Архитектура</span><b>Rust · Tauri · Svelte</b></div>
            {/if}
          </div>
        </div>
      </div>

    <!-- ── ABOUT ─────────────────────────────────────────────────────────────── -->
    {:else if activeTab === "about"}
      <div class="scroll">
        <div class="about-hero">
          <div class="about-mark">⬡</div>
          <div class="about-title">RAVENS NEXUS</div>
          <div class="dim">Серверная OSINT-платформа · v2.8 · Rust + Tauri + Svelte</div>
        </div>
        <div class="about-grid">
          {#each [
            ["◵","Social / Username","Поиск по 10+ платформам","FREE"],
            ["◈","IP Geolocation","Геолокация и данные об IP","FREE"],
            ["◆","WHOIS / DNS","Информация о домене","FREE"],
            ["⚠","HIBP Leaks","Проверка email в утечках","PRO"],
            ["⌗","Google Dorks","18+ поисковых запросов","PRO"],
            ["▤","Paste / Doxbin","Поиск в публичных пастах","PRO"],
            ["☇","Dark Web","Поиск через Ahmia","PRO"],
            ["☏","Phone OSINT","Анализ номера телефона","PRO"],
            ["⬡","IntelX","Поиск в базах IntelX","ELITE"],
            ["✷","AI Analysis","NVIDIA NIM Nemotron-Ultra","ELITE"],
          ] as [icon, name, desc, tier]}
            <div class="about-card">
              <span class="about-ico">{icon}</span>
              <div>
                <div class="about-name">{name}</div>
                <div class="dim sm">{desc}</div>
                <span class="tag" style="margin-top:6px;display:inline-block">{tier}</span>
              </div>
            </div>
          {/each}
        </div>
      </div>
    {/if}
  </main>
</div>

<style>
  /* ── Base ──────────────────────────────────────────────────────────────────── */
  .app {
    --bg: #0a0b10;
    --surface: rgba(22, 24, 33, 0.72);
    --surface-solid: #14161f;
    --elevated: rgba(30, 33, 45, 0.9);
    --text: #e6e8ef;
    --dim: #8a90a2;
    --border: rgba(255, 255, 255, 0.08);
    --border-hi: color-mix(in srgb, var(--accent) 45%, transparent);
    position: relative;
    display: flex; flex-direction: column;
    height: 100vh; overflow: hidden;
    background: radial-gradient(1200px 700px at 80% -10%, color-mix(in srgb, var(--accent) 14%, transparent), transparent 60%), var(--bg);
    color: var(--text);
    font-family: 'Inter', system-ui, -apple-system, 'Segoe UI', sans-serif;
    font-size: 13px;
  }
  .aurora {
    position: absolute; inset: 0; pointer-events: none; z-index: 0; opacity: 0.5;
    background:
      radial-gradient(600px 300px at 10% 110%, color-mix(in srgb, var(--accent2) 22%, transparent), transparent 70%),
      radial-gradient(500px 260px at 95% 10%, color-mix(in srgb, var(--accent) 18%, transparent), transparent 70%);
    animation: drift 18s ease-in-out infinite alternate;
  }
  @keyframes drift { from { transform: translate3d(-2%, -1%, 0) scale(1); } to { transform: translate3d(3%, 2%, 0) scale(1.08); } }
  .app > :not(.aurora) { position: relative; z-index: 1; }
  .mono { font-family: 'JetBrains Mono', ui-monospace, monospace; }
  .dim { color: var(--dim); }
  .sm { font-size: 11px; }

  /* ── Header ────────────────────────────────────────────────────────────────── */
  .header {
    display: flex; align-items: center; gap: 20px; padding: 10px 8px 10px 18px;
    background: color-mix(in srgb, var(--surface-solid) 78%, transparent);
    border-bottom: 1px solid var(--border);
    backdrop-filter: blur(14px);
    flex-shrink: 0;
  }
  .brand { display: flex; align-items: center; gap: 9px; }
  .brand-mark { font-size: 20px; color: var(--accent); filter: drop-shadow(0 0 8px color-mix(in srgb, var(--accent) 60%, transparent)); }
  .brand-name { font-weight: 800; letter-spacing: 3px; font-size: 15px;
    background: linear-gradient(120deg, var(--accent), var(--accent2)); -webkit-background-clip: text; background-clip: text; color: transparent; }
  .brand-sub { font-size: 9px; color: var(--dim); letter-spacing: 2px; padding-left: 8px; border-left: 1px solid var(--border); }
  .stats { display: flex; align-items: center; gap: 18px; margin-left: auto; }
  .stat { display: flex; flex-direction: column; line-height: 1.15; }
  .stat b { font-size: 13px; font-weight: 700; }
  .stat span { font-size: 8px; color: var(--dim); letter-spacing: 1.5px; }
  .tier-badge {
    display: inline-flex; align-items: center; gap: 7px; padding: 5px 12px; border-radius: 999px;
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    border: 1px solid var(--border-hi); font-size: 10px; letter-spacing: 1px; color: var(--accent); font-weight: 600;
  }
  .tier-badge .dot { width: 6px; height: 6px; border-radius: 50%; background: var(--accent); box-shadow: 0 0 8px var(--accent); animation: pulse 2s infinite; }
  @keyframes pulse { 0%,100% { opacity: 1; } 50% { opacity: 0.35; } }
  .win-controls { display: flex; gap: 2px; }
  .win-btn { width: 34px; height: 30px; border: none; background: transparent; color: var(--dim); cursor: pointer; border-radius: 7px; font-size: 12px; transition: 0.15s; }
  .win-btn:hover { background: rgba(255,255,255,0.08); color: var(--text); }
  .win-close:hover { background: #e11d48; color: #fff; }

  /* ── Nav ───────────────────────────────────────────────────────────────────── */
  .nav { display: flex; gap: 4px; padding: 8px 14px; flex-shrink: 0; }
  .nav-btn {
    display: flex; align-items: center; gap: 7px; padding: 8px 15px; border: none; border-radius: 10px;
    background: transparent; color: var(--dim); cursor: pointer; font-family: inherit; font-size: 12px; font-weight: 500;
    letter-spacing: 0.3px; transition: 0.18s;
  }
  .nav-btn .nav-ico { font-size: 13px; opacity: 0.85; }
  .nav-btn:hover { color: var(--text); background: rgba(255,255,255,0.04); }
  .nav-btn.active {
    color: #fff; background: linear-gradient(120deg, var(--accent), var(--accent2));
    box-shadow: 0 6px 20px color-mix(in srgb, var(--accent) 35%, transparent);
  }

  /* ── Content ───────────────────────────────────────────────────────────────── */
  .content { flex: 1; overflow: hidden; display: flex; flex-direction: column; min-height: 0; }
  .scroll { flex: 1; overflow-y: auto; padding: 18px 22px; }
  .section-head { display: flex; align-items: flex-end; justify-content: space-between; margin-bottom: 18px; }
  .section-title { font-size: 16px; font-weight: 700; letter-spacing: 0.5px; }
  .section-sub { font-size: 11px; color: var(--dim); margin-top: 2px; }

  .panel { background: var(--surface); border: 1px solid var(--border); border-radius: 14px; padding: 14px; backdrop-filter: blur(10px); }
  .panel-title { font-size: 10px; letter-spacing: 2px; color: var(--accent); text-transform: uppercase; margin-bottom: 12px; font-weight: 600; }

  .input {
    width: 100%; background: rgba(0,0,0,0.28); border: 1px solid var(--border); color: var(--text);
    padding: 10px 12px; border-radius: 10px; font-family: inherit; font-size: 12.5px; outline: none; transition: 0.18s;
  }
  .input:focus { border-color: var(--border-hi); box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 12%, transparent); }
  .grow { flex: 1; }
  .row { display: flex; gap: 8px; align-items: center; }

  .btn {
    padding: 10px 16px; border-radius: 10px; border: 1px solid var(--border-hi); background: transparent; color: var(--accent);
    cursor: pointer; font-family: inherit; font-size: 12px; font-weight: 600; letter-spacing: 0.4px; transition: 0.18s; white-space: nowrap;
  }
  .btn:hover { background: color-mix(in srgb, var(--accent) 10%, transparent); transform: translateY(-1px); }
  .btn:disabled { opacity: 0.45; cursor: default; transform: none; }
  .btn.primary { background: linear-gradient(120deg, var(--accent), var(--accent2)); color: #fff; border: none; box-shadow: 0 6px 18px color-mix(in srgb, var(--accent) 30%, transparent); }
  .btn.ghost { border-color: var(--border); color: var(--text); }
  .btn.sm { padding: 7px 12px; font-size: 11px; }
  .btn.xs { padding: 5px 10px; font-size: 10px; border-radius: 8px; }

  .chips { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 10px; }
  .chip { padding: 5px 11px; border: 1px solid var(--border); background: transparent; color: var(--dim); cursor: pointer; font-family: inherit; font-size: 10.5px; border-radius: 999px; transition: 0.15s; text-transform: capitalize; }
  .chip.sm { padding: 3px 9px; font-size: 10px; }
  .chip:hover { color: var(--text); }
  .chip.active { border-color: var(--border-hi); color: var(--accent); background: color-mix(in srgb, var(--accent) 12%, transparent); }

  .empty { text-align: center; padding: 48px 20px; color: var(--dim); display: flex; flex-direction: column; align-items: center; gap: 8px; }
  .empty-ico { font-size: 34px; opacity: 0.9; }
  .divider { height: 1px; background: var(--border); margin: 16px 0; }
  .kv { display: flex; justify-content: space-between; align-items: center; padding: 8px 0; border-bottom: 1px solid var(--border); font-size: 12px; }
  .kv span { color: var(--dim); }
  .tag { font-size: 8.5px; padding: 2px 7px; border-radius: 5px; background: color-mix(in srgb, var(--accent) 16%, transparent); color: var(--accent); font-weight: 700; letter-spacing: 0.5px; }
  .tag.warn { background: color-mix(in srgb, #f59e0b 20%, transparent); color: #f59e0b; }

  /* ── News ──────────────────────────────────────────────────────────────────── */
  .news-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 16px; }
  .news-card { text-align: left; padding: 0; background: var(--surface); border: 1px solid var(--border); border-radius: 16px; overflow: hidden; cursor: pointer; transition: 0.25s; display: flex; flex-direction: column; font-family: inherit; color: inherit; }
  .news-card:hover { transform: translateY(-4px); border-color: var(--border-hi); box-shadow: 0 16px 44px rgba(0,0,0,0.45); }
  .news-media { height: 150px; overflow: hidden; }
  .news-media img, .news-media video { width: 100%; height: 100%; object-fit: cover; transition: 0.4s; }
  .news-card:hover .news-media img, .news-card:hover .news-media video { transform: scale(1.06); }
  .news-body { padding: 13px 14px 8px; flex: 1; }
  .news-title { font-size: 13px; font-weight: 700; line-height: 1.4; margin-bottom: 6px; }
  .news-preview { font-size: 11px; color: var(--dim); line-height: 1.55; display: -webkit-box; -webkit-line-clamp: 3; -webkit-box-orient: vertical; overflow: hidden; }
  .news-foot { padding: 10px 14px; border-top: 1px solid var(--border); display: flex; justify-content: space-between; font-size: 10.5px; }
  .news-author { color: var(--accent); font-weight: 600; }
  .news-more { color: var(--dim); }

  .overlay { position: fixed; inset: 0; background: rgba(4,5,9,0.82); display: flex; align-items: center; justify-content: center; z-index: 100; padding: 26px; backdrop-filter: blur(6px); animation: fade 0.2s; }
  @keyframes fade { from { opacity: 0; } to { opacity: 1; } }
  .modal { position: relative; background: var(--surface-solid); border: 1px solid var(--border-hi); border-radius: 18px; overflow: hidden; max-width: 640px; width: 100%; max-height: 86vh; display: flex; flex-direction: column; animation: rise 0.25s; }
  @keyframes rise { from { transform: translateY(24px); opacity: 0; } to { transform: none; opacity: 1; } }
  .modal-media { max-height: 300px; overflow: hidden; }
  .modal-media img, .modal-media video { width: 100%; height: 300px; object-fit: cover; }
  .modal-body { padding: 22px; overflow-y: auto; }
  .modal-title { font-size: 19px; font-weight: 800; line-height: 1.3; margin-bottom: 8px; }
  .modal-meta { font-size: 11px; color: var(--accent); margin-bottom: 16px; }
  .modal-text { font-size: 13px; line-height: 1.75; white-space: pre-wrap; color: #cdd2df; }
  .attachments { display: flex; flex-direction: column; gap: 10px; margin-top: 16px; }
  .att-media { width: 100%; border-radius: 12px; max-height: 340px; object-fit: cover; background: #000; border: 1px solid var(--border); }
  .att-file { display: flex; align-items: center; gap: 9px; padding: 10px 12px; border-radius: 10px; border: 1px solid var(--border); background: rgba(0,0,0,0.25); color: var(--text); font-size: 12px; transition: 0.15s; }
  .att-file:hover { border-color: var(--border-hi); background: color-mix(in srgb, var(--accent) 8%, transparent); }
  .att-ico { font-size: 15px; flex-shrink: 0; }
  .att-name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .att-sz { color: var(--dim); font-size: 11px; flex-shrink: 0; }
  .att-dl { color: var(--accent); font-weight: 700; flex-shrink: 0; }
  .modal-close { position: absolute; top: 14px; right: 14px; width: 34px; height: 34px; border-radius: 50%; border: 1px solid var(--border); background: rgba(0,0,0,0.5); color: var(--text); cursor: pointer; transition: 0.15s; }
  .modal-close:hover { background: var(--accent); color: #fff; }

  /* ── Scan layout ───────────────────────────────────────────────────────────── */
  .scan-layout { flex: 1; display: flex; gap: 14px; padding: 16px 18px; min-height: 0; }
  .scan-sidebar { width: 258px; flex-shrink: 0; display: flex; flex-direction: column; gap: 12px; }
  .scan-sidebar .grow { flex: 1; overflow-y: auto; }
  .scan-main { flex: 1; display: flex; flex-direction: column; gap: 12px; min-width: 0; }
  .actions { display: flex; flex-direction: column; gap: 8px; }

  .modules { display: flex; flex-direction: column; gap: 2px; }
  .module { display: flex; align-items: center; gap: 9px; padding: 7px 8px; border-radius: 9px; font-size: 12px; color: var(--dim); cursor: pointer; transition: 0.15s; }
  .module:hover { background: rgba(255,255,255,0.03); }
  .module.on { color: var(--text); background: color-mix(in srgb, var(--accent) 8%, transparent); }
  .module.locked { opacity: 0.5; cursor: default; }
  .module input { accent-color: var(--accent); cursor: pointer; }
  .module-ico { width: 16px; text-align: center; color: var(--accent); }
  .module-label { flex: 1; }

  .term-head { display: flex; align-items: center; gap: 8px; padding: 9px 14px; background: var(--surface); border: 1px solid var(--border); border-radius: 12px; font-size: 11px; backdrop-filter: blur(10px); }
  .prompt { color: var(--accent); font-family: 'JetBrains Mono', monospace; font-weight: 600; }
  .term-head .dim { font-family: 'JetBrains Mono', monospace; }
  .rec { color: #ef4444; font-weight: 700; font-size: 10px; letter-spacing: 1px; animation: pulse 1.2s infinite; }
  .panel-tabs { margin-left: auto; display: flex; gap: 4px; align-items: center; }
  .ptab { padding: 5px 11px; border: none; background: transparent; color: var(--dim); cursor: pointer; font-family: inherit; font-size: 11px; border-radius: 8px; transition: 0.15s; }
  .ptab.active { color: var(--accent); background: color-mix(in srgb, var(--accent) 12%, transparent); }

  .term { flex: 1; display: flex; flex-direction: column; min-height: 0; }
  .filters { display: flex; flex-wrap: wrap; gap: 5px; margin-bottom: 10px; }
  .console { flex: 1; overflow-y: auto; font-family: 'JetBrains Mono', ui-monospace, monospace; font-size: 11.5px; line-height: 1.75; padding-right: 4px; }
  .ev { display: flex; gap: 9px; align-items: flex-start; padding: 1px 0; }
  .ev-time { color: var(--dim); font-size: 9.5px; min-width: 58px; opacity: 0.7; }
  .ev-mod { color: var(--accent); font-size: 9.5px; min-width: 58px; text-transform: uppercase; opacity: 0.85; }
  .ev-ico { width: 12px; flex-shrink: 0; }
  .ev-text { color: var(--text); word-break: break-word; flex: 1; }
  .ev-found .ev-text { color: #86efac; }
  .ev-found .ev-ico { color: #4ade80; }
  .ev-error .ev-text { color: #fca5a5; }
  .ev-running .ev-text { color: var(--accent); }
  .ev-done .ev-text { color: var(--dim); }
  .ev-info .ev-text { color: #93c5fd; }

  .scrolly { flex: 1; overflow-y: auto; }
  .dossier-h { color: var(--accent); font-weight: 700; margin-bottom: 6px; }
  .dossier-item { padding: 6px 0; border-bottom: 1px solid var(--border); font-size: 11.5px; font-family: 'JetBrains Mono', monospace; }
  .dossier-item .ev-mod { display: inline; }
  .ai-out { white-space: pre-wrap; font-family: 'JetBrains Mono', monospace; font-size: 12px; line-height: 1.7; color: #cdd2df; }

  .map { flex: 1; width: 100%; }
  .locked-tab { margin: auto; }

  /* Discord / Telegram tabs — reference repo card + disclaimer */
  .repo-card { margin-top: 16px; background: var(--surface); border: 1px solid var(--border); border-radius: 14px; padding: 16px; }
  .repo-head { display: flex; align-items: center; gap: 12px; }
  .repo-ico { font-size: 20px; color: var(--accent); }
  .repo-meta { flex: 1; min-width: 0; }
  .repo-title { font-weight: 700; font-size: 13px; }
  .repo-sub { font-size: 10.5px; color: var(--dim); margin-top: 2px; }
  .repo-badge { font-size: 9.5px; letter-spacing: 0.5px; text-transform: uppercase; color: var(--dim); border: 1px solid var(--border); border-radius: 999px; padding: 3px 9px; white-space: nowrap; }
  .repo-desc { font-size: 11.5px; color: var(--text); opacity: 0.85; margin: 12px 0; line-height: 1.6; }
  .repo-link { display: inline-block; font-size: 11.5px; color: var(--accent); text-decoration: none; word-break: break-all; }
  .repo-link:hover { text-decoration: underline; }
  .repo-warn { margin-top: 12px; font-size: 10.5px; line-height: 1.6; color: #fca5a5; background: rgba(239,68,68,0.08); border: 1px solid rgba(239,68,68,0.25); border-left: 3px solid #ef4444; border-radius: 8px; padding: 10px 12px; }

  /* Forwarder — channel pickers + mapping */
  .chan-list { max-height: 34vh; overflow-y: auto; border: 1px solid var(--border); border-radius: 10px; }
  .chan { display: flex; align-items: center; gap: 8px; padding: 8px 10px; border-bottom: 1px solid rgba(255,255,255,0.05); cursor: pointer; font-size: 11.5px; }
  .chan:last-child { border-bottom: none; }
  .chan:hover { background: rgba(255,255,255,0.04); }
  .chan.sel { background: color-mix(in srgb, var(--accent) 14%, transparent); }
  .chan input { accent-color: var(--accent); }
  .chan-name { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .chan-guild { color: var(--dim); margin-left: auto; font-size: 10px; white-space: nowrap; }
  .fwd-row { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; font-size: 11px; }
  .fwd-chip { background: var(--surface); border: 1px solid var(--border); border-radius: 8px; padding: 6px 10px; }
  .fwd-arrow { color: var(--accent); font-weight: 700; }

  /* ── Settings ──────────────────────────────────────────────────────────────── */
  .settings { display: flex; gap: 20px; }
  .settings-nav { width: 160px; flex-shrink: 0; display: flex; flex-direction: column; gap: 4px; }
  .snav { padding: 9px 14px; border: none; background: transparent; color: var(--dim); cursor: pointer; font-family: inherit; font-size: 12px; border-radius: 9px; text-align: left; transition: 0.15s; }
  .snav:hover { color: var(--text); background: rgba(255,255,255,0.04); }
  .snav.active { color: var(--accent); background: color-mix(in srgb, var(--accent) 12%, transparent); font-weight: 600; }
  .settings-body { flex: 1; max-width: 620px; }
  .field { margin-bottom: 16px; }
  .lbl { display: block; font-size: 10px; color: var(--dim); letter-spacing: 1px; text-transform: uppercase; margin-bottom: 7px; }
  .hint { font-size: 11px; color: var(--dim); margin-top: 6px; line-height: 1.5; }
  .token-status { font-size: 12px; margin-top: 10px; padding: 8px 12px; border-radius: 9px; background: rgba(255,255,255,0.03); }
  .token-status.ok { color: #4ade80; } .token-status.err { color: #fca5a5; } .token-status.pending { color: var(--accent); }
  .hwid-box { font-size: 10px; color: var(--dim); padding: 10px; background: rgba(0,0,0,0.3); border-radius: 9px; word-break: break-all; border: 1px solid var(--border); }
  .toggle { display: flex; align-items: center; gap: 9px; font-size: 12px; color: var(--dim); cursor: pointer; padding: 5px 0; }
  .toggle input { accent-color: var(--accent); }

  .subs { display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 14px; }
  .sub { position: relative; background: var(--surface); border: 1px solid var(--border); border-radius: 16px; padding: 20px; overflow: hidden; transition: 0.25s; }
  .sub:hover { transform: translateY(-4px); border-color: var(--border-hi); }
  .sub.highlight { border-color: var(--border-hi); box-shadow: 0 0 0 1px var(--border-hi), 0 12px 36px color-mix(in srgb, var(--accent) 18%, transparent); }
  .sub-ribbon { position: absolute; top: 14px; right: -26px; transform: rotate(38deg); background: linear-gradient(120deg, var(--accent), var(--accent2)); color: #fff; font-size: 8px; font-weight: 700; letter-spacing: 1px; padding: 3px 30px; }
  .sub-name { font-size: 11px; letter-spacing: 3px; color: var(--accent); font-weight: 700; }
  .sub-price { font-size: 26px; font-weight: 800; margin: 6px 0; }
  .sub-price span { font-size: 12px; color: var(--dim); font-weight: 400; }
  .sub-feats { list-style: none; margin: 12px 0; padding: 0; }
  .sub-feats li { font-size: 11px; color: var(--dim); padding: 3px 0; }
  .sub-feats li.ok { color: var(--text); } .sub-feats li.ok::before { content: "✓ "; color: #4ade80; }
  .sub-feats li.no::before { content: "✕ "; color: #ef4444; }

  .swatches { display: flex; gap: 10px; flex-wrap: wrap; }
  .swatch { display: flex; flex-direction: column; align-items: center; gap: 6px; padding: 10px 14px; border: 1px solid var(--border); border-radius: 12px; background: transparent; color: var(--dim); cursor: pointer; transition: 0.15s; }
  .swatch.active { border-color: var(--border-hi); color: var(--text); }
  .swatch-dot { width: 26px; height: 26px; border-radius: 50%; }

  /* ── About ─────────────────────────────────────────────────────────────────── */
  .about-hero { text-align: center; margin: 20px 0 26px; }
  .about-mark { font-size: 42px; color: var(--accent); filter: drop-shadow(0 0 14px color-mix(in srgb, var(--accent) 55%, transparent)); }
  .about-title { font-size: 26px; font-weight: 800; letter-spacing: 4px; margin-top: 6px;
    background: linear-gradient(120deg, var(--accent), var(--accent2)); -webkit-background-clip: text; background-clip: text; color: transparent; }
  .about-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(230px, 1fr)); gap: 12px; }
  .about-card { display: flex; gap: 12px; align-items: flex-start; background: var(--surface); border: 1px solid var(--border); border-radius: 14px; padding: 14px; transition: 0.2s; }
  .about-card:hover { border-color: var(--border-hi); transform: translateY(-2px); }
  .about-ico { font-size: 20px; color: var(--accent); }
  .about-name { font-size: 12.5px; font-weight: 700; }

  /* ── Compact ───────────────────────────────────────────────────────────────── */
  .compact { font-size: 12px; }
  .compact .panel { padding: 10px; }
  .compact .news-media { height: 120px; }

  /* ── Ban ───────────────────────────────────────────────────────────────────── */
  .ban-screen { position: fixed; inset: 0; z-index: 9999; background: #0a0b10; display: flex; align-items: center; justify-content: center; }
  .ban-card { text-align: center; max-width: 460px; padding: 40px; }
  .ban-icon { font-size: 60px; margin-bottom: 18px; }
  .ban-title { font-size: 22px; color: #fca5a5; letter-spacing: 3px; font-weight: 800; margin-bottom: 16px; }
  .ban-body { font-size: 13px; color: var(--dim); line-height: 1.7; margin-bottom: 22px; }
  .ban-body strong { color: #e6e8ef; }
  .ban-actions { display: flex; flex-direction: column; align-items: center; gap: 10px; margin-bottom: 22px; }
  .ban-hint { font-size: 11px; color: var(--dim); line-height: 1.6; }
  .ban-msg { font-size: 11px; color: #fbbf24; }
  .hwid-chip { font-size: 10px; color: var(--dim); font-family: 'JetBrains Mono', monospace; }

  /* ── Scrollbars ────────────────────────────────────────────────────────────── */
  .scroll::-webkit-scrollbar, .console::-webkit-scrollbar, .scrolly::-webkit-scrollbar, .grow::-webkit-scrollbar { width: 6px; }
  .scroll::-webkit-scrollbar-thumb, .console::-webkit-scrollbar-thumb, .scrolly::-webkit-scrollbar-thumb, .grow::-webkit-scrollbar-thumb {
    background: color-mix(in srgb, var(--accent) 40%, transparent); border-radius: 3px;
  }
</style>
