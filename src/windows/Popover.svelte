<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { api, hhmm, isoDate, minutesBetween, nextDay, type DayView, type Entry, type KalaError, type Project } from "../lib/api";
  import { dayLabel, duration, errorText, setLang, t } from "../lib/i18n.svelte";
  import Icon from "../lib/Icon.svelte";
  import QuickInput from "../lib/QuickInput.svelte";
  import DatePicker from "../lib/ui/DatePicker.svelte";
  import TimeInput from "../lib/ui/TimeInput.svelte";
  import { wapi } from "../lib/workspace-api";

  let projects = $state<Project[]>([]);
  let running = $state<Entry | null>(null);
  let day = $state(isoDate(new Date()));
  let view = $state<DayView | null>(null);
  let nowMs = $state(Date.now());
  let error = $state<KalaError | null>(null);
  let confirmDiscard = $state(false);
  let confirmDelete = $state<number | null>(null);
  let edit = $state<{ id: number; projectId: number; date: string; start: string; end: string; description: string } | null>(null);
  let quick: QuickInput;
  let exporting = $state(false);
  let exportProject = $state<number | null>(null);
  let exportMonth = $state(isoDate(new Date()).slice(0, 7));
  let exportFormat = $state<"xlsx" | "csv">("xlsx");
  let exported = $state<{ path: string; rows: number; totalHours: number } | null>(null);

  async function runExport() {
    const pid = exportProject ?? projects.find((p) => !p.archived)?.id;
    if (pid == null) return;
    try {
      exported = await api.exportMonth(pid, exportMonth, exportFormat);
      error = null;
    } catch (e) {
      error = e as KalaError;
    }
  }

  const activeProjects = $derived(projects.filter((p) => !p.archived));
  const projectById = $derived(new Map(projects.map((p) => [p.id, p])));
  const projectOf = (id: number) => projectById.get(id);
  const elapsed = $derived(running ? Math.max(0, Math.floor((nowMs - Date.parse(running.start)) / 60000)) : 0);

  async function load() {
    try {
      const [ps, run, dv, settings] = await Promise.all([api.listProjects(true), api.timerStatus(), api.listDay(day), api.getSettings()]);
      setLang(settings.language);
      projects = ps;
      running = run;
      view = dv;
      error = null;
    } catch (e) {
      error = e as KalaError;
    }
  }

  async function act(f: () => Promise<unknown>) {
    try {
      await f();
      await load();
    } catch (e) {
      error = e as KalaError;
    }
  }

  function shiftDay(delta: number) {
    const [y, m, d] = day.split("-").map(Number);
    day = isoDate(new Date(y, m - 1, d + delta));
    edit = null;
    load();
  }

  function startTimer(p: Project) {
    if (running?.projectId === p.id) return;
    act(() => api.timerStart(p.id, ""));
  }

  async function stop() {
    if (!running) return;
    try {
      const saved = await api.timerStop(null);
      day = saved.start.slice(0, 10);
      await load();
      if (!saved.description) openEdit(saved);
    } catch (e) {
      error = e as KalaError;
    }
  }

  function discard() {
    if (!running) return;
    if (!confirmDiscard) {
      confirmDiscard = true;
      return;
    }
    const id = running.id;
    act(async () => {
      await api.deleteEntry(id);
      confirmDiscard = false;
    });
  }

  function openEdit(e: Entry) {
    confirmDelete = null;
    edit = {
      id: e.id,
      projectId: e.projectId,
      date: e.start.slice(0, 10),
      start: e.start.slice(11, 16),
      end: (e.end ?? e.start).slice(11, 16),
      description: e.description,
    };
  }

  function saveEdit() {
    if (!edit) return;
    const { id, projectId, date, start, end, description } = edit;
    act(async () => {
      const endDate = end <= start ? nextDay(date) : date;
      await api.updateEntry(id, { projectId, start: `${date}T${start}:00`, end: `${endDate}T${end}:00`, description });
      edit = null;
    });
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.metaKey && /^[1-9]$/.test(e.key)) {
      const p = projects.filter((p) => !p.archived)[Number(e.key) - 1];
      if (p) {
        e.preventDefault();
        startTimer(p);
      }
    } else if (e.metaKey && e.key === ",") {
      e.preventDefault();
      api.openSettings();
    } else if (e.key === "Escape" && edit) {
      edit = null;
    } else if (e.key === "Escape" && !(e.target instanceof HTMLInputElement)) {
      api.hideWindow("popover");
    }
  }

  onMount(() => {
    load();
    const tick = setInterval(() => (nowMs = Date.now()), 15000);
    const un = listen("kala://refresh", () => {
      day = isoDate(new Date());
      edit = null;
      confirmDelete = null;
      confirmDiscard = false;
      nowMs = Date.now();
      load();
      quick?.focus();
    });
    return () => {
      clearInterval(tick);
      un.then((f) => f());
    };
  });
</script>

<svelte:window {onkeydown} />

<div class="pop">
  <header class="brand"><span class="brand-name"><Icon name="kala" size={22} />Kala</span><button class="btn quiet small" onclick={() => wapi.open()}>{t("openWorkspace")}<Icon name="external" size={13} /></button></header>
  <div class="capture">
    <QuickInput bind:this={quick} autofocus exampleProject={activeProjects[0]?.aliases[0] || activeProjects[0]?.name.toLowerCase() || "project"} onDone={load} onCancel={() => api.hideWindow("popover")} />
    <p class="hint">{t("quickHint")}</p>
  </div>

  {#if running}
    {@const p = projectOf(running.projectId)}
    <div class="running">
      <span class="dot" style:background={p?.color}></span>
      <div class="col"><span class="name">{p?.name}</span><span class="sub">{running.description || t("timer")}</span></div>
      <span class="elapsed">{Math.floor(elapsed / 60)}:{String(elapsed % 60).padStart(2, "0")}</span>
      <button class="btn primary small" onclick={stop}><Icon name="stop" size={13} />{t("stop")}</button>
      <button class="btn quiet small danger" onclick={discard} aria-label={t("discard")} title={t("discard")}>{#if confirmDiscard}{t("confirmDiscard")}{:else}<Icon name="close" size={14} />{/if}</button>
    </div>
  {/if}

  <div class="project-section" hidden={!!edit || exporting}>
    <div class="section-heading"><span>{t("projects")}</span><span class="hint">{t("startHint")}</span></div>
    <div class="project-list">
      {#each activeProjects as p, i (p.id)}
        <button class="project-button" class:active={running?.projectId === p.id} onclick={() => startTimer(p)} aria-label={`${t("timer")} ${p.name}`} aria-pressed={running?.projectId === p.id}>
          <span class="dot" style:background={p.color}></span><span class="name">{p.name}</span>
          {#if i < 9}<span class="kbd">⌘{i + 1}</span>{/if}
          <span class="play"><Icon name={running?.projectId === p.id ? "clock" : "play"} size={13} /></span>
        </button>
      {:else}
        <button class="setup-projects" onclick={() => api.openSettings()}><Icon name="plus" />{t("noProjectsHint")}</button>
      {/each}
    </div>
  </div>

  <div class="daynav" hidden={exporting}>
    <div class="day-title"><span class="name">{day === isoDate(new Date()) ? t("today") : t("timeEntries")}</span><span class="sub">{dayLabel(day)}</span></div>
    <div class="day-buttons">
      {#if day !== isoDate(new Date())}<button class="btn quiet small" onclick={() => { day = isoDate(new Date()); edit = null; load(); }}>{t("today")}</button>{/if}
      <button class="btn quiet icon" onclick={() => shiftDay(-1)} aria-label={t("prevDay")}><Icon name="left" /></button>
      <button class="btn quiet icon" onclick={() => shiftDay(1)} aria-label={t("nextDay")}><Icon name="right" /></button>
    </div>
  </div>

  <div class="entries" hidden={exporting}>
    {#each view?.entries ?? [] as e (e.id)}
      {#if edit?.id === e.id}
        <div class="edit">
          <div class="editor-heading"><span>{t("editEntry")}</span><button class="btn quiet icon" onclick={() => edit = null} aria-label={t("cancel")}><Icon name="close" size={14} /></button></div>
          <div class="field-grid">
            <label>{t("projects")}<select bind:value={edit.projectId}>
              {#each projects.filter((p) => !p.archived || p.id === e.projectId) as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
            </select></label>
            <label for="pop-date">{t("date")}<DatePicker id="pop-date" compact bind:value={edit.date} /></label>
            <label>{t("startTime")}<TimeInput bind:value={edit.start} /></label>
            <label>{t("endTime")}<TimeInput bind:value={edit.end} /></label>
          </div>
          <label>{t("description")}
            <!-- svelte-ignore a11y_autofocus -->
            <textarea rows="2" bind:value={edit.description} autofocus={!edit.description} placeholder={t("whatDid")} onkeydown={(ev) => { if (ev.key === "Enter" && !ev.shiftKey) { ev.preventDefault(); saveEdit(); } }}></textarea>
          </label>
          <div class="actions">
            {#if confirmDelete === e.id}
              <button class="btn quiet danger small" onclick={() => act(async () => { await api.deleteEntry(e.id); edit = null; confirmDelete = null; })}>{t("confirmDelete")}</button>
            {:else}
              <button class="btn quiet danger small" onclick={() => (confirmDelete = e.id)}><Icon name="trash" size={14} />{t("delete")}</button>
            {/if}
            <span class="spacer"></span><button class="btn small" onclick={() => (edit = null)}>{t("cancel")}</button><button class="btn primary small" onclick={saveEdit}>{t("save")}</button>
          </div>
        </div>
      {:else}
        <button class="entry" onclick={() => openEdit(e)}>
          <span class="entry-details"><span class="dot" style:background={projectOf(e.projectId)?.color}></span><span class="project-name">{projectOf(e.projectId)?.name}</span><span class="entry-time">{hhmm(e.start)}–{hhmm(e.end!)}</span><span class="duration">{duration(minutesBetween(e.start, e.end!))}</span></span>
          <span class="desc">{e.description || t("noDescription")}</span>
        </button>
      {/if}
    {:else}
      <div class="empty"><span class="empty-icon"><Icon name="clock" size={26} /></span><span class="name">{t("noEntries")}</span><p>{t("noEntriesHint")}</p></div>
    {/each}
  </div>

  {#if exporting}
    <div class="export">
      <div class="editor-heading"><span>{t("export")}</span><button class="btn quiet icon" onclick={() => exporting = false} aria-label={t("cancel")}><Icon name="close" size={14} /></button></div>
      <label>{t("projects")}<select bind:value={exportProject}>{#each projects as p (p.id)}<option value={p.id}>{p.name}{p.archived ? ` ${t("archivedTag")}` : ""}</option>{/each}</select></label>
      <div class="field-grid"><label>{t("month")}<input type="month" bind:value={exportMonth} /></label><label>{t("format")}<select bind:value={exportFormat}><option value="xlsx">Excel (.xlsx)</option><option value="csv">CSV (.csv)</option></select></label></div>
      <button class="btn primary" onclick={runExport} disabled={!projects.length}><Icon name="download" size={14} />{t("exportDo")}</button>
      {#if exported}<div class="done"><span>{t("exported", { file: exported.path.split("/").pop(), rows: exported.rows, hours: exported.totalHours })}</span><button class="link" onclick={() => api.revealPath(exported!.path)}>{t("showInFinder")}</button></div>{/if}
    </div>
  {/if}
  {#if error && errorText(error)}<div class="error" role="alert">{errorText(error)}</div>{/if}

  <footer>
    <div class="total"><span>{t("total")}</span><strong>{duration(view?.totalMinutes ?? 0)}</strong></div><span class="spacer"></span>
    <button class="btn small" class:selected={exporting} aria-expanded={exporting} onclick={() => { exporting = !exporting; exported = null; exportProject ??= activeProjects[0]?.id ?? null; }}><Icon name="download" size={14} />{t("export")}</button>
    <button class="btn quiet icon" onclick={() => api.openSettings()} aria-label={t("settings")} title={t("settings")}><Icon name="settings" /></button>
    <button class="btn quiet icon" onclick={() => api.quitApp()} aria-label={t("quit")} title={t("quit")}><Icon name="power" size={15} /></button>
  </footer>
</div>

<style>
  .pop { display: flex; flex-direction: column; height: 100vh; height: 100dvh; padding: 16px 16px 12px; gap: 12px; background: var(--background); border: 1px solid var(--separator); border-radius: 12px; overflow: hidden; }
  .brand { display: flex; align-items: center; justify-content: space-between; color: var(--secondary); font-size: 12px; }
  .brand-name { display: flex; align-items: center; gap: 8px; font-size: 16px; font-weight: 650; letter-spacing: -.4px; color: var(--label); }
  .brand-name :global(.mark) { color: var(--accent-text); }
  .capture { display: flex; flex-direction: column; gap: 7px; }
  .hint { font-size: 11px; color: var(--tertiary); margin: 0; }
  .running { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; padding: 11px; border: 1px solid color-mix(in srgb, var(--accent) 45%, var(--separator)); border-radius: 10px; background: var(--accent-soft); }
  .col { display: flex; flex-direction: column; min-width: 50px; flex: 1; }
  .name { font-weight: 600; overflow-wrap: anywhere; }
  .sub { color: var(--secondary); font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .elapsed { font-size: 19px; font-weight: 550; letter-spacing: -.5px; font-variant-numeric: tabular-nums; }
  .section-heading { display: flex; justify-content: space-between; align-items: baseline; gap: 8px; font-size: 12px; font-weight: 600; margin-bottom: 8px; }
  .section-heading .hint { font-weight: 400; }
  .project-section { flex-shrink: 0; }
  .project-section[hidden], .daynav[hidden], .entries[hidden] { display: none; }
  .project-list { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 6px; max-height: 124px; overflow-y: auto; padding: 2px; margin: -2px; }
  .project-button { display: flex; align-items: center; gap: 7px; padding: 9px 10px; background: var(--surface); border: 1px solid var(--separator); border-radius: 9px; text-align: left; min-width: 0; transition: background 120ms; }
  .project-button:hover { background: var(--hover); }
  .project-button.active { background: var(--accent-soft); border-color: var(--accent); }
  .project-button .name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .kbd { color: var(--tertiary); font-size: 10px; }
  .play { display: flex; color: var(--secondary); }
  .setup-projects { grid-column: 1 / -1; padding: 10px; display: flex; gap: 8px; border: 1px dashed var(--separator); background: transparent; color: var(--secondary); border-radius: 8px; text-align: left; }
  .dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
  .daynav { display: flex; align-items: center; gap: 8px; border-top: 1px solid var(--separator); padding-top: 12px; }
  .day-title { display: flex; flex-direction: column; gap: 1px; }
  .day-buttons { margin-left: auto; display: flex; align-items: center; gap: 2px; }
  .entries { flex: 1; min-height: 52px; overflow-y: auto; margin: -4px -4px 0; padding: 4px; }
  .entry { display: flex; flex-direction: column; gap: 6px; width: 100%; padding: 12px 10px; border: 0; border-bottom: 1px solid var(--separator); border-radius: 8px 8px 0 0; background: transparent; color: var(--label); text-align: left; transition: background 120ms; }
  .entry:hover { background: var(--hover); }
  .entry-details { display: flex; align-items: center; gap: 7px; width: 100%; font-size: 11px; color: var(--secondary); }
  .project-name { font-weight: 600; max-width: 95px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .entry-time { font-variant-numeric: tabular-nums; white-space: nowrap; }
  .duration { margin-left: auto; color: var(--label); font-size: 11px; font-weight: 550; padding: 2px 7px; background: var(--fill); border-radius: 5px; white-space: nowrap; }
  .desc { line-height: 1.5; overflow-wrap: anywhere; }
  .empty { height: 100%; min-height: 130px; display: flex; flex-direction: column; justify-content: center; align-items: center; gap: 8px; color: var(--secondary); text-align: center; }
  .empty-icon { display: grid; place-items: center; width: 48px; height: 48px; background: var(--surface); border-radius: 14px; color: var(--tertiary); margin-bottom: 4px; }
  .empty p { font-size: 12px; max-width: 240px; margin: 0; }
  .edit, .export { display: flex; flex-direction: column; gap: 10px; padding: 12px; border-radius: 11px; background: var(--surface); border: 1px solid var(--separator); }
  .editor-heading { display: flex; align-items: center; justify-content: space-between; font-weight: 600; margin: -5px -5px -4px 0; }
  .field-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; }
  label { display: flex; flex-direction: column; gap: 5px; color: var(--secondary); font-size: 11px; }
  label :global(input), label :global(.date-field), label select, label textarea { color: var(--label); font-size: 12px; }
  .actions { position: sticky; bottom: -12px; display: flex; align-items: center; gap: 6px; padding: 8px 0 2px; background: var(--surface); }
  .actions .danger { color: var(--red); }
  .spacer { flex: 1; }
  .error { color: var(--red); font-size: 12px; overflow-y: auto; max-height: 65px; }
  footer { display: flex; align-items: center; gap: 4px; padding-top: 10px; border-top: 1px solid var(--separator); }
  .total { display: flex; align-items: baseline; gap: 8px; color: var(--secondary); font-size: 12px; }
  .total strong { font-size: 20px; font-weight: 600; letter-spacing: -.5px; color: var(--label); }
  .selected { background: var(--accent-soft); color: var(--accent-text); }
  .export { flex: 1; min-height: 0; overflow-y: auto; }
  .done { display: flex; flex-direction: column; align-items: flex-start; gap: 6px; font-size: 12px; color: var(--secondary); overflow-wrap: anywhere; }
</style>
