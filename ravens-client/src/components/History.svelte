<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  export let token: string;

  let history: any[] = [];
  let loading = true;
  let error = "";

  onMount(async () => {
    try {
      history = await invoke("api_get", { path: "/osint/history", token });
    } catch (e: any) { error = String(e); }
    finally { loading = false; }
  });

  const TOOL_ICONS: Record<string, string> = {
    sherlock: "👤", holehe: "📧", whois: "🌐", dns: "🌐",
    ip: "📍", dorks: "🔍", image_geo: "🖼", geoip: "🗺",
    aircraft: "✈", earthquakes: "🌍",
  };
</script>

<div class="max-w-3xl mx-auto">
  <h2 class="text-xl font-bold text-white mb-1">Request History</h2>
  <p class="text-gray-500 text-sm mb-6">Your last 50 OSINT queries.</p>

  {#if loading}
    <p class="text-gray-500 text-sm">Loading...</p>
  {:else if error}
    <div class="card border-red-900/50 text-red-400 text-sm">{error}</div>
  {:else if history.length === 0}
    <div class="card text-center py-12 text-gray-600">
      <p class="text-4xl mb-3">📋</p>
      <p>No history yet — run some OSINT tools.</p>
    </div>
  {:else}
    <div class="space-y-2">
      {#each history as item}
        <div class="card flex items-center gap-4">
          <span class="text-2xl flex-shrink-0">{TOOL_ICONS[item.tool] ?? "🔗"}</span>
          <div class="flex-1 min-w-0">
            <p class="text-sm text-white font-mono truncate">{item.query}</p>
            <p class="text-xs text-gray-500">{item.tool} · {new Date(item.created_at).toLocaleString()} · {item.duration_ms}ms</p>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>
