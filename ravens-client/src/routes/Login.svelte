<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  const dispatch = createEventDispatcher();

  let mode: "login" | "register" = "login";
  let username = "", email = "", password = "";
  let error = "";
  let loading = false;

  async function submit() {
    error = "";
    loading = true;
    try {
      const hwid = await invoke<string>("get_hwid");
      const path = mode === "login" ? "/auth/login" : "/auth/register";
      const body = mode === "login"
        ? { email, password, hwid }
        : { username, email, password, hwid };

      const resp = await invoke<any>("api_post", { path, body, token: null });

      if (resp.error) { error = resp.detail || resp.error; return; }

      await invoke("save_token", { token: resp.token });

      const user = await invoke<any>("api_get", { path: "/user/me", token: resp.token });
      dispatch("login", { token: resp.token, user });
    } catch (e: any) {
      error = String(e);
    } finally {
      loading = false;
    }
  }
</script>

<div class="flex-1 flex items-center justify-center p-6">
  <div class="w-full max-w-md">
    <!-- Logo -->
    <div class="text-center mb-8">
      <div class="inline-flex items-center justify-center w-16 h-16 rounded-2xl
                  bg-raven-900/60 border border-raven-700/40 mb-4">
        <svg class="w-8 h-8 text-raven-400" viewBox="0 0 24 24" fill="currentColor">
          <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-1 15v-4H7l5-8v4h4l-5 8z"/>
        </svg>
      </div>
      <h1 class="text-2xl font-bold text-white tracking-tight">Ravens Nexus</h1>
      <p class="text-gray-500 text-sm mt-1">AI-Powered OSINT Platform</p>
    </div>

    <!-- Mode tabs -->
    <div class="flex bg-surface-900 rounded-xl p-1 mb-6 border border-raven-900/40">
      <button class="flex-1 py-2 rounded-lg text-sm font-medium transition-all
                     {mode === 'login' ? 'bg-raven-700 text-white shadow' : 'text-gray-500 hover:text-gray-300'}"
              on:click={() => mode = "login"}>Sign In</button>
      <button class="flex-1 py-2 rounded-lg text-sm font-medium transition-all
                     {mode === 'register' ? 'bg-raven-700 text-white shadow' : 'text-gray-500 hover:text-gray-300'}"
              on:click={() => mode = "register"}>Register</button>
    </div>

    <form on:submit|preventDefault={submit} class="space-y-4">
      {#if mode === "register"}
        <div>
          <label class="block text-xs font-medium text-gray-400 mb-1.5">Username</label>
          <input class="input" bind:value={username} placeholder="analyst_01"
                 minlength="3" maxlength="40" required />
        </div>
      {/if}

      <div>
        <label class="block text-xs font-medium text-gray-400 mb-1.5">Email</label>
        <input class="input" type="email" bind:value={email}
               placeholder="user@example.com" required />
      </div>

      <div>
        <label class="block text-xs font-medium text-gray-400 mb-1.5">Password</label>
        <input class="input" type="password" bind:value={password}
               placeholder="••••••••" minlength="8" required />
      </div>

      {#if error}
        <p class="text-red-400 text-sm bg-red-950/50 border border-red-900/50 rounded-lg px-3 py-2">
          {error}
        </p>
      {/if}

      <button type="submit" class="btn-primary w-full py-2.5 mt-2" disabled={loading}>
        {loading ? "Please wait..." : mode === "login" ? "Sign In" : "Create Account"}
      </button>
    </form>

    <p class="text-center text-xs text-gray-600 mt-6">
      Your session is bound to this device's hardware fingerprint for security.
    </p>
  </div>
</div>
