<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import { api } from "../lib/api";
  import { setLang } from "../lib/i18n.svelte";
  import QuickInput from "../lib/QuickInput.svelte";

  let box: HTMLDivElement;
  let quick: QuickInput;
  let exampleProject = $state("project");
  const hide = () => api.hideWindow("quick");

  onMount(() => {
    const win = getCurrentWindow();
    const ro = new ResizeObserver(() => win.setSize(new LogicalSize(640, Math.ceil(box.getBoundingClientRect().height) + 20)));
    ro.observe(box);
    const un = listen("kala://refresh", async () => {
      const [settings, projects] = await Promise.all([api.getSettings(), api.listProjects()]);
      setLang(settings.language);
      exampleProject = projects[0]?.aliases[0] || projects[0]?.name.toLowerCase() || "project";
      quick?.focus();
    });
    return () => {
      ro.disconnect();
      un.then((f) => f());
    };
  });
</script>

<div class="wrap" bind:this={box}>
  <QuickInput bind:this={quick} {exampleProject} autofocus onDone={hide} onCancel={hide} />
</div>

<style>
  .wrap { padding: 12px; background: var(--background); border: 1px solid var(--separator); border-radius: 12px; }
  .wrap :global(.entry-field input) { height: 48px; font-size: 17px; padding-left: 40px; padding-right: 44px; }
  .wrap :global(.input-icon) { top: 15px; }
  .wrap :global(.submit) { top: 9px; }
</style>
