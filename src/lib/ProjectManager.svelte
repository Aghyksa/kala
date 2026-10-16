<script lang="ts">
  import { untrack } from "svelte";
  import { api, PALETTE, type Project } from "./api";
  import { errorText } from "./i18n.svelte";
  import { wapi, type FieldDefinition, type FieldInput, type FieldKind } from "./workspace-api";
  import { wt } from "./workspace-i18n.svelte";
  import Icon from "./Icon.svelte";

  let { projects, fields, onChanged }: { projects: Project[]; fields: FieldDefinition[]; onChanged: () => void } = $props();
  const initialProject = untrack(() => projects.find((project) => !project.archived) ?? projects[0]);
  let initialized = $state(Boolean(initialProject));
  let selectedId = $state<number | null>(initialProject?.id ?? null);
  let savedProject = $state<Project | null>(null);
  let name = $state(initialProject?.name ?? "");
  let color = $state(initialProject?.color ?? "#5958d6");
  let aliases = $state(initialProject?.aliases.join(", ") ?? "");
  let roundingMin = $state(initialProject?.roundingMin ?? 0);
  let showArchived = $state(false);
  let pending = $state("");
  let error = $state("");
  let notice = $state("");
  let fieldDraft = $state<FieldInput | null>(null);
  let optionsText = $state("");
  const selectedProject = $derived(projects.find((project) => project.id === selectedId) ?? (savedProject?.id === selectedId ? savedProject : null));
  const listedProjects = $derived(projects.filter((project) => !project.archived || showArchived || project.id === selectedId));
  const projectFields = $derived(fields.filter((field) => field.projectId === selectedId).sort((a, b) => a.position - b.position || a.id - b.id));
  const activeFields = $derived(projectFields.filter((field) => !field.archived));
  const archivedFields = $derived(projectFields.filter((field) => field.archived));
  const fieldKinds: FieldKind[] = ["text", "number", "date", "select", "checkbox", "url"];
  const kindLabel = (kind: FieldKind) => ({
    text: wt("Text", "Teks"), number: wt("Number", "Angka"), date: wt("Date", "Tanggal"),
    select: wt("Select", "Pilihan"), checkbox: wt("Checkbox", "Centang"), url: "URL",
  })[kind];

  let dirty = $state(false);

  $effect(() => {
    if (!initialized && projects.length) selectProject(projects.find((project) => !project.archived) ?? projects[0]);
  });

  // Fresh project props (another window saved) replace the form unless the user has unsaved edits.
  $effect(() => {
    const project = selectedProject;
    if (project && !untrack(() => dirty || pending)) untrack(() => loadDraft(project));
  });

  function loadDraft(project: Project | null) {
    name = project?.name ?? "";
    color = project?.color ?? "#5958d6";
    aliases = project?.aliases.join(", ") ?? "";
    roundingMin = project?.roundingMin ?? 0;
    dirty = false;
  }

  function selectProject(project: Project | null) {
    initialized = true;
    selectedId = project?.id ?? null;
    loadDraft(project);
    fieldDraft = null;
    error = "";
    notice = "";
  }

  async function run(key: string, action: () => Promise<void>) {
    if (pending) return;
    pending = key;
    error = "";
    notice = "";
    try {
      await action();
      onChanged();
    } catch (err) {
      error = errorText(err);
    } finally {
      pending = "";
    }
  }

  function saveProject(event: SubmitEvent) {
    event.preventDefault();
    run("project", async () => {
      const project = await api.saveProject({
        id: selectedId, name: name.trim(), color,
        aliases: aliases.split(",").map((alias) => alias.trim()).filter(Boolean), roundingMin: Number(roundingMin),
        hourlyRate: selectedProject?.hourlyRate ?? 0, currency: selectedProject?.currency ?? "IDR",
      });
      savedProject = project;
      selectedId = project.id;
      loadDraft(project);
      notice = wt("Project saved.", "Project tersimpan.");
    });
  }

  function archiveProject() {
    if (!selectedProject) return;
    const project = selectedProject;
    run("archive-project", async () => {
      await api.archiveProject(project.id, !project.archived);
      savedProject = { ...project, archived: !project.archived };
      showArchived = !project.archived;
      notice = project.archived ? wt("Project restored.", "Project dipulihkan.") : wt("Project archived. Its entries are kept.", "Project diarsipkan. Catatan waktunya tetap tersimpan.");
    });
  }

  function editField(field?: FieldDefinition) {
    if (selectedId == null) return;
    fieldDraft = field ? { ...field, options: [...field.options] } : {
      id: null, projectId: selectedId, name: "", kind: "text", options: [], showInTable: true,
      showInExport: true, position: projectFields.reduce((max, item) => Math.max(max, item.position + 1), 0), archived: false,
    };
    optionsText = field?.options.join("\n") ?? "";
    error = "";
    notice = "";
  }

  function saveField(event: SubmitEvent) {
    event.preventDefault();
    if (!fieldDraft) return;
    const draft = { ...fieldDraft, name: fieldDraft.name.trim(), options: fieldDraft.kind === "select" ? optionsText.split("\n").map((option) => option.trim()).filter(Boolean) : [] };
    run("field", async () => {
      await wapi.saveField(draft);
      fieldDraft = null;
      notice = wt("Field saved.", "Field tersimpan.");
    });
  }

  function archiveField(field: FieldDefinition) {
    run(`archive-field-${field.id}`, async () => {
      await wapi.saveField({ ...field, archived: !field.archived });
      if (fieldDraft?.id === field.id) fieldDraft = null;
      notice = field.archived ? wt("Field restored.", "Field dipulihkan.") : wt("Field archived. Existing values are kept.", "Field diarsipkan. Nilai yang sudah dicatat tetap tersimpan.");
    });
  }

  function moveField(field: FieldDefinition, direction: -1 | 1) {
    const from = activeFields.findIndex((item) => item.id === field.id);
    const to = from + direction;
    if (from < 0 || to < 0 || to >= activeFields.length) return;
    const moved = { ...activeFields[from], position: activeFields[to].position };
    const displaced = { ...activeFields[to], position: activeFields[from].position };
    const ordered = [...activeFields];
    [ordered[from], ordered[to]] = [ordered[to], ordered[from]];
    run("reorder", async () => {
      try {
        if (moved.position !== displaced.position) {
          await wapi.saveField(moved);
          await wapi.saveField(displaced);
        } else {
          for (let position = 0; position < ordered.length; position++) {
            if (ordered[position].position !== position) await wapi.saveField({ ...ordered[position], position });
          }
        }
      } catch (err) {
        onChanged();
        throw err;
      }
    });
  }
</script>

<div class="project-manager">
  <aside class="project-list" aria-label={wt("Projects", "Project")}>
    <div class="list-heading"><h2>{wt("All projects", "Semua project")}</h2><button class="btn quiet icon" onclick={() => selectProject(null)} disabled={!!pending} aria-label={wt("Add project", "Tambah project")} title={wt("Add project", "Tambah project")}><Icon name="plus" size={17} /></button></div>
    <div class="project-items">
      {#each listedProjects as project (project.id)}
        <button class="project-item" class:chosen={project.id === selectedId} class:archived={project.archived} onclick={() => selectProject(project)} disabled={!!pending} aria-pressed={project.id === selectedId}>
          <span class="project-dot" style:background={project.color}></span><span class="project-item-name">{project.name}{#if project.archived}<small>{wt("Archived", "Diarsipkan")}</small>{/if}</span><Icon name="right" size={14} />
        </button>
      {/each}
      {#if !listedProjects.length}<p class="list-empty">{wt("Your projects will appear here.", "Project kamu akan muncul di sini.")}</p>{/if}
    </div>
    <label class="archived-toggle"><input type="checkbox" bind:checked={showArchived} disabled={!!pending} />{wt("Show archived", "Tampilkan arsip")}</label>
  </aside>

  <div class="project-detail">
    <div class="detail-heading"><div><h2>{selectedId == null ? wt("New project", "Project baru") : name || selectedProject?.name || wt("Project", "Project")}</h2><p>{wt("Name, color, quick-entry aliases, and the extra details each entry records.", "Nama, warna, alias catat cepat, dan detail tambahan yang dicatat tiap entri.")}</p></div>{#if selectedProject?.archived}<span class="status-badge">{wt("Archived", "Diarsipkan")}</span>{/if}</div>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    {#if notice}<p class="notice" role="status"><Icon name="check" size={15} />{notice}</p>{/if}
    <form class="project-form" onsubmit={saveProject} oninput={() => { dirty = true; }}>
      <div class="name-color"><div><label for="project-name">{wt("Project name", "Nama project")}</label><input id="project-name" bind:value={name} placeholder={wt("e.g. Website redesign", "contoh Desain ulang website")} maxlength="200" required disabled={!!pending} /></div><div class="color-control"><label for="project-color">{wt("Color", "Warna")}</label><input id="project-color" type="color" bind:value={color} disabled={!!pending} /></div></div>
      <div class="color-palette" aria-label={wt("Project colors", "Warna project")}>{#each ["#5958d6", ...PALETTE] as choice}<button type="button" class="color-choice" class:color-selected={color.toLowerCase() === choice.toLowerCase()} style:background={choice} onclick={() => { color = choice; dirty = true; }} aria-label={choice} aria-pressed={color.toLowerCase() === choice.toLowerCase()} disabled={!!pending}></button>{/each}</div>
      <label for="project-aliases">{wt("Aliases", "Alias")}</label><input id="project-aliases" bind:value={aliases} placeholder={wt("website, web", "website, web")} aria-describedby="aliases-hint" disabled={!!pending} /><p id="aliases-hint" class="hint">{wt("Separate aliases with commas. Use them in quick entry.", "Pisahkan alias dengan koma untuk dipakai saat catat cepat.")}</p>
      <div class="rounding-row"><div><label for="project-rounding">{wt("Time rounding", "Pembulatan waktu")}</label><p class="hint">{wt("Used when recording time with quick entry.", "Dipakai saat mencatat waktu lewat catat cepat.")}</p></div><select id="project-rounding" bind:value={roundingMin} disabled={!!pending}><option value={0}>{wt("No rounding", "Tanpa pembulatan")}</option>{#each [15, 30, 60] as minutes}<option value={minutes}>{minutes} {wt("minutes", "menit")}</option>{/each}</select></div>
      <div class="project-actions">{#if selectedProject}<button type="button" class="btn quiet" onclick={archiveProject} disabled={!!pending}>{pending === "archive-project" ? wt("Updating…", "Memperbarui…") : selectedProject.archived ? wt("Restore project", "Pulihkan project") : wt("Archive project", "Arsipkan project")}</button>{/if}<button class="btn primary" type="submit" disabled={!!pending}>{pending === "project" ? wt("Saving…", "Menyimpan…") : selectedId == null ? wt("Create project", "Buat project") : wt("Save project", "Simpan project")}</button></div>
    </form>

    <section class="field-section" aria-labelledby="custom-fields-title">
      <div class="fields-heading"><div><h3 id="custom-fields-title">{wt("Custom fields", "Field tambahan")}</h3><p>{wt("Capture the details that matter to this project.", "Catat informasi yang dibutuhkan project ini.")}</p></div><button class="btn small" onclick={() => editField()} disabled={!!pending || selectedId == null || !!selectedProject?.archived}><Icon name="plus" size={14} />{wt("Add field", "Tambah field")}</button></div>
      {#if selectedId == null}<div class="fields-empty"><Icon name="projects" size={24} /><p>{wt("Save your project to add custom fields.", "Simpan project untuk menambahkan field.")}</p></div>
      {:else}
        {#if !activeFields.length && !fieldDraft}<div class="fields-empty"><Icon name="general" size={24} /><p>{wt("No custom fields yet", "Belum ada field tambahan")}</p><span>{wt("Add a task code, category, link, or any project detail.", "Tambahkan kode tugas, kategori, tautan, atau detail project.")}</span></div>{/if}
        <div class="field-list">
          {#each activeFields as field, index (field.id)}
            <div class="field-row"><div class="reorder"><button class="btn quiet move" onclick={() => moveField(field, -1)} disabled={!!pending || index === 0} aria-label={wt(`Move ${field.name} up`, `Naikkan ${field.name}`)} title={wt("Move up", "Naikkan")}><Icon name="up" size={15} /></button><button class="btn quiet move" onclick={() => moveField(field, 1)} disabled={!!pending || index === activeFields.length - 1} aria-label={wt(`Move ${field.name} down`, `Turunkan ${field.name}`)} title={wt("Move down", "Turunkan")}><Icon name="down" size={15} /></button></div><div class="field-info"><strong>{field.name}</strong><div class="field-meta"><span>{kindLabel(field.kind)}</span>{#if field.showInTable}<span>{wt("Table", "Tabel")}</span>{/if}{#if field.showInExport}<span>Export</span>{/if}</div></div><button class="btn small quiet" onclick={() => editField(field)} disabled={!!pending}>{wt("Edit", "Edit")}</button><button class="btn small quiet archive-field" onclick={() => archiveField(field)} disabled={!!pending} aria-label={wt(`Archive ${field.name}`, `Arsipkan ${field.name}`)}>{wt("Archive", "Arsipkan")}</button></div>
          {/each}
        </div>
        {#if fieldDraft}
          <form class="field-form" onsubmit={saveField}>
            <div class="field-form-heading"><h4>{fieldDraft.id == null ? wt("New field", "Field baru") : wt("Edit field", "Edit field")}</h4><button type="button" class="btn quiet icon" onclick={() => { fieldDraft = null; }} disabled={!!pending} aria-label={wt("Cancel field changes", "Batalkan perubahan field")}><Icon name="close" size={15} /></button></div>
            <div class="field-basics"><div><label for="field-name">{wt("Field name", "Nama field")}</label><input id="field-name" bind:value={fieldDraft.name} required maxlength="120" placeholder={wt("e.g. Task code", "contoh Kode tugas")} disabled={!!pending} /></div><div><label for="field-kind">{wt("Type", "Tipe")}</label><select id="field-kind" bind:value={fieldDraft.kind} disabled={!!pending || fieldDraft.id != null}>{#each fieldKinds as kind}<option value={kind}>{kindLabel(kind)}</option>{/each}</select></div></div>
            {#if fieldDraft.id != null}<p class="hint">{wt("The field type stays fixed to keep existing values valid.", "Tipe field tetap agar nilai yang sudah dicatat tetap valid.")}</p>{/if}
            {#if fieldDraft.kind === "select"}<label for="field-options">{wt("Options", "Pilihan")}</label><textarea id="field-options" bind:value={optionsText} rows="4" required placeholder={wt("One option per line", "Satu pilihan per baris")} aria-describedby="options-hint" disabled={!!pending}></textarea><p class="hint" id="options-hint">{wt("One option per line. Previous entry values are kept when options change.", "Satu pilihan per baris. Nilai catatan lama tetap tersimpan saat pilihan berubah.")}</p>{/if}
            <div class="visibility"><label><input type="checkbox" bind:checked={fieldDraft.showInTable} disabled={!!pending} />{wt("Show in timesheet table", "Tampilkan di tabel catatan")}</label><label><input type="checkbox" bind:checked={fieldDraft.showInExport} disabled={!!pending} />{wt("Include in exports", "Sertakan di export")}</label></div>
            <div class="field-form-actions"><button type="button" class="btn small" onclick={() => { fieldDraft = null; }} disabled={!!pending}>{wt("Cancel", "Batal")}</button><button type="submit" class="btn primary small" disabled={!!pending}>{pending === "field" ? wt("Saving…", "Menyimpan…") : wt("Save field", "Simpan field")}</button></div>
          </form>
        {/if}
        {#if archivedFields.length}<details class="archived-fields"><summary>{wt("Archived fields", "Field diarsipkan")} <span>{archivedFields.length}</span></summary>{#each archivedFields as field (field.id)}<div class="field-row"><div class="field-info"><strong>{field.name}</strong><span class="hint">{kindLabel(field.kind)}</span></div><button class="btn small quiet" onclick={() => editField(field)} disabled={!!pending}>{wt("Edit", "Edit")}</button><button class="btn small" onclick={() => archiveField(field)} disabled={!!pending}>{wt("Restore", "Pulihkan")}</button></div>{/each}</details>{/if}
      {/if}
    </section>
  </div>
</div>

<style>
  .project-manager { display: grid; grid-template-columns: 238px minmax(0, 1fr); min-height: 580px; border: 1px solid var(--separator); border-radius: 13px; overflow: hidden; background: var(--surface); }
  .project-list { border-right: 1px solid var(--separator); background: color-mix(in srgb, var(--field) 65%, var(--surface)); display: flex; flex-direction: column; padding: 14px 10px; }
  .list-heading { display: flex; align-items: center; justify-content: space-between; padding: 2px 4px 13px 9px; }
  .list-heading h2 { font-size: 12px; font-weight: 650; margin: 0; }
  .project-items { display: grid; align-content: start; gap: 4px; flex: 1; }
  .project-item { display: flex; align-items: center; gap: 10px; width: 100%; border: 1px solid transparent; background: transparent; border-radius: 8px; text-align: left; padding: 10px; min-height: 42px; }
  .project-item:hover { background: var(--hover); } .project-item.chosen { background: var(--accent-soft); color: var(--accent-text); border-color: color-mix(in srgb, var(--accent) 15%, transparent); }
  .project-item.archived { color: var(--secondary); } .project-dot { width: 9px; height: 9px; border-radius: 3px; flex: 0 0 auto; }
  .project-item-name { min-width: 0; flex: 1; overflow-wrap: anywhere; font-size: 12px; font-weight: 550; }
  .project-item-name small { display: block; font-size: 10px; font-weight: 400; color: var(--tertiary); margin-top: 2px; }
  .archived-toggle { display: flex; align-items: center; gap: 8px; font-size: 11px; color: var(--secondary); padding: 18px 9px 5px; }
  .list-empty { color: var(--tertiary); font-size: 12px; padding: 8px 10px; }
  .project-detail { padding: 28px; min-width: 0; max-width: 900px; width: 100%; }
  .detail-heading { display: flex; justify-content: space-between; align-items: flex-start; gap: 16px; margin-bottom: 24px; }
  .detail-heading h2 { margin: 0 0 5px; font-size: 22px; letter-spacing: -.5px; font-weight: 650; overflow-wrap: anywhere; }
  .detail-heading p, .fields-heading p { margin: 0; font-size: 12px; color: var(--secondary); }
  .status-badge { font-size: 11px; padding: 4px 9px; border-radius: 6px; color: var(--secondary); background: var(--fill); }
  .project-form { display: grid; gap: 8px; }
  label { font-size: 12px; font-weight: 550; }
  .name-color { display: grid; grid-template-columns: minmax(0, 1fr) 58px; gap: 14px; }
  .name-color > div, .field-basics > div { display: grid; gap: 7px; }
  input[type="color"] { border: 1px solid var(--separator); background: var(--field); width: 100%; height: 35px; border-radius: 8px; padding: 4px; }
  .color-palette { display: flex; align-items: center; gap: 7px; margin: 2px 0 15px; padding-left: 1px; }
  .color-choice { width: 15px; height: 15px; border-radius: 50%; border: 2px solid var(--surface); outline: 1px solid transparent; }
  .color-choice.color-selected { outline-color: var(--secondary); }
  .hint { font-size: 11px; color: var(--secondary); margin: 0; line-height: 1.5; }
  .rounding-row { display: grid; grid-template-columns: minmax(0, 1fr) 175px; gap: 16px; align-items: center; margin-top: 18px; padding: 18px 0; border-top: 1px solid var(--separator); border-bottom: 1px solid var(--separator); }
  .rounding-row .hint { margin-top: 3px; }
  .project-actions { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding-top: 9px; } .project-actions .primary { margin-left: auto; }
  .field-section { margin-top: 34px; padding-top: 25px; border-top: 1px solid var(--separator); }
  .fields-heading { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; margin-bottom: 18px; }
  .fields-heading h3 { margin: 0 0 4px; font-size: 14px; font-weight: 650; }
  .fields-empty { border: 1px dashed var(--separator); border-radius: 9px; padding: 25px 16px; text-align: center; color: var(--tertiary); }
  .fields-empty p { color: var(--secondary); font-size: 12px; margin: 7px 0 3px; } .fields-empty span { font-size: 11px; }
  .field-list { display: grid; gap: 0; }
  .field-row { display: flex; gap: 8px; align-items: center; padding: 12px 0; border-bottom: 1px solid var(--separator); }
  .field-info { flex: 1; min-width: 0; display: grid; gap: 4px; } .field-info strong { font-size: 12px; font-weight: 550; overflow-wrap: anywhere; }
  .field-meta { display: flex; flex-wrap: wrap; gap: 5px; font-size: 10px; color: var(--secondary); } .field-meta span { background: var(--fill); padding: 1px 5px; border-radius: 4px; }
  .reorder { display: flex; gap: 0; } .move { min-height: 24px; width: 23px; padding: 1px; font-size: 14px; }
  .field-form { display: grid; gap: 8px; margin: 14px 0; padding: 16px; background: var(--field); border: 1px solid var(--separator); border-radius: 9px; }
  .field-form-heading { display: flex; justify-content: space-between; align-items: center; margin: -5px -5px 3px 0; } h4 { margin: 0; font-size: 13px; }
  .field-basics { display: grid; grid-template-columns: minmax(0, 1fr) 150px; gap: 12px; }
  .visibility { display: flex; gap: 18px; flex-wrap: wrap; margin-top: 7px; } .visibility label { display: flex; align-items: center; gap: 7px; font-size: 11px; font-weight: 400; }
  .field-form-actions { display: flex; justify-content: flex-end; gap: 7px; margin-top: 8px; }
  .archived-fields { margin-top: 18px; color: var(--secondary); } summary { font-size: 11px; } summary span { margin-left: 4px; font-variant-numeric: tabular-nums; }
  .archived-fields .field-row { padding-left: 10px; }
  .error, .notice { display: flex; gap: 7px; align-items: center; margin: -8px 0 18px; border-radius: 8px; padding: 10px 12px; font-size: 12px; overflow-wrap: anywhere; }
  .error { color: var(--red); background: color-mix(in srgb, var(--red) 8%, var(--surface)); border: 1px solid color-mix(in srgb, var(--red) 25%, var(--separator)); }
  .notice { color: var(--accent-text); background: var(--accent-soft); }
  @media (max-width: 1080px) { .project-manager { grid-template-columns: 200px minmax(0, 1fr); } .project-detail { padding: 22px; } .archive-field { padding-left: 5px; padding-right: 5px; } }
  @media (max-width: 800px) { .project-manager { grid-template-columns: 1fr; } .project-list { border-right: 0; border-bottom: 1px solid var(--separator); } .project-items { grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); } .archived-toggle { padding-top: 12px; } .project-detail { padding: 22px 18px; } .field-basics { grid-template-columns: minmax(0, 1fr) 120px; } .rounding-row { grid-template-columns: minmax(0, 1fr) 145px; } }
</style>
