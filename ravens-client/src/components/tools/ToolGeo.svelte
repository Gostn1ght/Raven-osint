<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  export let token: string;

  let tab: "geoip" | "aircraft" | "earthquakes" = "geoip";
  let ip = "";
  let result: any = null;
  let loading = false;
  let error = "";
  let map: any = null;
  let mapEl: HTMLDivElement;

  async function geoIp() {
    if (!ip.trim()) return;
    loading = true; error = ""; result = null;
    try {
      result = await invoke("api_post", { path: "/osint/geoip", body: { ip }, token });
      if (result?.error) { error = result.detail || result.error; result = null; return; }
      if (result.lat && result.lon) pinMap(result.lat, result.lon, result.city ?? result.ip);
    } catch (e: any) { error = String(e); }
    finally { loading = false; }
  }

  async function loadAircraft() {
    loading = true; error = "";
    try {
      result = await invoke("api_get", {
        path: "/osint/aircraft?lat_min=35&lat_max=70&lon_min=20&lon_max=80",
        token,
      });
    } catch (e: any) { error = String(e); }
    finally { loading = false; }
  }

  async function loadQuakes() {
    loading = true; error = "";
    try {
      result = await invoke("api_get", { path: "/osint/earthquakes?min_magnitude=4", token });
    } catch (e: any) { error = String(e); }
    finally { loading = false; }
  }

  function pinMap(lat: number, lon: number, label: string) {
    if (!map) return;
    map.setView([lat, lon], 8);
    (window as any).L.marker([lat, lon]).addTo(map).bindPopup(label).openPopup();
  }

  onMount(async () => {
    const L = await import("leaflet");
    await import("leaflet/dist/leaflet.css");
    map = L.map(mapEl, { zoomControl: true }).setView([20, 0], 2);
    L.tileLayer("https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png", {
      attribution: "© OpenStreetMap contributors",
    }).addTo(map);
  });

  onDestroy(() => { map?.remove(); });
</script>

<div class="max-w-3xl mx-auto">
  <h2 class="text-xl font-bold text-white mb-1">Geo Intelligence</h2>
  <p class="text-gray-500 text-sm mb-6">IP geolocation, live aircraft, and seismic activity.</p>

  <!-- Map -->
  <div bind:this={mapEl} class="w-full h-56 rounded-xl border border-raven-900/50 mb-5 overflow-hidden"></div>

  <!-- Tabs -->
  <div class="flex bg-surface-900 rounded-xl p-1 mb-4 border border-raven-900/40 w-fit">
    {#each (["geoip","aircraft","earthquakes"] as const) as t}
      <button on:click={() => { tab = t; result = null; }}
              class="px-4 py-1.5 rounded-lg text-sm font-medium transition-all
                     {tab === t ? 'bg-raven-700 text-white' : 'text-gray-500 hover:text-gray-300'}">
        {t === "geoip" ? "IP Geo" : t === "aircraft" ? "Aircraft" : "Earthquakes"}
      </button>
    {/each}
  </div>

  {#if tab === "geoip"}
    <form on:submit|preventDefault={geoIp} class="flex gap-3 mb-4">
      <input class="input flex-1 font-mono" bind:value={ip} placeholder="8.8.8.8" disabled={loading} />
      <button class="btn-primary px-6" disabled={loading || !ip.trim()}>
        {loading ? "..." : "Locate"}
      </button>
    </form>
    {#if result}
      <div class="card space-y-2">
        {#each Object.entries(result) as [k, v]}
          {#if v !== null && v !== ""}
            <div class="flex gap-4 py-1 border-b border-raven-900/30 last:border-0">
              <span class="text-xs text-gray-500 w-28 flex-shrink-0 font-mono capitalize">{k}</span>
              <span class="text-sm text-white font-mono">{v}</span>
            </div>
          {/if}
        {/each}
      </div>
    {/if}

  {:else if tab === "aircraft"}
    <button class="btn-primary mb-4" on:click={loadAircraft} disabled={loading}>
      {loading ? "Loading..." : "Load Aircraft (Europe)"}
    </button>
    {#if Array.isArray(result)}
      <p class="text-xs text-gray-500 mb-3">{result.length} aircraft in range</p>
      <div class="space-y-1 max-h-80 overflow-y-auto">
        {#each result.slice(0, 50) as a}
          <div class="card py-2 flex items-center gap-4">
            <span class="text-lg">{a.on_ground ? "🛬" : "✈"}</span>
            <div class="flex-1 min-w-0">
              <p class="text-sm font-mono text-white">{(a.callsign ?? a.icao24).trim()}</p>
              {#if a.lat && a.lon}<p class="text-xs text-gray-500">{a.lat.toFixed(2)}, {a.lon.toFixed(2)} · {a.altitude?.toFixed(0) ?? "—"}m</p>{/if}
            </div>
          </div>
        {/each}
      </div>
    {/if}

  {:else}
    <button class="btn-primary mb-4" on:click={loadQuakes} disabled={loading}>
      {loading ? "Loading..." : "Load Earthquakes (M4+)"}
    </button>
    {#if Array.isArray(result)}
      <div class="space-y-2">
        {#each result as q}
          <div class="card flex items-center gap-4">
            <div class="w-12 h-12 rounded-lg bg-raven-900/60 border border-raven-800/40
                        flex items-center justify-center text-lg font-bold
                        {q.magnitude >= 6 ? 'text-red-400' : q.magnitude >= 5 ? 'text-amber-400' : 'text-raven-400'}">
              {q.magnitude.toFixed(1)}
            </div>
            <div>
              <p class="text-sm text-white font-medium">{q.place}</p>
              <p class="text-xs text-gray-500">{new Date(q.time).toLocaleString()} · {q.depth.toFixed(1)}km depth</p>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  {/if}

  {#if error}
    <div class="card border-red-900/50 text-red-400 text-sm mt-4">{error}</div>
  {/if}
</div>
