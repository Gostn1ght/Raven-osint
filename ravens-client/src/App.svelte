<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount, onDestroy } from "svelte";

  // ── Types ────────────────────────────────────────────────────────────────────
  type Tab = "scan" | "map" | "settings" | "about";
  type PanelTab = "console" | "dossier" | "ravendb" | "ai";
  type TargetType = "username" | "email" | "ip" | "domain" | "phone";
  type ColorScheme = "crimson" | "matrix" | "arctic" | "gold" | "violet";

  interface OsintEvent {
    module: string;
    type: string;
    text: string;
  }

  interface MapPin {
    lat: number;
    lon: number;
    label: string;
    target: string;
  }

  // ── State ────────────────────────────────────────────────────────────────────
  let activeTab: Tab = "scan";
  let panelTab: PanelTab = "console";
  let target = "";
  let targetType: TargetType = "username";
  let scanning = false;
  let events: OsintEvent[] = [];
  let aiText = "";
  let aiInput = "";
  let nvidiaKey = "";
  let savedKey = false;
  let filterModule = "all";
  let mapPins: MapPin[] = [];
  let mapInstance: any = null;
  let mapEl: HTMLElement;
  let dossierText = "";
  let ravenDbUrls: {url: string; type: string}[] = [];
  let newDbUrl = "";
  let newDbType = "breach";
  let colorScheme: ColorScheme = "crimson";
  let scanTime = 0;
  let scanTimer: any = null;
  let ambientMode = true;
  let glitchActive = false;
  let particleCanvas: HTMLCanvasElement;
  let particleCtx: CanvasRenderingContext2D | null = null;
  let animFrame: number;

  // ── Color Schemes ─────────────────────────────────────────────────────────────
  const schemes: Record<ColorScheme, {accent: string; accent2: string; glow: string; name: string}> = {
    crimson: { accent: "#c0392b", accent2: "#e74c3c", glow: "rgba(192,57,43,0.4)", name: "Crimson" },
    matrix:  { accent: "#00ff41", accent2: "#39ff14", glow: "rgba(0,255,65,0.3)",  name: "Matrix" },
    arctic:  { accent: "#00b4d8", accent2: "#0077b6", glow: "rgba(0,180,216,0.35)", name: "Arctic" },
    gold:    { accent: "#f59e0b", accent2: "#fbbf24", glow: "rgba(245,158,11,0.35)", name: "Gold" },
    violet:  { accent: "#8b5cf6", accent2: "#a78bfa", glow: "rgba(139,92,246,0.35)", name: "Violet" },
  };

  $: cs = schemes[colorScheme];
  $: cssVars = `--accent:${cs.accent};--accent2:${cs.accent2};--glow:${cs.glow}`;

  // ── Module definitions ────────────────────────────────────────────────────────
  const moduleDefinitions: Record<string, {icon: string; label: string; pro?: boolean}> = {
    social:  { icon: "⇢", label: "Social" },
    hibp:    { icon: "⚠", label: "HIBP Leaks" },
    ip:      { icon: "♦", label: "IP Geo" },
    whois:   { icon: "◆", label: "WHOIS/DNS" },
    dorks:   { icon: "∞", label: "G.Dorks" },
    paste:   { icon: "☐", label: "Paste/Dox" },
    darkweb: { icon: "⊘", label: "DarkWeb" },
    phone:   { icon: "℡", label: "Phone" },
    intelx:  { icon: "⚙", label: "IntelX" },
    ai:      { icon: "⬡", label: "Ravens AI", pro: true },
  };

  let availableModules = Object.entries(moduleDefinitions);
  let selectedModules = new Set(Object.keys(moduleDefinitions).filter(k => k !== "ai"));

  // ── Computed ──────────────────────────────────────────────────────────────────
  $: foundCount = events.filter(e => e.type === "found").length;
  $: moduleCount = new Set(events.map(e => e.module)).size;
  $: errorCount = events.filter(e => e.type === "error").length;
  $: filteredEvents = filterModule === "all" ? events : events.filter(e => e.module === filterModule);
  $: threatLevel = foundCount > 50 ? "CRITICAL" : foundCount > 20 ? "HIGH" : foundCount > 5 ? "MEDIUM" : "LOW";
  $: threatColor = foundCount > 50 ? "#ef4444" : foundCount > 20 ? "#f59e0b" : foundCount > 5 ? "#3b82f6" : "#22c55e";

  // ── Helpers ───────────────────────────────────────────────────────────────────
  function setTab(t: string) { activeTab = t as Tab; if (t === "map") initMap(); }
  function setPanelTab(t: string) { panelTab = t as PanelTab; }
  function setTargetType(t: string) { targetType = t as TargetType; }
  function setScheme(s: string) { colorScheme = s as ColorScheme; saveScheme(); }

  async function saveScheme() {
    try { await invoke("save_config", { key: "color_scheme", value: colorScheme }); } catch {}
  }

  // ── Particle animation (3D starfield) ────────────────────────────────────────
  interface Particle { x: number; y: number; z: number; px: number; py: number; }
  let particles: Particle[] = [];

  function initParticles() {
    if (!particleCanvas) return;
    particleCtx = particleCanvas.getContext("2d");
    particles = Array.from({length: 120}, () => ({
      x: (Math.random() - 0.5) * 2000,
      y: (Math.random() - 0.5) * 2000,
      z: Math.random() * 1000,
      px: 0, py: 0,
    }));
    animateParticles();
  }

  function animateParticles() {
    if (!particleCtx || !particleCanvas) return;
    const W = particleCanvas.width;
    const H = particleCanvas.height;
    const cx = W / 2, cy = H / 2;
    const speed = 2;
    const accent = cs.accent;

    particleCtx.fillStyle = "rgba(10,11,15,0.15)";
    particleCtx.fillRect(0, 0, W, H);

    for (const p of particles) {
      p.z -= speed;
      if (p.z <= 0) {
        p.x = (Math.random() - 0.5) * 2000;
        p.y = (Math.random() - 0.5) * 2000;
        p.z = 1000;
      }
      const sx = (p.x / p.z) * W + cx;
      const sy = (p.y / p.z) * H + cy;
      const r = Math.max(0.3, (1 - p.z / 1000) * 2.5);
      const alpha = 1 - p.z / 1000;

      if (p.px && p.py) {
        particleCtx.beginPath();
        particleCtx.moveTo(p.px, p.py);
        particleCtx.lineTo(sx, sy);
        particleCtx.strokeStyle = accent + Math.floor(alpha * 180).toString(16).padStart(2,"0");
        particleCtx.lineWidth = r * 0.6;
        particleCtx.stroke();
      }
      particleCtx.beginPath();
      particleCtx.arc(sx, sy, r, 0, Math.PI * 2);
      particleCtx.fillStyle = accent + Math.floor(alpha * 255).toString(16).padStart(2,"0");
      particleCtx.fill();
      p.px = sx; p.py = sy;
    }
    animFrame = requestAnimationFrame(animateParticles);
  }

  function resizeCanvas() {
    if (!particleCanvas) return;
    particleCanvas.width = particleCanvas.offsetWidth;
    particleCanvas.height = particleCanvas.offsetHeight;
  }

  // ── Map (Leaflet via CDN) ─────────────────────────────────────────────────────
  async function initMap() {
    if (mapInstance || !mapEl) return;
    await new Promise<void>(resolve => {
      if ((window as any).L) { resolve(); return; }
      const css = document.createElement("link");
      css.rel = "stylesheet";
      css.href = "https://unpkg.com/leaflet@1.9.4/dist/leaflet.css";
      document.head.appendChild(css);
      const s = document.createElement("script");
      s.src = "https://unpkg.com/leaflet@1.9.4/dist/leaflet.js";
      s.onload = () => resolve();
      document.head.appendChild(s);
    });
    const L = (window as any).L;
    mapInstance = L.map(mapEl, { zoomControl: false, attributionControl: false }).setView([20, 0], 2);
    L.tileLayer("https://{s}.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}{r}.png", {
      maxZoom: 19,
    }).addTo(mapInstance);
    L.control.zoom({ position: "bottomright" }).addTo(mapInstance);
    // Add existing pins
    for (const pin of mapPins) addMapPin(pin);
  }

  function addMapPin(pin: MapPin) {
    if (!mapInstance) return;
    const L = (window as any).L;
    const accentColor = cs.accent;
    const icon = L.divIcon({
      className: "",
      html: `<div style="width:12px;height:12px;border-radius:50%;background:${accentColor};box-shadow:0 0 8px ${accentColor};border:2px solid white;"></div>`,
      iconSize: [12, 12],
      iconAnchor: [6, 6],
    });
    L.marker([pin.lat, pin.lon], {icon})
      .addTo(mapInstance)
      .bindPopup(`<div style="background:#111318;color:#e0e0e0;font-family:monospace;font-size:11px;padding:6px;border:1px solid ${accentColor}">
        <b style="color:${accentColor}">★ GEO PIN</b><br/>
        ${pin.label}<br/>
        <span style="color:#6b7280">${pin.target}</span>
      </div>`, {className: "raven-popup"});
  }

  // ── Scan ──────────────────────────────────────────────────────────────────────
  async function startScan() {
    if (!target.trim() || scanning) return;
    if (licenseTier === "FREE" && requestsRemaining <= 0) {
      tokenStatus = "Лимит FREE: 5 запросов в день. Активируй подписку, чтобы продолжить.";
      setTab("settings");
      settingsSection = "subscriptions";
      return;
    }
    scanning = true;
    events = [];
    aiText = "";
    dossierText = "";
    mapPins = [];
    scanTime = 0;
    panelTab = "console";
    glitchActive = true;
    setTimeout(() => glitchActive = false, 600);

    scanTimer = setInterval(() => scanTime++, 1000);

    try {
      const mods = Array.from(selectedModules);
      const result: OsintEvent[] = await invoke("osint_scan", {
        req: {
          target: target.trim(),
          target_type: targetType,
          modules: mods,
          nvidia_api_key: nvidiaKey || null,
        }
      });

      for (const ev of result) {
        if (ev.type === "stream") {
          aiText = ev.text;
          dossierText = ev.text;
        } else {
          events = [...events, ev];
          // Parse geo pins
          if (ev.text.startsWith("★ GEO_PIN:")) {
            const parts = ev.text.replace("★ GEO_PIN:", "").split("|");
            const [lat, lon] = parts[0].split(",").map(Number);
            const label = parts[1] || "";
            const pin: MapPin = { lat, lon, label, target: target };
            mapPins = [...mapPins, pin];
            if (mapInstance) addMapPin(pin);
          }
        }
      }
    } catch (e) {
      events = [...events, { module: "system", type: "error", text: String(e) }];
    } finally {
      scanning = false;
      clearInterval(scanTimer);
    }
  }

  async function runSingleModule(mod: string) {
    if (!target.trim() || scanning) return;
    scanning = true;
    try {
      const findings = events.filter(e => e.type === "found").map(e => e.text);
      const result: OsintEvent[] = await invoke("osint_module", {
        req: { module: mod, target: target.trim(), nvidia_api_key: nvidiaKey || null, findings }
      });
      for (const ev of result) {
        if (ev.type === "stream") { aiText = ev.text; dossierText = ev.text; }
        else events = [...events, ev];
      }
    } catch (e) {
      events = [...events, { module: mod, type: "error", text: String(e) }];
    } finally { scanning = false; }
  }

  async function sendAiMessage() {
    if (!aiInput.trim() || scanning) return;
    const msg = aiInput.trim();
    aiInput = "";
    events = [...events, { module: "ai", type: "info", text: `► ${msg}` }];
    await runSingleModule("ai");
  }

  function clearResults() { events = []; aiText = ""; dossierText = ""; mapPins = []; scanTime = 0; }

  function copyResults() {
    const text = events.map(e => `[${e.module.toUpperCase()}] ${e.text}`).join("\n")
      + (aiText ? "\n\n=== AI АНАЛИЗ ===\n" + aiText : "");
    navigator.clipboard.writeText(text);
  }

  function toggleModule(mod: string) {
    if (selectedModules.has(mod)) selectedModules.delete(mod);
    else selectedModules.add(mod);
    selectedModules = new Set(selectedModules);
  }

  function addRavenDb() {
    if (!newDbUrl.trim()) return;
    ravenDbUrls = [...ravenDbUrls, { url: newDbUrl.trim(), type: newDbType }];
    newDbUrl = "";
  }

  async function saveKey() {
    if (!nvidiaKey.trim()) return;
    try { await invoke("save_config", { key: "nvidia_key", value: nvidiaKey.trim() }); savedKey = true; } catch {}
  }
  async function clearKey() {
    try { await invoke("clear_config", { key: "nvidia_key" }); nvidiaKey = ""; savedKey = false; } catch {}
  }

  // ── Lifecycle ─────────────────────────────────────────────────────────────────
  onMount(async () => {
    // Load saved settings
    try { nvidiaKey = await invoke("load_config", { key: "nvidia_key" }) as string; savedKey = true; } catch {}
    try { colorScheme = (await invoke("load_config", { key: "color_scheme" }) as ColorScheme) || "crimson"; } catch {}

    // Init particles
    resizeCanvas();
    initParticles();
    window.addEventListener("resize", () => { resizeCanvas(); });
    try { accessToken = await invoke("load_config", { key: "access_token" }) as string; } catch {}
    refreshLicenseStatus();
  });

  onDestroy(() => {
    cancelAnimationFrame(animFrame);
    clearInterval(scanTimer);
    if (mapInstance) { mapInstance.remove(); mapInstance = null; }
  });

  // ── Subscription tiers (UI only, enforced by server) ─────────────────────────
  const tiers = [
    {
      name: "FREE",
      price: "0₽",
      color: "#6b7280",
      requests: 50,
      period: "месяц",
      features: ["5 модулей", "50 запросов/месяц", "Базовый OSINT", "Нет AI-анализа"],
      locked: ["AI Анализ", "IntelX", "Dark Web"],
    },
    {
      name: "PRO",
      price: "990₽",
      color: "#3b82f6",
      requests: 1000,
      period: "месяц",
      features: ["Все 10 модулей", "1000 запросов/месяц", "AI-анализ (NVIDIA NIM)", "Экспорт отчётов"],
      locked: [],
    },
    {
      name: "ELITE",
      price: "2990₽",
      color: "#c0392b",
      requests: -1,
      period: "месяц",
      features: ["Безлимитные запросы", "Приоритетный AI", "RavenDB кастомные базы", "Ранний доступ к модулям", "API-доступ"],
      locked: [],
    },
    {
      name: "ADMIN",
      price: "—",
      color: "#f59e0b",
      requests: -1,
      period: "выдаётся сервером",
      features: ["Всё из Elite", "Управление пользователями", "Выдача токенов", "Просмотр статистики", "Бан/разбан аккаунтов"],
      locked: [],
    },
  ];

  // ── Settings sections ─────────────────────────────────────────────────────────
  const settingsSections = ["appearance", "account", "subscriptions", "server", "advanced"] as const;
  type SettingsSection = typeof settingsSections[number];
  let settingsSection: SettingsSection = "appearance";
  function setSettingsSection(s: string) { settingsSection = s as SettingsSection; }

  const settingsLabels: Record<SettingsSection, string> = {
    appearance: "🎨 Внешний вид",
    account: "👤 Аккаунт",
    subscriptions: "⭐ Подписки",
    server: "🔗 Сервер",
    advanced: "⚙️ Дополнительно",
  };

  let serverUrl = "http://localhost:3000";
  let accessToken = "";
  let tokenStatus = "";
  let licenseTier: string = "FREE";
  let requestsRemaining: number = 5;
  let licenseExpires: string = "";
  let fontSize: "sm" | "md" | "lg" = "md";
  let compactMode = false;
  let showTimestamps = true;
  let autoScroll = true;

  async function refreshLicenseStatus() {
    try {
      const hwid: string = await invoke("get_hwid");
      const tok = (await invoke("load_config", { key: "access_token" }) as string).trim();
      if (!tok) { licenseTier = "FREE"; requestsRemaining = 5; return; }
      const resp = await fetch(`${serverUrl}/api/license/status/${encodeURIComponent(tok)}`);
      const data = await resp.json();
      if (data && !data.error) {
        licenseTier = data.tier || "FREE";
        requestsRemaining = data.requests_remaining ?? 0;
        licenseExpires = data.expires_at || "";
      } else {
        licenseTier = "FREE"; requestsRemaining = 5;
      }
    } catch {
      // offline -> keep last
    }
  }

async function activateToken() {
    if (!accessToken.trim()) return;
    tokenStatus = "Проверка...";
    try {
      const hwid: string = await invoke("get_hwid");
      const resp = await fetch(`${serverUrl}/api/license/activate`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ token: accessToken.trim(), hwid }),
      });
      const data = await resp.json();
      if (data.ok) {
        tokenStatus = `✓ Активирован: ${data.tier} до ${data.expires || "∞"}`;
        await invoke("save_config", { key: "access_token", value: accessToken.trim() });
        await refreshLicenseStatus();
      } else {
        tokenStatus = `✗ Ошибка: ${data.error}`;
      }
    } catch (e) {
      tokenStatus = `✗ Сервер недоступен (${serverUrl})`;
    }
  }
</script>

<!-- ── ROOT ────────────────────────────────────────────────────────────────── -->
<div class="root" style={cssVars}>

  <!-- Particle canvas (background) -->
  <canvas bind:this={particleCanvas} class="particle-bg"></canvas>

  <!-- ── HEADER ──────────────────────────────────────────────────────────────── -->
  <header class="header" class:glitch={glitchActive}>
    <div class="header-brand">
      <div class="brand-icon" style="background:var(--accent);box-shadow:0 0 16px var(--glow)">
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none">
          <path d="M12 2C8 2 5 5 5 9c0 2 1 4 2.5 5.5L4 22l4-2 2 2 2-2 2 2 4-2-3.5-7.5C16 11 17 9 17 7c0-3-2-5-5-5z" fill="white" opacity="0.9"/>
          <circle cx="10" cy="8" r="1.5" fill="white"/>
          <path d="M14 10l3 2-2 1" stroke="white" stroke-width="1" stroke-linecap="round"/>
        </svg>
      </div>
      <div>
        <div class="brand-name">RAVEN</div>
        <div class="brand-sub">GLOBAL THREAT INTERCEPT · OSINT PLATFORM</div>
      </div>
    </div>

    <!-- Stats -->
    <div class="header-stats">
      <div class="stat">
        <span class="stat-val" style="color:#22c55e">{foundCount}</span>
        <span class="stat-lbl">НАЙДЕНО</span>
      </div>
      <div class="stat">
        <span class="stat-val" style="color:#3b82f6">{moduleCount}/10</span>
        <span class="stat-lbl">МОДУЛЕЙ</span>
      </div>
      <div class="stat">
        <span class="stat-val" style="color:{threatColor}">{threatLevel}</span>
        <span class="stat-lbl">УГРОЗА</span>
      </div>
      <div class="stat">
        <span class="stat-val" style="color:#6b7280">{scanTime}s</span>
        <span class="stat-lbl">ВРЕМЯ</span>
      </div>
    </div>

    <!-- Tabs -->
    <nav class="header-tabs">
      {#each [["scan","◎ OSINT"],["map","◈ КАРТА"],["settings","⚙ НАСТРОЙКИ"],["about","⬡ RAVEN"]] as [t, label]}
        <button
          class="tab-btn"
          class:active={activeTab === t}
          style={activeTab === t ? `background:var(--accent);color:white;box-shadow:0 0 10px var(--glow)` : ""}
          on:click={() => setTab(t)}
        >{label}</button>
      {/each}
    </nav>

    <!-- Status -->
    <div class="header-status">
      <span class="status-dot {scanning ? 'scanning' : 'idle'}"></span>
      <span class="status-text">{scanning ? "SCANNING" : "ONLINE"}</span>
    </div>
  </header>

  <!-- ── MAIN CONTENT ─────────────────────────────────────────────────────────── -->
  <main class="main">

    <!-- ══════════ SCAN TAB ══════════ -->
    {#if activeTab === "scan"}
    <div class="scan-layout">

      <!-- LEFT: Controls -->
      <aside class="left-panel">
        <div class="panel-section">
          <div class="section-label">// ЦЕЛЬ</div>
          <input
            bind:value={target}
            placeholder="username / email / IP / domain / phone"
            class="target-input"
            on:keydown={(e) => e.key === "Enter" && startScan()}
          />
          <div class="type-row">
            {#each ["username","email","ip","domain","phone"] as t}
              <button
                class="type-btn"
                class:active-type={targetType === t}
                style={targetType === t ? `background:var(--accent);border-color:var(--accent)` : ""}
                on:click={() => setTargetType(t)}
              >{t.toUpperCase()}</button>
            {/each}
          </div>
        </div>

        <div class="panel-section">
          <div class="section-label">// МОДУЛИ ({selectedModules.size})</div>
          <div class="modules-grid">
            {#each availableModules as [mod, def]}
              <div class="module-item" class:selected={selectedModules.has(mod)}>
                <label class="module-label">
                  <input
                    type="checkbox"
                    checked={selectedModules.has(mod)}
                    on:change={() => toggleModule(mod)}
                    style="accent-color:var(--accent)"
                  />
                  <span class="mod-icon">{def.icon}</span>
                  <span class="mod-name">{def.label}</span>
                  {#if def.pro}<span class="pro-badge">PRO</span>{/if}
                </label>
                {#if target && selectedModules.has(mod)}
                  <button class="run-btn" on:click={() => runSingleModule(mod)} title="Запустить">▶</button>
                {/if}
              </div>
            {/each}
          </div>
        </div>

        <!-- Scan button -->
        <button
          class="scan-btn"
          class:scanning-btn={scanning}
          style={scanning ? "" : "background:var(--accent);box-shadow:0 0 16px var(--glow)"}
          on:click={startScan}
          disabled={scanning || !target.trim()}
        >
          {scanning ? "⚡ СКАНИРОВАНИЕ..." : "► НАЧАТЬ РАССЛЕДОВАНИЕ"}
        </button>

        <!-- Action row -->
        <div class="action-row">
          <button class="action-btn" on:click={clearResults}>✕ ОЧИСТИТЬ</button>
          <button class="action-btn" on:click={copyResults}>⬡ ОТЧЁТ</button>
          <button class="action-btn" on:click={() => { if (mapPins.length) setTab("map"); }}>◈ КАРТА</button>
        </div>

        <!-- Filter -->
        <div class="panel-section" style="margin-top:auto">
          <div class="section-label">// ФИЛЬТР</div>
          <select bind:value={filterModule} class="filter-select">
            <option value="all">ВСЕ МОДУЛИ</option>
            {#each [...new Set(events.map(e => e.module))] as mod}
              <option value={mod}>{mod.toUpperCase()}</option>
            {/each}
          </select>
        </div>
      </aside>

      <!-- RIGHT: Results -->
      <div class="right-panel">
        <!-- Panel tabs -->
        <div class="panel-tabs">
          {#each [["console","КОНСОЛЬ"],["dossier","ДОСЬЕ"],["ravendb","RAVENBD"],["ai","AI AGENT"]] as [t, label]}
            <button
              class="panel-tab"
              class:panel-tab-active={panelTab === t}
              style={panelTab === t ? `color:var(--accent);border-bottom-color:var(--accent)` : ""}
              on:click={() => setPanelTab(t)}
            >⬡ {label}</button>
          {/each}
          <div class="panel-tab-right">
            <span class="terminal-prompt">ravens@nexus:~$ osint {target || ""}</span>
            {#if scanning}<span class="scan-badge">REC</span>{/if}
          </div>
        </div>

        <!-- CONSOLE -->
        {#if panelTab === "console"}
        <div class="console-body" id="console-scroll">
          {#if events.length === 0 && !scanning}
            <div class="console-empty">
              <div class="empty-raven">🦅</div>
              <div class="empty-title">RAVEN OSINT v4 готов</div>
              <div class="empty-sub">Введите цель и запустите расследование</div>
              <div class="empty-sub" style="color:#374151;margin-top:8px">TOP SECRET // SI-TK // NOFORN</div>
            </div>
          {/if}

          {#each filteredEvents as ev}
            <div class="event-row event-{ev.type}">
              <span class="event-module mod-{ev.module}">[{ev.module.toUpperCase()}]</span>
              <span class="event-icon">
                {#if ev.type==="found"}★{:else if ev.type==="error"}✗{:else if ev.type==="running"}▶{:else if ev.type==="done"}✓{:else}·{/if}
              </span>
              <span class="event-text">
                {#if ev.text.startsWith("★ GEO_PIN:")}
                  {@const parts = ev.text.replace("★ GEO_PIN:","").split("|")}
                  <span style="color:#22c55e">★ GEO: {parts[1] || parts[0]}</span>
                  <a href="https://maps.google.com/?q={parts[0]}" target="_blank" class="event-link">[карта]</a>
                {:else if ev.text.startsWith("http") || ev.text.includes("://") || ev.text.startsWith("site:")}
                  <a href={ev.text.startsWith("site:") ? `https://www.google.com/search?q=${encodeURIComponent(ev.text)}` : ev.text}
                     target="_blank" class="event-link">{ev.text}</a>
                {:else}
                  {ev.text}
                {/if}
              </span>
            </div>
          {/each}

          {#if scanning}
            <div class="scanning-indicator">▶ Выполняется разведка...</div>
          {/if}
        </div>

        <!-- DOSSIER -->
        {:else if panelTab === "dossier"}
        <div class="dossier-body">
          {#if !dossierText && !aiText}
            <div class="console-empty">
              <div class="empty-title">ДОСЬЕ ПУСТО</div>
              <div class="empty-sub">Запустите AI-анализ для формирования досье</div>
            </div>
          {:else}
            <div class="dossier-header">
              <span style="color:var(--accent)">⬡ RAVENS NEXUS — ДОСЬЕ</span>
              <span style="color:#6b7280">ЦЕЛЬ: {target}</span>
            </div>
            <div class="dossier-content">{dossierText || aiText}</div>
          {/if}
        </div>

        <!-- RAVENDB -->
        {:else if panelTab === "ravendb"}
        <div class="ravendb-body">
          <div class="section-label" style="padding:12px 16px">⬡ RAVENDB — Пользовательские базы</div>
          <div class="ravendb-add">
            <input bind:value={newDbUrl} placeholder="https://..." class="db-input" />
            <select bind:value={newDbType} class="db-type-select">
              <option value="breach">Breach / Утечка</option>
              <option value="tor">Tor / Dark Web</option>
              <option value="paste">Paste / Dox</option>
              <option value="leak">Leak DB</option>
              <option value="other">Другое</option>
            </select>
            <button class="add-db-btn" on:click={addRavenDb} style="background:var(--accent)">+ ДОБАВИТЬ</button>
          </div>
          {#if ravenDbUrls.length === 0}
            <div class="console-empty" style="padding:32px">
              <div class="empty-sub">Базы не добавлены</div>
            </div>
          {:else}
            <div class="db-list">
              {#each ravenDbUrls as db, i}
                <div class="db-item">
                  <span class="db-type-badge">{db.type}</span>
                  <span class="db-url">{db.url}</span>
                  <button class="db-remove" on:click={() => ravenDbUrls = ravenDbUrls.filter((_,j) => j!==i)}>✕</button>
                </div>
              {/each}
            </div>
          {/if}
        </div>

        <!-- AI AGENT -->
        {:else if panelTab === "ai"}
        <div class="ai-body">
          <div class="ai-header">
            <span style="color:var(--accent)">⬡ RAVEN AI</span>
            <span style="color:#6b7280;font-size:10px">Nemotron-Ultra · NVIDIA NIM</span>
          </div>
          <div class="ai-output">
            {#if aiText}
              <div class="ai-text">{aiText}</div>
            {:else}
              <div class="console-empty">
                <div class="empty-sub">RAVEN AI · Nemotron-Ultra</div>
                <div class="empty-sub" style="color:#374151">Задай вопрос или запусти расследование — проанализирую все данные</div>
              </div>
            {/if}
          </div>
          <div class="ai-input-row">
            <input
              bind:value={aiInput}
              placeholder="Задай вопрос AI агенту..."
              class="ai-input"
              on:keydown={(e) => e.key === "Enter" && sendAiMessage()}
            />
            <button class="ai-send" on:click={sendAiMessage} style="background:var(--accent)">►</button>
          </div>
        </div>
        {/if}
      </div>
    </div>

    <!-- ══════════ MAP TAB ══════════ -->
    {:else if activeTab === "map"}
    {#if licenseTier === "FREE"}
      <div class="about-layout">
        <div class="about-hero">
          <h1 class="about-title" style="font-size:18px">◈ INTEL MAP</h1>
          <p class="about-sub">Карта появится после покупки или выдачи подписки.</p>
          <p class="about-version">Открой: Настройки → Подписки или Сервер → Активировать токен</p>
        </div>
      </div>
    {:else}
    <div class="map-layout">
      <div class="map-sidebar">
        <div class="section-label" style="padding:12px">◈ INTEL MAP</div>
        <div class="section-label" style="padding:4px 12px;font-size:9px">REAL-TIME PUBLIC DATA INTERCEPT</div>

        <div class="map-pins-list">
          {#if mapPins.length === 0}
            <div class="console-empty" style="padding:24px 12px">
              <div class="empty-sub">Запустите OSINT для появления геопинов</div>
            </div>
          {:else}
            {#each mapPins as pin}
              <div class="map-pin-item" on:click={() => mapInstance?.setView([pin.lat, pin.lon], 8)}>
                <span style="color:var(--accent)">★</span>
                <div>
                  <div class="pin-label">{pin.label}</div>
                  <div class="pin-coords">{pin.lat.toFixed(3)}, {pin.lon.toFixed(3)}</div>
                </div>
              </div>
            {/each}
          {/if}
        </div>

        <div class="map-legend">
          <div class="section-label" style="padding:8px 12px">DATA LAYERS</div>
          {#each [["✈","Commercial Flights"],["⚡","Earthquakes"],["◉","Threat Intel"],["☣","Malware/C2"],["◎","OSINT Pins"]] as [icon, label]}
            <div class="layer-item">
              <span style="color:var(--accent)">{icon}</span>
              <span>{label}</span>
              <span class="layer-live">ON</span>
            </div>
          {/each}
        </div>
      </div>

      <div class="map-container" bind:this={mapEl}></div>
    </div>
    {/if}

    <!-- ══════════ SETTINGS TAB ══════════ -->
    {:else if activeTab === "settings"}
    <div class="settings-layout">
      <!-- Settings nav -->
      <nav class="settings-nav">
        {#each settingsSections as sec}
          <button
            class="settings-nav-btn"
            class:settings-nav-active={settingsSection === sec}
            style={settingsSection === sec ? `border-left-color:var(--accent);color:var(--accent)` : ""}
            on:click={() => setSettingsSection(sec)}
          >{settingsLabels[sec]}</button>
        {/each}
      </nav>

      <div class="settings-content">

        <!-- APPEARANCE -->
        {#if settingsSection === "appearance"}
        <div class="settings-section">
          <h2 class="settings-title">🎨 Внешний вид</h2>

          <div class="setting-group">
            <div class="setting-label">Цветовая схема</div>
            <div class="scheme-grid">
              {#each Object.entries(schemes) as [key, scheme]}
                <button
                  class="scheme-btn"
                  class:scheme-active={colorScheme === key}
                  style="border-color:{colorScheme===key ? scheme.accent : '#1e2130'};box-shadow:{colorScheme===key ? `0 0 12px ${scheme.glow}` : 'none'}"
                  on:click={() => setScheme(key)}
                >
                  <div class="scheme-preview" style="background:{scheme.accent}"></div>
                  <span style="color:{colorScheme===key ? scheme.accent : '#9ca3af'}">{scheme.name}</span>
                </button>
              {/each}
            </div>
          </div>

          <div class="setting-group">
            <div class="setting-label">Размер шрифта</div>
            <div class="radio-row">
              {#each [["sm","Маленький"],["md","Средний"],["lg","Большой"]] as [v, label]}
                <button
                  class="radio-btn"
                  class:radio-active={fontSize === v}
                  style={fontSize===v ? "border-color:var(--accent);color:var(--accent)" : ""}
                  on:click={() => (fontSize = v)}
                >{label}</button>
              {/each}
            </div>
          </div>

          <div class="setting-group">
            <div class="setting-label">Интерфейс</div>
            <div class="toggle-list">
              <label class="toggle-item">
                <input type="checkbox" bind:checked={compactMode} style="accent-color:var(--accent)"/>
                Компактный режим
              </label>
              <label class="toggle-item">
                <input type="checkbox" bind:checked={showTimestamps} style="accent-color:var(--accent)"/>
                Показывать метки времени
              </label>
              <label class="toggle-item">
                <input type="checkbox" bind:checked={autoScroll} style="accent-color:var(--accent)"/>
                Авто-прокрутка консоли
              </label>
              <label class="toggle-item">
                <input type="checkbox" bind:checked={ambientMode} style="accent-color:var(--accent)"/>
                Фоновая анимация (3D звёзды)
              </label>
            </div>
          </div>
        </div>

        <!-- ACCOUNT -->
        {:else if settingsSection === "account"}
        <div class="settings-section">
          <h2 class="settings-title">👤 Аккаунт</h2>

          <div class="setting-group">
            <div class="setting-label">NVIDIA NIM API Key</div>
            <p class="setting-desc">Нужен для AI-анализа (Nemotron-Ultra). Получить: <a href="https://build.nvidia.com" target="_blank" class="event-link">build.nvidia.com</a></p>
            <div class="key-row">
              <input bind:value={nvidiaKey} type="password" placeholder="nvapi-..." class="key-input" />
              <button class="key-save-btn" on:click={saveKey} style="background:var(--accent)">Сохранить</button>
              {#if savedKey}<button class="key-clear-btn" on:click={clearKey}>✕</button>{/if}
            </div>
            {#if savedKey}<div class="key-saved">✓ СОХРАНЁН (AES-256-GCM, привязан к HWID)</div>{/if}
          </div>

          <div class="setting-group">
            <div class="setting-label">Device ID (HWID)</div>
            {#await invoke("get_hwid") then hwid}
              <code class="hwid-code">{hwid}</code>
            {:catch}
              <span style="color:#ef4444">Ошибка получения HWID</span>
            {/await}
          </div>
        </div>

        <!-- SUBSCRIPTIONS -->
        {:else if settingsSection === "subscriptions"}
        <div class="settings-section">
          <h2 class="settings-title">⭐ Подписки Ravens Nexus</h2>
          <p class="setting-desc" style="margin-bottom:16px">Управление подпиской через сервер лицензий. Токен привязывается к вашему HWID.</p>

          <div class="tiers-grid">
            {#each tiers as tier}
              <div class="tier-card" style="border-color:{tier.color};box-shadow:0 0 12px {tier.color}22">
                <div class="tier-name" style="color:{tier.color}">{tier.name}</div>
                <div class="tier-price">{tier.price}<span class="tier-period">/{tier.period}</span></div>
                <div class="tier-requests">{tier.requests === -1 ? "∞ запросов" : `${tier.requests} запросов`}</div>
                <ul class="tier-features">
                  {#each tier.features as f}
                    <li><span style="color:{tier.color}">✓</span> {f}</li>
                  {/each}
                  {#each tier.locked as f}
                    <li style="opacity:0.4"><span>✗</span> {f}</li>
                  {/each}
                </ul>
                {#if tier.name !== "ADMIN" && tier.name !== "FREE"}
                  <button class="tier-btn" style="background:{tier.color}">Купить</button>
                {:else if tier.name === "FREE"}
                  <button class="tier-btn" style="border:1px solid {tier.color};color:{tier.color};background:transparent">Текущий</button>
                {:else}
                  <button class="tier-btn" style="border:1px solid {tier.color};color:{tier.color};background:transparent" disabled>Выдаётся сервером</button>
                {/if}
              </div>
            {/each}
          </div>
        </div>

        <!-- SERVER -->
        {:else if settingsSection === "server"}
        <div class="settings-section">
          <h2 class="settings-title">🔗 Сервер лицензий</h2>

          <div class="setting-group">
            <div class="setting-label">URL сервера</div>
            <input bind:value={serverUrl} class="key-input" placeholder="http://localhost:3000" />
          </div>

          <div class="setting-group">
            <div class="setting-label">Токен доступа</div>
            <p class="setting-desc">Токен выдаётся администратором через сервер. Привязывается к вашему HWID.</p>
            <div class="key-row">
              <input bind:value={accessToken} type="text" placeholder="RVN-XXXX-XXXX-XXXX" class="key-input" style="font-family:monospace;letter-spacing:2px" />
              <button class="key-save-btn" on:click={activateToken} style="background:var(--accent)">Активировать</button>
            </div>
            {#if tokenStatus}
              <div class="key-saved" style="color:{tokenStatus.startsWith('✓') ? '#22c55e' : '#ef4444'}">{tokenStatus}</div>
            {/if}
          </div>

          <div class="setting-group">
            <div class="setting-label">Статус лицензии</div>
            <div class="license-status">
              <div class="ls-row"><span class="ls-key">Тариф:</span><span class="ls-val" style="color:var(--accent)">FREE</span></div>
              <div class="ls-row"><span class="ls-key">Запросов осталось:</span><span class="ls-val">{requestsRemaining}{licenseTier === "ELITE" || licenseTier === "ADMIN" ? "" : " / день"}</span></div>
              <div class="ls-row"><span class="ls-key">Сервер:</span><span class="ls-val" style="color:#6b7280">{serverUrl}</span></div>
            </div>
          </div>
        </div>

        <!-- ADVANCED -->
        {:else if settingsSection === "advanced"}
        <div class="settings-section">
          <h2 class="settings-title">⚙️ Дополнительно</h2>

          <div class="setting-group">
            <div class="setting-label">Таймаут запросов</div>
            <div class="radio-row">
              {#each ["5s","10s","20s","30s"] as v}
                <button class="radio-btn">{v}</button>
              {/each}
            </div>
          </div>

          <div class="setting-group">
            <div class="setting-label">User-Agent</div>
            <input class="key-input" value="Ravens-OSINT/4.0" />
          </div>

          <div class="setting-group">
            <div class="setting-label">Экспорт данных</div>
            <div class="action-row" style="margin-top:8px">
              <button class="action-btn" on:click={copyResults}>📋 Копировать JSON</button>
              <button class="action-btn">📄 Экспорт TXT</button>
              <button class="action-btn">🗑 Очистить всё</button>
            </div>
          </div>
        </div>
        {/if}

      </div>
    </div>

    <!-- ══════════ ABOUT TAB ══════════ -->
    {:else if activeTab === "about"}
    <div class="about-layout">
      <div class="about-hero">
        <div class="about-icon" style="background:var(--accent);box-shadow:0 0 32px var(--glow)">
          <svg width="48" height="48" viewBox="0 0 24 24" fill="none">
            <path d="M12 2C8 2 5 5 5 9c0 2 1 4 2.5 5.5L4 22l4-2 2 2 2-2 2 2 4-2-3.5-7.5C16 11 17 9 17 7c0-3-2-5-5-5z" fill="white"/>
            <circle cx="10" cy="8" r="1.5" fill="white" opacity="0.8"/>
          </svg>
        </div>
        <h1 class="about-title">RAVEN NEXUS</h1>
        <p class="about-sub">Элитная OSINT-система разведки</p>
        <p class="about-version">v0.1.0 · Rust + Tauri + Svelte · Standalone Desktop</p>
      </div>

      <div class="modules-about-grid">
        {#each [
          ["⇢","Social / Username","Поиск по 10+ платформам","FREE"],
          ["⚠","HIBP Leaks","Проверка email в базах утечек","FREE"],
          ["♦","IP Geo","Геолокация и информация об IP","FREE"],
          ["◆","WHOIS / DNS","Информация о домене","FREE"],
          ["∞","Google Dorks","18+ поисковых запросов","FREE"],
          ["☐","Paste / Doxbin","Поиск в публичных пастах","PRO"],
          ["⊘","Dark Web","Поиск через Ahmia","PRO"],
          ["℡","Phone OSINT","Анализ телефонного номера","PRO"],
          ["⚙","IntelX","Поиск в базах IntelX","ELITE"],
          ["⬡","AI Анализ","NVIDIA NIM Nemotron-Ultra","ELITE"],
        ] as [icon, name, desc, tier]}
          <div class="module-about-card">
            <div class="mac-icon" style="color:var(--accent)">{icon}</div>
            <div>
              <div class="mac-name">{name}</div>
              <div class="mac-desc">{desc}</div>
            </div>
            <div class="mac-tier" style="color:{tier==='FREE'?'#6b7280':tier==='PRO'?'#3b82f6':tier==='ELITE'?'#c0392b':'#f59e0b'}">{tier}</div>
          </div>
        {/each}
      </div>
    </div>
    {/if}

  </main>
</div>

<style>
  :global(*) { box-sizing: border-box; margin: 0; padding: 0; }
  :global(html, body) { height: 100%; background: #0a0b0f; color: #e0e0e0; font-family: 'Inter', system-ui, sans-serif; overflow: hidden; }
  :global(#app) { height: 100vh; display: flex; flex-direction: column; }
  :global(::-webkit-scrollbar) { width: 4px; height: 4px; }
  :global(::-webkit-scrollbar-track) { background: #111318; }
  :global(::-webkit-scrollbar-thumb) { background: #1e2130; border-radius: 2px; }
  :global(::-webkit-scrollbar-thumb:hover) { background: var(--accent, #c0392b); }

  /* Root */
  .root { height: 100vh; display: flex; flex-direction: column; position: relative; overflow: hidden; }
  .particle-bg { position: absolute; inset: 0; width: 100%; height: 100%; z-index: 0; pointer-events: none; }

  /* Header */
  .header { display: flex; align-items: center; gap: 16px; padding: 0 16px; height: 48px; background: rgba(13,14,19,0.95); border-bottom: 1px solid #1e2130; position: relative; z-index: 10; flex-shrink: 0; backdrop-filter: blur(8px); }
  .header.glitch { animation: glitch 0.3s ease; }
  @keyframes glitch { 0%,100%{transform:none} 20%{transform:translateX(-2px) skewX(-1deg)} 60%{transform:translateX(2px) skewX(1deg)} }
  .header-brand { display: flex; align-items: center; gap: 10px; }
  .brand-icon { width: 32px; height: 32px; border-radius: 8px; display: flex; align-items: center; justify-content: center; flex-shrink: 0; }
  .brand-name { font-size: 14px; font-weight: 800; letter-spacing: 4px; color: white; font-family: 'JetBrains Mono', monospace; }
  .brand-sub { font-size: 8px; color: #6b7280; letter-spacing: 2px; font-family: monospace; }
  .header-stats { display: flex; gap: 16px; margin: 0 auto; }
  .stat { text-align: center; }
  .stat-val { display: block; font-size: 13px; font-weight: 700; font-family: monospace; }
  .stat-lbl { display: block; font-size: 8px; color: #6b7280; letter-spacing: 1px; }
  .header-tabs { display: flex; gap: 2px; }
  .tab-btn { padding: 4px 10px; font-size: 10px; font-family: monospace; border-radius: 4px; border: none; cursor: pointer; background: transparent; color: #6b7280; transition: all 0.15s; letter-spacing: 1px; }
  .tab-btn:hover { color: white; background: #1e2130; }
  .tab-btn.active { color: white; }
  .header-status { display: flex; align-items: center; gap: 6px; }
  .status-dot { width: 6px; height: 6px; border-radius: 50%; background: #22c55e; }
  .status-dot.scanning { background: var(--accent); animation: pulse-scan 0.8s infinite; }
  .status-dot.idle { background: #22c55e; }
  .status-text { font-size: 9px; font-family: monospace; color: #6b7280; letter-spacing: 2px; }
  @keyframes pulse-scan { 0%,100%{opacity:1} 50%{opacity:0.3} }

  /* Main */
  .main { flex: 1; overflow: hidden; position: relative; z-index: 1; }

  /* Scan layout */
  .scan-layout { display: flex; height: 100%; }
  .left-panel { width: 220px; flex-shrink: 0; background: rgba(13,14,19,0.9); border-right: 1px solid #1e2130; display: flex; flex-direction: column; gap: 0; overflow-y: auto; padding: 12px; gap: 10px; }
  .panel-section { display: flex; flex-direction: column; gap: 6px; }
  .section-label { font-size: 9px; font-family: monospace; color: #6b7280; letter-spacing: 2px; padding: 2px 0; }
  .target-input { background: #111318; border: 1px solid #1e2130; border-radius: 4px; padding: 6px 8px; font-size: 11px; font-family: monospace; color: white; outline: none; width: 100%; transition: border-color 0.15s; }
  .target-input:focus { border-color: var(--accent); }
  .type-row { display: flex; flex-wrap: wrap; gap: 3px; }
  .type-btn { padding: 2px 5px; font-size: 8px; font-family: monospace; border: 1px solid #1e2130; border-radius: 3px; cursor: pointer; background: transparent; color: #6b7280; transition: all 0.1s; }
  .type-btn:hover { color: white; border-color: #374151; }
  .type-btn.active-type { color: white; }
  .modules-grid { display: flex; flex-direction: column; gap: 2px; }
  .module-item { display: flex; align-items: center; justify-content: space-between; padding: 3px 4px; border-radius: 3px; transition: background 0.1s; }
  .module-item:hover { background: #111318; }
  .module-item.selected { }
  .module-label { display: flex; align-items: center; gap: 5px; cursor: pointer; flex: 1; font-size: 10px; font-family: monospace; color: #9ca3af; }
  .mod-icon { color: var(--accent); width: 12px; text-align: center; }
  .mod-name { flex: 1; }
  .pro-badge { font-size: 7px; background: #1e3a5f; color: #3b82f6; padding: 1px 3px; border-radius: 2px; }
  .run-btn { font-size: 9px; color: var(--accent); background: none; border: none; cursor: pointer; opacity: 0; padding: 0 4px; }
  .module-item:hover .run-btn { opacity: 1; }
  .scan-btn { width: 100%; padding: 8px; font-size: 11px; font-family: monospace; font-weight: 700; border: none; border-radius: 4px; cursor: pointer; color: white; letter-spacing: 1px; transition: all 0.2s; }
  .scan-btn:disabled { opacity: 0.5; cursor: not-allowed; }
  .scanning-btn { background: #7f1d1d; color: #fca5a5; animation: pulse-scan 1s infinite; }
  .action-row { display: flex; gap: 4px; }
  .action-btn { flex: 1; padding: 4px; font-size: 9px; font-family: monospace; background: transparent; border: 1px solid #1e2130; border-radius: 3px; color: #6b7280; cursor: pointer; transition: all 0.1s; }
  .action-btn:hover { color: white; border-color: var(--accent); }
  .filter-select { width: 100%; background: #111318; border: 1px solid #1e2130; border-radius: 3px; color: #9ca3af; font-size: 10px; font-family: monospace; padding: 4px 6px; outline: none; }

  /* Right panel */
  .right-panel { flex: 1; display: flex; flex-direction: column; overflow: hidden; }
  .panel-tabs { display: flex; align-items: center; background: rgba(13,14,19,0.9); border-bottom: 1px solid #1e2130; padding: 0 12px; height: 36px; gap: 0; flex-shrink: 0; }
  .panel-tab { padding: 0 12px; height: 36px; font-size: 10px; font-family: monospace; background: transparent; border: none; border-bottom: 2px solid transparent; color: #6b7280; cursor: pointer; letter-spacing: 1px; transition: all 0.15s; }
  .panel-tab:hover { color: white; }
  .panel-tab-active { color: var(--accent); }
  .panel-tab-right { margin-left: auto; display: flex; align-items: center; gap: 8px; }
  .terminal-prompt { font-size: 9px; font-family: monospace; color: #374151; }
  .scan-badge { font-size: 8px; font-family: monospace; background: var(--accent); color: white; padding: 1px 5px; border-radius: 2px; animation: pulse-scan 1s infinite; }

  /* Console */
  .console-body { flex: 1; overflow-y: auto; padding: 8px 12px; font-family: monospace; font-size: 11px; display: flex; flex-direction: column; gap: 1px; }
  .console-empty { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; color: #374151; text-align: center; padding: 40px; }
  .empty-raven { font-size: 48px; opacity: 0.3; }
  .empty-title { font-size: 13px; font-weight: 700; color: #4b5563; }
  .empty-sub { font-size: 10px; color: #374151; }
  .event-row { display: flex; gap: 8px; padding: 1px 0; line-height: 1.5; animation: slideIn 0.15s ease; }
  @keyframes slideIn { from{opacity:0;transform:translateY(4px)} to{opacity:1;transform:none} }
  .event-module { width: 64px; flex-shrink: 0; font-size: 9px; opacity: 0.6; }
  .event-icon { width: 10px; flex-shrink: 0; }
  .event-text { flex: 1; word-break: break-all; color: #d1d5db; }
  .event-found .event-text, .event-found .event-icon { color: #22c55e; }
  .event-error .event-text, .event-error .event-icon { color: #ef4444; }
  .event-running .event-icon { color: #f59e0b; }
  .event-done .event-icon { color: #22c55e; }
  .event-link { color: #3b82f6; text-decoration: none; }
  .event-link:hover { text-decoration: underline; }
  .mod-social { color: #3b82f6; }
  .mod-hibp { color: #ef4444; }
  .mod-ip { color: #f59e0b; }
  .mod-whois { color: #8b5cf6; }
  .mod-dorks { color: #06b6d4; }
  .mod-paste { color: #f97316; }
  .mod-darkweb { color: #6b7280; }
  .mod-phone { color: #22c55e; }
  .mod-intelx { color: #ec4899; }
  .mod-ai { color: #a78bfa; }
  .scanning-indicator { color: var(--accent); font-size: 10px; animation: pulse-scan 0.8s infinite; margin-top: 4px; }

  /* Dossier */
  .dossier-body { flex: 1; overflow-y: auto; display: flex; flex-direction: column; }
  .dossier-header { display: flex; justify-content: space-between; padding: 8px 16px; background: rgba(13,14,19,0.8); border-bottom: 1px solid #1e2130; font-size: 10px; font-family: monospace; flex-shrink: 0; }
  .dossier-content { padding: 16px; font-family: monospace; font-size: 11px; line-height: 1.7; color: #d1d5db; white-space: pre-wrap; flex: 1; overflow-y: auto; }

  /* RavenDB */
  .ravendb-body { flex: 1; overflow-y: auto; display: flex; flex-direction: column; }
  .ravendb-add { display: flex; gap: 6px; padding: 8px 16px; border-bottom: 1px solid #1e2130; flex-shrink: 0; }
  .db-input { flex: 1; background: #111318; border: 1px solid #1e2130; border-radius: 3px; padding: 5px 8px; font-size: 11px; font-family: monospace; color: white; outline: none; }
  .db-input:focus { border-color: var(--accent); }
  .db-type-select { background: #111318; border: 1px solid #1e2130; border-radius: 3px; color: #9ca3af; font-size: 10px; padding: 4px; outline: none; }
  .add-db-btn { padding: 5px 10px; font-size: 10px; font-family: monospace; border: none; border-radius: 3px; color: white; cursor: pointer; white-space: nowrap; }
  .db-list { display: flex; flex-direction: column; padding: 8px 16px; gap: 4px; }
  .db-item { display: flex; align-items: center; gap: 8px; padding: 6px 8px; background: #111318; border-radius: 3px; border: 1px solid #1e2130; }
  .db-type-badge { font-size: 8px; font-family: monospace; background: #1e2130; color: #6b7280; padding: 1px 4px; border-radius: 2px; flex-shrink: 0; }
  .db-url { flex: 1; font-size: 10px; font-family: monospace; color: #9ca3af; word-break: break-all; }
  .db-remove { background: none; border: none; color: #6b7280; cursor: pointer; font-size: 11px; padding: 0 4px; }
  .db-remove:hover { color: #ef4444; }

  /* AI */
  .ai-body { flex: 1; display: flex; flex-direction: column; overflow: hidden; }
  .ai-header { display: flex; align-items: center; justify-content: space-between; padding: 8px 16px; border-bottom: 1px solid #1e2130; font-size: 10px; font-family: monospace; flex-shrink: 0; }
  .ai-output { flex: 1; overflow-y: auto; padding: 12px 16px; }
  .ai-text { font-family: monospace; font-size: 11px; line-height: 1.7; color: #d1d5db; white-space: pre-wrap; }
  .ai-input-row { display: flex; gap: 6px; padding: 8px 12px; border-top: 1px solid #1e2130; flex-shrink: 0; background: rgba(13,14,19,0.9); }
  .ai-input { flex: 1; background: #111318; border: 1px solid #1e2130; border-radius: 4px; padding: 6px 10px; font-size: 11px; font-family: monospace; color: white; outline: none; }
  .ai-input:focus { border-color: var(--accent); }
  .ai-send { padding: 6px 14px; font-family: monospace; font-size: 12px; border: none; border-radius: 4px; color: white; cursor: pointer; }

  /* Map */
  .map-layout { display: flex; height: 100%; }
  .map-sidebar { width: 200px; flex-shrink: 0; background: rgba(13,14,19,0.9); border-right: 1px solid #1e2130; display: flex; flex-direction: column; overflow-y: auto; }
  .map-container { flex: 1; }
  .map-pins-list { flex: 1; overflow-y: auto; padding: 4px 0; }
  .map-pin-item { display: flex; align-items: flex-start; gap: 8px; padding: 6px 12px; cursor: pointer; transition: background 0.1s; }
  .map-pin-item:hover { background: #111318; }
  .pin-label { font-size: 10px; font-family: monospace; color: #d1d5db; }
  .pin-coords { font-size: 9px; color: #6b7280; font-family: monospace; }
  .map-legend { border-top: 1px solid #1e2130; }
  .layer-item { display: flex; align-items: center; gap: 8px; padding: 4px 12px; font-size: 10px; font-family: monospace; color: #9ca3af; }
  .layer-live { margin-left: auto; font-size: 8px; color: #22c55e; }
  :global(.raven-popup .leaflet-popup-content-wrapper) { background: #111318 !important; border: 1px solid var(--accent, #c0392b) !important; border-radius: 4px !important; }
  :global(.raven-popup .leaflet-popup-tip) { background: #111318 !important; }

  /* Settings */
  .settings-layout { display: flex; height: 100%; }
  .settings-nav { width: 180px; flex-shrink: 0; background: rgba(13,14,19,0.9); border-right: 1px solid #1e2130; display: flex; flex-direction: column; padding: 12px 0; }
  .settings-nav-btn { padding: 10px 16px; font-size: 11px; background: transparent; border: none; border-left: 2px solid transparent; color: #6b7280; cursor: pointer; text-align: left; transition: all 0.1s; }
  .settings-nav-btn:hover { color: white; background: #111318; }
  .settings-nav-active { color: var(--accent); background: rgba(192,57,43,0.05); }
  .settings-content { flex: 1; overflow-y: auto; padding: 24px; }
  .settings-section { max-width: 640px; }
  .settings-title { font-size: 16px; font-weight: 700; color: white; margin-bottom: 20px; }
  .setting-group { margin-bottom: 24px; }
  .setting-label { font-size: 11px; font-family: monospace; color: #9ca3af; letter-spacing: 1px; margin-bottom: 8px; text-transform: uppercase; }
  .setting-desc { font-size: 11px; color: #6b7280; margin-bottom: 8px; line-height: 1.5; }
  .scheme-grid { display: flex; gap: 8px; flex-wrap: wrap; }
  .scheme-btn { display: flex; flex-direction: column; align-items: center; gap: 4px; padding: 8px 12px; background: #111318; border: 1px solid #1e2130; border-radius: 6px; cursor: pointer; transition: all 0.15s; font-size: 10px; font-family: monospace; }
  .scheme-preview { width: 24px; height: 24px; border-radius: 50%; }
  .radio-row { display: flex; gap: 6px; }
  .radio-btn { padding: 5px 12px; font-size: 10px; font-family: monospace; background: #111318; border: 1px solid #1e2130; border-radius: 3px; color: #6b7280; cursor: pointer; transition: all 0.1s; }
  .radio-btn:hover, .radio-active { color: white; }
  .toggle-list { display: flex; flex-direction: column; gap: 8px; }
  .toggle-item { display: flex; align-items: center; gap: 8px; font-size: 11px; color: #9ca3af; cursor: pointer; }
  .key-row { display: flex; gap: 6px; }
  .key-input { flex: 1; background: #111318; border: 1px solid #1e2130; border-radius: 4px; padding: 7px 10px; font-size: 11px; color: white; outline: none; }
  .key-input:focus { border-color: var(--accent); }
  .key-save-btn { padding: 7px 14px; font-size: 10px; font-family: monospace; border: none; border-radius: 4px; color: white; cursor: pointer; white-space: nowrap; }
  .key-clear-btn { padding: 7px 10px; font-size: 10px; background: transparent; border: 1px solid #1e2130; border-radius: 4px; color: #6b7280; cursor: pointer; }
  .key-saved { font-size: 10px; font-family: monospace; color: #22c55e; margin-top: 6px; }
  .hwid-code { font-size: 10px; font-family: monospace; color: #6b7280; word-break: break-all; background: #111318; padding: 8px; border-radius: 4px; display: block; }
  .tiers-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 12px; }
  .tier-card { background: #111318; border: 1px solid; border-radius: 8px; padding: 16px; display: flex; flex-direction: column; gap: 8px; }
  .tier-name { font-size: 12px; font-weight: 800; font-family: monospace; letter-spacing: 2px; }
  .tier-price { font-size: 20px; font-weight: 700; color: white; }
  .tier-period { font-size: 10px; color: #6b7280; font-weight: 400; }
  .tier-requests { font-size: 10px; color: #6b7280; font-family: monospace; }
  .tier-features { list-style: none; display: flex; flex-direction: column; gap: 4px; font-size: 10px; color: #9ca3af; flex: 1; }
  .tier-features li { display: flex; gap: 5px; }
  .tier-btn { padding: 7px; font-size: 10px; font-family: monospace; border: none; border-radius: 4px; color: white; cursor: pointer; margin-top: 4px; }
  .license-status { background: #111318; border: 1px solid #1e2130; border-radius: 4px; padding: 12px; display: flex; flex-direction: column; gap: 6px; }
  .ls-row { display: flex; justify-content: space-between; font-size: 11px; font-family: monospace; }
  .ls-key { color: #6b7280; }
  .ls-val { color: #d1d5db; }

  /* About */
  .about-layout { overflow-y: auto; padding: 32px; display: flex; flex-direction: column; align-items: center; gap: 32px; }
  .about-hero { text-align: center; display: flex; flex-direction: column; align-items: center; gap: 12px; }
  .about-icon { width: 80px; height: 80px; border-radius: 20px; display: flex; align-items: center; justify-content: center; }
  .about-title { font-size: 28px; font-weight: 900; letter-spacing: 6px; color: white; font-family: 'JetBrains Mono', monospace; }
  .about-sub { font-size: 13px; color: #9ca3af; }
  .about-version { font-size: 10px; color: #374151; font-family: monospace; }
  .modules-about-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 8px; width: 100%; max-width: 800px; }
  .module-about-card { display: flex; align-items: center; gap: 10px; padding: 10px 12px; background: #111318; border: 1px solid #1e2130; border-radius: 6px; }
  .mac-icon { font-size: 16px; width: 20px; text-align: center; flex-shrink: 0; }
  .mac-name { font-size: 11px; font-weight: 600; color: white; }
  .mac-desc { font-size: 9px; color: #6b7280; }
  .mac-tier { font-size: 8px; font-family: monospace; margin-left: auto; flex-shrink: 0; font-weight: 700; }
</style>
