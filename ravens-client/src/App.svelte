<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import Login from "./routes/Login.svelte";
  import Dashboard from "./routes/Dashboard.svelte";
  import Titlebar from "./components/Titlebar.svelte";

  let token: string | null = null;
  let user: any = null;
  let serverOk = false;
  let loading = true;

  onMount(async () => {
    // Check server health
    for (let i = 0; i < 10; i++) {
      serverOk = await invoke<boolean>("server_health");
      if (serverOk) break;
      await new Promise(r => setTimeout(r, 1000));
    }

    // Try to restore session
    token = await invoke<string | null>("load_token");
    if (token) {
      try {
        user = await invoke("api_get", { path: "/user/me", token });
      } catch {
        token = null;
        await invoke("clear_token");
      }
    }
    loading = false;
  });

  function handleLogin(e: CustomEvent) {
    token = e.detail.token;
    user  = e.detail.user;
  }

  async function handleLogout() {
    await invoke("clear_token");
    token = null;
    user  = null;
  }
</script>

<div class="flex flex-col h-screen overflow-hidden bg-surface-950">
  <Titlebar {user} on:logout={handleLogout} />

  {#if loading}
    <!-- Boot screen -->
    <div class="flex-1 flex flex-col items-center justify-center gap-6">
      <div class="relative">
        <div class="w-20 h-20 rounded-full bg-raven-900/50 border border-raven-700/40 flex items-center justify-center">
          <svg class="w-10 h-10 text-raven-400 animate-pulse" viewBox="0 0 24 24" fill="currentColor">
            <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-1 15v-4H7l5-8v4h4l-5 8z"/>
          </svg>
        </div>
        <div class="absolute inset-0 rounded-full border-2 border-raven-500/20 animate-ping"></div>
      </div>
      <div class="text-center">
        <p class="text-raven-300 font-mono text-sm">
          {serverOk ? "Initializing..." : "Connecting to server..."}
        </p>
        <div class="mt-2 flex gap-1 justify-center">
          {#each [0,1,2] as i}
            <div class="w-1.5 h-1.5 bg-raven-500 rounded-full animate-bounce"
                 style="animation-delay: {i * 0.15}s"></div>
          {/each}
        </div>
      </div>
    </div>

  {:else if !serverOk}
    <div class="flex-1 flex items-center justify-center">
      <div class="card text-center max-w-sm">
        <p class="text-red-400 font-medium mb-2">⚠ Server Offline</p>
        <p class="text-gray-500 text-sm">The Ravens Nexus server could not be reached.<br/>
           Make sure ravens-nexus-server is running.</p>
        <button class="btn-primary mt-4 w-full"
                on:click={() => location.reload()}>Retry</button>
      </div>
    </div>

  {:else if !token}
    <Login on:login={handleLogin} />

  {:else}
    <Dashboard {token} {user} on:logout={handleLogout} />
  {/if}
</div>
