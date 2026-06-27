<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  export let token: string;

  let username = "";
  let result: any = null;
  let loading = false;
  let error = "";

  async function search() {
    if (!username.trim()) return;
    loading = true; error = ""; result = null;
    try {
      result = await invoke("api_post", {
        path: "/osint/username",
        body: { username: username.trim() },
        token,
      });
      if (result?.error) { error = result.detail || result.error; result = null; }
    } catch (e: any) { error = String(e); }
    finally { loading = false; }
  }

  $: found    = result?.hits?.filter((h: any) => h.status === "found") ?? [];
  $: notFound = result?.hits?.filter((h: any) => h.status !== "found") ?? [];

  const CAT_ICONS: Record<string, string> = {
    dev: "💻", social: "👥", gaming: "🎮", music: "🎵",
    photo: "📷", design: "🎨", security: "🔒", blog: "✍",
    ai: "🤖", leak: "⚠", default: "🔗",
  };
</script>

<div class="max-w-3xl mx-auto">
  <h2 class="text-xl font-bold text-white mb-1">Username Search</h2>
  <p class="text-gray-500 text-sm mb-6">Find a username across {result?.total ?? "100+"} platforms simultaneously.</p>

  <form on:submit|preventDefault={search} class="flex gap-3 mb-6">
    <input class="input flex-1 font-mono" bind:value={username}
           placeholder="target_username" disabled={loading} />
    <button class="btn-primary px-6 whitespace-nowrap" type="submit" disabled={loading || !username.trim()}>
      {loading ? "Scanning..." : "Search"}
    </button>
  </form>

  {#if loading}
    <div class="card flex items-center gap-4 mb-6">
      <div class="relative flex-shrink-0">
        <div class="w-10 h-10 rounded-full border-2 border-raven-700 border-t-raven-400 animate-spin"></div>
      </div>
      <div>
        <p class="text-white font-medium">Probing platforms...</p>
        <p class="text-gray-500 text-sm">Sending async requests across all sites. This may take 15–30 seconds.</p>
      </div>
    </div>
  {/if}

  {#if error}
    <div class="card border-red-900/50 bg-red-950/30 text-red-400 text-sm mb-6">{error}</div>
  {/if}

  {#if result}
    <!-- Summary -->
    <div class="grid grid-cols-3 gap-4 mb-6">
      <div class="card text-center">
        <p class="text-3xl font-bold text-raven-400">{result.found}</p>
        <p class="text-xs text-gray-500 mt-1">Accounts Found</p>
      </div>
      <div class="card text-center">
        <p class="text-3xl font-bold text-gray-500">{result.total - result.found}</p>
        <p class="text-xs text-gray-500 mt-1">Not Found</p>
      </div>
      <div class="card text-center">
        <p class="text-3xl font-bold text-white">{(result.elapsed_ms / 1000).toFixed(1)}s</p>
        <p class="text-xs text-gray-500 mt-1">Elapsed</p>
      </div>
    </div>

    <!-- Found accounts -->
    {#if found.length}
      <h3 class="text-sm font-semibold text-emerald-400 mb-3">✓ Found ({found.length})</h3>
      <div class="grid gap-2 mb-6">
        {#each found as hit}
          <a href={hit.url} target="_blank" rel="noopener"
             class="card hover:border-raven-700/60 hover:bg-raven-900/20 transition-all
                    flex items-center justify-between group cursor-pointer">
            <div class="flex items-center gap-3">
              <span class="text-lg">{CAT_ICONS[hit.category] ?? CAT_ICONS.default}</span>
              <div>
                <p class="text-white font-medium text-sm">{hit.site}</p>
                <p class="text-gray-500 text-xs font-mono truncate max-w-xs">{hit.url}</p>
              </div>
            </div>
            <div class="flex items-center gap-2">
              <span class="badge badge-found">{hit.category}</span>
              <span class="text-gray-600 group-hover:text-raven-400 transition-colors text-xs">↗</span>
            </div>
          </a>
        {/each}
      </div>
    {/if}
  {/if}
</div>
