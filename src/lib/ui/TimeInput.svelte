<script lang="ts">
  import { parseTime, stepTime } from "../dates";

  let { value = $bindable(""), id, disabled = false, required = false, onchange }: {
    value?: string;
    id?: string;
    disabled?: boolean;
    required?: boolean;
    onchange?: (value: string) => void;
  } = $props();

  let text = $state(value);
  let focused = false;
  $effect(() => { if (!focused) text = value; });

  function commit(next: string) {
    text = next;
    if (next === value) return;
    value = next;
    onchange?.(next);
  }

  // Commit while typing once the time is unambiguous (13:30, 1330), so a click right after still sees it.
  function input(e: Event & { currentTarget: HTMLInputElement }) {
    const raw = e.currentTarget.value.trim();
    const time = /^\d{1,2}[:.]\d{2}$|^\d{4}$/.test(raw) ? parseTime(raw) : null;
    if (time && time !== value) { value = time; onchange?.(time); }
  }

  function keydown(e: KeyboardEvent) {
    if (e.key === "Enter") { const time = parseTime(text); if (time) commit(time); return; }
    if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;
    e.preventDefault();
    commit(stepTime(parseTime(text) ?? (value || "09:00"), (e.shiftKey ? 60 : 15) * (e.key === "ArrowUp" ? 1 : -1)));
  }
</script>

<input {id} type="text" inputmode="numeric" autocomplete="off" spellcheck="false" maxlength="5" placeholder="09:00" {disabled} {required}
  bind:value={text} onfocus={() => (focused = true)} onblur={() => { focused = false; commit(parseTime(text) ?? value); }} oninput={input} onkeydown={keydown} />

<style>
  input { font-variant-numeric: tabular-nums; }
</style>
