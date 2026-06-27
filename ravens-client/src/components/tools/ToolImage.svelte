<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  export let token: string;

  let imageUrl = "";
  let result: any = null;
  let loading = false;
  let error = "";

  async function search() {
    if (!imageUrl.trim()) return;
    loading = true; error = ""; result = null;
    try {
      result = await invoke("api_post", { path: "/osint/image", body: { url: imageUrl.trim() }, token });
      if (result?.error) { error = result.detail || result.error; result = null; }
    } catch (e: any) { error = String(e); }
    finally { loading = false; }
  }
</script>

<div class="max-w-3xl mx-auto">
  <h2 class="text-xl font-bold text-white mb-1">Image Intelligence</h2>
  <p class="text-gray-500 text-sm mb-6">Geolocate images with Picarta AI and reverse-search with Lenso.</p>

  <form on:submit|preventDefault={search} class="flex gap-3 mb-6">
    <input class="input flex-1 font-mono" bind:value={imageUrl}
           placeholder="https://example.com/photo.jpg" disabled={loading} />
    <button class="btn-primary px-6" type="submit" disabled={loading || !imageUrl.trim()}>
      {loading ? "Analyzing..." : "Analyze"}
    </button>
  </form>

  {#if imageUrl}
    <img src={imageUrl} alt="target" class="w-48 h-32 object-cover rounded-xl border border-raven-900/50 mb-6" />
  {/if}

  {#if error}
    <div class="card border-red-900/50 text-red-400 text-sm mb-4">{error}</div>
  {/if}

  {#if result}
    <!-- Geo result -->
    {#if result.geo}
      <div class="card mb-4">
        <h3 class="text-sm font-semibold text-raven-300 mb-3">📍 Geolocation (Picarta)</h3>
        <div class="grid grid-cols-2 gap-3 text-sm">
          <div><p class="text-gray-500 text-xs">Latitude</p><p class="font-mono text-white">{result.geo.lat?.toFixed(5)}</p></div>
          <div><p class="text-gray-500 text-xs">Longitude</p><p class="font-mono text-white">{result.geo.lon?.toFixed(5)}</p></div>
          <div><p class="text-gray-500 text-xs">Country</p><p class="text-white">{result.geo.country ?? "—"}</p></div>
          <div><p class="text-gray-500 text-xs">Confidence</p>
            <p class="text-white">{result.geo.confidence != null ? (result.geo.confidence * 100).toFixed(1) + "%" : "—"}</p></div>
        </div>
        {#if result.geo.lat && result.geo.lon}
          <a href="https://maps.google.com/?q={result.geo.lat},{result.geo.lon}" target="_blank"
             class="btn-ghost text-xs mt-3 inline-block">Open in Google Maps ↗</a>
        {/if}
      </div>
    {/if}

    <!-- Reverse search -->
    {#if result.reverse_search?.matches?.length}
      <div class="card">
        <h3 class="text-sm font-semibold text-raven-300 mb-3">🔍 Reverse Search (Lenso)</h3>
        <div class="space-y-2">
          {#each result.reverse_search.matches.slice(0, 10) as m}
            <a href={m.url} target="_blank" rel="noopener"
               class="flex items-center gap-3 p-2 rounded-lg hover:bg-raven-900/30 transition-all">
              <img src={m.url} alt="" class="w-12 h-9 object-cover rounded border border-raven-900/40" />
              <div class="flex-1 min-w-0">
                <p class="text-xs text-white truncate">{m.title ?? m.url}</p>
                <p class="text-xs text-gray-500">{m.source ?? ""} {m.similarity != null ? `· ${(m.similarity*100).toFixed(0)}%` : ""}</p>
              </div>
              {#if m.category}<span class="badge bg-raven-900/60 text-raven-300 border border-raven-800/50">{m.category}</span>{/if}
            </a>
          {/each}
        </div>
      </div>
    {/if}
  {/if}
</div>
