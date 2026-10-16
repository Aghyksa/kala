<script lang="ts">
  import { onMount, tick, untrack } from "svelte";
  import { addDays, addMonths, monthGrid, monthStart, toIso, weekStart } from "../dates";
  import { formatDate, monthLabel, weekdayNames } from "../i18n.svelte";
  import { wt } from "../workspace-i18n.svelte";
  import Icon from "../Icon.svelte";

  let { value = null, from = null, to = null, autofocus = false, onSelect, onHover }: {
    value?: string | null;
    from?: string | null;
    to?: string | null;
    autofocus?: boolean;
    onSelect: (date: string) => void;
    onHover?: (date: string | null) => void;
  } = $props();

  const today = toIso(new Date());
  let focused = $state(untrack(() => value ?? from ?? today));
  let month = $state(monthStart(untrack(() => focused)));
  let grid: HTMLElement;
  const days = $derived(monthGrid(month));
  const weeks = $derived(Array.from({ length: 6 }, (_, i) => days.slice(i * 7, i * 7 + 7)));
  const low = $derived(from && to ? (from < to ? from : to) : from);
  const high = $derived(from && to ? (from < to ? to : from) : from);

  onMount(() => { if (autofocus) focusDay(); });

  async function focusDay() {
    await tick();
    grid?.querySelector<HTMLButtonElement>(`[data-date="${focused}"]`)?.focus();
  }

  function move(date: string) {
    focused = date;
    if (monthStart(date) !== month) month = monthStart(date);
    focusDay();
  }

  function keydown(e: KeyboardEvent) {
    const steps: Record<string, () => string> = {
      ArrowLeft: () => addDays(focused, -1), ArrowRight: () => addDays(focused, 1),
      ArrowUp: () => addDays(focused, -7), ArrowDown: () => addDays(focused, 7),
      Home: () => weekStart(focused), End: () => addDays(weekStart(focused), 6),
      PageUp: () => addMonths(focused, -1), PageDown: () => addMonths(focused, 1),
    };
    const step = steps[e.key];
    if (!step) return;
    e.preventDefault();
    move(step());
  }

  function shiftMonth(delta: number) {
    month = addMonths(month, delta);
    focused = month;
  }
</script>

<div class="calendar">
  <div class="head">
    <strong aria-live="polite">{monthLabel(month)}</strong>
    <button type="button" class="nav" onclick={() => shiftMonth(-1)} aria-label={wt("Previous month", "Bulan sebelumnya")}><Icon name="left" size={15} /></button>
    <button type="button" class="nav" onclick={() => shiftMonth(1)} aria-label={wt("Next month", "Bulan berikutnya")}><Icon name="right" size={15} /></button>
  </div>
  <div class="grid" role="grid" aria-label={monthLabel(month)} bind:this={grid} onkeydown={keydown} onmouseleave={() => onHover?.(null)} tabindex="-1">
    <div class="row" role="row">{#each weekdayNames() as name}<span class="weekday" role="columnheader">{name}</span>{/each}</div>
    {#each weeks as week}
      <div class="row" role="row">
        {#each week as date (date)}
          {@const inRange = !!low && !!high && date > low && date < high}
          {@const edge = date === low || date === high}
          <span class="cell" role="gridcell" aria-selected={date === value || edge} class:in-range={inRange} class:range-start={!!high && low !== high && date === low} class:range-end={!!low && low !== high && date === high}>
            <button type="button" data-date={date} tabindex={date === focused ? 0 : -1}
              class:outside={date.slice(0, 7) !== month.slice(0, 7)} class:today={date === today} class:selected={date === value || edge}
              aria-current={date === today ? "date" : undefined}
              aria-label={formatDate(date, { weekday: "long", day: "numeric", month: "long", year: "numeric" })}
              onclick={() => { focused = date; onSelect(date); }} onmouseenter={() => onHover?.(date)} onfocus={() => { focused = date; }}>
              {Number(date.slice(8))}
            </button>
          </span>
        {/each}
      </div>
    {/each}
  </div>
</div>

<style>
  .calendar { width: 252px; }
  .head { display: flex; align-items: center; gap: 2px; padding: 0 2px 10px 6px; }
  .head strong { flex: 1; font-size: 13px; font-weight: 600; text-transform: capitalize; }
  .nav { display: grid; place-items: center; width: 28px; height: 28px; border: 0; border-radius: 7px; background: none; color: var(--secondary); }
  .nav:hover { background: var(--hover); color: var(--label); }
  .grid { display: grid; gap: 2px; }
  .grid:focus { outline: none; }
  .row { display: grid; grid-template-columns: repeat(7, 36px); }
  .weekday { text-align: center; font-size: 10px; color: var(--tertiary); padding-bottom: 4px; }
  .cell { display: grid; place-items: center; height: 34px; }
  .cell.in-range { background: var(--accent-soft); }
  .cell.range-start { background: linear-gradient(to right, transparent 50%, var(--accent-soft) 50%); }
  .cell.range-end { background: linear-gradient(to left, transparent 50%, var(--accent-soft) 50%); }
  .cell button { width: 32px; height: 32px; border: 0; border-radius: 8px; background: none; font-size: 12px; font-variant-numeric: tabular-nums; color: var(--label); }
  .cell button:hover { background: var(--hover); }
  .cell.in-range button:hover { background: color-mix(in srgb, var(--accent) 18%, transparent); }
  .cell button.outside { color: var(--tertiary); }
  .cell button.today { color: var(--accent-text); font-weight: 650; box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 45%, transparent); }
  .cell button.selected { background: var(--accent); color: #fff; font-weight: 600; box-shadow: none; }
  .cell button:focus-visible { outline: 2px solid var(--accent-text); outline-offset: 1px; }
</style>
