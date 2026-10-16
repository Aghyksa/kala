<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import { api } from "../lib/api";
  import { setLang } from "../lib/i18n.svelte";
  import QuickInput from "../lib/QuickInput.svelte";

  let box: HTMLDivElement;
  let quick: QuickInput;
  const hide = () => api.hideWindow("quick");

  onMount(() => {
    const win = getCurrentWindow();
    const ro = new ResizeObserver(() => win.setSize(new LogicalSize(640, Math.ceil(box.getBoundingClientRect().height) + 20)));
    ro.observe(box);
    const un = listen("kala://refresh", async () => {
      setLang((await api.getSettings()).language);
      quick?.focus();
    });
    return () => {
      ro.disconnect();
      un.then((f) => f());
    };
  });
</script>

<div class="wrap" bind:this={box}>
  <QuickInput bind:this={quick} autofocus onDone={hide} onCancel={hide} />
</div>

<style>
  .wrap { padding: 10px; }
  .wrap :global(input) { font-size: 18px; padding: 10px 12px; border: 0; background: transparent; box-shadow: none; }
</style>
