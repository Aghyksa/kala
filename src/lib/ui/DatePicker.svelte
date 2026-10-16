<script lang="ts">
  import { toIso } from "../dates";
  import { formatDate } from "../i18n.svelte";
  import { wt } from "../workspace-i18n.svelte";
  import Icon from "../Icon.svelte";
  import Calendar from "./Calendar.svelte";
  import Floating from "./Floating.svelte";

  let { value = $bindable(""), id, disabled = false, clearable = false, compact = false, onchange }: {
    value?: string;
    id?: string;
    disabled?: boolean;
    clearable?: boolean;
    compact?: boolean;
    onchange?: (value: string) => void;
  } = $props();

  const uid = $props.id();
  let open = $state(false);
  let trigger = $state<HTMLButtonElement>();

  function pick(date: string) {
    open = false;
    trigger?.focus();
    if (date === value) return;
    value = date;
    onchange?.(date);
  }
</script>

<button bind:this={trigger} {id} type="button" class="date-field" class:empty={!value} {disabled} aria-haspopup="dialog" aria-expanded={open} aria-describedby={`${uid}-value`}
  onclick={() => (open = !open)} onkeydown={(e) => { if (e.key === "ArrowDown" && !open) { e.preventDefault(); open = true; } }}>
  <Icon name="calendar" size={15} />
  <span id={`${uid}-value`}>{value ? formatDate(value, compact ? { day: "numeric", month: "short", year: "numeric" } : { weekday: "short", day: "numeric", month: "short", year: "numeric" }) : wt("Choose a date", "Pilih tanggal")}</span>
</button>
<Floating anchor={trigger} bind:open label={wt("Choose a date", "Pilih tanggal")}>
  <div class="picker">
    <Calendar value={value || null} autofocus onSelect={pick} />
    <div class="actions">
      <button type="button" class="btn quiet small" onclick={() => pick(toIso(new Date()))}>{wt("Today", "Hari ini")}</button>
      {#if clearable && value}<button type="button" class="btn quiet small" onclick={() => pick("")}>{wt("Clear", "Kosongkan")}</button>{/if}
    </div>
  </div>
</Floating>

<style>
  .date-field { display: flex; align-items: center; gap: 8px; width: 100%; min-width: 0; min-height: 34px; padding: 7px 10px; border: 1px solid var(--separator); border-radius: 8px; background: var(--field); color: var(--label); text-align: left; font-variant-numeric: tabular-nums; transition: border-color 120ms, box-shadow 120ms; }
  .date-field :global(svg) { color: var(--tertiary); }
  .date-field span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .date-field:hover { border-color: color-mix(in srgb, var(--accent) 35%, var(--separator)); }
  .date-field[aria-expanded="true"], .date-field:focus-visible { border-color: var(--accent); box-shadow: var(--focus); outline: none; }
  .date-field.empty span { color: var(--tertiary); }
  .picker { padding: 12px 10px 8px; }
  .actions { display: flex; justify-content: space-between; border-top: 1px solid var(--separator); margin-top: 8px; padding-top: 6px; }
</style>
