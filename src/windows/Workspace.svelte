<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, isoDate, type Entry, type Project } from "../lib/api";
  import { matchPreset, presetRange, type Preset } from "../lib/dates";
  import { dayLabel, duration, errorText, rangeLabel, setLang } from "../lib/i18n.svelte";
  import { wapi, type FieldDefinition, type SavedView, type ViewConfig, type WorkspaceData, type WorkspaceEntry, type WorkspaceQuery, type ExportResult } from "../lib/workspace-api";
  import { wt } from "../lib/workspace-i18n.svelte";
  import Icon from "../lib/Icon.svelte";
  import QuickInput from "../lib/QuickInput.svelte";
  import EntryEditor from "../lib/EntryEditor.svelte";
  import ProjectManager from "../lib/ProjectManager.svelte";
  import Overview from "../lib/workspace/Overview.svelte";
  import EntryTable from "../lib/workspace/EntryTable.svelte";
  import DateRangePicker from "../lib/ui/DateRangePicker.svelte";
  import Floating from "../lib/ui/Floating.svelte";

  type Page = "dashboard" | "timesheet" | "projects" | "reports";
  const [monthFrom, monthTo] = presetRange("month");
  const defaultQuery: WorkspaceQuery = { from: monthFrom, to: monthTo, projectId:null, search:"", fieldId:null, fieldValue:null, limit:50, offset:0 };
  const baseKeys = ["date","project","description","start","end","hours"];
  let page = $state<Page>("dashboard");
  let query = $state<WorkspaceQuery>({...defaultQuery});
  let preset = $state<Preset>("month");
  let today = isoDate(new Date());
  let projects = $state<Project[]>([]);
  let fields = $state<FieldDefinition[]>([]);
  let views = $state<SavedView[]>([]);
  let data = $state<WorkspaceData | null>(null);
  let columns = $state<string[]>([...baseKeys]);
  let ready = $state(false);
  let loading = $state(false);
  let error = $state<unknown>(null);
  let running = $state<Entry | null>(null);
  let nowMs = $state(Date.now());
  let searchDraft = $state("");
  let fieldDraft = $state("");
  let groupBy = $state<ViewConfig["groupBy"]>("project");
  let editing = $state(false);
  let editingEntry = $state<WorkspaceEntry | null>(null);
  let selectedViewId = $state<number | null>(null);
  let showSaveView = $state(false);
  let viewName = $state("");
  let savingView = $state(false);
  let viewError = $state("");
  let viewDialog = $state<HTMLDialogElement|null>(null);
  let confirmViewDelete = $state(false);
  let exportBusy = $state(false);
  let exported = $state<ExportResult | null>(null);
  let timerProject = $state<number | null>(null);
  let timerDescription = $state("");
  let timerBusy = $state(false);
  let timerOpen = $state(false);
  let timerButton = $state<HTMLButtonElement>();
  let columnsOpen = $state(false);
  let columnsButton = $state<HTMLButtonElement>();
  let requestVersion = 0;
  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  let refreshTimer: ReturnType<typeof setTimeout> | undefined;
  const projectMap = $derived(new Map(projects.map(p=>[p.id,p])));
  const activeProjects = $derived(projects.filter(p=>!p.archived));
  const scopedFields = $derived(fields.filter(f=>!f.archived && (query.projectId == null || f.projectId === query.projectId)));
  const dayTotals = $derived(data ? new Map(data.byDay.map(d=>[d.date,d.minutes])) : null);
  const elapsed = $derived(running ? Math.max(0,Math.floor((nowMs-Date.parse(running.start))/60000)) : 0);
  const navigation = $derived([
    {id:"dashboard" as const,name:wt("Dashboard","Dashboard"),icon:"dashboard" as const},
    {id:"timesheet" as const,name:wt("Timesheet","Timesheet"),icon:"table" as const},
    {id:"projects" as const,name:wt("Projects","Project"),icon:"projects" as const},
    {id:"reports" as const,name:wt("Reports","Laporan"),icon:"chart" as const},
  ]);
  const baseOptions = $derived([
    {key:"no",name:"#"},{key:"date",name:wt("Date","Tanggal")},{key:"project",name:"Project"},
    {key:"description",name:wt("Description","Deskripsi")},{key:"start",name:wt("Start","Mulai")},
    {key:"end",name:wt("End","Selesai")},{key:"hours",name:wt("Duration","Durasi")},
  ]);
  const columnOptions = $derived([...baseOptions,...scopedFields.map(f=>({key:`field:${f.id}`,name:query.projectId==null ? `${projectMap.get(f.projectId)?.name} / ${f.name}` : f.name}))]);
  const groups = $derived(data ? groupBy === "project" ? data.byProject.map(p=>({key:String(p.projectId),name:projectMap.get(p.projectId)?.name ?? "",color:projectMap.get(p.projectId)?.color,minutes:p.minutes,entries:p.entries})) : groupBy === "day" ? data.byDay.map(d=>({key:d.date,name:dayLabel(d.date),color:undefined,minutes:d.minutes,entries:d.entries})) : data.fieldGroups.map(g=>({key:g.value,name:g.value && g.value!=="null" ? g.value : wt("Not set","Belum diisi"),color:undefined,minutes:g.minutes,entries:g.entries})) : []);

  // Timesheet rows sit under day headings, so its default columns skip the date.
  function standardColumns(target: Page = page) {
    const keys = target === "timesheet" ? baseKeys.filter(k=>k!=="date") : baseKeys;
    return [...keys,...fields.filter(f=>!f.archived && (query.projectId==null || f.projectId===query.projectId) && (target === "reports" ? f.showInExport : f.showInTable)).map(f=>`field:${f.id}`)];
  }
  function navigate(next:Page) { page=next;query.offset=0;columns=standardColumns(next);exported=null;columnsOpen=false; }
  async function loadMeta() {
    const [ps,fs,sv,settings,run]=await Promise.all([api.listProjects(true),wapi.fields(null,true),wapi.views(),api.getSettings(),api.timerStatus()]);
    projects=ps; fields=fs; views=sv; running=run; setLang(settings.language);
    timerProject ??= ps.find(p=>!p.archived)?.id ?? null;
  }
  async function loadData(snapshot:WorkspaceQuery) {
    const version=++requestVersion;
    loading=true;
    try { const next=await wapi.data(snapshot); if(version===requestVersion){data=next;error=null;} }
    catch(e) { if(version===requestVersion) error=e; }
    finally { if(version===requestVersion) loading=false; }
  }
  async function refresh() {
    try { await Promise.all([loadMeta(),loadData({...query})]); } catch(e) { error=e; }
  }
  function scheduleRefresh() { clearTimeout(refreshTimer);refreshTimer=setTimeout(refresh,90); }
  $effect(()=>{ if(ready) loadData({...query}); });
  $effect(()=>{ if(showSaveView && viewDialog && !viewDialog.open) viewDialog.showModal(); });
  onMount(()=>{
    let disposed=false;
    loadMeta().then(()=>{if(!disposed){ready=true;columns=standardColumns();}}).catch(e=>error=e);
    const unsub=Promise.all([listen("kala://data-changed",scheduleRefresh),listen("kala://refresh",scheduleRefresh)]);
    const timer=setInterval(updateClock,15000);
    return ()=>{disposed=true;clearInterval(timer);clearTimeout(searchTimer);clearTimeout(refreshTimer);unsub.then(fns=>fns.forEach(fn=>fn()));};
  });
  function chooseProject(id:number|null) { query={...query,projectId:id,fieldId:null,fieldValue:null,offset:0};fieldDraft="";if(groupBy==="field")groupBy="project";columns=standardColumns(); }
  function validDate(value:unknown): value is string {
    if(typeof value!=="string" || !/^\d{4}-\d{2}-\d{2}$/.test(value) || Number(value.slice(0,4))<1900)return false;
    const time=Date.parse(`${value}T00:00:00Z`);
    return Number.isFinite(time) && new Date(time).toISOString().slice(0,10)===value;
  }
  function setRange(from:string,to:string,next:Preset=matchPreset(from,to)) { preset=next;query={...query,from,to,offset:0}; }
  function updateClock() {
    const now=new Date();nowMs=now.getTime();const nextToday=isoDate(now);
    if(nextToday===today)return;
    today=nextToday;if(preset!=="custom")setRange(...presetRange(preset),preset);
  }
  function focus() { updateClock();scheduleRefresh(); }
  function search() { clearTimeout(searchTimer);searchTimer=setTimeout(()=>query={...query,search:searchDraft,offset:0},250); }
  function toggleColumn(key:string) { columns=columns.includes(key) ? columns.filter(c=>c!==key) : [...columns,key]; }
  function openEditor(entry:WorkspaceEntry|null=null) { editingEntry=entry;editing=true; }
  async function startTimer() {
    if(timerProject==null || timerBusy)return;timerBusy=true;
    try {await api.timerStart(timerProject,timerDescription);timerDescription="";timerOpen=false;scheduleRefresh();}catch(e){error=e;}finally{timerBusy=false;}
  }
  async function stopTimer() {
    if(timerBusy)return;timerBusy=true;
    try { const entry=await api.timerStop(null); scheduleRefresh(); if(!entry.description)openEditor({...entry,fields:{}}); }catch(e){error=e;}finally{timerBusy=false;}
  }
  async function runExport(format:"xlsx"|"csv") {
    exportBusy=true;exported=null;
    try {exported=await wapi.export({...query,offset:0},[...columns],format);error=null;}catch(e){error=e;}finally{exportBusy=false;}
  }
  function useView(view:SavedView) {
    const c=view.config;
    if(!c || !c.query || !validDate(c.query.from) || !validDate(c.query.to) || c.query.from>c.query.to || !Array.isArray(c.columns)){error=wt("This saved view is invalid.","Tampilan tersimpan tidak valid.");return;}
    query={...defaultQuery,...c.query,offset:0,limit:50};searchDraft=query.search;fieldDraft=query.fieldValue??"";preset="custom";
    const allowed=new Set([...baseOptions.map(o=>o.key),...fields.map(f=>`field:${f.id}`)]);
    columns=c.columns.filter(key=>allowed.has(key));if(!columns.length)columns=standardColumns("reports");
    groupBy=["project","day","field"].includes(c.groupBy) && (c.groupBy!=="field" || query.fieldId!=null)?c.groupBy:"project";page="reports";selectedViewId=view.id;exported=null;confirmViewDelete=false;
  }
  async function saveView() {
    if(!viewName.trim() || savingView)return;savingView=true;viewError="";
    try { const view=await wapi.saveView({id:selectedViewId,name:viewName.trim(),config:{query:{...query,offset:0},columns:[...columns],groupBy}});selectedViewId=view.id;showSaveView=false;scheduleRefresh();error=null;}catch(e){viewError=errorText(e);}finally{savingView=false;}
  }
  async function deleteView() {
    if(selectedViewId==null)return;if(!confirmViewDelete){confirmViewDelete=true;return;}
    try {await wapi.deleteView(selectedViewId);selectedViewId=null;confirmViewDelete=false;scheduleRefresh();}catch(e){error=e;}
  }
  function keyboard(e:KeyboardEvent) { if((e.metaKey||e.ctrlKey)&&e.key==="n"&&!editing){e.preventDefault();openEditor();} if(e.key==="Escape"){showSaveView=false;confirmViewDelete=false;} }
</script>

<svelte:window onfocus={focus} onkeydown={keyboard} />
<div class="workspace">
  <aside class="sidebar">
    <div class="identity"><span class="brand-icon"><Icon name="kala" size={24}/></span><strong>Kala</strong></div>
    <nav aria-label={wt("Main navigation","Navigasi utama")}>{#each navigation as item}<button class:active={page===item.id} aria-current={page===item.id?"page":undefined} onclick={()=>{selectedViewId=null;navigate(item.id);}}><Icon name={item.icon} size={17}/>{item.name}</button>{/each}</nav>
    <div class="sidebar-section"><div class="sidebar-label"><span>Project</span><button class="btn quiet icon" onclick={()=>navigate("projects")} aria-label={wt("Manage projects","Kelola project")} title={wt("Manage projects","Kelola project")}><Icon name="plus" size={14}/></button></div><div class="project-nav">{#each activeProjects as project (project.id)}<button class:selected={query.projectId===project.id && page!=="projects"} onclick={()=>{chooseProject(project.id);navigate("timesheet");}}><span class="dot" style:background={project.color}></span><span>{project.name}</span></button>{:else}<button class="add-project" onclick={()=>navigate("projects")}>{wt("Add your first project","Tambah project pertama")}</button>{/each}</div></div>
    {#if views.length}<div class="sidebar-section"><div class="sidebar-label"><span>{wt("Saved reports","Laporan tersimpan")}</span></div><div class="project-nav">{#each views as view (view.id)}<button class:selected={selectedViewId===view.id && page==="reports"} onclick={()=>useView(view)}><Icon name="chart" size={13}/><span>{view.name}</span></button>{/each}</div></div>{/if}
    <div class="sidebar-bottom">
      {#if running}<div class="live-timer" role="status"><div class="timer-project"><span class="dot" style:background={projectMap.get(running.projectId)?.color}></span><span>{projectMap.get(running.projectId)?.name}</span></div><div class="timer-controls"><strong>{Math.floor(elapsed/60)}:{String(elapsed%60).padStart(2,"0")}</strong><button class="btn primary small" disabled={timerBusy} onclick={stopTimer}><Icon name="stop" size={12}/>{wt("Stop","Stop")}</button></div><p>{running.description || wt("Add a description when you stop.","Deskripsi bisa diisi saat berhenti.")}</p></div>{/if}
      <button class="settings-link" onclick={()=>api.openSettings()}><Icon name="settings" size={17}/>{wt("Settings","Pengaturan")}</button>
    </div>
  </aside>
  <div class="main-column">
    <header class="toolbar">
      <h1>{navigation.find(n=>n.id===page)?.name}</h1>
      {#if page!=="projects"}
        <div class="filters">
          <DateRangePicker from={query.from} to={query.to} onChange={setRange}/>
          <label class="project-filter"><span class="sr-only">{wt("Filter by project","Filter project")}</span><select value={query.projectId??"all"} onchange={e=>chooseProject(e.currentTarget.value==="all"?null:Number(e.currentTarget.value))}><option value="all">{wt("All projects","Semua project")}</option>{#each projects as p (p.id)}<option value={p.id}>{p.name}{p.archived?wt(" (archived)"," (arsip)"):""}</option>{/each}</select></label>
        </div>
      {/if}
      <div class="top-actions">
        {#if loading}<span class="syncing" role="status">{wt("Updating…","Memperbarui…")}</span>{/if}
        {#if !running}<button bind:this={timerButton} class="btn small" aria-label={wt("Start timer","Mulai timer")} aria-haspopup="dialog" aria-expanded={timerOpen} disabled={!activeProjects.length} onclick={()=>timerOpen=!timerOpen}><Icon name="play" size={13}/><span class="label">{wt("Start timer","Mulai timer")}</span></button>{/if}
        <button class="btn primary small" onclick={()=>openEditor()} disabled={!activeProjects.length}><Icon name="plus" size={14}/>{wt("Log time","Catat waktu")}</button>
      </div>
    </header>
    <Floating anchor={timerButton} bind:open={timerOpen} align="end" label={wt("Start timer","Mulai timer")}>
      <form class="menu-form" onsubmit={(e)=>{e.preventDefault();startTimer();}}>
        <label>Project<select bind:value={timerProject}>{#each activeProjects as p (p.id)}<option value={p.id}>{p.name}</option>{/each}</select></label>
        <label>{wt("What are you working on?","Sedang mengerjakan apa?")}<input bind:value={timerDescription} placeholder={wt("Optional","Opsional")}/></label>
        <button class="btn primary" disabled={timerProject==null||timerBusy}><Icon name="play" size={13}/>{wt("Start","Mulai")}</button>
      </form>
    </Floating>
    <main>
      {#if error}<div class="error-banner" role="alert"><span>{typeof error==="string"?error:errorText(error)}</span><button class="btn quiet icon" onclick={()=>error=null} aria-label={wt("Dismiss error","Tutup pesan")}><Icon name="close" size={14}/></button></div>{/if}
      {#if page==="projects"}
        <ProjectManager {projects} {fields} onChanged={scheduleRefresh}/>
      {:else if !data}
        <div class="loading-state" role="status">{#if error}<p>{wt("Your workspace could not load.","Workspace belum bisa dimuat.")}</p><button class="btn small" onclick={refresh}>{wt("Try again","Coba lagi")}</button>{:else}{wt("Loading your workspace…","Memuat workspace…")}{/if}</div>
      {:else if page==="dashboard"}
        <div class="quick-capture"><QuickInput exampleProject={activeProjects[0]?.aliases[0]||activeProjects[0]?.name.toLowerCase()||"project"} onDone={scheduleRefresh} onCancel={()=>{}}/></div>
        <Overview {data} {projects} from={query.from} to={query.to} onEdit={openEditor} onAdd={activeProjects.length?()=>openEditor():undefined} onTimesheet={()=>navigate("timesheet")} onRange={(from,to)=>{setRange(from,to);navigate("timesheet");}} onProject={(id)=>{chooseProject(id);navigate("timesheet");}}/>
      {:else}
        <div class="table-toolbar"><div class="search-field"><Icon name="search" size={15}/><input type="search" aria-label={wt("Search entries","Cari catatan")} placeholder={wt("Search descriptions","Cari deskripsi")} bind:value={searchDraft} oninput={search}/></div>
          <span class="entry-count"><strong>{duration(data.totalMinutes)}</strong><span>·</span>{data.total} {wt("entries","catatan")}</span><span class="toolbar-spacer"></span>
          <button bind:this={columnsButton} class="btn small" aria-haspopup="dialog" aria-expanded={columnsOpen} onclick={()=>columnsOpen=!columnsOpen}><Icon name="columns" size={14}/>{wt("Columns","Kolom")}</button>
          {#if page==="reports"}<button class="btn small" onclick={()=>{viewName=views.find(v=>v.id===selectedViewId)?.name||"";viewError="";showSaveView=true;}}><Icon name="plus" size={14}/>{wt("Save view","Simpan tampilan")}</button>{/if}
        </div>
        <Floating anchor={columnsButton} bind:open={columnsOpen} align="end" label={wt("Visible columns","Kolom ditampilkan")}>
          <fieldset class="column-menu"><legend>{page==="reports"?wt("Columns in table and export","Kolom di tabel dan export"):wt("Visible columns","Kolom ditampilkan")}</legend>{#each columnOptions as option (option.key)}<label><input type="checkbox" checked={columns.includes(option.key)} disabled={columns.length===1&&columns.includes(option.key)} onchange={()=>toggleColumn(option.key)}/>{option.name}</label>{/each}</fieldset>
        </Floating>
        {#if page==="reports"}
          <div class="report-controls"><label>{wt("Group summary by","Kelompokkan ringkasan")}<select bind:value={groupBy}><option value="project">Project</option><option value="day">{wt("Day","Hari")}</option><option value="field" disabled={!query.fieldId}>{wt("Custom field","Field tambahan")}</option></select></label>
            <label>{wt("Custom field","Field tambahan")}<select bind:value={query.fieldId} onchange={()=>{query.offset=0;query.fieldValue=null;fieldDraft="";if(!query.fieldId&&groupBy==="field")groupBy="project";}}><option value={null}>{wt("None","Tidak ada")}</option>{#each scopedFields as field (field.id)}<option value={field.id}>{query.projectId==null?`${projectMap.get(field.projectId)?.name} / `:""}{field.name}</option>{/each}</select></label>
            {#if query.fieldId}<form class="field-filter" onsubmit={e=>{e.preventDefault();query={...query,fieldValue:fieldDraft.trim()||null,offset:0};}}><label>{wt("Only this value","Hanya nilai ini")}<input bind:value={fieldDraft} placeholder={wt("Any value","Semua nilai")}/></label><button class="btn small">{wt("Apply","Terapkan")}</button></form>{/if}
          </div>
          <section class="report-summary"><div class="summary-title"><div><h2>{rangeLabel(query.from,query.to)}</h2><span>{data.total} {wt("entries","catatan")}{query.projectId!=null?` · ${projectMap.get(query.projectId)?.name}`:""}</span></div><strong>{duration(data.totalMinutes)}</strong></div><div class="group-list">{#each groups as group (group.key)}<div><span class="group-name">{#if group.color}<i style:background={group.color}></i>{/if}{group.name}</span><span class="group-bar"><span style:width={`${data.totalMinutes?group.minutes/data.totalMinutes*100:0}%`} style:background={group.color||"var(--accent)"}></span></span><span class="group-entries">{group.entries} {wt("entries","catatan")}</span><strong>{duration(group.minutes)}</strong></div>{:else}<p>{wt("Nothing matches these filters yet.","Belum ada catatan yang cocok dengan filter ini.")}</p>{/each}</div>
            <div class="export-toolbar"><span>{wt("Exports every matching entry with the columns above.","Export berisi semua catatan yang cocok, dengan kolom di tabel.")}</span>{#if selectedViewId!=null}<button class="btn quiet small danger" onclick={deleteView}>{confirmViewDelete?wt("Confirm delete","Yakin hapus?"):wt("Delete view","Hapus tampilan")}</button>{/if}<button class="btn small" disabled={exportBusy||!data.total} onclick={()=>runExport("csv")}>CSV</button><button class="btn primary small" disabled={exportBusy||!data.total} onclick={()=>runExport("xlsx")}><Icon name="download" size={14}/>{exportBusy?wt("Exporting…","Mengekspor…"):"Excel"}</button></div>
            {#if exported}<div class="export-success" role="status"><Icon name="check" size={15}/><span>{exported.rows} {wt("entries exported to Downloads.","catatan di-export ke Downloads.")}</span><button class="link" onclick={()=>api.revealPath(exported!.path)}>{wt("Show in Finder","Tampilkan di Finder")}</button></div>{/if}
          </section>
        {/if}
        <EntryTable entries={data.entries} {projects} {fields} {columns} offset={query.offset} dayTotals={page==="timesheet"?dayTotals:null} onEdit={openEditor} onAdd={activeProjects.length?()=>openEditor():undefined}/>
        {#if data.total > 25 || query.offset > 0}<div class="pagination"><span>{data.total?query.offset+1:0}–{Math.min(query.offset+data.entries.length,data.total)} {wt("of","dari")} {data.total}</span><label><span class="sr-only">{wt("Rows per page","Baris per halaman")}</span><select bind:value={query.limit} onchange={()=>query.offset=0}><option value={25}>25 / {wt("page","halaman")}</option><option value={50}>50 / {wt("page","halaman")}</option><option value={100}>100 / {wt("page","halaman")}</option></select></label><button class="btn small" disabled={query.offset===0||loading} onclick={()=>query.offset=Math.max(0,query.offset-query.limit)}><Icon name="left" size={14}/>{wt("Previous","Sebelumnya")}</button><button class="btn small" disabled={query.offset+data.entries.length>=data.total||loading} onclick={()=>query.offset+=query.limit}>{wt("Next","Berikutnya")}<Icon name="right" size={14}/></button></div>{/if}
      {/if}
    </main>
  </div>
</div>
{#if editing}<EntryEditor entry={editingEntry} {projects} {fields} defaultProjectId={query.projectId} onSaved={()=>{editing=false;scheduleRefresh();}} onClose={()=>editing=false}/>{/if}
{#if showSaveView}<dialog bind:this={viewDialog} class="save-dialog" aria-labelledby="view-title" oncancel={e=>{e.preventDefault();showSaveView=false;}}><h2 id="view-title">{wt("Save report view","Simpan tampilan laporan")}</h2><p>{wt("Keeps the period, filters, columns, and grouping.","Menyimpan periode, filter, kolom, dan pengelompokan.")}</p><form onsubmit={e=>{e.preventDefault();saveView();}}><label>{wt("View name","Nama tampilan")}<input bind:value={viewName} required maxlength="100" placeholder={wt("e.g. Weekly project review","mis. Rekap mingguan NTI")}/></label>{#if viewError}<p class="view-error" role="alert">{viewError}</p>{/if}<div class="dialog-actions"><button type="button" class="btn" onclick={()=>showSaveView=false}>{wt("Cancel","Batal")}</button>{#if selectedViewId!=null}<button type="button" class="btn" onclick={()=>{selectedViewId=null;saveView();}} disabled={!viewName.trim()||savingView}>{wt("Save as new","Simpan baru")}</button>{/if}<button class="btn primary" disabled={!viewName.trim()||savingView}>{savingView?wt("Saving…","Menyimpan…"):wt("Save view","Simpan tampilan")}</button></div></form></dialog>{/if}

<style>
  .workspace { display: flex; height: 100vh; height: 100dvh; background: var(--background); }
  .sidebar { flex: 0 0 208px; display: flex; flex-direction: column; background: var(--sidebar); border-right: 1px solid var(--separator); padding: 18px 10px 14px; min-height: 0; }
  .identity { display: flex; align-items: center; gap: 9px; padding: 0 8px 22px; }
  .brand-icon { width: 32px; height: 32px; display: grid; place-items: center; color: var(--accent-text); background: var(--accent-soft); border-radius: 9px; }
  .identity strong { font-size: 17px; letter-spacing: -.4px; font-weight: 650; }
  nav { display: flex; flex-direction: column; gap: 2px; }
  nav button,.settings-link { border: 0; background: none; display: flex; align-items: center; gap: 10px; padding: 8px 10px; border-radius: 7px; text-align: left; color: var(--secondary); }
  nav button:hover,.settings-link:hover { background: var(--fill); color: var(--label); }
  nav button.active { background: var(--accent-soft); color: var(--accent-text); font-weight: 600; }
  .sidebar-section { margin-top: 22px; min-height: 0; display: flex; flex-direction: column; }
  .sidebar-label { display: flex; align-items: center; justify-content: space-between; font-size: 11px; font-weight: 600; color: var(--tertiary); padding: 0 4px 6px 10px; }
  .sidebar-label .icon { width: 24px; min-height: 24px; padding: 4px; }
  .project-nav { display: flex; flex-direction: column; gap: 1px; max-height: 190px; overflow-y: auto; padding: 2px; margin: -2px; }
  .project-nav button { display: flex; align-items: center; gap: 9px; border: 0; background: none; padding: 7px 10px; color: var(--secondary); text-align: left; border-radius: 7px; font-size: 12px; min-width: 0; }
  .project-nav button>span:last-child { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .project-nav button:hover,.project-nav button.selected { background: var(--fill); color: var(--label); }
  .add-project { color: var(--accent-text) !important; }
  .dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
  .sidebar-bottom { margin-top: auto; padding-top: 16px; }
  .settings-link { width: 100%; }
  .live-timer { padding: 12px; margin-bottom: 10px; background: var(--surface); border: 1px solid color-mix(in srgb, var(--accent) 35%, var(--separator)); border-radius: 10px; }
  .timer-project { display: flex; align-items: center; gap: 7px; font-size: 12px; font-weight: 550; overflow: hidden; }
  .timer-controls { display: flex; align-items: center; justify-content: space-between; margin: 8px 0 4px; }
  .timer-controls strong { font-size: 24px; font-weight: 600; font-variant-numeric: tabular-nums; letter-spacing: -.8px; }
  .live-timer p { font-size: 11px; color: var(--secondary); line-height: 1.45; margin: 4px 0 0; overflow-wrap: anywhere; }
  .main-column { flex: 1; min-width: 0; min-height: 0; display: flex; flex-direction: column; }
  .toolbar { display: flex; align-items: center; gap: 12px; height: 56px; flex-shrink: 0; padding: 0 24px; border-bottom: 1px solid var(--separator); background: var(--surface); }
  h1 { font-size: 16px; font-weight: 650; letter-spacing: -.3px; margin: 0 8px 0 0; white-space: nowrap; }
  .filters { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .project-filter select { min-height: 32px; width: 150px; font-size: 12px; padding: 5px 28px 5px 9px; background-color: var(--surface); }
  .top-actions { display: flex; align-items: center; gap: 8px; margin-left: auto; }
  .syncing { color: var(--tertiary); font-size: 11px; animation: reveal 0s 400ms both; }
  @keyframes reveal { from { visibility: hidden; } }
  main { flex: 1; min-height: 0; overflow-y: auto; padding: 24px 24px 32px; }
  .menu-form { display: flex; flex-direction: column; gap: 12px; width: 260px; padding: 14px; }
  label { display: flex; flex-direction: column; gap: 5px; font-size: 11px; color: var(--secondary); min-width: 0; }
  label input, label select { color: var(--label); font-size: 12px; }
  .error-banner { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 8px 12px; border: 1px solid color-mix(in srgb, var(--red) 35%, var(--separator)); background: color-mix(in srgb, var(--red) 7%, var(--surface)); border-radius: 8px; color: var(--red); font-size: 12px; margin-bottom: 16px; overflow-wrap: anywhere; }
  .loading-state { padding: 80px 20px; display: flex; flex-direction: column; align-items: center; gap: 12px; color: var(--secondary); }
  .loading-state p { margin: 0; }
  .quick-capture { margin-bottom: 16px; }
  .table-toolbar { display: flex; align-items: center; gap: 10px; margin-bottom: 14px; flex-wrap: wrap; }
  .search-field { position: relative; width: 240px; }
  .search-field :global(svg) { position: absolute; top: 9px; left: 10px; color: var(--tertiary); pointer-events: none; }
  .search-field input { padding-left: 32px; font-size: 12px; min-height: 32px; background: var(--surface); }
  .entry-count { display: flex; align-items: center; gap: 8px; font-size: 12px; color: var(--secondary); font-variant-numeric: tabular-nums; }
  .entry-count strong { color: var(--label); font-weight: 600; }
  .entry-count>span { color: var(--tertiary); }
  .toolbar-spacer { flex: 1; }
  .column-menu { display: flex; flex-direction: column; gap: 10px; width: 250px; max-height: 360px; overflow-y: auto; margin: 0; padding: 14px; border: 0; }
  .column-menu legend { font-size: 11px; font-weight: 600; color: var(--secondary); padding: 0; margin-bottom: 10px; float: left; width: 100%; }
  .column-menu label { flex-direction: row; align-items: center; gap: 9px; color: var(--label); font-size: 12px; }
  .report-controls { display: flex; align-items: flex-end; gap: 14px; flex-wrap: wrap; margin-bottom: 16px; }
  .report-controls>label { flex: 1; min-width: 150px; max-width: 230px; }
  .report-controls select, .report-controls input { background-color: var(--surface); }
  .field-filter { display: flex; align-items: flex-end; gap: 8px; flex: 1; min-width: 220px; max-width: 360px; }
  .field-filter>label { flex: 1; }
  .report-summary { border: 1px solid var(--separator); border-radius: 12px; background: var(--surface); margin-bottom: 16px; padding: 18px 20px 14px; }
  .summary-title { display: flex; align-items: flex-end; justify-content: space-between; gap: 16px; padding-bottom: 14px; }
  h2 { font-size: 14px; font-weight: 600; margin: 0; text-transform: capitalize; }
  .summary-title div>span { display: block; font-size: 11px; color: var(--secondary); margin-top: 4px; }
  .summary-title>strong { font-size: 28px; font-weight: 650; letter-spacing: -.9px; font-variant-numeric: tabular-nums; }
  .group-list { max-height: 240px; overflow-y: auto; }
  .group-list>div { display: flex; align-items: center; gap: 16px; padding: 10px 0; border-top: 1px solid var(--separator); font-size: 12px; }
  .group-name { display: flex; align-items: center; gap: 7px; width: 160px; flex-shrink: 0; overflow-wrap: anywhere; }
  .group-name>i { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
  .group-bar { height: 5px; background: var(--fill); border-radius: 3px; flex: 1; overflow: hidden; }
  .group-bar>span { height: 100%; display: block; border-radius: inherit; }
  .group-entries { font-size: 11px; color: var(--secondary); white-space: nowrap; font-variant-numeric: tabular-nums; }
  .group-list strong { font-weight: 600; min-width: 70px; text-align: right; white-space: nowrap; font-variant-numeric: tabular-nums; }
  .group-list>p { color: var(--secondary); font-size: 12px; padding-top: 10px; border-top: 1px solid var(--separator); margin: 0; }
  .export-toolbar { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; margin-top: 14px; padding-top: 14px; border-top: 1px solid var(--separator); }
  .export-toolbar>span { color: var(--secondary); font-size: 11px; margin-right: auto; }
  .export-success { display: flex; align-items: center; gap: 9px; padding: 9px 12px; border-radius: 8px; background: var(--accent-soft); color: var(--accent-text); font-size: 12px; margin-top: 12px; flex-wrap: wrap; }
  .pagination { display: flex; align-items: center; gap: 8px; margin-top: 14px; font-size: 12px; color: var(--secondary); font-variant-numeric: tabular-nums; }
  .pagination>span { margin-right: auto; }
  .pagination select { font-size: 12px; min-height: 28px; padding: 4px 28px 4px 8px; background-color: var(--surface); }
  .save-dialog::backdrop { background: #0006; }
  .view-error { color: var(--red) !important; }
  .save-dialog { color: var(--label); width: min(430px,calc(100% - 48px)); padding: 24px; background: var(--surface); border: 1px solid var(--separator); border-radius: 14px; box-shadow: 0 20px 60px #0003; }
  .save-dialog h2 { text-transform: none; }
  .save-dialog p { font-size: 12px; color: var(--secondary); margin: 8px 0 18px; }
  .dialog-actions { display: flex; justify-content: flex-end; flex-wrap: wrap; gap: 7px; margin-top: 20px; }
  .sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); }
  @media(max-width:1000px) { .sidebar { flex-basis: 180px; } .toolbar, main { padding-left: 18px; padding-right: 18px; } .top-actions .label { display: none; } .project-filter select { width: 128px; } .group-name { width: 120px; } .group-bar { display: none; } }
</style>
