<script lang="ts">
  import type { Project } from "../api";
  import { dayLabel, duration } from "../i18n.svelte";
  import type { FieldDefinition, WorkspaceEntry } from "../workspace-api";
  import { wt } from "../workspace-i18n.svelte";
  import Icon from "../Icon.svelte";
  let { entries, projects, fields, columns, offset = 0, dayTotals = null, onEdit, onAdd }: {
    entries: WorkspaceEntry[]; projects: Project[]; fields: FieldDefinition[]; columns: string[]; offset?: number;
    dayTotals?: Map<string, number> | null; onEdit: (entry: WorkspaceEntry) => void; onAdd?: () => void;
  } = $props();
  const projectMap = $derived(new Map(projects.map(p => [p.id, p])));
  const fieldMap = $derived(new Map(fields.map(f => [`field:${f.id}`, f])));
  const minutes = (entry: WorkspaceEntry) => entry.end ? Math.round((Date.parse(entry.end) - Date.parse(entry.start)) / 60000) : 0;
  function title(key: string) {
    const labels: Record<string,string> = { no: "#", date: wt("Date","Tanggal"), project: wt("Project","Project"), start: wt("Start","Mulai"), end: wt("End","Selesai"), description: wt("Description","Deskripsi"), hours: wt("Duration","Durasi") };
    const field = fieldMap.get(key);
    return labels[key] || (field ? `${projectMap.get(field.projectId)?.name} / ${field.name}` : key);
  }
  function value(entry: WorkspaceEntry, key: string, index: number) {
    if (key === "no") return String(offset + index + 1);
    if (key === "date") return dayLabel(entry.start.slice(0,10));
    if (key === "start" || key === "end") return (entry[key] || "").slice(11,16);
    if (key === "hours") return duration(minutes(entry));
    const v = entry.fields[key.slice(6)];
    return v == null ? "" : typeof v === "boolean" ? (v ? wt("Yes","Ya") : wt("No","Tidak")) : String(v);
  }
  const newDay = (index: number) => index === 0 || entries[index - 1].start.slice(0,10) !== entries[index].start.slice(0,10);
</script>

<div class="table-scroll">
  <table>
    <thead><tr>{#each columns as key}<th class:description={key === "description"} class:custom={key.startsWith("field:")} class:numeric={key === "hours" || key === "start" || key === "end"}>{title(key)}</th>{/each}<th class="action-head"><span class="sr-only">{wt("Edit entry","Edit catatan")}</span></th></tr></thead>
    <tbody>
      {#each entries as entry, index (entry.id)}
        {#if dayTotals && newDay(index)}
          {@const date = entry.start.slice(0,10)}
          <tr class="day-row"><th colspan={columns.length + 1} scope="rowgroup"><span class="day-label">{dayLabel(date)}<strong>{duration(dayTotals.get(date) ?? 0)}</strong></span></th></tr>
        {/if}
        <tr>
          {#each columns as key}
            <td class:description={key === "description"} class:nowrap={key === "date" || key === "project"} class:numeric={key === "hours" || key === "start" || key === "end"} class:strong={key === "hours"}>
              {#if key === "project"}<span class="project-tag"><span class="dot" style:background={projectMap.get(entry.projectId)?.color}></span>{projectMap.get(entry.projectId)?.name ?? ""}</span>
              {:else if key === "description"}<button class="entry-title" class:untitled={!entry.description} onclick={() => onEdit(entry)}>{entry.description || wt("No description","Tanpa deskripsi")}</button>
              {:else if fieldMap.get(key)?.kind === "select" && value(entry,key,index)}<span class="field-tag">{value(entry,key,index)}</span>
              {:else if fieldMap.get(key)?.kind === "url" && value(entry,key,index)}<span class="url">{value(entry,key,index).replace(/^https?:\/\//, "")}</span>
              {:else}{value(entry,key,index)}{/if}
            </td>
          {/each}
          <td class="action"><button class="btn quiet icon" onclick={() => onEdit(entry)} aria-label={wt("Edit entry","Edit catatan")}><Icon name="right" size={15} /></button></td>
        </tr>
      {:else}
        <tr><td colspan={columns.length + 1}><div class="empty"><Icon name="table" size={24} /><strong>{wt("No entries match this view","Belum ada catatan yang cocok")}</strong><span>{wt("Try another period or project, or log time for this one.","Coba periode atau project lain, atau catat waktu untuk periode ini.")}</span>{#if onAdd}<button class="btn small" onclick={onAdd}><Icon name="plus" size={14} />{wt("Log time","Catat waktu")}</button>{/if}</div></td></tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .table-scroll { overflow: auto; border: 1px solid var(--separator); border-radius: 12px; background: var(--surface); }
  table { width: 100%; border-collapse: collapse; text-align: left; font-size: 12px; }
  thead th { position: sticky; top: 0; z-index: 1; font-weight: 500; color: var(--secondary); font-size: 11px; background: var(--field); white-space: nowrap; padding: 10px 14px; border-bottom: 1px solid var(--separator); }
  td { padding: 10px 14px; border-bottom: 1px solid var(--separator); max-width: 280px; overflow-wrap: anywhere; color: var(--secondary); vertical-align: top; }
  tbody tr:last-child td { border-bottom: 0; }
  tbody tr:not(.day-row):hover { background: var(--hover); }
  th.description { min-width: 220px; width: 40%; }
  th.custom { min-width: 104px; }
  .day-row th { padding: 14px 14px 7px; border-bottom: 1px solid var(--separator); background: var(--surface); font-size: 11px; font-weight: 600; color: var(--label); }
  .day-label { position: sticky; left: 14px; display: inline-flex; gap: 10px; }
  .day-label strong { font-weight: 500; color: var(--secondary); font-variant-numeric: tabular-nums; }
  .entry-title { padding: 0; border: 0; background: none; text-align: left; color: var(--label); line-height: 1.5; }
  .entry-title:hover { color: var(--accent-text); }
  .entry-title.untitled { color: var(--tertiary); font-style: italic; }
  .nowrap { white-space: nowrap; }
  .numeric { min-width: 64px; font-variant-numeric: tabular-nums; white-space: nowrap; }
  th.numeric, td.numeric { text-align: right; }
  .strong { color: var(--label); font-weight: 550; }
  .project-tag { display: inline-flex; align-items: center; gap: 7px; color: var(--label); }
  .dot { width: 7px; height: 7px; border-radius: 50%; flex-shrink: 0; }
  .field-tag { display: inline-block; white-space: nowrap; padding: 2px 7px; border-radius: 5px; background: var(--fill); color: var(--label); font-size: 11px; }
  .url { color: var(--accent-text); }
  .action, .action-head { padding: 4px 8px; width: 36px; vertical-align: middle; }
  .empty { padding: 44px 16px; display: flex; flex-direction: column; align-items: center; gap: 8px; text-align: center; color: var(--tertiary); }
  .empty strong { color: var(--label); font-weight: 550; }
  .empty span { max-width: 340px; color: var(--secondary); }
  .empty .btn { margin-top: 6px; }
  .sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); }
</style>
