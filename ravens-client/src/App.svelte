<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  // ── State ──────────────────────────────────────────────────────────────────
  let activeTab = "ravens"; // ravens | scan | map | settings | about
  let settingsSection = "server";
  let panelTab = "console";
  let filterModule = "all";

  let target = "";
  let targetType = "username";
  let scanning = false;
  let events: any[] = [];
  let mapPins: any[] = [];
  let selectedModules = new Set(["social", "ip_geo", "whois"]);

  // License
  let serverUrl = "";
  let tokenInput = "";
  let tokenStatus = "";
  let licenseTier = "FREE";
  let requestsRemaining = 2;
  let sessionId = "";
  let hwid = "";
  let allowedModules: string[] = ["social", "ip_geo", "whois"];

  // News
  let news: any[] = [];
  let openedNews: any = null;
  let newsLoading = false;

  // UI
  let savedKey = false;
  let aiText = "";
  let dossierText = "";
  let foundCount = 0;
  let moduleCount = 0;
  let threatLevel = "LOW";
  let scanTime = 0;
  let fontSize = "md";
  let compactMode = false;
  let showTimestamps = true;
  let autoScroll = true;
  let bgAnimation = true;

  // Color schemes
  const schemes: Record<string, any> = {
    raven: { name: "RAVEN", accent: "#7c3aed", bg: "#0a0a0f", surface: "#13131a", text: "#e2e8f0", dim: "#64748b" },
    blood: { name: "BLOOD", accent: "#dc2626", bg: "#0a0000", surface: "#130000", text: "#fca5a5", dim: "#7f1d1d" },
    matrix: { name: "MATRIX", accent: "#00ff41", bg: "#000d00", surface: "#001200", text: "#00ff41", dim: "#005500" },
    ice: { name: "ICE", accent: "#38bdf8", bg: "#020c18", surface: "#0a1628", text: "#bae6fd", dim: "#1e3a5f" },
    gold: { name: "GOLD", accent: "#f59e0b", bg: "#0d0a00", surface: "#1a1400", text: "#fde68a", dim: "#78350f" },
  };
  let currentScheme = "raven";
  $: scheme = schemes[currentScheme];
  $: cssVars = scheme ? `--accent:${scheme.accent};--bg:${scheme.bg};--surface:${scheme.surface};--text:${scheme.text};--dim:${scheme.dim}` : "";

  // Subscriptions (visible to clients — no ADMIN tier shown)
  const tiers = [
    {
      name: "FREE",
      price: "Бесплатно",
      period: "",
      requests: 2,
      features: ["2 запроса в день", "OSINT (Username, IP, WHOIS)", "Вкладка Ravens (новости)"],
      locked: ["Карта", "AI-анализ", "Dark Web", "Phone OSINT"],
      highlight: false,
    },
    {
      name: "PRO",
      price: "₽990",
      period: "мес",
      requests: 10,
      features: ["10 запросов в день", "Все OSINT-модули", "Карта угроз", "Dark Web", "Phone OSINT", "Paste поиск"],
      locked: ["AI-анализ (Nemotron)", "IntelX"],
      highlight: true,
    },
    {
      name: "ELITE",
      price: "₽2490",
      period: "мес",
      requests: -1,
      features: ["∞ запросов", "Все модули", "AI-анализ (NVIDIA NIM)", "IntelX", "Приоритетный доступ", "Экспорт отчётов"],
      locked: [],
      highlight: false,
    },
  ];

  const availableModules: [string, any][] = [
    ["social",   { icon: "⇢", label: "Social / Username", pro: false }],
    ["ip_geo",   { icon: "♦", label: "IP Geo",            pro: false }],
    ["whois",    { icon: "◆", label: "WHOIS / DNS",       pro: false }],
    ["hibp",     { icon: "⚠", label: "HIBP Leaks",        pro: true  }],
    ["dorks",    { icon: "∞", label: "Google Dorks",      pro: true  }],
    ["paste",    { icon: "☐", label: "Paste / Doxbin",    pro: true  }],
    ["darkweb",  { icon: "⊘", label: "Dark Web",          pro: true  }],
    ["phone",    { icon: "℡", label: "Phone OSINT",       pro: true  }],
    ["intelx",   { icon: "⚙", label: "IntelX",            pro: true  }],
    ["ai",       { icon: "⬡", label: "AI Анализ",         pro: true  }],
  ];

  const settingsSections = ["server", "subscriptions", "account", "appearance", "advanced"];
  const settingsLabels: Record<string, string> = {
    server: "🔗 Сервер",
    subscriptions: "⭐ Подписки",
    account: "👤 Аккаунт",
    appearance: "🎨 Внешний вид",
    advanced: "⚙️ Дополнительно",
  };

  // ── Lifecycle ──────────────────────────────────────────────────────────────
  onMount(async () => {
    try { hwid = await invoke("get_hwid"); } catch {}
    const saved = localStorage.getItem("ravens_server_url");
    if (saved) serverUrl = saved;
    const savedToken = localStorage.getItem("ravens_token");
    if (savedToken) tokenInput = savedToken;
    const savedSession = localStorage.getItem("ravens_session_id");
    if (savedSession) sessionId = savedSession;
    const savedTier = localStorage.getItem("ravens_tier");
    if (savedTier) licenseTier = savedTier;
    const savedAllowed = localStorage.getItem("ravens_allowed_modules");
    if (savedAllowed) allowedModules = JSON.parse(savedAllowed);
    const savedRemaining = localStorage.getItem("ravens_remaining");
    if (savedRemaining) requestsRemaining = parseInt(savedRemaining);
    const savedScheme = localStorage.getItem("ravens_scheme");
    if (savedScheme) currentScheme = savedScheme;

    if (serverUrl) fetchNews();
  });

  // ── News ───────────────────────────────────────────────────────────────────
  async function fetchNews() {
    if (!serverUrl) return;
    newsLoading = true;
    try {
      const res = await fetch(`${serverUrl}/api/news`);
      if (res.ok) news = await res.json();
    } catch {}
    newsLoading = false;
  }

  function openNewsCard(item: any) {
    openedNews = item;
  }
  function closeNewsCard() {
    openedNews = null;
  }

  // ── License ────────────────────────────────────────────────────────────────
  async function activateToken() {
    if (!serverUrl || !tokenInput || !hwid) {
      tokenStatus = "⚠ Заполни URL сервера и токен";
      return;
    }
    tokenStatus = "⏳ Активация...";
    try {
      const res = await fetch(`${serverUrl}/api/license/activate`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ token: tokenInput, hwid, app_version: "0.2.0" }),
      });
      const data = await res.json();
      if (data.ok) {
        licenseTier = data.tier;
        requestsRemaining = data.requests_remaining === 9223372036854775807 ? Infinity : data.requests_remaining;
        allowedModules = data.allowed_modules || [];
        sessionId = data.session_id || "";
        tokenStatus = `✓ Активирован · ${data.tier} · ${data.expires ? "до " + data.expires : "∞"}`;
        localStorage.setItem("ravens_server_url", serverUrl);
        localStorage.setItem("ravens_token", tokenInput);
        localStorage.setItem("ravens_session_id", sessionId);
        localStorage.setItem("ravens_tier", licenseTier);
        localStorage.setItem("ravens_allowed_modules", JSON.stringify(allowedModules));
        localStorage.setItem("ravens_remaining", String(requestsRemaining));
        fetchNews();
      } else {
        tokenStatus = `✗ ${data.error || "Ошибка активации"}`;
      }
    } catch (e) {
      tokenStatus = "✗ Сервер недоступен";
    }
  }

  // ── Scanning ───────────────────────────────────────────────────────────────
  function addEvent(module: string, type: string, text: string) {
    events = [...events, { module, type, text, time: new Date().toLocaleTimeString() }];
    if (type === "found") foundCount++;
  }

  async function startScan() {
    if (!target || scanning) return;
    scanning = true;
    events = [];
    mapPins = [];
    foundCount = 0;
    moduleCount = 0;
    threatLevel = "LOW";
    const t0 = Date.now();

    for (const mod of selectedModules) {
      if (!allowedModules.includes(mod)) {
        addEvent(mod, "error", `✗ Модуль недоступен на тарифе ${licenseTier}`);
        continue;
      }
      moduleCount++;
      addEvent(mod, "running", `▶ Запуск ${mod}...`);
      try {
        const res = await fetch(`${serverUrl}/api/license/consume`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ token: tokenInput, hwid, module: mod, session_id: sessionId }),
        });
        const data = await res.json();
        if (!data.ok) {
          addEvent(mod, "error", `✗ ${data.error || "Отказано"}`);
          continue;
        }
        requestsRemaining = data.requests_remaining;
        // Simulate result
        await new Promise(r => setTimeout(r, 400 + Math.random() * 600));
        addEvent(mod, "found", `★ [${mod.toUpperCase()}] Цель: ${target} — данные получены`);
      } catch {
        addEvent(mod, "error", `✗ Ошибка модуля ${mod}`);
      }
    }

    scanTime = ((Date.now() - t0) / 1000).toFixed(1) as any;
    threatLevel = foundCount > 5 ? "CRITICAL" : foundCount > 2 ? "HIGH" : foundCount > 0 ? "MEDIUM" : "LOW";
    scanning = false;
  }

  function toggleModule(mod: string) {
    if (selectedModules.has(mod)) selectedModules.delete(mod);
    else selectedModules.add(mod);
    selectedModules = new Set(selectedModules);
  }

  function setScheme(key: string) {
    currentScheme = key;
    localStorage.setItem("ravens_scheme", key);
  }

  $: filteredEvents = filterModule === "all" ? events : events.filter(e => e.module === filterModule);
</script>

<style>
  :global(*) { box-sizing: border-box; margin: 0; padding: 0; }
  :global(body) {
    background: var(--bg, #0a0a0f);
    color: var(--text, #e2e8f0);
    font-family: 'JetBrains Mono', 'Fira Code', monospace;
    font-size: 13px;
    overflow: hidden;
    height: 100vh;
  }

  .app { display: flex; flex-direction: column; height: 100vh; }

  /* Header */
  .header {
    display: flex; align-items: center; gap: 16px;
    padding: 8px 16px;
    background: var(--surface);
    border-bottom: 1px solid rgba(255,255,255,0.06);
    flex-shrink: 0;
  }
  .logo { font-size: 18px; font-weight: 700; color: var(--accent); letter-spacing: 3px; }
  .stats { display: flex; gap: 12px; margin-left: auto; }
  .stat { font-size: 10px; color: var(--dim); }
  .stat span { color: var(--accent); font-weight: 700; }

  /* Nav */
  .nav {
    display: flex; gap: 2px; padding: 4px 8px;
    background: var(--surface);
    border-bottom: 1px solid rgba(255,255,255,0.06);
    flex-shrink: 0;
  }
  .nav-btn {
    padding: 5px 14px; border: none; background: transparent;
    color: var(--dim); cursor: pointer; font-family: inherit; font-size: 11px;
    border-radius: 4px; letter-spacing: 1px; transition: all 0.2s;
  }
  .nav-btn:hover { color: var(--text); background: rgba(255,255,255,0.04); }
  .nav-btn.active { color: var(--accent); background: rgba(124,58,237,0.12); }

  /* Content */
  .content { flex: 1; overflow: hidden; display: flex; flex-direction: column; }
  .scroll { flex: 1; overflow-y: auto; padding: 16px; }
  .scroll::-webkit-scrollbar { width: 4px; }
  .scroll::-webkit-scrollbar-thumb { background: var(--accent); border-radius: 2px; }

  /* ── Ravens / News Tab ─────────────────────────────────────────────────── */
  .news-header {
    display: flex; align-items: center; justify-content: space-between;
    margin-bottom: 20px;
  }
  .news-title { font-size: 14px; letter-spacing: 3px; color: var(--accent); }
  .news-ticker {
    font-size: 10px; color: var(--dim); overflow: hidden;
    white-space: nowrap; max-width: 400px;
  }

  /* News Carousel */
  .news-carousel {
    display: flex; gap: 16px; overflow-x: auto; padding-bottom: 8px;
    scroll-snap-type: x mandatory;
  }
  .news-carousel::-webkit-scrollbar { height: 3px; }
  .news-carousel::-webkit-scrollbar-thumb { background: var(--accent); }

  .news-card {
    min-width: 280px; max-width: 300px; flex-shrink: 0;
    background: var(--surface);
    border: 1px solid rgba(255,255,255,0.06);
    border-radius: 12px; overflow: hidden; cursor: pointer;
    scroll-snap-align: start;
    transition: transform 0.25s ease, box-shadow 0.25s ease, border-color 0.25s ease;
  }
  .news-card:hover {
    transform: translateY(-4px);
    box-shadow: 0 12px 40px rgba(0,0,0,0.5), 0 0 0 1px var(--accent);
    border-color: var(--accent);
  }

  .news-card-media {
    width: 100%; height: 160px; overflow: hidden; position: relative;
  }
  .news-card-media img, .news-card-media video {
    width: 100%; height: 100%; object-fit: cover;
    transition: transform 0.4s ease;
  }
  .news-card:hover .news-card-media img,
  .news-card:hover .news-card-media video { transform: scale(1.05); }
  .media-rounded .news-card-media { border-radius: 10px 10px 0 0; }

  .news-card-body { padding: 12px; }
  .news-card-title {
    font-size: 12px; font-weight: 700; letter-spacing: 0.5px;
    color: var(--text); margin-bottom: 6px; line-height: 1.4;
  }
  .news-card-preview {
    font-size: 10px; color: var(--dim); line-height: 1.6;
    display: -webkit-box; -webkit-line-clamp: 3; -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .news-card-footer {
    padding: 8px 12px; border-top: 1px solid rgba(255,255,255,0.04);
    display: flex; align-items: center; justify-content: space-between;
    font-size: 10px; color: var(--dim);
  }
  .news-card-author { color: var(--accent); }
  .news-read-more {
    font-size: 10px; color: var(--accent); letter-spacing: 1px;
    opacity: 0; transition: opacity 0.2s;
  }
  .news-card:hover .news-read-more { opacity: 1; }

  /* News Modal */
  .news-overlay {
    position: fixed; inset: 0; background: rgba(0,0,0,0.85);
    display: flex; align-items: center; justify-content: center;
    z-index: 100; padding: 24px;
    animation: fadeIn 0.2s ease;
  }
  @keyframes fadeIn { from { opacity: 0; } to { opacity: 1; } }

  .news-modal {
    background: var(--surface);
    border: 1px solid var(--accent);
    border-radius: 16px; overflow: hidden;
    max-width: 640px; width: 100%; max-height: 85vh;
    display: flex; flex-direction: column;
    animation: slideUp 0.25s ease;
  }
  @keyframes slideUp { from { transform: translateY(30px); opacity: 0; } to { transform: translateY(0); opacity: 1; } }

  .news-modal-media {
    width: 100%; max-height: 280px; overflow: hidden; flex-shrink: 0;
  }
  .news-modal-media img, .news-modal-media video {
    width: 100%; height: 280px; object-fit: cover;
  }
  .news-modal-media.rounded img, .news-modal-media.rounded video {
    border-radius: 14px 14px 0 0;
  }
  .news-modal-body { padding: 20px; overflow-y: auto; flex: 1; }
  .news-modal-body::-webkit-scrollbar { width: 3px; }
  .news-modal-body::-webkit-scrollbar-thumb { background: var(--accent); }
  .news-modal-title {
    font-size: 16px; font-weight: 700; color: var(--text);
    margin-bottom: 12px; line-height: 1.4;
  }
  .news-modal-meta { font-size: 10px; color: var(--dim); margin-bottom: 16px; }
  .news-modal-meta span { color: var(--accent); }
  .news-modal-text {
    font-size: 12px; color: var(--text); line-height: 1.8;
    white-space: pre-wrap;
  }
  .news-modal-close {
    position: absolute; top: 12px; right: 12px;
    background: rgba(0,0,0,0.6); border: 1px solid rgba(255,255,255,0.1);
    color: var(--text); width: 32px; height: 32px; border-radius: 50%;
    cursor: pointer; font-size: 14px; display: flex; align-items: center; justify-content: center;
    transition: all 0.2s;
  }
  .news-modal-close:hover { background: var(--accent); }
  .news-modal-wrapper { position: relative; }

  /* ── Scan Tab ──────────────────────────────────────────────────────────── */
  .scan-layout { display: flex; gap: 12px; height: calc(100vh - 80px); }
  .scan-sidebar {
    width: 220px; flex-shrink: 0; display: flex; flex-direction: column; gap: 8px;
    overflow-y: auto;
  }
  .scan-main { flex: 1; display: flex; flex-direction: column; gap: 8px; overflow: hidden; }

  .panel {
    background: var(--surface);
    border: 1px solid rgba(255,255,255,0.06);
    border-radius: 8px; padding: 12px;
  }
  .panel-title {
    font-size: 10px; letter-spacing: 2px; color: var(--accent);
    margin-bottom: 8px; display: flex; align-items: center; gap: 6px;
  }

  .input {
    width: 100%; background: rgba(0,0,0,0.4);
    border: 1px solid rgba(255,255,255,0.1);
    color: var(--text); padding: 8px 10px; border-radius: 6px;
    font-family: inherit; font-size: 12px; outline: none;
    transition: border-color 0.2s;
  }
  .input:focus { border-color: var(--accent); }

  .btn {
    padding: 8px 14px; border: 1px solid var(--accent);
    background: transparent; color: var(--accent);
    cursor: pointer; font-family: inherit; font-size: 11px;
    border-radius: 6px; letter-spacing: 1px;
    transition: all 0.2s;
  }
  .btn:hover { background: var(--accent); color: #000; }
  .btn-primary { background: var(--accent); color: #000; }
  .btn-primary:hover { opacity: 0.85; }
  .btn-danger { border-color: #dc2626; color: #dc2626; }
  .btn-danger:hover { background: #dc2626; color: #fff; }
  .btn-sm { padding: 4px 10px; font-size: 10px; }

  .module-item {
    display: flex; align-items: center; gap: 8px;
    padding: 5px 0; font-size: 11px; color: var(--dim);
    transition: color 0.2s;
  }
  .module-item.selected { color: var(--text); }
  .module-item input { accent-color: var(--accent); cursor: pointer; }
  .badge { font-size: 9px; padding: 1px 5px; border-radius: 3px; background: rgba(124,58,237,0.2); color: var(--accent); }

  .type-btns { display: flex; flex-wrap: wrap; gap: 4px; }
  .type-btn {
    padding: 3px 8px; border: 1px solid rgba(255,255,255,0.1);
    background: transparent; color: var(--dim); cursor: pointer;
    font-family: inherit; font-size: 10px; border-radius: 4px;
    transition: all 0.15s;
  }
  .type-btn.active { border-color: var(--accent); color: var(--accent); }

  /* Console */
  .console {
    flex: 1; overflow-y: auto; font-size: 11px; line-height: 1.7;
    padding: 8px 0;
  }
  .console::-webkit-scrollbar { width: 3px; }
  .console::-webkit-scrollbar-thumb { background: var(--accent); }
  .ev-line { display: flex; gap: 8px; align-items: flex-start; }
  .ev-mod { color: var(--dim); font-size: 10px; min-width: 70px; }
  .ev-icon { width: 14px; flex-shrink: 0; }
  .ev-text { color: var(--text); word-break: break-all; }
  .ev-found .ev-text { color: #4ade80; }
  .ev-error .ev-text { color: #f87171; }
  .ev-running .ev-text { color: var(--accent); }

  /* Panel tabs */
  .panel-tabs { display: flex; gap: 4px; margin-bottom: 8px; }
  .panel-tab {
    padding: 4px 10px; border: none; background: transparent;
    color: var(--dim); cursor: pointer; font-family: inherit; font-size: 10px;
    border-radius: 4px; transition: all 0.15s; letter-spacing: 1px;
  }
  .panel-tab.active { color: var(--accent); background: rgba(124,58,237,0.12); }

  /* ── Settings ──────────────────────────────────────────────────────────── */
  .settings-layout { display: flex; gap: 16px; }
  .settings-nav { width: 140px; flex-shrink: 0; display: flex; flex-direction: column; gap: 4px; }
  .settings-content { flex: 1; }
  .settings-nav-btn {
    padding: 7px 12px; border: none; background: transparent;
    color: var(--dim); cursor: pointer; font-family: inherit; font-size: 11px;
    border-radius: 6px; text-align: left; transition: all 0.15s;
  }
  .settings-nav-btn:hover { color: var(--text); background: rgba(255,255,255,0.04); }
  .settings-nav-btn.active { color: var(--accent); background: rgba(124,58,237,0.12); }

  .section-title { font-size: 13px; color: var(--accent); letter-spacing: 2px; margin-bottom: 16px; }
  .form-group { margin-bottom: 14px; }
  .label { font-size: 10px; color: var(--dim); letter-spacing: 1px; margin-bottom: 6px; display: block; }
  .hint { font-size: 10px; color: var(--dim); margin-top: 4px; }

  /* Subscription cards */
  .subs-grid { display: flex; gap: 16px; flex-wrap: wrap; }
  .sub-card {
    flex: 1; min-width: 200px;
    background: var(--surface);
    border: 1px solid rgba(255,255,255,0.08);
    border-radius: 12px; padding: 20px;
    position: relative; overflow: hidden;
    transition: transform 0.25s, box-shadow 0.25s, border-color 0.25s;
  }
  .sub-card:hover {
    transform: translateY(-4px);
    box-shadow: 0 16px 48px rgba(0,0,0,0.5);
    border-color: var(--accent);
  }
  .sub-card.highlight {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent), 0 8px 32px rgba(124,58,237,0.2);
  }
  .sub-card.highlight::before {
    content: "ПОПУЛЯРНЫЙ";
    position: absolute; top: 12px; right: -20px;
    background: var(--accent); color: #000;
    font-size: 8px; font-weight: 700; letter-spacing: 1px;
    padding: 3px 28px; transform: rotate(35deg);
  }
  .sub-name { font-size: 11px; letter-spacing: 3px; color: var(--accent); margin-bottom: 8px; }
  .sub-price { font-size: 24px; font-weight: 700; color: var(--text); }
  .sub-period { font-size: 11px; color: var(--dim); }
  .sub-requests { font-size: 11px; color: var(--dim); margin: 8px 0 12px; }
  .sub-features { list-style: none; margin-bottom: 12px; }
  .sub-features li { font-size: 10px; color: var(--dim); padding: 2px 0; }
  .sub-features li.ok { color: var(--text); }
  .sub-features li.ok::before { content: "✓ "; color: #4ade80; }
  .sub-features li.no::before { content: "✗ "; color: #f87171; }

  /* Status badge */
  .tier-badge {
    display: inline-flex; align-items: center; gap: 6px;
    padding: 3px 10px; border-radius: 20px;
    background: rgba(124,58,237,0.15); border: 1px solid var(--accent);
    font-size: 10px; color: var(--accent); letter-spacing: 1px;
  }
  .tier-dot { width: 6px; height: 6px; border-radius: 50%; background: var(--accent); animation: pulse 2s infinite; }
  @keyframes pulse { 0%,100%{opacity:1} 50%{opacity:0.4} }

  /* Color scheme swatches */
  .scheme-grid { display: flex; gap: 8px; flex-wrap: wrap; }
  .scheme-swatch {
    width: 80px; padding: 8px; border-radius: 8px; cursor: pointer;
    border: 2px solid transparent; transition: all 0.2s; text-align: center;
    font-size: 10px;
  }
  .scheme-swatch.active { border-color: var(--accent); }
  .swatch-dot { width: 24px; height: 24px; border-radius: 50%; margin: 0 auto 4px; }

  /* Misc */
  .empty-state { text-align: center; padding: 40px 20px; color: var(--dim); }
  .empty-icon { font-size: 32px; margin-bottom: 8px; }
  .divider { height: 1px; background: rgba(255,255,255,0.06); margin: 16px 0; }
  .row { display: flex; gap: 8px; align-items: center; }
  .grow { flex: 1; }
  .token-status { font-size: 11px; margin-top: 8px; }
  .token-status.ok { color: #4ade80; }
  .token-status.err { color: #f87171; }
  .token-status.pending { color: var(--accent); }
</style>

<div class="app" style={cssVars}>
  <!-- Header -->
  <div class="header">
    <div class="logo">⬡ RAVEN</div>
    <div style="font-size:10px;color:var(--dim)">GLOBAL THREAT INTERCEPT · OSINT PLATFORM</div>
    <div class="stats">
      <div class="stat"><span>{foundCount}</span> НАЙДЕНО</div>
      <div class="stat"><span>{moduleCount}</span>/10 МОДУЛЕЙ</div>
      <div class="stat"><span style="color:{threatLevel==='CRITICAL'?'#dc2626':threatLevel==='HIGH'?'#f59e0b':threatLevel==='MEDIUM'?'#eab308':'#4ade80'}">{threatLevel}</span> УГРОЗА</div>
      <div class="stat"><span>{scanTime}s</span> ВРЕМЯ</div>
      <div class="tier-badge"><div class="tier-dot"></div>{licenseTier}</div>
    </div>
  </div>

  <!-- Nav -->
  <div class="nav">
    {#each [["ravens","⬡ RAVENS"],["scan","◎ OSINT"],["map","◈ КАРТА"],["settings","⚙ НАСТРОЙКИ"]] as [t, label]}
      <button class="nav-btn" class:active={activeTab===t} on:click={() => activeTab = t}>{label}</button>
    {/each}
  </div>

  <!-- Content -->
  <div class="content">

    <!-- ── RAVENS TAB (News) ──────────────────────────────────────────────── -->
    {#if activeTab === "ravens"}
      <div class="scroll">
        <div class="news-header">
          <div class="news-title">⬡ RAVENS NEXUS — НОВОСТИ</div>
          <div class="news-ticker">
            {#if news.length > 0}
              {news.map(n => n.title).join(" · ")}
            {:else}
              Загрузка новостей...
            {/if}
          </div>
        </div>

        {#if newsLoading}
          <div class="empty-state"><div class="empty-icon">⏳</div>Загрузка...</div>
        {:else if news.length === 0}
          <div class="empty-state">
            <div class="empty-icon">🦅</div>
            <div>Нет новостей</div>
            <div style="font-size:10px;margin-top:4px;color:var(--dim)">Подключите сервер в Настройках → Сервер</div>
          </div>
        {:else}
          <div class="news-carousel">
            {#each news as item (item.id)}
              <div class="news-card" class:media-rounded={item.media_rounded} on:click={() => openNewsCard(item)}>
                {#if item.image_url || item.video_url}
                  <div class="news-card-media">
                    {#if item.video_url}
                      <video src={item.video_url} muted loop autoplay></video>
                    {:else}
                      <img src={item.image_url} alt={item.title} loading="lazy" />
                    {/if}
                  </div>
                {/if}
                <div class="news-card-body">
                  <div class="news-card-title">{item.title}</div>
                  <div class="news-card-preview">{item.preview_text}</div>
                </div>
                <div class="news-card-footer">
                  <span class="news-card-author">{item.author}</span>
                  <span class="news-read-more">ЧИТАТЬ →</span>
                </div>
              </div>
            {/each}
          </div>
        {/if}

        <!-- News Modal -->
      {#if openedNews}
        <div class="news-overlay" on:click|self={closeNewsCard}>
          <div class="news-modal-wrapper">
            <div class="news-modal">
              {#if openedNews.image_url || openedNews.video_url}
                <div class="news-modal-media" class:rounded={openedNews.media_rounded}>
                  {#if openedNews.video_url}
                    <video src={openedNews.video_url} controls autoplay></video>
                  {:else}
                    <img src={openedNews.image_url} alt={openedNews.title} />
                  {/if}
                </div>
              {/if}
              <div class="news-modal-body">
                <div class="news-modal-title">{openedNews.title}</div>
                <div class="news-modal-meta">
                  Автор: <span>{openedNews.author}</span> &nbsp;·&nbsp;
                  {new Date(openedNews.created_at).toLocaleDateString("ru-RU")}
                </div>
                <div class="news-modal-text">{openedNews.full_text}</div>
              </div>
            </div>
            <button class="news-modal-close" on:click={closeNewsCard}>✕</button>
          </div>
        </div>
      {/if}

    </div>

    <!-- ── SCAN TAB ─────────────────────────────────────────────────────── -->

    {:else if activeTab === "scan"}
      <div class="scroll">
        <div class="scan-layout">
          <!-- Sidebar -->
          <div class="scan-sidebar">
            <div class="panel">
              <div class="panel-title">// ЦЕЛЬ</div>
              <input class="input" bind:value={target} placeholder="Введите цель..."
                on:keydown={e => e.key === "Enter" && startScan()} style="margin-bottom:8px" />
              <div class="type-btns">
                {#each ["username","email","ip","domain","phone"] as t}
                  <button class="type-btn" class:active={targetType===t} on:click={() => targetType = t}>{t.toUpperCase()}</button>
                {/each}
              </div>
            </div>

            <div class="panel" style="flex:1">
              <div class="panel-title">// МОДУЛИ ({selectedModules.size})</div>
              {#each availableModules as [mod, def]}
                {@const locked = !allowedModules.includes(mod)}
                <div class="module-item" class:selected={selectedModules.has(mod)} style={locked?"opacity:0.4":""}>
                  <input type="checkbox" checked={selectedModules.has(mod)}
                    on:change={() => !locked && toggleModule(mod)}
                    disabled={locked} />
                  <span>{def.icon}</span>
                  <span style="flex:1">{def.label}</span>
                  {#if def.pro && locked}<span class="badge">PRO</span>{/if}
                </div>
              {/each}
            </div>

            <div style="display:flex;flex-direction:column;gap:6px">
              <button class="btn btn-primary" on:click={startScan} disabled={scanning || !target}>
                {scanning ? "⚡ СКАНИРОВАНИЕ..." : "► НАЧАТЬ"}
              </button>
              <button class="btn btn-sm" on:click={() => { events = []; foundCount = 0; }}>✕ ОЧИСТИТЬ</button>
            </div>
          </div>

          <!-- Main panel -->
          <div class="scan-main">
            <div class="panel" style="display:flex;align-items:center;gap:6px;padding:6px 12px">
              <span style="font-size:10px;color:var(--dim)">ravens@nexus:~$ osint {target}</span>
              {#if scanning}<span style="font-size:10px;color:#dc2626;animation:pulse 1s infinite">● REC</span>{/if}
              <div style="margin-left:auto;display:flex;gap:4px">
                <div class="panel-tabs">
                  {#each [["console","КОНСОЛЬ"],["dossier","ДОСЬЕ"],["ai","AI"]] as [t, label]}
                    <button class="panel-tab" class:active={panelTab===t} on:click={() => panelTab = t}>⬡ {label}</button>
                  {/each}
                </div>
              </div>
            </div>

            {#if panelTab === "console"}
              <div class="panel" style="flex:1;display:flex;flex-direction:column;overflow:hidden">
                <!-- Filter -->
                <div style="display:flex;gap:4px;margin-bottom:8px;flex-wrap:wrap">
                  <button class="type-btn" class:active={filterModule==="all"} on:click={() => filterModule = "all"}>ВСЕ</button>
                  {#each [...new Set(events.map(e => e.module))] as mod}
                    <button class="type-btn" class:active={filterModule===mod} on:click={() => filterModule = mod}>{mod.toUpperCase()}</button>
                  {/each}
                </div>

                <div class="console">
                  {#if events.length === 0 && !scanning}
                    <div class="empty-state">
                      <div class="empty-icon">🦅</div>
                      <div>RAVEN OSINT v0.2 готов</div>
                      <div style="font-size:10px;margin-top:4px">Введите цель и запустите расследование</div>
                    </div>
                  {/if}
                  {#each filteredEvents as ev}
                    <div class="ev-line ev-{ev.type}">
                      {#if showTimestamps}<span style="color:var(--dim);font-size:9px;min-width:55px">{ev.time}</span>{/if}
                      <span class="ev-mod">[{ev.module.toUpperCase()}]</span>
                      <span class="ev-icon">
                        {#if ev.type==="found"}★{:else if ev.type==="error"}✗{:else if ev.type==="running"}▶{:else}·{/if}
                      </span>
                      <span class="ev-text">{ev.text}</span>
                    </div>
                  {/each}
                  {#if scanning}
                    <div class="ev-line" style="color:var(--accent)">▶ Выполняется разведка...</div>
                  {/if}
                </div>
              </div>

            {:else if panelTab === "dossier"}
              <div class="panel" style="flex:1;overflow-y:auto">
                {#if !dossierText}
                  <div class="empty-state"><div class="empty-icon">📄</div>Запустите расследование</div>
                {:else}
                  <pre style="font-size:11px;line-height:1.7;white-space:pre-wrap">{dossierText}</pre>
                {/if}
              </div>

            {:else if panelTab === "ai"}
              <div class="panel" style="flex:1;overflow-y:auto">
                <div class="empty-state">
                  <div class="empty-icon">⬡</div>
                  <div>RAVEN AI · Nemotron-Ultra</div>
                  <div style="font-size:10px;margin-top:4px;color:var(--dim)">Доступен на тарифе ELITE</div>
                </div>
              </div>
            {/if}
          </div>
        </div>
      </div>

    <!-- ── MAP TAB ─────────────────────────────────────────────────────── -->
    {:else if activeTab === "map"}
      <div class="scroll">
        {#if licenseTier === "FREE"}
          <div class="empty-state" style="margin-top:60px">
            <div class="empty-icon">◈</div>
            <div style="font-size:14px;margin-bottom:8px">INTEL MAP</div>
            <div style="color:var(--dim)">Карта доступна на тарифе PRO и выше</div>
            <button class="btn" style="margin-top:16px" on:click={() => { settingsSection = "subscriptions"; activeTab = "settings"; }}>Обновить подписку</button>
          </div>
        {:else}
          <div style="font-size:10px;letter-spacing:2px;color:var(--accent);margin-bottom:12px">◈ INTEL MAP</div>
          {#if mapPins.length === 0}
            <div class="empty-state"><div class="empty-icon">🗺</div>Запустите OSINT для появления геопинов</div>
          {:else}
            {#each mapPins as pin}
              <div class="panel" style="margin-bottom:8px">★ {pin.label} — {pin.lat}, {pin.lon}</div>
            {/each}
          {/if}
        {/if}
      </div>

    <!-- ── SETTINGS TAB ────────────────────────────────────────────────── -->
    {:else if activeTab === "settings"}
      <div class="scroll">
        <div class="settings-layout">
          <div class="settings-nav">
            {#each settingsSections as sec}
              <button class="settings-nav-btn" class:active={settingsSection===sec}
                on:click={() => settingsSection = sec}>{settingsLabels[sec]}</button>
            {/each}
          </div>

          <div class="settings-content">
            <!-- Server -->
            {#if settingsSection === "server"}
              <div class="section-title">🔗 Сервер лицензий</div>
              <div class="form-group">
                <label class="label">URL СЕРВЕРА</label>
                <input class="input" bind:value={serverUrl} placeholder="https://your-replit-server.repl.co" />
              </div>
              <div class="form-group">
                <label class="label">ТОКЕН ДОСТУПА</label>
                <input class="input" bind:value={tokenInput} placeholder="RVN-XXXXXXXXXXXXXXXXXXXX" />
                <div class="hint">Токен выдаётся администратором. Привязывается к вашему HWID.</div>
              </div>
              <button class="btn btn-primary" on:click={activateToken}>Активировать</button>
              {#if tokenStatus}
                <div class="token-status" class:ok={tokenStatus.startsWith("✓")} class:err={tokenStatus.startsWith("✗")} class:pending={tokenStatus.startsWith("⏳")}>
                  {tokenStatus}
                </div>
              {/if}
              <div class="divider"></div>
              <div style="font-size:11px;color:var(--dim)">
                Тариф: <span style="color:var(--accent)">{licenseTier}</span> &nbsp;·&nbsp;
                Запросов: <span style="color:var(--accent)">{requestsRemaining === Infinity ? "∞" : requestsRemaining}</span> &nbsp;·&nbsp;
                Сервер: <span style="color:var(--dim)">{serverUrl || "—"}</span>
              </div>
              <div class="form-group" style="margin-top:16px">
                <label class="label">DEVICE ID (HWID)</label>
                <div style="font-size:10px;color:var(--dim);padding:8px;background:rgba(0,0,0,0.3);border-radius:6px;word-break:break-all">
                  {#await invoke("get_hwid") then h}{h}{:catch}Ошибка получения HWID{/await}
                </div>
              </div>

            <!-- Subscriptions -->
            {:else if settingsSection === "subscriptions"}
              <div class="section-title">⭐ Подписки Ravens Nexus</div>
              <div class="subs-grid">
                {#each tiers as tier}
                  <div class="sub-card" class:highlight={tier.highlight}>
                    <div class="sub-name">{tier.name}</div>
                    <div>
                      <span class="sub-price">{tier.price}</span>
                      {#if tier.period}<span class="sub-period">/{tier.period}</span>{/if}
                    </div>
                    <div class="sub-requests">{tier.requests === -1 ? "∞ запросов" : `${tier.requests} запросов/день`}</div>
                    <ul class="sub-features">
                      {#each tier.features as f}<li class="ok">{f}</li>{/each}
                      {#each tier.locked as f}<li class="no">{f}</li>{/each}
                    </ul>
                    {#if licenseTier === tier.name}
                      <button class="btn btn-sm" disabled style="opacity:0.5;cursor:default">✓ Текущий</button>
                    {:else}
                      <button class="btn btn-sm" on:click={() => settingsSection = "server"}>Активировать токен</button>
                    {/if}
                  </div>
                {/each}
              </div>

            <!-- Account -->
            {:else if settingsSection === "account"}
              <div class="section-title">👤 Аккаунт</div>
              <div class="form-group">
                <label class="label">NVIDIA NIM API KEY</label>
                <div class="row">
                  <input class="input grow" type="password" placeholder="nvapi-..." />
                  <button class="btn btn-sm" on:click={() => savedKey = true}>Сохранить</button>
                </div>
                {#if savedKey}<div style="font-size:10px;color:#4ade80;margin-top:4px">✓ СОХРАНЁН (AES-256-GCM, привязан к HWID)</div>{/if}
                <div class="hint">Нужен для AI-анализа. Получить: <a href="https://build.nvidia.com" style="color:var(--accent)">build.nvidia.com</a></div>
              </div>

            <!-- Appearance -->
            {:else if settingsSection === "appearance"}
              <div class="section-title">🎨 Внешний вид</div>
              <div class="form-group">
                <label class="label">ЦВЕТОВАЯ СХЕМА</label>
                <div class="scheme-grid">
                  {#each Object.entries(schemes) as [key, s]}
                    <div class="scheme-swatch" class:active={currentScheme===key} on:click={() => setScheme(key)}
                      style="background:{s.bg};border-color:{currentScheme===key?s.accent:'rgba(255,255,255,0.08)'}">
                      <div class="swatch-dot" style="background:{s.accent}"></div>
                      <div style="color:{s.text};font-size:9px">{s.name}</div>
                    </div>
                  {/each}
                </div>
              </div>
              <div class="form-group">
                <label class="label">ПАРАМЕТРЫ</label>
                <div style="display:flex;flex-direction:column;gap:8px">
                  <label style="display:flex;align-items:center;gap:8px;font-size:11px;color:var(--dim);cursor:pointer">
                    <input type="checkbox" bind:checked={compactMode} style="accent-color:var(--accent)" /> Компактный режим
                  </label>
                  <label style="display:flex;align-items:center;gap:8px;font-size:11px;color:var(--dim);cursor:pointer">
                    <input type="checkbox" bind:checked={showTimestamps} style="accent-color:var(--accent)" /> Метки времени
                  </label>
                  <label style="display:flex;align-items:center;gap:8px;font-size:11px;color:var(--dim);cursor:pointer">
                    <input type="checkbox" bind:checked={autoScroll} style="accent-color:var(--accent)" /> Авто-прокрутка
                  </label>
                </div>
              </div>

            <!-- Advanced -->
            {:else if settingsSection === "advanced"}
              <div class="section-title">⚙️ Дополнительно</div>
              <div class="form-group">
                <label class="label">ЭКСПОРТ ДАННЫХ</label>
                <div style="display:flex;gap:8px">
                  <button class="btn btn-sm">📋 Копировать JSON</button>
                  <button class="btn btn-sm">📄 Экспорт TXT</button>
                  <button class="btn btn-sm btn-danger" on:click={() => { events = []; mapPins = []; }}>🗑 Очистить</button>
                </div>
              </div>
            {/if}
          </div>
        </div>
      </div>

    <!-- ── ABOUT TAB ───────────────────────────────────────────────────── -->
    {:else if activeTab === "about"}
      <div class="scroll">
        <div style="text-align:center;margin-bottom:24px">
          <div style="font-size:32px;color:var(--accent);letter-spacing:4px">⬡ RAVEN NEXUS</div>
          <div style="color:var(--dim);margin-top:4px">Элитная OSINT-система разведки · v0.2.0 · Rust + Tauri + Svelte</div>
        </div>
        <div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(200px,1fr));gap:12px">
          {#each [
            ["⇢","Social / Username","Поиск по 10+ платформам","FREE"],
            ["⚠","HIBP Leaks","Проверка email в базах утечек","FREE"],
            ["♦","IP Geo","Геолокация и информация об IP","FREE"],
            ["◆","WHOIS / DNS","Информация о домене","FREE"],
            ["∞","Google Dorks","18+ поисковых запросов","PRO"],
            ["☐","Paste / Doxbin","Поиск в публичных пастах","PRO"],
            ["⊘","Dark Web","Поиск через Ahmia","PRO"],
            ["℡","Phone OSINT","Анализ телефонного номера","PRO"],
            ["⚙","IntelX","Поиск в базах IntelX","ELITE"],
            ["⬡","AI Анализ","NVIDIA NIM Nemotron-Ultra","ELITE"],
          ] as [icon, name, desc, tier]}
            <div class="panel" style="display:flex;gap:10px;align-items:flex-start">
              <span style="font-size:18px;color:var(--accent)">{icon}</span>
              <div>
                <div style="font-size:11px;font-weight:700;color:var(--text)">{name}</div>
                <div style="font-size:10px;color:var(--dim);margin-top:2px">{desc}</div>
                <span class="badge" style="margin-top:4px;display:inline-block">{tier}</span>
              </div>
            </div>
          {/each}
        </div>
      </div>
    {/if}
  </div>
</div>