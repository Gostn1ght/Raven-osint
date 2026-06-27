<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  export let token: string;

  let domain = "";
  let tab: "whois" | "dns" | "ip" = "whois";
  let result: any = null;
  let loading = false;
  let error = "";

  const paths: Record<typeof tab, string> = {
    whois: "/osint/whois",
    dns:   "/osint/dns",
    ip:    "/osint/ip",
  };

  async function lookup() {
    if (!domain.trim()) return;
    loading = true; error = ""; result = null;
    try {
      const body = tab === "ip" ? { ip: domain.trim() } : { domain: domain.trim() };
      result = await invoke("api_post", { path: paths[tab], body, token });
      if (result?.error) { error = result.detail || result.error; result = null; }
    } catch (e: any) { error = String(e); }
    finally { loading = false; }
  }

  function kv(key: string, val: any) {
    if (!val) return "";
    if (Array.isArray(val)) return val.length ? val : null;
    return val;
  }
</script>

<div class="max-w-3xl mx-auto">
  <h2 class="text-xl font-bold text-white mb-1">WHOIS / DNS / IP</h2>
  <p class="text-gray-500 text-sm mb-6">Domain and network intelligence.</p>

  <!-- Tabs -->
  <div class="flex bg-surface-900 rounded-xl p-1 mb-4 border border-raven-900/40 w-fit">
    {#each (["whois","dns","ip"] as const) as t}
      <button on:click={() => { tab = t; result = null; }}
              class="px-4 py-1.5 rounded-lg text-sm font-medium transition-all
                     {tab === t ? 'bg-raven-700 text-white' : 'text-gray-500 hover:text-gray-300'}">
        {t.toUpperCase()}
      </button>
    {/each}
  </div>

  <form on:submit|preventDefault={lookup} class="flex gap-3 mb-6">
    <input class="input flex-1 font-mono" bind:value={domain}
           placeholder={tab === "ip" ? "1.2.3.4" : "example.com"} disabled={loading} />
    <button class="btn-primary px-6" type="submit" disabled={loading || !domain.trim()}>
      {loading ? "Looking up..." : "Lookup"}
    </button>
  </form>

  {#if error}
    <div class="card border-red-900/50 text-red-400 text-sm mb-4">{error}</div>
  {/if}

  {#if result}
    <div class="card space-y-2">
      {#each Object.entries(result) as [key, val]}
        {#if key !== "raw" && val !== null && val !== "" && !(Array.isArray(val) && val.length === 0)}
          <div class="flex gap-4 py-1.5 border-b border-raven-900/30 last:border-0">
            <span class="text-xs text-gray-500 w-36 flex-shrink-0 font-mono capitalize">{key.replace(/_/g," ")}</span>
            <span class="text-sm text-white font-mono break-all">
              {Array.isArray(val) ? val.join(", ") : String(val)}
            </span>
          </div>
        {/if}
      {/each}
    </div>
    {#if result.raw}
      <details class="mt-3">
        <summary class="text-xs text-gray-600 cursor-pointer hover:text-gray-400">Raw WHOIS data</summary>
        <pre class="text-xs text-gray-500 font-mono mt-2 p-3 bg-surface-900 rounded-lg
                    border border-raven-900/30 overflow-auto max-h-64 whitespace-pre-wrap">{result.raw}</pre>
      </details>
    {/if}
  {/if}
</div>
