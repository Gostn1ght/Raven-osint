<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import Sidebar from "../components/Sidebar.svelte";
  import ToolUsername from "../components/tools/ToolUsername.svelte";
  import ToolEmail    from "../components/tools/ToolEmail.svelte";
  import ToolWhois    from "../components/tools/ToolWhois.svelte";
  import ToolDorks    from "../components/tools/ToolDorks.svelte";
  import ToolGeo      from "../components/tools/ToolGeo.svelte";
  import ToolImage    from "../components/tools/ToolImage.svelte";
  import History      from "../components/History.svelte";
  import Profile      from "../components/Profile.svelte";

  export let token: string;
  export let user: any;

  const dispatch = createEventDispatcher();

  let active = "username";

  const TOOLS: Record<string, any> = {
    username: ToolUsername,
    email:    ToolEmail,
    whois:    ToolWhois,
    dorks:    ToolDorks,
    geo:      ToolGeo,
    image:    ToolImage,
    history:  History,
    profile:  Profile,
  };

  $: Component = TOOLS[active] ?? ToolUsername;
</script>

<div class="flex flex-1 overflow-hidden">
  <Sidebar {user} {active} on:navigate={(e) => active = e.detail}
           on:logout={() => dispatch("logout")} />

  <main class="flex-1 overflow-y-auto p-6 bg-surface-950">
    <svelte:component this={Component} {token} {user} />
  </main>
</div>
