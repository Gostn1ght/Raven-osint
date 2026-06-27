<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { createEventDispatcher } from "svelte";
  export let user: any;
  const dispatch = createEventDispatcher();
  const win = getCurrentWindow();
</script>

<!-- Custom drag-region titlebar -->
<div data-tauri-drag-region
     class="h-9 flex items-center justify-between px-3
            bg-surface-950 border-b border-raven-900/40 flex-shrink-0 select-none">

  <div class="flex items-center gap-2 text-xs text-gray-600 font-mono">
    <span class="text-raven-500">◈</span>
    <span>Ravens Nexus</span>
    {#if user}
      <span class="text-raven-700">·</span>
      <span class="text-gray-500">{user.email}</span>
    {/if}
  </div>

  <!-- Window controls (Windows style) -->
  <div class="flex items-center gap-0.5 ml-auto">
    <button on:click={() => win.minimize()}
            class="w-8 h-7 rounded hover:bg-white/10 text-gray-500 hover:text-white
                   transition-colors text-xs flex items-center justify-center">─</button>
    <button on:click={() => win.toggleMaximize()}
            class="w-8 h-7 rounded hover:bg-white/10 text-gray-500 hover:text-white
                   transition-colors text-xs flex items-center justify-center">▢</button>
    <button on:click={() => win.close()}
            class="w-8 h-7 rounded hover:bg-red-600 text-gray-500 hover:text-white
                   transition-colors text-xs flex items-center justify-center">✕</button>
  </div>
</div>
