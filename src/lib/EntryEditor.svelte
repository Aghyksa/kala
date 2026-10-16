<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { api, isoDate, pad, type Project } from "./api";
  import { duration, errorText } from "./i18n.svelte";
  import { wapi, type FieldDefinition, type FieldValue, type WorkspaceEntry } from "./workspace-api";
  import { wt } from "./workspace-i18n.svelte";
  import Icon from "./Icon.svelte";
  import DatePicker from "./ui/DatePicker.svelte";
  import TimeInput from "./ui/TimeInput.svelte";

  let { entry, projects, fields, defaultProjectId, onSaved, onClose }: {
    entry: WorkspaceEntry | null;
    projects: Project[];
    fields: FieldDefinition[];
    defaultProjectId: number | null;
    onSaved: () => void;
    onClose: () => void;
  } = $props();

  const now = new Date();
  const hourAgo = new Date(now.getTime() - 60 * 60 * 1000);
  const time = (date: Date) => `${pad(date.getHours())}:${pad(date.getMinutes())}`;
  let projectId = $state<number | null>(untrack(() => entry?.projectId ?? defaultProjectId ?? projects.find((project) => !project.archived)?.id ?? null));
  let date = $state(untrack(() => entry?.start.slice(0, 10) ?? isoDate(hourAgo)));
  let endDate = $state(untrack(() => entry?.end?.slice(0, 10) ?? isoDate(now)));
  let start = $state(untrack(() => entry?.start.slice(11, 16) ?? time(hourAgo)));
  let end = $state(untrack(() => entry?.end?.slice(11, 16) ?? time(now)));
  let description = $state(untrack(() => entry?.description ?? ""));
  let values = $state<Record<string, FieldValue>>(untrack(() => ({ ...entry?.fields })));
  let pending = $state(false);
  let error = $state("");
  let confirmDelete = $state(false);
  let dialog: HTMLDialogElement;
  let nameInput: HTMLSelectElement;
  const projectOptions = $derived(projects.filter((project) => !project.archived || project.id === entry?.projectId));
  const visibleFields = $derived(fields.filter((field) => field.projectId === projectId && !field.archived).sort((a, b) => a.position - b.position || a.id - b.id));
  const archivedFields = $derived(fields.filter((field) => field.projectId === projectId && field.archived && values[String(field.id)] != null));
  const entryMinutes = $derived(date && endDate && start && end
    ? (Date.parse(`${endDate}T${end}:00Z`) - Date.parse(`${date}T${start}:00Z`)) / 60000 : 0);
  const timeProblem = $derived(entryMinutes <= 0 ? wt("End must be after start.", "Selesai harus setelah mulai.")
    : entryMinutes > 1440 ? wt("One entry can be at most 24 hours.", "Satu catatan maksimal 24 jam.") : "");
  let lastStartDate = untrack(() => date);

  // Moving the start date carries a same-day end date along with it.
  function startDateChanged(next: string) {
    if (endDate === lastStartDate) endDate = next;
    lastStartDate = next;
  }

  onMount(() => {
    const previousFocus = document.activeElement as HTMLElement | null;
    dialog.showModal();
    nameInput.focus();
    return () => previousFocus?.focus();
  });

  function changeProject() {
    values = projectId === entry?.projectId ? { ...entry.fields } : {};
    confirmDelete = false;
  }

  function close() {
    if (!pending) onClose();
  }

  async function save(event: SubmitEvent) {
    event.preventDefault();
    if (pending) return;
    error = "";
    if (projectId == null) {
      error = wt("Choose a project first.", "Pilih project dulu.");
      return;
    }
    if (timeProblem) {
      error = timeProblem;
      return;
    }
    const fieldValues: Record<string, FieldValue> = {};
    for (const field of fields.filter((field) => field.projectId === projectId)) {
      const value = values[String(field.id)];
      if (value == null || value === "") continue;
      if (field.kind === "number") {
        const number = Number(value);
        if (!Number.isFinite(number)) {
          error = wt(`Enter a valid number for ${field.name}.`, `Masukkan angka yang valid untuk ${field.name}.`);
          return;
        }
        fieldValues[String(field.id)] = number;
      } else {
        fieldValues[String(field.id)] = value;
      }
    }
    pending = true;
    try {
      const startValue = `${date}T${start}`;
      const endValue = `${endDate}T${end}`;
      await wapi.saveEntry({
        id: entry?.id ?? null, projectId, description: description.trim(),
        start: entry && entry.start.slice(0, 16) === startValue ? entry.start : `${startValue}:00`,
        end: entry?.end && entry.end.slice(0, 16) === endValue ? entry.end : `${endValue}:00`, fields: fieldValues,
      });
      onSaved();
      onClose();
    } catch (err) {
      error = errorText(err);
    } finally {
      pending = false;
    }
  }

  async function remove() {
    if (!entry || pending) return;
    if (!confirmDelete) {
      confirmDelete = true;
      return;
    }
    pending = true;
    error = "";
    try {
      await api.deleteEntry(entry.id);
      onSaved();
      onClose();
    } catch (err) {
      error = errorText(err);
    } finally {
      pending = false;
    }
  }
</script>

<dialog bind:this={dialog} class="entry-sheet" aria-labelledby="entry-editor-title" oncancel={(event) => { event.preventDefault(); close(); }} onclick={(event) => { if (event.target === dialog) close(); }} onkeydown={(event) => { if (event.key === "Escape") { event.preventDefault(); close(); } }}>
  <form onsubmit={save}>
    <header>
      <div><h2 id="entry-editor-title">{entry ? wt("Edit entry", "Edit catatan") : wt("Add time entry", "Tambah catatan waktu")}</h2></div>
      <button type="button" class="btn quiet icon" onclick={close} disabled={pending} aria-label={wt("Close editor", "Tutup editor")}><Icon name="close" size={18} /></button>
    </header>
    <div class="sheet-body">
      <section class="form-section">
        <label for="entry-project">{wt("Project", "Project")}</label>
        <select id="entry-project" bind:this={nameInput} bind:value={projectId} onchange={changeProject} required disabled={pending || !projectOptions.length}>
          {#if projectId == null}<option value={null} disabled>{wt("Choose a project", "Pilih project")}</option>{/if}
          {#each projectOptions as project (project.id)}<option value={project.id}>{project.name}{project.archived ? wt(" (archived)", " (arsip)") : ""}</option>{/each}
        </select>
        {#if !projectOptions.length}<p class="muted">{wt("Add a project in Projects before creating an entry.", "Tambahkan project di menu Project sebelum membuat catatan.")}</p>{/if}
        <label for="entry-description">{wt("Description", "Deskripsi")}</label>
        <textarea id="entry-description" bind:value={description} rows="3" placeholder={wt("What did you work on?", "Ngerjain apa?")} disabled={pending}></textarea>
      </section>
      <section class="form-section">
        <h3>{wt("Time", "Waktu")}</h3>
        <div class="time-row">
          <label for="entry-date">{wt("Start", "Mulai")}</label>
          <DatePicker id="entry-date" bind:value={date} disabled={pending} onchange={startDateChanged} />
          <TimeInput id="entry-start" bind:value={start} required disabled={pending} />
        </div>
        <div class="time-row">
          <label for="entry-end-date">{wt("End", "Selesai")}</label>
          <DatePicker id="entry-end-date" bind:value={endDate} disabled={pending} />
          <TimeInput id="entry-end" bind:value={end} required disabled={pending} />
        </div>
        <div class="duration" class:invalid={!!timeProblem} aria-live="polite"><Icon name="clock" />{#if timeProblem}<span>{timeProblem}</span>{:else}<strong>{duration(entryMinutes)}</strong><span>{wt("Duration", "Durasi")}</span>{/if}</div>
      </section>
      {#if visibleFields.length}
        <section class="form-section custom-fields">
          <div class="section-title"><h3>{wt("Project fields", "Field project")}</h3><span class="muted">{wt("Optional", "Opsional")}</span></div>
          {#each visibleFields as field (field.id)}
            {@const key = String(field.id)}
            {#if field.kind === "checkbox"}
              <label class="check-field" for={`entry-field-${key}`}><input id={`entry-field-${key}`} type="checkbox" checked={values[key] === true} onchange={(event) => { values[key] = event.currentTarget.checked; }} disabled={pending} /><span>{field.name}</span></label>
            {:else}
              <label for={`entry-field-${key}`}>{field.name}</label>
              {#if field.kind === "select"}
                <select id={`entry-field-${key}`} value={String(values[key] ?? "")} onchange={(event) => { values[key] = event.currentTarget.value; }} disabled={pending}>
                  <option value="">{wt("Not set", "Belum diisi")}</option>
                  {#if typeof values[key] === "string" && values[key] !== "" && !field.options.includes(String(values[key]))}<option value={String(values[key])}>{values[key]} · {wt("previous value", "nilai sebelumnya")}</option>{/if}
                  {#each field.options as option}<option value={option}>{option}</option>{/each}
                </select>
              {:else if field.kind === "number"}
                <input id={`entry-field-${key}`} type="number" step="any" value={values[key] == null ? "" : String(values[key])} oninput={(event) => { values[key] = event.currentTarget.value; }} disabled={pending} />
              {:else if field.kind === "date"}
                <DatePicker id={`entry-field-${key}`} value={String(values[key] ?? "")} clearable onchange={(next) => { values[key] = next; }} disabled={pending} />
              {:else if field.kind === "url"}
                <input id={`entry-field-${key}`} type="url" placeholder="https://" value={String(values[key] ?? "")} oninput={(event) => { values[key] = event.currentTarget.value; }} disabled={pending} />
              {:else}
                <input id={`entry-field-${key}`} type="text" value={String(values[key] ?? "")} oninput={(event) => { values[key] = event.currentTarget.value; }} disabled={pending} />
              {/if}
            {/if}
          {/each}
        </section>
      {/if}
      {#if archivedFields.length}<section class="form-section"><h3>{wt("Archived fields", "Field diarsipkan")}</h3><p class="muted">{wt("These values are kept with this entry.", "Nilai ini tetap tersimpan pada catatan.")}</p><dl>{#each archivedFields as field}<div><dt>{field.name}</dt><dd>{typeof values[String(field.id)] === "boolean" ? values[String(field.id)] ? wt("Yes", "Ya") : wt("No", "Tidak") : values[String(field.id)]}</dd></div>{/each}</dl></section>{/if}
      {#if error}<p class="error" role="alert">{error}</p>{/if}
      {#if confirmDelete}<p class="delete-note" role="alert">{wt("Delete this entry permanently? Click Delete entry again to confirm.", "Hapus catatan ini permanen? Klik Hapus catatan lagi untuk konfirmasi.")}</p>{/if}
    </div>
    <footer>
      {#if entry}<button type="button" class="btn danger" onclick={remove} disabled={pending}><Icon name="trash" />{confirmDelete ? wt("Confirm delete", "Konfirmasi hapus") : wt("Delete entry", "Hapus catatan")}</button>{/if}
      <div class="footer-actions"><button type="button" class="btn" onclick={close} disabled={pending}>{wt("Cancel", "Batal")}</button><button type="submit" class="btn primary" disabled={pending || projectId == null}>{pending ? wt("Saving…", "Menyimpan…") : wt("Save entry", "Simpan catatan")}</button></div>
    </footer>
  </form>
</dialog>

<style>
  .entry-sheet { position: fixed; inset: 0 0 0 auto; height: 100%; max-height: 100%; width: min(480px, 100vw); max-width: 100vw; margin: 0; padding: 0; border: 0; border-left: 1px solid var(--separator); background: var(--surface); color: var(--label); box-shadow: -14px 0 50px #0002; }
  .entry-sheet::backdrop { background: #15172766; backdrop-filter: blur(2px); }
  form { height: 100%; display: flex; flex-direction: column; }
  header { display: flex; justify-content: space-between; gap: 16px; padding: 25px 26px 22px; border-bottom: 1px solid var(--separator); }
  h2 { font-size: 19px; font-weight: 650; letter-spacing: -.4px; margin: 2px 0 0; }
  h3 { font-size: 13px; margin: 0 0 3px; font-weight: 650; }
  .sheet-body { padding: 24px 26px; overflow-y: auto; flex: 1; min-height: 0; }
  .form-section { display: grid; gap: 9px; padding-bottom: 24px; margin-bottom: 24px; border-bottom: 1px solid var(--separator); }
  .form-section:last-child { margin-bottom: 0; }
  label { font-size: 12px; font-weight: 550; }
  select + label, input + label, .sheet-body :global(.date-field + label) { margin-top: 7px; }
  .time-row { display: grid; grid-template-columns: 58px minmax(0, 1fr) 86px; align-items: center; gap: 8px; }
  .time-row label { color: var(--secondary); font-weight: 500; }
  .duration { display: flex; align-items: center; gap: 7px; font-size: 12px; background: var(--fill); color: var(--label); border-radius: 8px; padding: 10px 12px; margin-top: 3px; }
  .duration strong { font-variant-numeric: tabular-nums; }
  .duration span { margin-left: auto; font-size: 11px; color: var(--secondary); }
  .duration.invalid { color: var(--red); background: color-mix(in srgb, var(--red) 8%, var(--surface)); }
  .duration.invalid span { margin-left: 0; color: inherit; font-size: 12px; }
  .section-title { display: flex; align-items: center; justify-content: space-between; margin-bottom: 5px; }
  .check-field { display: flex; align-items: center; gap: 9px; min-height: 32px; }
  .muted { color: var(--secondary); font-size: 12px; margin: 0; }
  .error, .delete-note { font-size: 12px; color: var(--red); background: color-mix(in srgb, var(--red) 8%, var(--surface)); border: 1px solid color-mix(in srgb, var(--red) 25%, var(--separator)); border-radius: 8px; padding: 10px 12px; overflow-wrap: anywhere; }
  dl { margin: 4px 0 0; font-size: 12px; display: grid; gap: 9px; }
  dl > div { display: flex; justify-content: space-between; gap: 16px; }
  dt { color: var(--secondary); } dd { margin: 0; overflow-wrap: anywhere; text-align: right; }
  footer { display: flex; align-items: center; justify-content: space-between; gap: 10px; padding: 16px 20px; border-top: 1px solid var(--separator); background: var(--surface); }
  .footer-actions { margin-left: auto; display: flex; gap: 8px; }
  @media (max-width: 460px) { header, .sheet-body { padding: 20px; } footer { flex-wrap: wrap; } .footer-actions { flex: 1; justify-content: flex-end; } }
</style>
