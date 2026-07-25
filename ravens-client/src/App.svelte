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

  // ── Discord ───────────────────────────────────────────────────────────────────
  let dcBotToken = "";
  let dcBotStatus: "idle" | "connecting" | "ok" | "error" = "idle";
  let dcBotInfo: any = null;
  let dcBotError = "";
  let dcUserTokenInput = "";
  let dcUsers: Array<{ token: string; info: any; status: "ok" | "error" }> = [];
  let dcAddingUser = false;

  // ── Discord OSINT state ───────────────────────────────────────────────────────
  let dcOsintGuildId = "";
  let dcOsintGuildResult: any = null;
  let dcOsintGuildLoading = false;
  let dcOsintGuildError = "";
  let dcOsintUserId = "";
  let dcOsintUserResult: any = null;
  let dcOsintUserLoading = false;
  let dcOsintUserError = "";
  let dcInviteInput = "";
  let dcInviteResult: any = null;
  let dcInviteLoading = false;
  let dcInviteError = "";
  let dcGatewayGuilds: any[] = [];
  let dcGatewayLoaded = false;

  // ── Telegram ──────────────────────────────────────────────────────────────────
  let tgBotToken = "";
  let tgBotStatus: "idle" | "connecting" | "ok" | "error" = "idle";
  let tgBotInfo: any = null;
  let tgBotError = "";
  let tgPhoneInput = "";
  let tgPasswordInput = "";
  let tgAccounts: Array<{ phone: string; has2fa: boolean }> = [];
  let tgAddingAccount = false;

  // ── Telegram OSINT state ──────────────────────────────────────────────────────
  let tgOsintTarget = "";
  let tgOsintResult: any = null;
  let tgOsintLoading = false;
  let tgOsintError = "";

  // ── TG→DC forwarding config ───────────────────────────────────────────────────
  let tgFwdConfigs: Array<{ tgChannel: string; dcChannelId: string; label: string }> = [];
  let tgFwdTgInput = "";
  let tgFwdDcIdInput = "";
  let tgFwdLabelInput = "";

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

    // ── Load saved Discord state ─────────────────────────────────────────────
    try {
      const savedDcBot = await invoke<string>("load_config", { key: "dc_bot_token" }).catch(() => "");
      if (savedDcBot) { dcBotToken = savedDcBot; connectDiscordBot(); }
    } catch {}
    try {
      const savedDcUsers = await invoke<string>("load_config", { key: "dc_users" }).catch(() => "[]");
      const parsedDcUsers: Array<{ token: string }> = JSON.parse(savedDcUsers || "[]");
      for (const u of parsedDcUsers) {
        try {
          const res = await fetch("https://discord.com/api/v10/users/@me", { headers: { Authorization: u.token } });
          const info = res.ok ? await res.json() : null;
          dcUsers = [...dcUsers, { token: u.token, info, status: res.ok ? "ok" : "error" }];
        } catch { dcUsers = [...dcUsers, { token: u.token, info: null, status: "error" }]; }
      }
    } catch {}

    // ── Load saved Telegram state ────────────────────────────────────────────
    try {
      const savedTgBot = await invoke<string>("load_config", { key: "tg_bot_token" }).catch(() => "");
      if (savedTgBot) { tgBotToken = savedTgBot; connectTgBot(); }
    } catch {}
    try {
      const savedTgPhones: string[] = JSON.parse(
        await invoke<string>("load_config", { key: "tg_accounts" }).catch(() => "[]") || "[]"
      );
      for (const phone of savedTgPhones) {
        try {
          const stored = await invoke<string>("load_config", { key: `tg_acc_${phone.replace(/\D/g, "")}` }).catch(() => "{}");
          const parsed = JSON.parse(stored || "{}");
          tgAccounts = [...tgAccounts, { phone, has2fa: !!parsed.password }];
        } catch { tgAccounts = [...tgAccounts, { phone, has2fa: false }]; }
      }
    } catch {}

    // ── Load TG→DC forwarding configs ────────────────────────────────────────
    try {
      const savedFwd = await invoke<string>("load_config", { key: "tg_fwd_configs" }).catch(() => "[]");
      tgFwdConfigs = JSON.parse(savedFwd || "[]");
    } catch {}
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
  // Fetch gateway guilds when Discord tab is opened.
  $: if (activeTab === "discord" && serverUrl) dcFetchGatewayGuilds();
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

  function toggleModule(mod: string) {
    if (selectedModules.has(mod)) selectedModules.delete(mod);
    else selectedModules.add(mod);
    selectedModules = new Set(selectedModules);
  }
  function setScheme(key: string) { currentScheme = key; localStorage.setItem("ravens_scheme", key); }
  function toggleCompactMode() { compactMode = !compactMode; localStorage.setItem("ravens_compact", String(compactMode)); }

  // ── Discord functions ─────────────────────────────────────────────────────────
  async function connectDiscordBot() {
    if (!dcBotToken.trim()) return;
    dcBotStatus = "connecting"; dcBotError = "";
    try {
      const res = await fetch("https://discord.com/api/v10/users/@me", {
        headers: { Authorization: "Bot " + dcBotToken.trim() },
      });
      if (res.ok) {
        dcBotInfo = await res.json();
        dcBotStatus = "ok";
        invoke("save_config", { key: "dc_bot_token", value: dcBotToken.trim() }).catch(() => {});
      } else {
        dcBotStatus = "error";
        dcBotError = res.status === 401 ? "Неверный токен бота" : `Ошибка ${res.status}`;
      }
    } catch { dcBotStatus = "error"; dcBotError = "Нет соединения с Discord"; }
  }

  function disconnectDiscordBot() {
    dcBotToken = ""; dcBotInfo = null; dcBotStatus = "idle";
    invoke("clear_config", { key: "dc_bot_token" }).catch(() => {});
  }

  async function addDiscordUser() {
    if (!dcUserTokenInput.trim() || dcAddingUser) return;
    dcAddingUser = true;
    const t = dcUserTokenInput.trim();
    try {
      const res = await fetch("https://discord.com/api/v10/users/@me", {
        headers: { Authorization: t },
      });
      const info = res.ok ? await res.json() : null;
      dcUsers = [...dcUsers, { token: t, info, status: res.ok ? "ok" : "error" }];
    } catch {
      dcUsers = [...dcUsers, { token: t, info: null, status: "error" }];
    }
    dcUserTokenInput = "";
    saveDiscordUsers();
    dcAddingUser = false;
  }

  function removeDiscordUser(i: number) {
    dcUsers = dcUsers.filter((_, idx) => idx !== i);
    saveDiscordUsers();
  }

  function saveDiscordUsers() {
    invoke("save_config", { key: "dc_users", value: JSON.stringify(dcUsers.map((u) => ({ token: u.token }))) }).catch(() => {});
  }

  // ── Telegram functions ────────────────────────────────────────────────────────
  async function connectTgBot() {
    if (!tgBotToken.trim()) return;
    tgBotStatus = "connecting"; tgBotError = "";
    try {
      const res = await fetch(`https://api.telegram.org/bot${tgBotToken.trim()}/getMe`);
      const data = await res.json();
      if (data.ok) {
        tgBotInfo = data.result; tgBotStatus = "ok";
        invoke("save_config", { key: "tg_bot_token", value: tgBotToken.trim() }).catch(() => {});
      } else {
        tgBotStatus = "error";
        tgBotError = data.description || "Неверный токен";
      }
    } catch { tgBotStatus = "error"; tgBotError = "Нет соединения с Telegram"; }
  }

  function disconnectTgBot() {
    tgBotToken = ""; tgBotInfo = null; tgBotStatus = "idle";
    invoke("clear_config", { key: "tg_bot_token" }).catch(() => {});
  }

  function addTgAccount() {
    if (!tgPhoneInput.trim() || tgAddingAccount) return;
    tgAddingAccount = true;
    const phone = tgPhoneInput.trim();
    const has2fa = !!tgPasswordInput.trim();
    const key = `tg_acc_${phone.replace(/\D/g, "")}`;
    invoke("save_config", { key, value: JSON.stringify({ phone, password: tgPasswordInput.trim() }) }).catch(() => {});
    tgAccounts = [...tgAccounts, { phone, has2fa }];
    tgPhoneInput = ""; tgPasswordInput = "";
    invoke("save_config", { key: "tg_accounts", value: JSON.stringify(tgAccounts.map((a) => a.phone)) }).catch(() => {});
    tgAddingAccount = false;
  }

  function removeTgAccount(i: number) {
    const acc = tgAccounts[i];
    invoke("clear_config", { key: `tg_acc_${acc.phone.replace(/\D/g, "")}` }).catch(() => {});
    tgAccounts = tgAccounts.filter((_, idx) => idx !== i);
    invoke("save_config", { key: "tg_accounts", value: JSON.stringify(tgAccounts.map((a) => a.phone)) }).catch(() => {});
  }

  // ── Discord OSINT functions ───────────────────────────────────────────────────
  async function dcLookupGuild() {
    if (!dcOsintGuildId.trim() || dcOsintGuildLoading) return;
    if (!tokenInput || !sessionId) { dcOsintGuildError = "Сначала активируйте лицензию"; return; }
    dcOsintGuildLoading = true; dcOsintGuildError = ""; dcOsintGuildResult = null;
    try {
      const res: any = await invoke("discord_osint_run", {
        token: tokenInput, sessionId, endpoint: "guild-info",
        botToken: dcBotToken, target: dcOsintGuildId.trim(),
      });
      if (res?.ok) dcOsintGuildResult = res;
      else dcOsintGuildError = res?.error || "Ошибка";
    } catch (e: any) { dcOsintGuildError = `${e}`; }
    dcOsintGuildLoading = false;
  }

  async function dcLookupUser() {
    if (!dcOsintUserId.trim() || dcOsintUserLoading) return;
    if (!tokenInput || !sessionId) { dcOsintUserError = "Сначала активируйте лицензию"; return; }
    dcOsintUserLoading = true; dcOsintUserError = ""; dcOsintUserResult = null;
    try {
      const res: any = await invoke("discord_osint_run", {
        token: tokenInput, sessionId, endpoint: "user-info",
        botToken: dcBotToken, target: dcOsintUserId.trim(),
      });
      if (res?.ok) dcOsintUserResult = res;
      else dcOsintUserError = res?.error || "Ошибка";
    } catch (e: any) { dcOsintUserError = `${e}`; }
    dcOsintUserLoading = false;
  }

  async function dcParseInvite() {
    if (!dcInviteInput.trim() || dcInviteLoading) return;
    if (!tokenInput || !sessionId) { dcInviteError = "Сначала активируйте лицензию"; return; }
    dcInviteLoading = true; dcInviteError = ""; dcInviteResult = null;
    try {
      const res: any = await invoke("discord_osint_run", {
        token: tokenInput, sessionId, endpoint: "invite-info",
        botToken: "", target: dcInviteInput.trim(),
      });
      if (res?.ok) dcInviteResult = res;
      else dcInviteError = res?.error || "Неверная ссылка";
    } catch (e: any) { dcInviteError = `${e}`; }
    dcInviteLoading = false;
  }

  async function dcFetchGatewayGuilds() {
    if (!serverUrl || dcGatewayLoaded) return;
    try {
      const res = await fetch(`${serverUrl}/api/discord/state`);
      if (res.ok) { const d = await res.json(); dcGatewayGuilds = d.guilds || []; dcGatewayLoaded = true; }
    } catch {}
  }

  // ── Telegram OSINT functions ──────────────────────────────────────────────────
  async function tgLookupChat() {
    if (!tgOsintTarget.trim() || tgOsintLoading) return;
    if (!tokenInput || !sessionId) { tgOsintError = "Сначала активируйте лицензию"; return; }
    tgOsintLoading = true; tgOsintError = ""; tgOsintResult = null;
    try {
      const res: any = await invoke("telegram_osint_run", {
        token: tokenInput, sessionId, botToken: tgBotToken, target: tgOsintTarget.trim(),
      });
      if (res?.ok) tgOsintResult = res;
      else tgOsintError = res?.error || "Ошибка";
    } catch (e: any) { tgOsintError = `${e}`; }
    tgOsintLoading = false;
  }

  // ── TG→DC forwarding functions ────────────────────────────────────────────────
  function addTgFwdConfig() {
    if (!tgFwdTgInput.trim() || !tgFwdDcIdInput.trim()) return;
    tgFwdConfigs = [...tgFwdConfigs, {
      tgChannel: tgFwdTgInput.trim(),
      dcChannelId: tgFwdDcIdInput.trim(),
      label: tgFwdLabelInput.trim() || tgFwdTgInput.trim(),
    }];
    tgFwdTgInput = ""; tgFwdDcIdInput = ""; tgFwdLabelInput = "";
    invoke("save_config", { key: "tg_fwd_configs", value: JSON.stringify(tgFwdConfigs) }).catch(() => {});
  }

  function removeTgFwdConfig(i: number) {
    tgFwdConfigs = tgFwdConfigs.filter((_, idx) => idx !== i);
    invoke("save_config", { key: "tg_fwd_configs", value: JSON.stringify(tgFwdConfigs) }).catch(() => {});
  }

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
    {#each [["ravens","⬡","Лента"],["scan","◎","OSINT"],["map","◈","Карта"],["discord","💬","Discord"],["telegram","✈","Telegram"],["settings","⚙","Настройки"],["about","𝓲","О системе"]] as [t, icon, label]}
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
            <div class="section-title">💬 Discord</div>
            <div class="section-sub">OSINT, управление аккаунтами, парсинг серверов</div>
          </div>
        </div>

        <!-- ── BOT CONNECTION ─────────────────────────────────────────────────── -->
        <div class="intg-card">
          <div class="panel-title">БОТ · ПОДКЛЮЧЕНИЕ</div>
          {#if dcBotStatus === "ok" && dcBotInfo}
            <div class="intg-connected">
              <div class="intg-avatar dc">🤖</div>
              <div class="intg-info">
                <div class="intg-name">{dcBotInfo.username}{dcBotInfo.discriminator && dcBotInfo.discriminator !== "0" ? "#" + dcBotInfo.discriminator : ""}</div>
                <div class="dim sm">ID: {dcBotInfo.id} · Bot · application_id: {dcBotInfo.id}</div>
              </div>
              <span class="badge-conn">● ONLINE</span>
              <button class="btn ghost sm" on:click={disconnectDiscordBot}>Отключить</button>
            </div>
          {:else}
            <div class="field">
              <label class="lbl">Bot Token</label>
              <div class="row">
                <input class="input grow" type="password" bind:value={dcBotToken}
                  placeholder="Токен из Discord Developer Portal"
                  on:keydown={(e) => e.key === "Enter" && connectDiscordBot()} />
                <button class="btn primary sm" on:click={connectDiscordBot}
                  disabled={dcBotStatus === "connecting" || !dcBotToken}>
                  {dcBotStatus === "connecting" ? "⏳" : "Подключить"}
                </button>
              </div>
              {#if dcBotError}<div class="intg-err">✗ {dcBotError}</div>{/if}
            </div>
            <div class="hint">Создайте приложение на <a href="https://discord.com/developers/applications" target="_blank">discord.com/developers</a> → Bot → Token.</div>
          {/if}
        </div>

        <!-- ── BOT OSINT: GUILD LOOKUP ────────────────────────────────────────── -->
        {#if dcBotStatus === "ok"}
        <div class="intg-card">
          <div class="panel-title">БОТ · OSINT — СЕРВЕР</div>
          <div class="hint" style="margin-bottom:10px">Введите Guild ID (бот должен состоять в сервере). Получите ID через режим разработчика Discord.</div>
          <div class="field">
            <div class="row">
              <input class="input grow mono" bind:value={dcOsintGuildId} placeholder="Guild ID (18-значное число)"
                on:keydown={(e) => e.key === "Enter" && dcLookupGuild()} />
              <button class="btn primary sm" on:click={dcLookupGuild} disabled={dcOsintGuildLoading || !dcOsintGuildId}>
                {dcOsintGuildLoading ? "⏳" : "Поиск"}
              </button>
            </div>
            {#if dcOsintGuildError}<div class="intg-err">✗ {dcOsintGuildError}</div>{/if}
          </div>
          {#if dcOsintGuildResult}
            {@const g = dcOsintGuildResult.guild}
            <div class="osint-result">
              {#if g.icon}<img class="osint-icon" src="https://cdn.discordapp.com/icons/{g.id}/{g.icon}.webp?size=64" alt="icon" />{/if}
              <div class="osint-main">
                <div class="osint-title">{g.name}</div>
                <div class="osint-row"><span class="osint-lbl">ID</span><span class="mono">{g.id}</span></div>
                <div class="osint-row"><span class="osint-lbl">Владелец</span><span class="mono">{g.owner_id}</span></div>
                <div class="osint-row"><span class="osint-lbl">Участники</span>{g.approximate_member_count ?? g.member_count ?? "—"}</div>
                <div class="osint-row"><span class="osint-lbl">Онлайн</span>{g.approximate_presence_count ?? "—"}</div>
                <div class="osint-row"><span class="osint-lbl">Регион</span>{g.preferred_locale ?? "—"}</div>
                <div class="osint-row"><span class="osint-lbl">Создан</span>{g.id ? new Date(Number((BigInt(g.id) >> 22n) + 1420070400000n)).toLocaleDateString("ru") : "—"}</div>
                {#if g.description}<div class="osint-row"><span class="osint-lbl">Описание</span>{g.description}</div>{/if}
                {#if g.vanity_url_code}<div class="osint-row"><span class="osint-lbl">Vanity</span>discord.gg/{g.vanity_url_code}</div>{/if}
                {#if g.features?.length}<div class="osint-row"><span class="osint-lbl">Функции</span><span class="mono" style="font-size:10px">{g.features.join(", ")}</span></div>{/if}
                {#if dcOsintGuildResult.channels?.length}
                  <div class="osint-row"><span class="osint-lbl">Каналы ({dcOsintGuildResult.channels.length})</span></div>
                  <div class="osint-chanlist">
                    {#each dcOsintGuildResult.channels.slice(0, 30) as c}
                      <div class="osint-chan">{c.type === 0 ? "💬" : c.type === 2 ? "🔊" : c.type === 4 ? "📁" : "•"} {c.name}<span class="dim sm"> {c.id}</span></div>
                    {/each}
                    {#if dcOsintGuildResult.channels.length > 30}<div class="dim sm">... и ещё {dcOsintGuildResult.channels.length - 30}</div>{/if}
                  </div>
                {/if}
              </div>
            </div>
          {/if}
        </div>

        <!-- ── BOT OSINT: USER LOOKUP ─────────────────────────────────────────── -->
        <div class="intg-card">
          <div class="panel-title">БОТ · OSINT — ПОЛЬЗОВАТЕЛЬ</div>
          <div class="hint" style="margin-bottom:10px">Поиск пользователя Discord по ID. Работает для любого публичного аккаунта.</div>
          <div class="field">
            <div class="row">
              <input class="input grow mono" bind:value={dcOsintUserId} placeholder="User ID (18-значное число)"
                on:keydown={(e) => e.key === "Enter" && dcLookupUser()} />
              <button class="btn primary sm" on:click={dcLookupUser} disabled={dcOsintUserLoading || !dcOsintUserId}>
                {dcOsintUserLoading ? "⏳" : "Поиск"}
              </button>
            </div>
            {#if dcOsintUserError}<div class="intg-err">✗ {dcOsintUserError}</div>{/if}
          </div>
          {#if dcOsintUserResult}
            {@const u = dcOsintUserResult.user}
            <div class="osint-result">
              {#if u.avatar}<img class="osint-icon" src="https://cdn.discordapp.com/avatars/{u.id}/{u.avatar}.webp?size=64" alt="avatar" />{/if}
              <div class="osint-main">
                <div class="osint-title">{u.username}{u.discriminator && u.discriminator !== "0" ? "#" + u.discriminator : ""}</div>
                <div class="osint-row"><span class="osint-lbl">ID</span><span class="mono">{u.id}</span></div>
                <div class="osint-row"><span class="osint-lbl">Бот</span>{u.bot ? "✓ Да" : "Нет"}</div>
                <div class="osint-row"><span class="osint-lbl">Создан</span>{u.id ? new Date(Number((BigInt(u.id) >> 22n) + 1420070400000n)).toLocaleDateString("ru") : "—"}</div>
                {#if u.global_name}<div class="osint-row"><span class="osint-lbl">Global name</span>{u.global_name}</div>{/if}
                {#if u.public_flags}<div class="osint-row"><span class="osint-lbl">Badges</span><span class="mono">{u.public_flags}</span></div>{/if}
                {#if u.banner_color}<div class="osint-row"><span class="osint-lbl">Цвет профиля</span>{u.banner_color}</div>{/if}
              </div>
            </div>
          {/if}
        </div>

        <!-- ── BOT GUILDS ─────────────────────────────────────────────────────── -->
        {#if dcGatewayGuilds.length > 0}
        <div class="intg-card">
          <div class="panel-title">БОТ · СЕРВЕРЫ ({dcGatewayGuilds.length})</div>
          <div class="hint" style="margin-bottom:8px">Серверы, на которых состоит бот (данные Gateway).</div>
          <div class="intg-list">
            {#each dcGatewayGuilds as gld}
              <div class="intg-row" style="cursor:pointer" on:click={() => { dcOsintGuildId = gld.id; dcLookupGuild(); }}>
                <div class="intg-avatar dc" style="font-size:14px">🏠</div>
                <div class="intg-info">
                  <div class="intg-name">{gld.name}</div>
                  <div class="dim sm">ID: {gld.id} · {gld.member_count} участников · {gld.channels?.length ?? 0} каналов</div>
                </div>
                <span class="dim sm" style="flex-shrink:0">→ OSINT</span>
              </div>
            {/each}
          </div>
        </div>
        {/if}
        {/if}

        <!-- ── INVITE / SERVER PARSER ─────────────────────────────────────────── -->
        <div class="intg-card">
          <div class="panel-title">ПАРСЕР СЕРВЕРА — по ссылке-приглашению</div>
          <div class="hint" style="margin-bottom:10px">Парсит публичную информацию сервера Discord по invite-ссылке. Не требует токена бота.</div>
          <div class="field">
            <div class="row">
              <input class="input grow" bind:value={dcInviteInput} placeholder="https://discord.gg/... или просто код"
                on:keydown={(e) => e.key === "Enter" && dcParseInvite()} />
              <button class="btn primary sm" on:click={dcParseInvite} disabled={dcInviteLoading || !dcInviteInput}>
                {dcInviteLoading ? "⏳" : "Парсить"}
              </button>
            </div>
            {#if dcInviteError}<div class="intg-err">✗ {dcInviteError}</div>{/if}
          </div>
          {#if dcInviteResult}
            {@const inv = dcInviteResult.invite}
            {@const srv = inv.guild}
            <div class="osint-result">
              {#if srv?.icon}<img class="osint-icon" src="https://cdn.discordapp.com/icons/{srv.id}/{srv.icon}.webp?size=64" alt="icon" />{/if}
              <div class="osint-main">
                <div class="osint-title">{srv?.name ?? "Неизвестно"}</div>
                <div class="osint-row"><span class="osint-lbl">Guild ID</span><span class="mono">{srv?.id ?? "—"}</span></div>
                <div class="osint-row"><span class="osint-lbl">Код</span><span class="mono">{inv.code}</span></div>
                <div class="osint-row"><span class="osint-lbl">Участники</span>{inv.approximate_member_count ?? "—"}</div>
                <div class="osint-row"><span class="osint-lbl">Онлайн</span>{inv.approximate_presence_count ?? "—"}</div>
                <div class="osint-row"><span class="osint-lbl">Тип</span>{inv.type === 0 ? "Постоянная" : inv.type === 1 ? "Временная" : "—"}</div>
                {#if inv.expires_at}<div class="osint-row"><span class="osint-lbl">Истекает</span>{new Date(inv.expires_at).toLocaleString("ru")}</div>{/if}
                {#if inv.inviter}<div class="osint-row"><span class="osint-lbl">Создал</span>{inv.inviter.username} · <span class="mono">{inv.inviter.id}</span></div>{/if}
                {#if inv.channel}<div class="osint-row"><span class="osint-lbl">Канал</span>#{inv.channel.name} · <span class="mono">{inv.channel.id}</span></div>{/if}
                {#if srv?.description}<div class="osint-row"><span class="osint-lbl">Описание</span>{srv.description}</div>{/if}
                {#if srv?.vanity_url_code}<div class="osint-row"><span class="osint-lbl">Vanity</span>discord.gg/{srv.vanity_url_code}</div>{/if}
                {#if srv?.features?.length}<div class="osint-row"><span class="osint-lbl">Функции</span><span class="mono" style="font-size:10px">{srv.features.join(", ")}</span></div>{/if}
                {#if srv?.nsfw_level !== undefined}<div class="osint-row"><span class="osint-lbl">NSFW уровень</span>{srv.nsfw_level}</div>{/if}
                {#if srv?.verification_level !== undefined}<div class="osint-row"><span class="osint-lbl">Верификация</span>{["Нет","Низкий","Средний","Высокий","Очень высокий"][srv.verification_level] ?? srv.verification_level}</div>{/if}
              </div>
            </div>
          {/if}
        </div>

        <!-- ── USER ACCOUNTS ──────────────────────────────────────────────────── -->
        <div class="intg-card">
          <div class="panel-title">АККАУНТЫ · {dcUsers.length}</div>
          {#if dcUsers.length > 0}
            <div class="intg-list">
              {#each dcUsers as u, i}
                <div class="intg-row" class:intg-row-err={u.status === "error"}>
                  <div class="intg-avatar dc">👤</div>
                  <div class="intg-info">
                    {#if u.info}
                      <div class="intg-name">{u.info.username}{u.info.discriminator && u.info.discriminator !== "0" ? "#" + u.info.discriminator : ""}</div>
                      <div class="dim sm">ID: {u.info.id}</div>
                    {:else}
                      <div class="intg-name mono" style="font-size:10px">{u.token.substring(0, 28)}…</div>
                      <div class="dim sm" style="color:#fca5a5">Не удалось проверить токен</div>
                    {/if}
                  </div>
                  <button class="btn ghost sm intg-del" on:click={() => removeDiscordUser(i)}>✕</button>
                </div>
              {/each}
            </div>
            <div class="divider"></div>
          {/if}
          <div class="field">
            <label class="lbl">Добавить аккаунт (user token)</label>
            <div class="row">
              <input class="input grow" type="password" bind:value={dcUserTokenInput}
                placeholder="User token"
                on:keydown={(e) => e.key === "Enter" && addDiscordUser()} />
              <button class="btn sm" on:click={addDiscordUser} disabled={dcAddingUser || !dcUserTokenInput}>
                {dcAddingUser ? "⏳" : "+ Добавить"}
              </button>
            </div>
          </div>
          <div class="hint warn-hint">⚠ Использование user-token нарушает ToS Discord. Только для личного использования.</div>
        </div>
      </div>

    <!-- ── TELEGRAM ────────────────────────────────────────────────────────────── -->
    {:else if activeTab === "telegram"}
      <div class="scroll">
        <div class="section-head">
          <div>
            <div class="section-title">✈ Telegram</div>
            <div class="section-sub">OSINT через бота, аккаунты, пересылка в Discord</div>
          </div>
        </div>

        <!-- ── BOT CONNECTION ─────────────────────────────────────────────────── -->
        <div class="intg-card">
          <div class="panel-title">БОТ · ПОДКЛЮЧЕНИЕ</div>
          {#if tgBotStatus === "ok" && tgBotInfo}
            <div class="intg-connected">
              <div class="intg-avatar tg">🤖</div>
              <div class="intg-info">
                <div class="intg-name">@{tgBotInfo.username}</div>
                <div class="dim sm">{tgBotInfo.first_name} · ID: {tgBotInfo.id}{tgBotInfo.can_join_groups ? " · может вступать в группы" : ""}</div>
              </div>
              <span class="badge-conn">● ONLINE</span>
              <button class="btn ghost sm" on:click={disconnectTgBot}>Отключить</button>
            </div>
          {:else}
            <div class="field">
              <label class="lbl">Bot Token (@BotFather)</label>
              <div class="row">
                <input class="input grow" type="password" bind:value={tgBotToken}
                  placeholder="123456789:ABCDefGhIjKlMnOpQrStUvWxYz"
                  on:keydown={(e) => e.key === "Enter" && connectTgBot()} />
                <button class="btn primary sm" on:click={connectTgBot}
                  disabled={tgBotStatus === "connecting" || !tgBotToken}>
                  {tgBotStatus === "connecting" ? "⏳" : "Подключить"}
                </button>
              </div>
              {#if tgBotError}<div class="intg-err">✗ {tgBotError}</div>{/if}
            </div>
            <div class="hint">Напишите <a href="https://t.me/BotFather" target="_blank">@BotFather</a> → /newbot → скопируйте токен.</div>
          {/if}
        </div>

        <!-- ── BOT OSINT: CHAT / CHANNEL LOOKUP ───────────────────────────────── -->
        {#if tgBotStatus === "ok"}
        <div class="intg-card">
          <div class="panel-title">БОТ · OSINT — ЧАТ / КАНАЛ</div>
          <div class="hint" style="margin-bottom:10px">Введите @username публичного канала/группы или числовой chat_id. Бот должен быть в чате для получения списка администраторов.</div>
          <div class="field">
            <div class="row">
              <input class="input grow mono" bind:value={tgOsintTarget} placeholder="@channel или -100xxxxxxxxxx"
                on:keydown={(e) => e.key === "Enter" && tgLookupChat()} />
              <button class="btn primary sm" on:click={tgLookupChat} disabled={tgOsintLoading || !tgOsintTarget}>
                {tgOsintLoading ? "⏳" : "Поиск"}
              </button>
            </div>
            {#if tgOsintError}<div class="intg-err">✗ {tgOsintError}</div>{/if}
          </div>
          {#if tgOsintResult}
            {@const c = tgOsintResult.chat}
            <div class="osint-result">
              {#if c.photo?.big_file_id || c.username}
                <div class="intg-avatar tg" style="width:48px;height:48px;font-size:22px">
                  {c.type === "channel" ? "📢" : c.type === "supergroup" || c.type === "group" ? "👥" : "👤"}
                </div>
              {/if}
              <div class="osint-main">
                <div class="osint-title">{c.title ?? c.first_name ?? "—"}</div>
                {#if c.username}<div class="osint-row"><span class="osint-lbl">Username</span><a href="https://t.me/{c.username}" target="_blank">@{c.username}</a></div>{/if}
                <div class="osint-row"><span class="osint-lbl">ID</span><span class="mono">{c.id}</span></div>
                <div class="osint-row"><span class="osint-lbl">Тип</span>{c.type}</div>
                {#if tgOsintResult.member_count !== null && tgOsintResult.member_count !== undefined}
                  <div class="osint-row"><span class="osint-lbl">Участников</span>{tgOsintResult.member_count.toLocaleString("ru")}</div>
                {/if}
                {#if c.description}<div class="osint-row"><span class="osint-lbl">Описание</span>{c.description}</div>{/if}
                {#if c.invite_link}<div class="osint-row"><span class="osint-lbl">Invite</span><a href="{c.invite_link}" target="_blank">{c.invite_link}</a></div>{/if}
                {#if c.is_verified}<div class="osint-row"><span class="osint-lbl">Статус</span>✓ Верифицирован</div>{/if}
                {#if c.is_scam}<div class="osint-row"><span class="osint-lbl">Статус</span><span style="color:#fca5a5">⚠ Скам</span></div>{/if}
                {#if c.slow_mode_delay}<div class="osint-row"><span class="osint-lbl">Slow mode</span>{c.slow_mode_delay}с</div>{/if}
                {#if tgOsintResult.admins && Array.isArray(tgOsintResult.admins) && tgOsintResult.admins.length > 0}
                  <div class="osint-row"><span class="osint-lbl">Администраторы ({tgOsintResult.admins.length})</span></div>
                  {#each tgOsintResult.admins as adm}
                    <div class="osint-chan">
                      {adm.status === "creator" ? "👑" : "🛡"} 
                      {adm.user?.first_name ?? ""}{adm.user?.last_name ? " " + adm.user.last_name : ""}
                      {adm.user?.username ? " @" + adm.user.username : ""}
                      <span class="dim sm"> · {adm.user?.id}</span>
                      {#if adm.custom_title}<span class="dim sm"> · {adm.custom_title}</span>{/if}
                    </div>
                  {/each}
                {/if}
              </div>
            </div>
          {/if}
        </div>
        {/if}

        <!-- ── USER ACCOUNTS ──────────────────────────────────────────────────── -->
        <div class="intg-card">
          <div class="panel-title">АККАУНТЫ · {tgAccounts.length}</div>
          {#if tgAccounts.length > 0}
            <div class="intg-list">
              {#each tgAccounts as acc, i}
                <div class="intg-row">
                  <div class="intg-avatar tg">👤</div>
                  <div class="intg-info">
                    <div class="intg-name">{acc.phone}</div>
                    <div class="dim sm">{#if acc.has2fa}🔐 Облачный пароль сохранён{:else}Без облачного пароля{/if}</div>
                  </div>
                  <button class="btn ghost sm intg-del" on:click={() => removeTgAccount(i)}>✕</button>
                </div>
              {/each}
            </div>
            <div class="divider"></div>
          {/if}
          <div class="field">
            <label class="lbl">Номер телефона</label>
            <input class="input" bind:value={tgPhoneInput} placeholder="+79991234567" />
          </div>
          <div class="field">
            <label class="lbl">Облачный пароль (2FA) — если включён</label>
            <input class="input" type="password" bind:value={tgPasswordInput}
              placeholder="Оставьте пустым, если 2FA не установлен" />
            <div class="hint">Пароль шифруется HWID и хранится локально.</div>
          </div>
          <button class="btn primary sm" on:click={addTgAccount} disabled={!tgPhoneInput || tgAddingAccount}>
            {tgAddingAccount ? "⏳" : "+ Добавить аккаунт"}
          </button>
        </div>

        <!-- ── TG→DC FORWARDING ────────────────────────────────────────────────── -->
        <div class="intg-card">
          <div class="panel-title">TG → DISCORD · ПЕРЕСЫЛКА</div>
          <div class="hint warn-hint" style="margin-bottom:12px">
            ⚙ Конфигурация пересылки сообщений из TG-каналов через Discord бота. Пересылка активируется когда MTProto (Telegram аккаунт) будет подключён к серверу.
          </div>
          {#if tgFwdConfigs.length > 0}
            <div class="intg-list" style="margin-bottom:12px">
              {#each tgFwdConfigs as cfg, i}
                <div class="intg-row">
                  <div class="intg-avatar tg" style="font-size:14px">✈</div>
                  <div class="intg-info">
                    <div class="intg-name">{cfg.label}</div>
                    <div class="dim sm">{cfg.tgChannel} → DC канал <span class="mono">{cfg.dcChannelId}</span></div>
                  </div>
                  <button class="btn ghost sm intg-del" on:click={() => removeTgFwdConfig(i)}>✕</button>
                </div>
              {/each}
            </div>
            <div class="divider"></div>
          {/if}
          <div class="field">
            <label class="lbl">TG канал/группа (@username или ID)</label>
            <input class="input mono" bind:value={tgFwdTgInput} placeholder="@channel или -100xxxxxxxxxx" />
          </div>
          <div class="field">
            <label class="lbl">Discord Channel ID (куда пересылать)</label>
            <input class="input mono" bind:value={tgFwdDcIdInput} placeholder="Числовой ID канала Discord" />
          </div>
          <div class="field">
            <label class="lbl">Метка (опционально)</label>
            <input class="input" bind:value={tgFwdLabelInput} placeholder="Например: Новости → Общий" />
          </div>
          <button class="btn sm" on:click={addTgFwdConfig} disabled={!tgFwdTgInput || !tgFwdDcIdInput}>+ Добавить правило</button>
          {#if !dcBotStatus || dcBotStatus !== "ok"}
            <div class="hint" style="margin-top:10px">⚠ Для пересылки нужен подключённый Discord бот (вкладка Discord).</div>
          {/if}
        </div>
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

  /* ── OSINT result panels ──────────────────────────────────────────────────── */
  .osint-result {
    display: flex; gap: 14px; margin-top: 12px;
    padding: 14px; border-radius: 10px;
    background: rgba(0,0,0,0.28); border: 1px solid var(--border-hi);
  }
  .osint-icon { width: 48px; height: 48px; border-radius: 50%; flex-shrink: 0; object-fit: cover; }
  .osint-main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 4px; }
  .osint-title { font-size: 15px; font-weight: 700; margin-bottom: 4px; }
  .osint-row { display: flex; gap: 8px; font-size: 12px; flex-wrap: wrap; }
  .osint-lbl { color: var(--accent); font-weight: 600; min-width: 100px; flex-shrink: 0; }
  .osint-chanlist { margin-top: 6px; display: flex; flex-direction: column; gap: 2px; }
  .osint-chan { font-size: 11px; color: var(--text2); padding: 2px 6px; border-radius: 4px; background: rgba(255,255,255,0.03); }

  /* ── Discord / Telegram integration cards ─────────────────────────────────── */
  .intg-card {
    background: var(--surface); border: 1px solid var(--border); border-radius: 14px;
    padding: 18px; margin-bottom: 14px; backdrop-filter: blur(10px);
    max-width: 640px;
  }
  .intg-connected {
    display: flex; align-items: center; gap: 12px;
    padding: 12px 14px; border-radius: 10px;
    background: color-mix(in srgb, var(--accent) 7%, transparent);
    border: 1px solid var(--border-hi);
  }
  .intg-avatar {
    width: 36px; height: 36px; border-radius: 50%;
    display: flex; align-items: center; justify-content: center;
    font-size: 18px; flex-shrink: 0;
  }
  .intg-avatar.dc { background: color-mix(in srgb, #5865f2 25%, transparent); }
  .intg-avatar.tg { background: color-mix(in srgb, #26a5e4 25%, transparent); }
  .intg-info { flex: 1; min-width: 0; }
  .intg-name { font-size: 13px; font-weight: 700; }
  .badge-conn {
    font-size: 10px; color: #4ade80; font-weight: 700;
    letter-spacing: 1px; flex-shrink: 0;
  }
  .intg-err { font-size: 11px; color: #fca5a5; margin-top: 8px; }
  .intg-list { display: flex; flex-direction: column; gap: 6px; margin-bottom: 4px; }
  .intg-row {
    display: flex; align-items: center; gap: 10px;
    padding: 9px 12px; border-radius: 10px;
    background: rgba(0,0,0,0.22); border: 1px solid var(--border);
  }
  .intg-row-err { border-color: rgba(248,113,113,0.25); }
  .intg-del { color: #fca5a5 !important; border-color: rgba(248,113,113,0.4) !important; }
  .warn-hint { color: #fbbf24 !important; }
</style>
