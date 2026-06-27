<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  export let token: string;
  export let user: any;

  let sub: any = null;
  let bio = user?.bio ?? "";
  let saving = false;
  let saved = false;
  let hwid = "";

  onMount(async () => {
    [sub, hwid] = await Promise.all([
      invoke("api_get", { path: "/user/subscription", token }),
      invoke("get_hwid"),
    ]);
  });

  async function saveProfile() {
    saving = true;
    await invoke("api_post", { path: "/user/me", body: { bio }, token });
    saving = false; saved = true;
    setTimeout(() => saved = false, 2000);
  }

  const PLAN_COLORS: Record<string, string> = {
    free:  "bg-gray-800 text-gray-400",
    basic: "bg-blue-900/60 text-blue-300",
    pro:   "bg-violet-900/60 text-violet-300",
    elite: "bg-amber-900/60 text-amber-300",
  };
</script>

<div class="max-w-2xl mx-auto">
  <h2 class="text-xl font-bold text-white mb-6">Profile</h2>

  <div class="card mb-4">
    <div class="flex items-center gap-4 mb-4">
      <div class="w-14 h-14 rounded-xl bg-raven-900/60 border border-raven-700/40
                  flex items-center justify-center text-2xl">
        {user?.username?.[0]?.toUpperCase() ?? "?"}
      </div>
      <div>
        <p class="text-white font-semibold">{user?.username}</p>
        <p class="text-gray-500 text-sm">{user?.email}</p>
        <span class="badge mt-1 bg-raven-900/60 text-raven-300 border border-raven-800/50">{user?.rank}</span>
      </div>
    </div>

    <label class="block text-xs text-gray-400 mb-1.5">Bio</label>
    <textarea class="input resize-none h-20" bind:value={bio} placeholder="Tell us about yourself..."></textarea>
    <button class="btn-primary mt-3" on:click={saveProfile} disabled={saving}>
      {saving ? "Saving..." : saved ? "✓ Saved" : "Save Profile"}
    </button>
  </div>

  {#if sub}
    <div class="card mb-4">
      <h3 class="text-sm font-semibold text-gray-400 mb-3">Subscription</h3>
      <div class="flex items-center justify-between">
        <div>
          <span class="badge text-sm {PLAN_COLORS[sub.plan] ?? 'bg-gray-800 text-gray-400'}">{sub.plan.toUpperCase()}</span>
          {#if sub.expires_at}
            <p class="text-xs text-gray-500 mt-1">Expires: {new Date(sub.expires_at).toLocaleDateString()}</p>
          {/if}
        </div>
        <div class="text-right">
          <p class="text-white font-mono text-sm">{sub.requests_used} / {sub.requests_limit}</p>
          <p class="text-xs text-gray-500">requests used</p>
        </div>
      </div>
      <div class="mt-3 h-1.5 bg-surface-950 rounded-full overflow-hidden">
        <div class="h-full bg-raven-600 rounded-full transition-all"
             style="width: {Math.min(100, sub.requests_used / sub.requests_limit * 100)}%"></div>
      </div>
    </div>
  {/if}

  <div class="card">
    <h3 class="text-sm font-semibold text-gray-400 mb-2">Hardware Fingerprint (HWID)</h3>
    <p class="font-mono text-xs text-raven-400 break-all">{hwid || "Loading..."}</p>
    <p class="text-xs text-gray-600 mt-1.5">Your session is locked to this machine for anti-sharing protection.</p>
  </div>
</div>
