<script lang="ts">
  import { tick } from "svelte";
  import { matchPreset, PRESETS, presetRange, shiftRange, type Preset } from "../dates";
  import { rangeLabel } from "../i18n.svelte";
  import { wt } from "../workspace-i18n.svelte";
  import Icon from "../Icon.svelte";
  import Calendar from "./Calendar.svelte";
  import Floating from "./Floating.svelte";

  let { from, to, onChange }: { from: string; to: string; onChange: (from: string, to: string, preset: Preset) => void } = $props();

  let open = $state(false);
  let trigger = $state<HTMLButtonElement>();
  let presetList = $state<HTMLElement>();
  let start = $state<string | null>(null);
  let hover = $state<string | null>(null);
  const current = $derived(matchPreset(from, to));
  const names: Record<Exclude<Preset, "custom">, () => string> = {
    today: () => wt("Today", "Hari ini"), yesterday: () => wt("Yesterday", "Kemarin"),
    week: () => wt("This week", "Minggu ini"), lastWeek: () => wt("Last week", "Minggu lalu"),
    month: () => wt("This month", "Bulan ini"), lastMonth: () => wt("Last month", "Bulan lalu"),
    last30: () => wt("Last 30 days", "30 hari terakhir"), year: () => wt("This year", "Tahun ini"),
  };

  $effect(() => {
    if (!open) return;
    start = null;
    hover = null;
    tick().then(() => presetList?.querySelector<HTMLButtonElement>("[aria-pressed='true'], button")?.focus());
  });

  function apply(nextFrom: string, nextTo: string) {
    open = false;
    trigger?.focus();
    onChange(nextFrom, nextTo, matchPreset(nextFrom, nextTo));
  }

  function pickDay(date: string) {
    if (!start) { start = date; return; }
    apply(date < start ? date : start, date < start ? start : date);
  }
</script>

<div class="range" role="group" aria-label={wt("Date range", "Rentang tanggal")}>
  <button type="button" class="step" onclick={() => apply(...shiftRange(from, to, -1))} aria-label={wt("Previous period", "Periode sebelumnya")}><Icon name="left" size={15} /></button>
  <button bind:this={trigger} type="button" class="trigger" aria-haspopup="dialog" aria-expanded={open} onclick={() => (open = !open)}>
    <Icon name="calendar" size={15} /><span>{rangeLabel(from, to)}</span><Icon name="down" size={13} />
  </button>
  <button type="button" class="step" onclick={() => apply(...shiftRange(from, to, 1))} aria-label={wt("Next period", "Periode berikutnya")}><Icon name="right" size={15} /></button>
</div>
<Floating anchor={trigger} bind:open label={wt("Choose a date range", "Pilih rentang tanggal")}>
  <div class="panel">
    <div class="presets" bind:this={presetList}>
      {#each PRESETS as preset}
        <button type="button" aria-pressed={current === preset} onclick={() => apply(...presetRange(preset))}>{names[preset]()}</button>
      {/each}
    </div>
    <div class="custom">
      <Calendar from={start ?? from} to={start ? (hover ?? start) : to} onSelect={pickDay} onHover={(date) => (hover = date)} />
      <p class="hint" aria-live="polite">{start ? wt("Now pick the end date.", "Sekarang pilih tanggal akhir.") : wt("Pick a start date, then an end date.", "Pilih tanggal awal, lalu tanggal akhir.")}</p>
    </div>
  </div>
</Floating>

<style>
  .range { display: flex; align-items: stretch; height: 32px; border: 1px solid var(--separator); border-radius: 8px; background: var(--surface); overflow: hidden; }
  .range button { border: 0; background: none; color: var(--label); }
  .range button:hover { background: var(--hover); }
  .range button:focus-visible { outline: 2px solid var(--accent-text); outline-offset: -2px; }
  .step { width: 30px; display: grid; place-items: center; color: var(--secondary) !important; }
  .trigger { display: flex; align-items: center; gap: 7px; padding: 0 10px; border-left: 1px solid var(--separator) !important; border-right: 1px solid var(--separator) !important; font-size: 12px; font-weight: 500; font-variant-numeric: tabular-nums; white-space: nowrap; }
  .trigger :global(svg) { color: var(--tertiary); }
  .trigger[aria-expanded="true"] { background: var(--accent-soft) !important; color: var(--accent-text); }
  .panel { display: flex; }
  .presets { display: flex; flex-direction: column; gap: 1px; padding: 8px; width: 152px; border-right: 1px solid var(--separator); }
  .presets button { border: 0; background: none; text-align: left; padding: 7px 10px; border-radius: 7px; font-size: 12px; color: var(--label); }
  .presets button:hover { background: var(--hover); }
  .presets button[aria-pressed="true"] { background: var(--accent-soft); color: var(--accent-text); font-weight: 600; }
  .presets button:focus-visible { outline: 2px solid var(--accent-text); outline-offset: -2px; }
  .custom { padding: 12px 10px 10px; }
  .hint { margin: 8px 4px 0; font-size: 11px; color: var(--secondary); }
</style>
