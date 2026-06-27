<script lang="ts">
  import { createEventDispatcher } from "svelte";
  export let user: any;
  export let active: string;
  const dispatch = createEventDispatcher();

  const nav = [
    { id: "username", icon: "👤", label: "Username Search" },
    { id: "email",    icon: "📧", label: "Email Check" },
    { id: "whois",    icon: "🌐", label: "WHOIS / DNS" },
    { id: "dorks",    icon: "🔍", label: "Google Dorks" },
    { id: "geo",      icon: "🗺",  label: "Geo Intel" },
    { id: "image",    icon: "🖼",  label: "Image Search" },
  ];
  const bottom = [
    { id: "history", icon: "📋", label: "History" },
    { id: "profile", icon: "⚙",  label: "Profile" },
  ];

  const RANK_COLORS: Record<string, string> = {
    ANALYST: "bg-blue-900/60 text-blue-300",
    PRO:     "bg-violet-900/60 text-violet-300",
    ELITE:   "bg-amber-900/60 text-amber-300",
    ADMIN:   "bg-red-900/60 text-red-300",
  };
</script>

<aside class="w-56 flex-shrink-0 bg-surface-900 border-r border-raven-900/40
              flex flex-col py-4 select-none">
  <!-- Logo -->
  <div class="px-4 mb-6 flex items-center gap-2.5">
    <div class="w-8 h-8 rounded-lg bg-raven-700/50 border border-raven-600/40
                flex items-center justify-center text-raven-300 text-sm">⚡</div>
    <span class="font-bold text-white tracking-tight text-sm">Ravens Nexus</span>
  </div>

  <!-- Main nav -->
  <nav class="flex-1 px-2 space-y-0.5">
    <p class="px-2 text-xs font-semibold text-gray-600 uppercase tracking-wider mb-2">Tools</p>
    {#each nav as item}
      <button
        on:click={() => dispatch("navigate", item.id)}
        class="w-full flex items-center gap-3 px-3 py-2 rounded-lg text-sm transition-all
               {active === item.id
                 ? 'bg-raven-700/60 text-white font-medium border border-raven-600/40'
                 : 'text-gray-400 hover:bg-raven-900/30 hover:text-gray-200'}"
      >
        <span class="text-base leading-none">{item.icon}</span>
        {item.label}
      </button>
    {/each}
  </nav>

  <!-- Bottom nav -->
  <div class="px-2 space-y-0.5 border-t border-raven-900/40 pt-4 mt-4">
    {#each bottom as item}
      <button
        on:click={() => dispatch("navigate", item.id)}
        class="w-full flex items-center gap-3 px-3 py-2 rounded-lg text-sm transition-all
               {active === item.id
                 ? 'bg-raven-700/60 text-white font-medium border border-raven-600/40'
                 : 'text-gray-400 hover:bg-raven-900/30 hover:text-gray-200'}"
      >
        <span class="text-base leading-none">{item.icon}</span>
        {item.label}
      </button>
    {/each}
    <button on:click={() => dispatch("logout")}
            class="w-full flex items-center gap-3 px-3 py-2 rounded-lg text-sm
                   text-gray-500 hover:bg-red-950/40 hover:text-red-400 transition-all">
      <span class="text-base leading-none">🚪</span> Sign Out
    </button>
  </div>

  <!-- User chip -->
  {#if user}
    <div class="mx-2 mt-3 px-3 py-2.5 rounded-xl bg-raven-950/60 border border-raven-900/50">
      <p class="text-xs text-white font-medium truncate">{user.username}</p>
      <span class="badge mt-1 {RANK_COLORS[user.rank] ?? 'bg-gray-800 text-gray-400'}">
        {user.rank}
      </span>
    </div>
  {/if}
</aside>
