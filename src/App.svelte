<script lang="ts">
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import Popover from "./windows/Popover.svelte";
  import Quick from "./windows/Quick.svelte";
  import Settings from "./windows/Settings.svelte";

  const label = getCurrentWebviewWindow().label;
</script>

{#if label === "popover"}
  <Popover />
{:else if label === "quick"}
  <Quick />
{:else if label === "settings"}
  <Settings />
{/if}
