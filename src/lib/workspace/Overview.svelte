<script lang="ts">
  import type { Project } from "../api";
  import { addDays, daysBetween, parse, toIso } from "../dates";
  import { duration, formatDate, rangeLabel } from "../i18n.svelte";
  import type { WorkspaceData, WorkspaceEntry } from "../workspace-api";
  import { wt } from "../workspace-i18n.svelte";
  import Icon from "../Icon.svelte";
  import EntryTable from "./EntryTable.svelte";
  let { data, projects, from, to, onEdit, onAdd, onTimesheet, onRange, onProject }: {
    data: WorkspaceData; projects: Project[]; from: string; to: string;
    onEdit: (e: WorkspaceEntry) => void; onAdd?: () => void; onTimesheet: () => void;
    onRange: (from: string, to: string) => void; onProject: (id: number) => void;
  } = $props();

  const today = toIso(new Date());
  const projectMap = $derived(new Map(projects.map(p => [p.id, p])));
  const totals = $derived([...data.byProject].sort((a, b) => b.minutes - a.minutes));
  const span = $derived(daysBetween(from, to) + 1);
  const step = $derived(Math.max(1, Math.ceil(span / 42)));
  // Bins cost O(bins + active days), so a multi-year range stays cheap.
  const bins = $derived.by(() => {
    const list = Array.from({ length: Math.ceil(span / step) }, (_, i) => {
      const start = addDays(from, i * step);
      const end = step === 1 ? start : addDays(start, step - 1) < to ? addDays(start, step - 1) : to;
      return { start, end, minutes: 0, future: start > today };
    });
    for (const day of data.byDay) {
      const bin = list[Math.floor(daysBetween(from, day.date) / step)];
      if (bin) bin.minutes += day.minutes;
    }
    return list;
  });
  const maxHours = $derived.by(() => {
    const hours = Math.max(1, Math.ceil(Math.max(...bins.map(b => b.minutes)) / 60));
    return hours > 1 && hours % 2 ? hours + 1 : hours;
  });
  const peak = $derived(bins.reduce<(typeof bins)[number] | null>((best, bin) => bin.minutes > (best?.minutes ?? 0) ? bin : best, null));
  let active = $state(-1);
  const shown = $derived(bins[active] ?? peak);
  const binLabel = (bin: { start: string; end: string }) => bin.start === bin.end ? formatDate(bin.start, { weekday: "short", day: "numeric", month: "short" }) : rangeLabel(bin.start, bin.end);
  const labelEvery = $derived(span <= 14 ? 1 : step === 1 ? 7 : Math.ceil(bins.length / 6));
  const firstLabel = $derived(step === 1 && span > 14 ? bins.findIndex(b => parse(b.start).getDay() === 1) : 0);
  const showLabel = (i: number) => (i - firstLabel) % labelEvery === 0 && i >= firstLabel;

  let plot: HTMLElement;
  function keydown(e: KeyboardEvent) {
    const next = e.key === "ArrowRight" ? active + 1 : e.key === "ArrowLeft" ? active - 1 : e.key === "Home" ? 0 : e.key === "End" ? bins.length - 1 : null;
    if (next == null) return;
    e.preventDefault();
    active = Math.min(bins.length - 1, Math.max(0, next));
    plot.querySelectorAll<HTMLButtonElement>("button")[active]?.focus();
  }
  const tabStop = $derived(active >= 0 ? active : Math.max(0, bins.map(b => !b.future).lastIndexOf(true)));
</script>

<section class="summary" aria-label={wt("Period summary","Ringkasan periode")}>
  <div class="focal"><span>{wt("Tracked time","Waktu tercatat")}</span><strong>{duration(data.totalMinutes)}</strong><small>{rangeLabel(from, to)}</small></div>
  <dl>
    <div><dt>{wt("Entries","Catatan")}</dt><dd>{data.total}</dd></div>
    <div><dt>{wt("Projects","Project")}</dt><dd>{data.byProject.length}</dd></div>
    <div><dt>{wt("Active days","Hari aktif")}</dt><dd>{data.byDay.length}</dd></div>
    <div><dt>{wt("Per active day","Per hari aktif")}</dt><dd>{duration(data.byDay.length ? Math.round(data.totalMinutes / data.byDay.length) : 0)}</dd></div>
  </dl>
</section>

<div class="insights">
  <section class="card activity">
    <div class="section-head">
      <div><h2>{step === 1 ? wt("Hours per day","Jam per hari") : wt(`Hours per ${step} days`,`Jam per ${step} hari`)}</h2><p>{wt("Select a bar to open that period in the timesheet.","Pilih batang untuk membuka periode itu di timesheet.")}</p></div>
      {#if shown && shown.minutes}<div class="readout" aria-live="polite"><span>{active >= 0 ? binLabel(shown) : wt(`Longest: ${binLabel(shown)}`,`Terlama: ${binLabel(shown)}`)}</span><strong>{duration(shown.minutes)}</strong></div>{/if}
    </div>
    <div class="chart">
      <div class="y-axis" aria-hidden="true"><span>{maxHours}{wt("h","j")}</span><span>{maxHours % 2 ? "" : `${maxHours / 2}${wt("h","j")}`}</span><span>0</span></div>
      <div class="plot" bind:this={plot} role="toolbar" tabindex="-1" aria-label={wt("Hours per day","Jam per hari")} onkeydown={keydown} onmouseleave={() => (active = -1)}>
        {#each bins as bin, i (bin.start)}
          <button type="button" class="slot" class:future={bin.future} class:today={step === 1 && bin.start === today} tabindex={i === tabStop ? 0 : -1}
            aria-label={`${binLabel(bin)}, ${duration(bin.minutes)}`} onmouseenter={() => (active = i)} onfocus={() => (active = i)} onclick={() => onRange(bin.start, bin.end)}>
            <span class="bar-area"><span class="bar" class:zero={!bin.minutes} style:height={bin.minutes ? `${Math.max(2, bin.minutes / (maxHours * 60) * 100)}%` : undefined}></span></span>
            <span class="tick">{showLabel(i) ? formatDate(bin.start, span <= 7 ? { weekday: "short", day: "numeric" } : { day: "numeric", month: "short" }) : ""}</span>
          </button>
        {/each}
      </div>
    </div>
  </section>

  <section class="card breakdown">
    <div class="section-head"><div><h2>{wt("Time by project","Waktu per project")}</h2><p>{wt("Share of tracked time in this period.","Porsi waktu tercatat di periode ini.")}</p></div></div>
    <div class="project-totals">
      {#each totals.slice(0, 6) as total (total.projectId)}
        {@const project = projectMap.get(total.projectId)}
        <button type="button" class="project-row" onclick={() => onProject(total.projectId)} aria-label={wt(`Open ${project?.name} in the timesheet`,`Buka ${project?.name} di timesheet`)}>
          <span class="total-label"><span class="name"><i style:background={project?.color}></i>{project?.name ?? ""}</span><span class="share">{data.totalMinutes ? Math.round(total.minutes / data.totalMinutes * 100) : 0}%</span><strong>{duration(total.minutes)}</strong></span>
          <span class="track"><span style:width={`${data.totalMinutes ? total.minutes / data.totalMinutes * 100 : 0}%`} style:background={project?.color}></span></span>
        </button>
      {:else}
        <p class="no-data">{wt("No time logged in this period yet.","Belum ada waktu tercatat di periode ini.")}</p>
      {/each}
      {#if totals.length > 6}<p class="more">{wt(`${totals.length - 6} more projects in Reports`,`${totals.length - 6} project lain ada di Laporan`)}</p>{/if}
    </div>
  </section>
</div>

<section class="recent">
  <div class="section-head"><div><h2>{wt("Latest entries","Catatan terbaru")}</h2></div><button class="btn quiet small" onclick={onTimesheet}>{wt("Open timesheet","Buka timesheet")}<Icon name="right" size={14} /></button></div>
  <EntryTable entries={data.entries.slice(0, 6)} {projects} fields={[]} columns={["date","project","description","hours"]} {onEdit} {onAdd} />
</section>

<style>
  .summary { display: flex; align-items: stretch; gap: 28px; padding: 20px 24px; border: 1px solid var(--separator); border-radius: 12px; background: var(--surface); }
  .focal { display: flex; flex-direction: column; min-width: 150px; }
  .focal span, dt { font-size: 11px; color: var(--secondary); }
  .focal strong { font-size: 34px; line-height: 1.15; font-weight: 650; letter-spacing: -1.2px; margin: 6px 0 4px; font-variant-numeric: tabular-nums; }
  .focal small { font-size: 11px; color: var(--tertiary); }
  dl { flex: 1; display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); margin: 0; border-left: 1px solid var(--separator); }
  dl > div { padding: 2px 0 2px 22px; display: flex; flex-direction: column; justify-content: center; gap: 6px; min-width: 0; }
  dd { margin: 0; font-size: 19px; font-weight: 600; letter-spacing: -.4px; font-variant-numeric: tabular-nums; overflow-wrap: anywhere; }
  .insights { display: grid; grid-template-columns: minmax(0, 1.75fr) minmax(0, 1fr); gap: 16px; margin: 16px 0 28px; }
  .card { border: 1px solid var(--separator); border-radius: 12px; padding: 18px 20px 16px; background: var(--surface); min-width: 0; }
  .section-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; margin-bottom: 16px; }
  h2 { font-size: 13px; font-weight: 600; margin: 0; }
  .section-head p { font-size: 11px; color: var(--tertiary); margin: 4px 0 0; }
  .readout { display: flex; flex-direction: column; align-items: flex-end; gap: 2px; font-size: 11px; color: var(--secondary); text-align: right; flex-shrink: 0; }
  .readout strong { font-size: 15px; color: var(--label); font-weight: 600; font-variant-numeric: tabular-nums; }
  .chart { display: flex; gap: 8px; height: 172px; }
  .y-axis { position: relative; height: calc(100% - 20px); min-width: 22px; color: var(--tertiary); font-size: 10px; font-variant-numeric: tabular-nums; }
  .y-axis span { position: absolute; right: 0; transform: translateY(-50%); }
  .y-axis span:nth-child(2) { top: 50%; } .y-axis span:nth-child(3) { top: 100%; }
  .plot { position: relative; flex: 1; min-width: 0; display: flex; gap: 3px; }
  .plot::before { content: ""; position: absolute; inset: 0 0 auto; height: calc((100% - 20px) / 2); border-top: 1px dashed var(--separator); border-bottom: 1px dashed var(--separator); pointer-events: none; }
  .slot { flex: 1; min-width: 0; display: flex; flex-direction: column; padding: 0; border: 0; background: none; border-radius: 4px; }
  .bar-area { flex: 1; display: flex; align-items: flex-end; justify-content: center; border-bottom: 1px solid var(--separator); }
  .bar { width: min(100%, 22px); background: color-mix(in srgb, var(--accent) 78%, var(--surface)); border-radius: 3px 3px 0 0; }
  .bar.zero { height: 0; }
  .slot.today .bar { background: var(--accent); }
  .slot:hover .bar, .slot:focus-visible .bar { background: var(--accent-text); }
  .slot:hover .bar-area, .slot:focus-visible .bar-area { background: var(--hover); }
  .slot:focus-visible { outline: 2px solid var(--accent-text); outline-offset: 1px; }
  .slot.future .bar-area { border-bottom-style: dashed; }
  .tick { height: 20px; padding-top: 5px; font-size: 10px; color: var(--tertiary); white-space: nowrap; overflow: visible; text-align: left; font-variant-numeric: tabular-nums; }
  .slot.today .tick { color: var(--accent-text); font-weight: 600; }
  .project-totals { display: flex; flex-direction: column; gap: 4px; }
  .project-row { display: flex; flex-direction: column; gap: 8px; padding: 9px 8px; margin: 0 -8px; border: 0; border-radius: 8px; background: none; text-align: left; }
  .project-row:hover { background: var(--hover); }
  .total-label { display: flex; align-items: center; gap: 10px; font-size: 12px; }
  .name { flex: 1; display: flex; align-items: center; gap: 7px; min-width: 0; overflow-wrap: anywhere; }
  .name i { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
  .share { font-size: 11px; color: var(--tertiary); font-variant-numeric: tabular-nums; }
  .total-label strong { font-weight: 600; white-space: nowrap; font-variant-numeric: tabular-nums; min-width: 52px; text-align: right; }
  .track { height: 5px; border-radius: 3px; background: var(--fill); overflow: hidden; }
  .track span { display: block; height: 100%; border-radius: inherit; }
  .no-data, .more { font-size: 12px; color: var(--secondary); line-height: 1.6; margin: 6px 0; }
  .more { font-size: 11px; color: var(--tertiary); }
  .recent .section-head { align-items: center; margin-bottom: 12px; }
  @media (max-width: 1100px) {
    .summary { flex-direction: column; gap: 16px; }
    dl { border-left: 0; border-top: 1px solid var(--separator); padding-top: 14px; }
    dl > div { padding-left: 0; }
    .insights { grid-template-columns: 1fr; }
  }
</style>
