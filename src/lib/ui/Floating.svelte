<script lang="ts">
  import type { Snippet } from "svelte";

  // Fixed-position panel: escapes scroll/overflow clipping but stays inside an open <dialog>, so it is not made inert.
  let { anchor, open = $bindable(false), align = "start", label, children }: {
    anchor: HTMLElement | undefined;
    open: boolean;
    align?: "start" | "end";
    label: string;
    children: Snippet;
  } = $props();

  let panel = $state<HTMLElement>();
  let pos = $state<{ top: number; left: number } | null>(null);

  function place() {
    if (!anchor || !panel) return;
    const a = anchor.getBoundingClientRect();
    const p = panel.getBoundingClientRect();
    const below = a.bottom + 6;
    const above = a.top - 6 - p.height;
    // Prefer below, then above; if neither fits, slide up until the panel is fully on screen.
    const top = below + p.height <= innerHeight - 8 ? below : above >= 8 ? above : innerHeight - 8 - p.height;
    const left = align === "end" ? a.right - p.width : a.left;
    pos = { top: Math.max(8, top), left: Math.min(Math.max(8, left), innerWidth - p.width - 8) };
  }

  function close(refocus: boolean) {
    open = false;
    if (refocus) anchor?.focus();
  }

  $effect(() => {
    if (!open || !panel) return;
    place();
    const down = (e: PointerEvent) => {
      const target = e.target as Node;
      if (!panel?.contains(target) && !anchor?.contains(target)) close(false);
    };
    // Capture phase so Escape closes only this panel, not the dialog or window behind it.
    const key = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopPropagation();
      close(true);
    };
    addEventListener("pointerdown", down, true);
    addEventListener("keydown", key, true);
    addEventListener("resize", place);
    addEventListener("scroll", place, true);
    return () => {
      removeEventListener("pointerdown", down, true);
      removeEventListener("keydown", key, true);
      removeEventListener("resize", place);
      removeEventListener("scroll", place, true);
      pos = null;
    };
  });

  function focusout(e: FocusEvent) {
    const next = e.relatedTarget as Node | null;
    if (next && !panel?.contains(next) && !anchor?.contains(next)) close(false);
  }
</script>

{#if open}
  <div bind:this={panel} class="floating" role="dialog" aria-label={label} tabindex="-1" style:top={`${pos?.top ?? 0}px`} style:left={`${pos?.left ?? 0}px`} style:visibility={pos ? "visible" : "hidden"} onfocusout={focusout}>
    {@render children()}
  </div>
{/if}

<style>
  .floating { position: fixed; z-index: 50; background: var(--surface); color: var(--label); border: 1px solid var(--separator); border-radius: 12px; box-shadow: 0 12px 32px #0000002e, 0 1px 3px #0000001a; }
  .floating:focus { outline: none; }
</style>
