<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  export let token: string;

  let email = "";
  let result: any = null;
  let loading = false;
  let error = "";

  async function check() {
    if (!email.trim()) return;
    loading = true; error = ""; result = null;
    try {
      result = await invoke("api_post", { path: "/osint/email", body: { email }, token });
      if (result?.error) { error = result.detail || result.error; result = null; }
    } catch (e: any) { error = String(e); }
    finally { loading = false; }
  }
</script>

<div class="max-w-3xl mx-auto">
  <h2 class="text-xl font-bold text-white mb-1">Email Intelligence</h2>
  <p class="text-gray-500 text-sm mb-6">Check where an email address is registered across services.</p>

  <form on:submit|preventDefault={check} class="flex gap-3 mb-6">
    <input class="input flex-1" type="email" bind:value={email}
           placeholder="target@example.com" disabled={loading} />
    <button class="btn-primary px-6" type="submit" disabled={loading || !email.trim()}>
      {loading ? "Checking..." : "Check"}
    </button>
  </form>

  {#if error}
    <div class="card border-red-900/50 text-red-400 text-sm mb-4">{error}</div>
  {/if}

  {#if result}
    <div class="grid grid-cols-2 gap-4 mb-6">
      <div class="card text-center">
        <p class="text-3xl font-bold text-raven-400">{result.found}</p>
        <p class="text-xs text-gray-500 mt-1">Registered On</p>
      </div>
      <div class="card text-center">
        <p class="text-3xl font-bold text-white">{(result.elapsed_ms/1000).toFixed(1)}s</p>
        <p class="text-xs text-gray-500 mt-1">Elapsed</p>
      </div>
    </div>

    <div class="space-y-2">
      {#each result.hits as hit}
        <div class="card flex items-center justify-between
                    {hit.registered ? 'border-emerald-900/40' : 'border-raven-900/30 opacity-50'}">
          <div>
            <p class="text-sm font-medium text-white">{hit.site}</p>
            <p class="text-xs text-gray-500">{hit.category}</p>
            {#if hit.meta?.breaches}
              <p class="text-xs text-red-400 mt-1">Breaches: {hit.meta.breaches.join(", ")}</p>
            {/if}
          </div>
          <span class="badge {hit.registered ? 'badge-found' : 'bg-gray-900 text-gray-600'}">
            {hit.registered ? "Registered" : "Not Found"}
          </span>
        </div>
      {/each}
    </div>
  {/if}
</div>
