<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount, tick } from "svelte";

  // ── Types ──────────────────────────────────────────────────────────────────
  interface OsintEvent {
    module: string;
    type: "running" | "found" | "info" | "error" | "done" | "stream";
    text: string;
  }

  type TargetType = "username" | "email" | "ip" | "domain" | "phone";
  type Tab = "scan" | "settings" | "about";

  // ── State ──────────────────────────────────────────────────────────────────
  let target = "";
  let targetType: TargetType = "username";
  let activeTab: Tab = "scan";
  let scanning = false;
  let events: OsintEvent[] = [];
  let aiText = "";
  let terminalEl: HTMLDivElement;

  // Settings
  let nvidiaKey = "";
  let savedKey = false;

  // Module selection
  const MODULE_DEFS: Record<string, { label: string; icon: string; types: TargetType[] }> = {
    social:  { label: "Social / Username", icon: "👤", types: ["username"] },
    hibp:    { label: "HIBP / Leaks",      icon: "🔓", types: ["email"] },
    ip:      { label: "IP Geo",            icon: "🌐", types: ["ip", "domain"] },
    whois:   { label: "WHOIS / DNS",       icon: "🔍", types: ["domain", "ip"] },
    dorks:   { label: "Google Dorks",      icon: "🎯", types: ["username","email","domain","ip","phone"] },
    paste:   { label: "Paste / Doxbin",    icon: "📋", types: ["username","email","domain","ip","phone"] },
    darkweb: { label: "Dark Web",          icon: "🕸️", types: ["username","email","domain","ip","phone"] },
    phone:   { label: "Phone OSINT",       icon: "📱", types: ["phone"] },
    intelx:  { label: "IntelX",           icon: "🔮", types: ["username","email","domain","ip","phone"] },
  };

  let selectedModules: Set<string> = new Set(["social","hibp","ip","whois","dorks","paste","darkweb","phone","intelx"]);

  // Stats
  let stats = { found: 0, modules: 0, errors: 0 };

  // ── Computed ───────────────────────────────────────────────────────────────
  $: availableModules = Object.entries(MODULE_DEFS).filter(
    ([_, def]) => def.types.includes(targetType)
  );

  $: filteredEvents = events.filter(e => e.type !== "stream");

  $: {
    stats.found   = events.filter(e => e.type === "found").length;
    stats.modules = new Set(events.filter(e => e.type === "done").map(e => e.module)).size;
    stats.errors  = events.filter(e => e.type === "error").length;
  }

  // ── Lifecycle ──────────────────────────────────────────────────────────────
  onMount(async () => {
    try {
      const k = await invoke<string>("load_config", { key: "nvidia_key" });
      if (k) { nvidiaKey = k; savedKey = true; }
    } catch (_) {}
  });

  // ── Methods ────────────────────────────────────────────────────────────────
  async function scrollToBottom() {
    await tick();
    if (terminalEl) terminalEl.scrollTop = terminalEl.scrollHeight;
  }

  async function startScan() {
    if (!target.trim() || scanning) return;
    scanning = true;
    events = [];
    aiText = "";

    const modules = [...selectedModules].filter(m =>
      MODULE_DEFS[m]?.types.includes(targetType)
    );

    try {
      const result = await invoke<OsintEvent[]>("osint_scan", {
        req: {
          target: target.trim(),
          target_type: targetType,
          modules,
          nvidia_api_key: nvidiaKey || null,
        }
      });

      for (const ev of result) {
        if (ev.type === "stream") {
          aiText += ev.text;
        } else {
          events = [...events, ev];
        }
        await scrollToBottom();
      }
    } catch (e) {
      events = [...events, { module: "system", type: "error", text: String(e) }];
    } finally {
      scanning = false;
      await scrollToBottom();
    }
  }

  async function runSingleModule(mod: string) {
    if (scanning) return;
    scanning = true;

    try {
      const result = await invoke<OsintEvent[]>("osint_module", {
        req: {
          module: mod,
          target: target.trim(),
          nvidia_api_key: nvidiaKey || null,
          findings: events.filter(e => e.type === "found").map(e => e.text),
        }
      });

      for (const ev of result) {
        if (ev.type === "stream") {
          aiText += ev.text;
        } else {
          events = [...events, ev];
        }
        await scrollToBottom();
      }
    } catch (e) {
      events = [...events, { module: mod, type: "error", text: String(e) }];
    } finally {
      scanning = false;
    }
  }

  async function saveKey() {
    try {
      await invoke("save_config", { key: "nvidia_key", value: nvidiaKey });
      savedKey = true;
    } catch (e) {
      alert("Ошибка сохранения: " + e);
    }
  }

  async function clearKey() {
    try {
      await invoke("clear_config", { key: "nvidia_key" });
      nvidiaKey = "";
      savedKey = false;
    } catch (_) {}
  }

  function toggleModule(mod: string) {
    if (selectedModules.has(mod)) {
      selectedModules.delete(mod);
    } else {
      selectedModules.add(mod);
    }
    selectedModules = new Set(selectedModules);
  }

  function clearResults() {
    events = [];
    aiText = "";
  }

  function copyResults() {
    const text = events.map(e => `[${e.module}][${e.type}] ${e.text}`).join("\n")
      + (aiText ? "\n\n=== AI АНАЛИЗ ===\n" + aiText : "");
    navigator.clipboard.writeText(text);
  }

  function eventColor(ev: OsintEvent): string {
    switch (ev.type) {
      case "found":   return "#22c55e";
      case "error":   return "#ef4444";
      case "info":    return "#3b82f6";
      case "running": return "#f59e0b";
      case "done":    return "#6b7280";
      default:        return "#e0e0e0";
    }
  }

  function moduleColor(mod: string): string {
    const colors: Record<string, string> = {
      social: "#3b82f6", hibp: "#ef4444", ip: "#f59e0b",
      whois: "#8b5cf6", dorks: "#06b6d4", paste: "#f97316",
      darkweb: "#6b7280", phone: "#22c55e", intelx: "#ec4899",
      ai: "#a78bfa", system: "#e0e0e0",
    };
    return colors[mod] || "#e0e0e0";
  }

  function detectType(val: string): TargetType {
    val = val.trim();
    if (/^\+?[\d\s\-\(\)]{7,15}$/.test(val)) return "phone";
    if (/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(val)) return "email";
    if (/^(\d{1,3}\.){3}\d{1,3}$/.test(val)) return "ip";
    if (/^[a-zA-Z0-9]([a-zA-Z0-9\-]{0,61}[a-zA-Z0-9])?(\.[a-zA-Z]{2,})+$/.test(val)) return "domain";
    return "username";
  }

  function onTargetInput() {
    if (target.length > 3) {
      targetType = detectType(target);
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) startScan();
  }
</script>

<!-- ═══════════════════════════════════════════════════════════════ TEMPLATE -->
<div class="flex flex-col h-screen bg-[#0a0b0f] text-[#e0e0e0] select-none">

  <!-- ── TOPBAR ─────────────────────────────────────────────────────────── -->
  <header class="flex items-center justify-between px-5 py-3 border-b border-[#1e2130] bg-[#0d0f14]">
    <div class="flex items-center gap-3">
      <div class="w-8 h-8 rounded bg-[#c0392b] flex items-center justify-center text-white font-bold text-sm glow-red">
        R
      </div>
      <div>
        <div class="font-bold text-sm tracking-widest text-white">RAVENS NEXUS</div>
        <div class="text-[10px] text-[#6b7280] tracking-wider">OSINT INTELLIGENCE SYSTEM</div>
      </div>
    </div>

    <!-- Stats -->
    <div class="flex items-center gap-6 text-xs terminal">
      <div class="flex items-center gap-2">
        <span class="text-[#6b7280]">НАЙДЕНО</span>
        <span class="text-[#22c55e] font-bold">{stats.found}</span>
      </div>
      <div class="flex items-center gap-2">
        <span class="text-[#6b7280]">МОДУЛЕЙ</span>
        <span class="text-[#3b82f6] font-bold">{stats.modules}</span>
      </div>
      <div class="flex items-center gap-2">
        <span class="text-[#6b7280]">ОШИБОК</span>
        <span class="text-[#ef4444] font-bold">{stats.errors}</span>
      </div>
    </div>

    <!-- Nav tabs -->
    <nav class="flex gap-1">
      {#each [["scan","🔍 РАЗВЕДКА"],["settings","⚙️ НАСТРОЙКИ"],["about","ℹ️ О СИСТЕМЕ"]] as [t, label]}
        <button
          class="px-3 py-1.5 text-xs rounded transition-all {activeTab === t
            ? 'bg-[#c0392b] text-white'
            : 'text-[#6b7280] hover:text-white hover:bg-[#1e2130]'}"
          on:click={() => activeTab = t as Tab}
        >{label}</button>
      {/each}
    </nav>
  </header>

  <!-- ── MAIN ───────────────────────────────────────────────────────────── -->
  <main class="flex flex-1 overflow-hidden">

    <!-- ── SCAN TAB ─────────────────────────────────────────────────────── -->
    {#if activeTab === "scan"}
    <div class="flex flex-1 overflow-hidden">

      <!-- LEFT PANEL: controls -->
      <aside class="w-72 flex-shrink-0 border-r border-[#1e2130] bg-[#0d0f14] flex flex-col overflow-y-auto">

        <!-- Target input -->
        <div class="p-4 border-b border-[#1e2130]">
          <label class="text-[10px] text-[#6b7280] tracking-widest mb-2 block">ЦЕЛЬ</label>
          <input
            bind:value={target}
            on:input={onTargetInput}
            on:keydown={handleKeydown}
            placeholder="username / email / IP / домен / телефон"
            class="w-full bg-[#111318] border border-[#1e2130] rounded px-3 py-2 text-sm terminal
                   text-white placeholder-[#374151] outline-none
                   focus:border-[#c0392b] focus:ring-1 focus:ring-[#c0392b] transition-all"
          />

          <!-- Target type selector -->
          <div class="flex flex-wrap gap-1 mt-2">
            {#each ["username","email","ip","domain","phone"] as t}
              <button
                class="px-2 py-0.5 text-[10px] rounded transition-all {targetType === t
                  ? 'bg-[#c0392b] text-white'
                  : 'bg-[#111318] text-[#6b7280] border border-[#1e2130] hover:border-[#c0392b]'}"
                on:click={() => targetType = t as TargetType}
              >{t.toUpperCase()}</button>
            {/each}
          </div>
        </div>

        <!-- Module selection -->
        <div class="p-4 border-b border-[#1e2130] flex-1">
          <label class="text-[10px] text-[#6b7280] tracking-widest mb-3 block">МОДУЛИ</label>
          <div class="space-y-1">
            {#each availableModules as [mod, def]}
              <label class="flex items-center gap-2 p-2 rounded cursor-pointer
                            hover:bg-[#111318] transition-all group">
                <input
                  type="checkbox"
                  checked={selectedModules.has(mod)}
                  on:change={() => toggleModule(mod)}
                  class="accent-[#c0392b] w-3.5 h-3.5"
                />
                <span class="text-xs">{def.icon} {def.label}</span>
                {#if target && selectedModules.has(mod)}
                  <button
                    class="ml-auto text-[9px] text-[#6b7280] hover:text-[#c0392b] opacity-0 group-hover:opacity-100 transition-all"
                    on:click|stopPropagation={() => { if (target) runSingleModule(mod); }}
                    title="Запустить только этот модуль"
                  >▶</button>
                {/if}
              </label>
            {/each}
          </div>

          <!-- AI module (always available if key set) -->
          {#if nvidiaKey}
            <div class="mt-3 pt-3 border-t border-[#1e2130]">
              <button
                class="w-full py-2 text-xs rounded border border-[#a78bfa] text-[#a78bfa]
                       hover:bg-[#a78bfa] hover:text-black transition-all"
                on:click={() => runSingleModule("ai")}
                disabled={scanning || !target}
              >🤖 AI АНАЛИЗ</button>
            </div>
          {/if}
        </div>

        <!-- Scan button -->
        <div class="p-4">
          <button
            class="w-full py-3 rounded font-bold text-sm tracking-widest transition-all
                   {scanning
                     ? 'bg-[#7f1d1d] text-[#fca5a5] cursor-wait scanning'
                     : 'bg-[#c0392b] hover:bg-[#e74c3c] text-white glow-red'}"
            on:click={startScan}
            disabled={scanning || !target.trim()}
          >
            {scanning ? "⚡ СКАНИРОВАНИЕ..." : "⚡ НАЧАТЬ РАЗВЕДКУ"}
          </button>

          <div class="flex gap-2 mt-2">
            <button
              class="flex-1 py-1.5 text-xs rounded border border-[#1e2130] text-[#6b7280]
                     hover:text-white hover:border-[#374151] transition-all"
              on:click={clearResults}
            >🗑 Очистить</button>
            <button
              class="flex-1 py-1.5 text-xs rounded border border-[#1e2130] text-[#6b7280]
                     hover:text-white hover:border-[#374151] transition-all"
              on:click={copyResults}
            >📋 Копировать</button>
          </div>
        </div>
      </aside>

      <!-- RIGHT PANEL: terminal output -->
      <div class="flex-1 flex flex-col overflow-hidden">

        <!-- Terminal header -->
        <div class="flex items-center justify-between px-4 py-2 border-b border-[#1e2130] bg-[#0d0f14]">
          <div class="flex items-center gap-2">
            <div class="w-2 h-2 rounded-full bg-[#ef4444]"></div>
            <div class="w-2 h-2 rounded-full bg-[#f59e0b]"></div>
            <div class="w-2 h-2 rounded-full bg-[#22c55e]"></div>
            <span class="text-xs text-[#6b7280] ml-2 terminal">
              ravens@nexus:~$ osint {target || "<target>"}
            </span>
          </div>
          {#if scanning}
            <div class="flex items-center gap-2 text-xs text-[#f59e0b] scanning">
              <div class="w-1.5 h-1.5 rounded-full bg-[#f59e0b]"></div>
              СКАНИРОВАНИЕ
            </div>
          {:else if events.length > 0}
            <div class="text-xs text-[#6b7280]">{events.length} событий</div>
          {/if}
        </div>

        <!-- Terminal body -->
        <div
          bind:this={terminalEl}
          class="flex-1 overflow-y-auto p-4 terminal text-xs leading-relaxed"
          style="background: #0a0b0f;"
        >
          {#if events.length === 0 && !scanning && !aiText}
            <div class="text-center py-20 text-[#374151]">
              <div class="text-4xl mb-4">🦅</div>
              <div class="text-sm mb-2">RAVENS NEXUS готов к работе</div>
              <div class="text-xs">Введите цель и выберите модули для начала разведки</div>
            </div>
          {/if}

          {#each filteredEvents as ev (ev)}
            <div class="slide-in flex items-start gap-2 mb-0.5 hover:bg-[#111318] px-1 rounded">
              <!-- Module badge -->
              <span
                class="shrink-0 text-[9px] font-bold w-16 text-right"
                style="color: {moduleColor(ev.module)}"
              >[{ev.module.toUpperCase()}]</span>

              <!-- Type indicator -->
              <span class="shrink-0 w-4 text-center" style="color: {eventColor(ev)}">
                {#if ev.type === "found"}★{:else if ev.type === "error"}✗{:else if ev.type === "running"}▶{:else if ev.type === "done"}✓{:else}·{/if}
              </span>

              <!-- Text -->
              <span style="color: {eventColor(ev)}; word-break: break-all;">
                {#if ev.text.startsWith("★ GEO_PIN:")}
                  {@const parts = ev.text.replace("★ GEO_PIN:","").split("|")}
                  {@const coords = parts[0]}
                  {@const label = parts[1] || ""}
                  <span class="text-[#22c55e]">★ GEO: {label}</span>
                  <a
                    href="https://maps.google.com/?q={coords}"
                    target="_blank"
                    class="ml-2 text-[#3b82f6] underline"
                  >[Открыть на карте]</a>
                {:else if ev.text.startsWith("http") || ev.text.includes("://") || ev.text.startsWith("site:")}
                  {ev.text}
                {:else}
                  {ev.text}
                {/if}
              </span>
            </div>
          {/each}

          <!-- AI output -->
          {#if aiText}
            <div class="mt-4 p-3 rounded border border-[#a78bfa]/30 bg-[#111318]">
              <div class="text-[10px] text-[#a78bfa] mb-2 tracking-widest">═══ AI АНАЛИЗ ═══</div>
              <pre class="text-[#e0e0e0] text-xs whitespace-pre-wrap leading-relaxed">{aiText}</pre>
            </div>
          {/if}

          <!-- Scanning indicator -->
          {#if scanning}
            <div class="flex items-center gap-2 mt-2 text-[#f59e0b] scanning">
              <span>▶</span>
              <span>Выполняется разведка...</span>
            </div>
          {/if}
        </div>
      </div>
    </div>

    <!-- ── SETTINGS TAB ──────────────────────────────────────────────────── -->
    {:else if activeTab === "settings"}
    <div class="flex-1 p-8 overflow-y-auto">
      <div class="max-w-xl mx-auto">
        <h2 class="text-lg font-bold mb-6 text-white">Настройки</h2>

        <!-- NVIDIA Key -->
        <div class="bg-[#111318] border border-[#1e2130] rounded-lg p-5 mb-4">
          <div class="flex items-center gap-2 mb-3">
            <span class="text-[#a78bfa]">🤖</span>
            <h3 class="font-semibold text-sm">NVIDIA NIM API Key</h3>
            {#if savedKey}
              <span class="ml-auto text-[10px] text-[#22c55e] bg-[#052e16] px-2 py-0.5 rounded">СОХРАНЁН</span>
            {/if}
          </div>
          <p class="text-xs text-[#6b7280] mb-3">
            Нужен для AI-анализа через NVIDIA NIM (Nemotron-Ultra).
            Получить: <a href="https://build.nvidia.com" target="_blank" class="text-[#a78bfa] underline">build.nvidia.com</a>
          </p>
          <div class="flex gap-2">
            <input
              bind:value={nvidiaKey}
              type="password"
              placeholder="nvapi-..."
              class="flex-1 bg-[#0a0b0f] border border-[#1e2130] rounded px-3 py-2 text-sm terminal
                     text-white placeholder-[#374151] outline-none focus:border-[#a78bfa] transition-all"
            />
            <button
              class="px-4 py-2 bg-[#a78bfa] hover:bg-[#c4b5fd] text-black text-sm font-bold rounded transition-all"
              on:click={saveKey}
            >Сохранить</button>
            {#if savedKey}
              <button
                class="px-3 py-2 border border-[#1e2130] text-[#6b7280] hover:text-[#ef4444] text-sm rounded transition-all"
                on:click={clearKey}
              >✕</button>
            {/if}
          </div>
          <p class="text-[10px] text-[#374151] mt-2">
            Ключ хранится локально в зашифрованном виде (AES-256-GCM, привязан к устройству)
          </p>
        </div>

        <!-- HWID -->
        <div class="bg-[#111318] border border-[#1e2130] rounded-lg p-5">
          <div class="flex items-center gap-2 mb-3">
            <span>🔑</span>
            <h3 class="font-semibold text-sm">Device ID</h3>
          </div>
          <p class="text-xs text-[#6b7280] mb-2">Уникальный идентификатор этого устройства (используется для шифрования)</p>
          <div class="terminal text-xs text-[#6b7280] bg-[#0a0b0f] rounded p-2 break-all">
            {#await invoke("get_hwid") then hwid}
              {hwid}
            {:catch}
              <span class="text-[#ef4444]">Ошибка получения HWID</span>
            {/await}
          </div>
        </div>
      </div>
    </div>

    <!-- ── ABOUT TAB ─────────────────────────────────────────────────────── -->
    {:else if activeTab === "about"}
    <div class="flex-1 p-8 overflow-y-auto">
      <div class="max-w-xl mx-auto text-center">
        <div class="w-16 h-16 rounded-xl bg-[#c0392b] flex items-center justify-center text-white text-3xl font-bold mx-auto mb-4 glow-red">
          R
        </div>
        <h1 class="text-2xl font-bold text-white mb-2">RAVENS NEXUS</h1>
        <p class="text-[#6b7280] text-sm mb-6">Элитная OSINT-система разведки</p>

        <div class="grid grid-cols-2 gap-3 text-left mb-6">
          {#each [
            ["🔍","Social / Username","Поиск по 10+ платформам"],
            ["🔓","HIBP Leaks","Проверка email в базах утечек"],
            ["🌐","IP Geo","Геолокация и информация об IP"],
            ["🔍","WHOIS / DNS","Информация о домене и DNS-записи"],
            ["🎯","Google Dorks","18+ поисковых запросов"],
            ["📋","Paste / Doxbin","Поиск в публичных пастах"],
            ["🕸️","Dark Web","Поиск через Ahmia"],
            ["📱","Phone OSINT","Анализ телефонного номера"],
            ["🔮","IntelX","Поиск в базах IntelX"],
            ["🤖","AI Анализ","NVIDIA NIM Nemotron-Ultra"],
          ] as [icon, name, desc]}
            <div class="bg-[#111318] border border-[#1e2130] rounded p-3">
              <div class="flex items-center gap-2 mb-1">
                <span>{icon}</span>
                <span class="text-sm font-semibold text-white">{name}</span>
              </div>
              <p class="text-xs text-[#6b7280]">{desc}</p>
            </div>
          {/each}
        </div>

        <div class="text-xs text-[#374151]">
          Ravens Nexus v0.1.0 · Standalone Desktop · Rust + Tauri + Svelte
        </div>
      </div>
    </div>
    {/if}
  </main>
</div>
