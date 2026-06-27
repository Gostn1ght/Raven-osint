<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  export let token: string;

  let target = "";
  let filetype = "";
  let result: any = null;
  let loading = false;
  let error = "";
  let copied = "";

  const CATEGORIES = [
    { id: "LoginPortals",   label: "Login Portals",    icon: "🔐" },
    { id: "ExposedFiles",   label: "Exposed Files",    icon: "📂" },
    { id: "DatabaseDumps",  label: "Database Dumps",   icon: "💾" },
    { id: "Subdomains",     label: "Subdomains",       icon: "🌐" },
    { id: "Emails",         label: "Email Addresses",  icon: "📧" },
    { id: "Configs",        label: "Config Files",     icon: "⚙" },
    { id: "ApiKeys",        label: "API Keys",         icon: "🔑" },
    { id: "SocialProfiles", label: "Social Profiles",  icon: "👥" },
    { id: "CameraFeeds",    label: "Camera Feeds",     icon: "📹" },
  ];

  let selected = new Set(["LoginPortals", "ExposedFiles", "Subdomains"]);

  function toggleCat(id: string) {
    if (selected.has(id)) selected.delete(id);
    else selected.add(id);
    selected = new Set(selected); // trigger reactivity
  }

  async function generate() {
    if (!target.trim() || !selected.size) return;
    loading = true; error = ""; result = null;
    try {
      result = await invoke("api_post", {
        path: "/osint/dorks",
        body: {
          target: target.trim(),
          categories: [...selected],
          filetype: filetype || null,
        },
        token,
      });
      if (result?.error) { error = result.detail || result.error; result = null; }
    } catch (e: any) { error = String(e); }
    finally { loading = false; }
  }

  async function copy(text: string, id: string) {
    await navigator.clipboard.writeText(text);
    copied = id;
    setTimeout(() => copied = "", 1500);
  }
</script>

<div class="max-w-3xl mx-auto">
  <h2 class="text-xl font-bold text-white mb-1">Google Dorks Generator</h2>
  <p class="text-gray-500 text-sm mb-6">Generate targeted search operators for OSINT reconnaissance.</p>

  <div class="card mb-4">
    <div class="flex gap-3 mb-4">
      <div class="flex-1">
        <label class="block text-xs text-gray-400 mb-1.5">Target Domain / Keyword</label>
        <input class="input font-mono" bind:value={target} placeholder="example.com" />
      </div>
      <div class="w-36">
        <label class="block text-xs text-gray-400 mb-1.5">File Type (optional)</label>
        <input class="input" bind:value={filetype} placeholder="pdf, xlsx..." />
      </div>
    </div>

    <label class="block text-xs text-gray-400 mb-2.5">Categories</label>
    <div class="flex flex-wrap gap-2 mb-4">
      {#each CATEGORIES as cat}
        <button on:click={() => toggleCat(cat.id)}
                class="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium
                       transition-all border
                       {selected.has(cat.id)
                         ? 'bg-raven-700/60 border-raven-600/60 text-white'
                         : 'bg-transparent border-raven-900/50 text-gray-500 hover:border-raven-700/40 hover:text-gray-300'}">
          {cat.icon} {cat.label}
        </button>
      {/each}
    </div>

    <button class="btn-primary w-full" on:click={generate} disabled={loading || !target.trim() || !selected.size}>
      {loading ? "Generating..." : "Generate Dorks"}
    </button>
  </div>

  {#if error}
    <div class="card border-red-900/50 text-red-400 text-sm mb-4">{error}</div>
  {/if}

  {#if result?.dorks}
    <p class="text-xs text-gray-500 mb-3">Generated {result.dorks.length} dorks for <span class="text-raven-400 font-mono">{result.target}</span></p>
    <div class="space-y-2">
      {#each result.dorks as dork}
        <div class="card flex items-center justify-between gap-4 group">
          <div class="flex-1 min-w-0">
            <p class="text-xs text-gray-500 mb-1">{dork.category} · {dork.description}</p>
            <p class="font-mono text-sm text-raven-300 truncate">{dork.query}</p>
          </div>
          <div class="flex gap-2 flex-shrink-0">
            <button on:click={() => copy(dork.query, dork.query)}
                    class="btn-ghost text-xs px-2 py-1">
              {copied === dork.query ? "✓" : "Copy"}
            </button>
            <a href={dork.google_url} target="_blank" rel="noopener"
               class="btn-ghost text-xs px-2 py-1">Open ↗</a>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>
