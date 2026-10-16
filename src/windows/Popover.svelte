<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { api, hhmm, isoDate, minutesBetween, nextDay, type DayView, type Entry, type KalaError, type Project } from "../lib/api";
  import { dayLabel, duration, errorText, setLang, t } from "../lib/i18n.svelte";
  import QuickInput from "../lib/QuickInput.svelte";

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

  const projectOf = (id: number) => projects.find((p) => p.id === id);
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
      error = null;
    } catch (e) {
      error = e as KalaError;
    }
    await load();
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
  <QuickInput bind:this={quick} autofocus onDone={load} onCancel={() => api.hideWindow("popover")} />

  {#if running}
    {@const p = projectOf(running.projectId)}
    <div class="running">
      <span class="dot" style:background={p?.color}></span>
      <div class="col">
        <span class="name">{p?.name}</span>
        <span class="sub">{running.description || t("noDescription")}</span>
      </div>
      <span class="time">{Math.floor(elapsed / 60)}:{String(elapsed % 60).padStart(2, "0")}</span>
      <button class="btn" onclick={stop}>■ {t("stop")}</button>
      <button class="btn danger" onclick={discard} aria-label={t("discard")}>{confirmDiscard ? t("confirmDiscard") : "✕"}</button>
    </div>
  {/if}

  <div class="section">{t("projects")}</div>
  <div class="list">
    {#each projects.filter((p) => !p.archived) as p, i (p.id)}
      <button class="row" onclick={() => startTimer(p)} aria-label={`${t("timer")} ${p.name}`}>
        <span class="dot" style:background={p.color}></span>
        <span class="name">{p.name}</span>
        {#if running?.projectId === p.id}<span class="sub">▶</span>{/if}
        {#if i < 9}<span class="kbd">⌘{i + 1}</span>{/if}
      </button>
    {/each}
  </div>

  <div class="daynav">
    <button class="icon" onclick={() => shiftDay(-1)} aria-label={t("prevDay")}>◀</button>
    <span class="name">{dayLabel(day)}</span>
    <button class="icon" onclick={() => shiftDay(1)} aria-label={t("nextDay")}>▶</button>
    {#if day !== isoDate(new Date())}
      <button class="btn small" onclick={() => { day = isoDate(new Date()); load(); }}>{t("today")}</button>
    {/if}
  </div>

  <div class="entries">
    {#each view?.entries ?? [] as e (e.id)}
      {#if edit?.id === e.id}
        <div class="edit">
          <select bind:value={edit.projectId}>
            {#each projects.filter((p) => !p.archived || p.id === e.projectId) as p (p.id)}
              <option value={p.id}>{p.name}</option>
            {/each}
          </select>
          <div class="times">
            <input type="date" bind:value={edit.date} />
            <input type="time" bind:value={edit.start} />
            <span>–</span>
            <input type="time" bind:value={edit.end} />
          </div>
          <!-- svelte-ignore a11y_autofocus -->
          <input bind:value={edit.description} placeholder={t("describeIt")} autofocus={!edit.description} onkeydown={(ev) => ev.key === "Enter" && saveEdit()} />
          <div class="actions">
            {#if confirmDelete === e.id}
              <button class="btn danger" onclick={() => act(async () => { await api.deleteEntry(e.id); edit = null; confirmDelete = null; })}>{t("confirmDelete")}</button>
            {:else}
              <button class="btn danger" onclick={() => (confirmDelete = e.id)}>{t("delete")}</button>
            {/if}
            <span class="spacer"></span>
            <button class="btn" onclick={() => (edit = null)}>{t("cancel")}</button>
            <button class="btn primary" onclick={saveEdit}>{t("save")}</button>
          </div>
        </div>
      {:else}
        <button class="row entry" onclick={() => openEdit(e)}>
          <span class="dot" style:background={projectOf(e.projectId)?.color}></span>
          <span class="time">{hhmm(e.start)}–{hhmm(e.end!)}</span>
          <span class="desc">{e.description || t("noDescription")}</span>
          <span class="sub">{duration(minutesBetween(e.start, e.end!))}</span>
        </button>
      {/if}
    {:else}
      <div class="empty">{t("noEntries")}</div>
    {/each}
  </div>

  {#if exporting}
    <div class="export">
      <div class="times">
        <select bind:value={exportProject} aria-label={t("projects")}>
          {#each projects as p (p.id)}<option value={p.id}>{p.name}{p.archived ? ` ${t("archivedTag")}` : ""}</option>{/each}
        </select>
        <input type="month" bind:value={exportMonth} aria-label={t("month")} />
        <select bind:value={exportFormat} aria-label={t("format")}>
          <option value="xlsx">Excel</option>
          <option value="csv">CSV</option>
        </select>
      </div>
      <button class="btn primary" onclick={runExport}>{t("exportDo")}</button>
      {#if exported}
        <div class="done">
          <span>✓ {t("exported", { file: exported.path.split("/").pop(), rows: exported.rows, hours: exported.totalHours })}</span>
          <button class="link" onclick={() => api.revealPath(exported!.path)}>{t("showInFinder")}</button>
        </div>
      {/if}
    </div>
  {/if}

  {#if error && errorText(error)}<div class="error" role="alert">{errorText(error)}</div>{/if}

  <div class="footer">
    <span>{t("total")} <b>{duration(view?.totalMinutes ?? 0)}</b></span>
    <span class="spacer"></span>
    <button class="btn" onclick={() => { exporting = !exporting; exported = null; exportProject ??= projects.find((p) => !p.archived)?.id ?? null; }}>{t("export")}</button>
    <button class="btn" onclick={() => api.openSettings()}>{t("settings")}</button>
    <button class="btn" onclick={() => api.quitApp()}>{t("quit")}</button>
  </div>
</div>

<style>
  .pop { display: flex; flex-direction: column; height: 100vh; box-sizing: border-box; padding: 10px; gap: 4px; }
  .running {
    display: flex; align-items: center; gap: 8px; margin-top: 6px; padding: 7px 8px;
    border-radius: 7px; background: color-mix(in srgb, var(--accent) 18%, transparent);
  }
  .col { display: flex; flex-direction: column; min-width: 0; flex: 1; }
  .section { font-size: 11px; font-weight: 600; color: var(--secondary); padding: 8px 6px 2px; }
  .list { display: flex; flex-direction: column; }
  .row {
    display: flex; align-items: center; gap: 8px; width: 100%; padding: 5px 6px; border: 0; border-radius: 6px;
    background: transparent; color: var(--label); font: inherit; text-align: left;
  }
  .row:hover { background: var(--fill); }
  .row:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .name { font-weight: 500; }
  .sub { color: var(--secondary); font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .kbd { margin-left: auto; color: var(--tertiary); font-size: 11px; }
  .time { font-variant-numeric: tabular-nums; color: var(--secondary); font-size: 12px; white-space: nowrap; }
  .desc { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .entry .sub { margin-left: auto; }
  .dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
  .daynav { display: flex; align-items: center; gap: 6px; padding: 10px 2px 2px; border-top: 1px solid var(--separator); margin-top: 6px; }
  .icon { border: 0; background: transparent; color: var(--secondary); font-size: 11px; padding: 3px 6px; border-radius: 5px; }
  .icon:hover { background: var(--fill); }
  .entries { flex: 1; overflow-y: auto; display: flex; flex-direction: column; }
  .empty { color: var(--tertiary); padding: 14px 6px; font-size: 12px; }
  .edit { display: flex; flex-direction: column; gap: 6px; padding: 8px; border-radius: 7px; background: var(--fill); }
  .times { display: flex; gap: 4px; align-items: center; }
  .actions { display: flex; gap: 6px; }
  .spacer { flex: 1; }
  .error { color: var(--red); font-size: 12px; padding: 4px 6px; }
  .footer { display: flex; align-items: center; gap: 6px; padding-top: 8px; border-top: 1px solid var(--separator); font-size: 12px; color: var(--secondary); }
  .footer b { color: var(--label); font-weight: 600; }
  .export { display: flex; flex-direction: column; gap: 6px; padding: 8px; border-radius: 7px; background: var(--fill); margin-top: 4px; }
  .export .times select, .export .times input { flex: 1; min-width: 0; }
  .done { display: flex; flex-wrap: wrap; gap: 6px; font-size: 12px; color: var(--secondary); }
</style>
