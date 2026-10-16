<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
  import { onMount } from "svelte";
  import { api, type KalaError, type Project, type Settings } from "../lib/api";
  import Icon from "../lib/Icon.svelte";
  import { errorText, setLang, t } from "../lib/i18n.svelte";

  import ProjectManager from "../lib/ProjectManager.svelte";
  import TimeInput from "../lib/ui/TimeInput.svelte";
  import { wapi, type FieldDefinition } from "../lib/workspace-api";

  let settings = $state<Settings | null>(null);
  let projects = $state<Project[]>([]);
  let fields = $state<FieldDefinition[]>([]);
  let recording = $state(false);
  let shortcutError = $state<KalaError | null>(null);
  let error = $state<KalaError | null>(null);
  let autostart = $state(false);
  let snippet = $state("");
  let copied = $state(false);
  let status = $state("");
  let shortcutLive = $state(true);
  const pages = [
    { id: "general", label: "general", hint: "generalHint", icon: "general" },
    { id: "projects", label: "projects", hint: "projectsHint", icon: "projects" },
    { id: "reminder", label: "reminder", hint: "reminderHint", icon: "bell" },
    { id: "integrations", label: "integrations", hint: "integrationsHint", icon: "plug" },
  ] as const;
  let selected = $state<(typeof pages)[number]["id"]>("general");
  const currentPage = $derived(pages.find((page) => page.id === selected)!);

  const MOD_LABEL: Record<string, string> = { Command: "⌘", Control: "⌃", Alt: "⌥", Shift: "⇧" };
  const pretty = (acc: string | null) =>
    acc ? acc.split("+").map((p) => MOD_LABEL[p] ?? p).join("") : t("shortcutNone");

  async function load() {
    const s = await api.getSettings();
    if (!s.onboarded) {
      s.language = navigator.language.toLowerCase().startsWith("id") ? "id" : "en";
      s.onboarded = true;
      await api.saveSettings(s);
    }
    setLang(s.language);
    settings = s;
    await reloadProjects();
    shortcutLive = !s.shortcut || (await api.shortcutRegistered());
    autostart = await isEnabled().catch(() => false);
    snippet = await api.mcpConfigSnippet();
  }

  async function saveSettings() {
    if (!settings) return;
    try {
      await api.saveSettings(settings);
      setLang(settings.language);
      error = null;
      flash();
    } catch (e) {
      error = e as KalaError;
    }
  }

  function flash() {
    status = t("saved");
    setTimeout(() => (status = ""), 1500);
  }

  function accelerator(e: KeyboardEvent): string | null {
    const mods = [e.metaKey && "Command", e.ctrlKey && "Control", e.altKey && "Alt", e.shiftKey && "Shift"].filter(Boolean);
    if (!(e.metaKey || e.ctrlKey || e.altKey) || ["Meta", "Control", "Alt", "Shift"].includes(e.key)) return null;
    const key = e.code.startsWith("Key") ? e.code.slice(3) : e.code.startsWith("Digit") ? e.code.slice(5) : e.code;
    return [...mods, key].join("+");
  }

  async function onRecordKey(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    if (e.key === "Escape") {
      recording = false;
      return;
    }
    const acc = accelerator(e);
    if (!acc) return;
    recording = false;
    try {
      settings = await api.setShortcut(acc);
      shortcutError = null;
      shortcutLive = await api.shortcutRegistered();
      flash();
    } catch (err) {
      shortcutError = err as KalaError;
    }
  }

  async function reloadProjects() {
    try {
      [projects, fields] = await Promise.all([api.listProjects(true), wapi.fields(null, true)]);
      error = null;
    } catch (err) {
      error = err as KalaError;
    }
  }

  async function toggleAutostart() {
    autostart ? await disable() : await enable();
    autostart = await isEnabled();
  }

  async function copy() {
    await navigator.clipboard.writeText(snippet);
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }

  const toggleDay = (d: number) => {
    if (!settings) return;
    settings.workDays = settings.workDays.includes(d) ? settings.workDays.filter((x) => x !== d) : [...settings.workDays, d].sort();
    saveSettings();
  };

  const reload = () => load().catch((e) => error = e as KalaError);

  onMount(() => {
    reload();
    const un = listen("kala://refresh", reload);
    return () => {
      un.then((f) => f());
    };
  });
</script>

<svelte:window onkeydown={onRecordKey} />

<div class="settings-shell">
  <aside>
    <div class="identity"><span class="app-mark"><Icon name="kala" size={29} /></span><div><strong>Kala</strong><span>{t("settings")}</span></div></div>
    <nav aria-label={t("settings")}>
      {#each pages as page}
        <button class:active={selected === page.id} aria-current={selected === page.id ? "page" : undefined} onclick={() => { selected = page.id; recording = false; }}><Icon name={page.icon} size={17} />{t(page.label)}</button>
      {/each}
    </nav>
    <div class="sidebar-note"><Icon name="kala" size={15} /><span>Kala</span></div>
  </aside>

  <div class="settings-main">
    <header><h1>{t(currentPage.label)}</h1><p>{t(currentPage.hint)}</p></header>
    <main>
      {#if settings}
        <div hidden={selected !== "general"} class="page">
          <section class="group">
            <div class="section-title"><h2>{t("profile")}</h2><p>{t("profileHint")}</p></div>
            <div class="profile-fields">
              <label>{t("yourName")}<input bind:value={settings.userName} onchange={saveSettings} autocomplete="name" /></label>
              <label>{t("language")}<select bind:value={settings.language} onchange={saveSettings}><option value="id">Bahasa Indonesia</option><option value="en">English</option></select></label>
            </div>
          </section>
          <section class="group">
            <div class="section-title"><h2 id="appearance-title">{t("appearance")}</h2><p>{t("appearanceHint")}</p></div>
            <div class="segmented" role="radiogroup" aria-labelledby="appearance-title">
              {#each [["system", t("themeSystem")], ["light", t("themeLight")], ["dark", t("themeDark")]] as [value, label] (value)}
                <label><input type="radio" name="theme" {value} bind:group={settings.theme} onchange={saveSettings} /><span>{label}</span></label>
              {/each}
            </div>
          </section>
          <section class="group">
            <div class="section-title"><h2>{t("shortcut")}</h2><p>{t("shortcutDescription")}</p></div>
            {#if !settings.shortcut}<p class="note">{t("shortcutFirstRun")}</p>{/if}
            <div class="shortcut-controls"><span class="kbd" class:recording><Icon name="keyboard" size={18} />{recording ? t("shortcutPress") : pretty(settings.shortcut)}</span><button class="btn" class:primary={recording} onclick={() => { recording = !recording; shortcutError = null; }}>{recording ? t("cancel") : t("shortcutRecord")}</button></div>
            <p class="hint">{t("shortcutHint")}</p>
            {#if shortcutError}<p class="error" role="alert">{errorText(shortcutError)}</p>{/if}
            {#if !shortcutLive}<p class="error" role="alert">{t("shortcutInactive")}</p>{/if}
          </section>
          <section class="group"><div class="switch-row"><div><h2>{t("launchAtLogin")}</h2><p>{t("launchHint")}</p></div><label class="switch"><input type="checkbox" role="switch" checked={autostart} onchange={toggleAutostart} aria-label={t("launchAtLogin")} /><span></span></label></div></section>
        </div>

        <div hidden={selected !== "projects"} class="page">
          <ProjectManager {projects} {fields} onChanged={reloadProjects} />
        </div>

        <div hidden={selected !== "reminder"} class="page">
          <section class="group"><div class="switch-row"><div><h2>{t("reminderOn")}</h2><p>{t("reminderDescription")}</p></div><label class="switch"><input type="checkbox" role="switch" bind:checked={settings.reminderEnabled} onchange={saveSettings} aria-label={t("reminderOn")} /><span></span></label></div></section>
          <section class="group"><h2>{t("schedule")}</h2><div class="two-fields"><label>{t("workStart")}<TimeInput value={settings.workStart.slice(0, 5)} onchange={(time) => { settings!.workStart = `${time}:00`; saveSettings(); }} /></label><label>{t("reminderTime")}<TimeInput value={settings.reminderTime.slice(0, 5)} onchange={(time) => { settings!.reminderTime = `${time}:00`; saveSettings(); }} /></label></div>
            <fieldset class="workdays"><legend>{t("workDays")}</legend><div class="days">{#each t("days").split(",") as day, i (i)}<label class="day"><input type="checkbox" checked={settings.workDays.includes(i + 1)} onchange={() => toggleDay(i + 1)} /><span>{day}</span></label>{/each}</div></fieldset>
          </section>
        </div>

        <div hidden={selected !== "integrations"} class="page">
          <section class="group"><div class="integration-heading"><span class="integration-icon"><Icon name="plug" size={22} /></span><div><h2>{t("mcp")}</h2><p>{t("mcpHint")}</p></div></div><pre>{snippet}</pre><div class="copy-row"><button class="btn" onclick={copy}><Icon name={copied ? "check" : "copy"} size={14} />{copied ? t("copied") : t("copy")}</button></div></section>
        </div>
      {/if}
    </main>
    <footer>{#if error}<p class="error" role="alert">{errorText(error)}</p>{:else}<span class:saved={!!status} role="status"><Icon name="check" size={14} />{status || t("automaticSave")}</span>{/if}</footer>
  </div>
</div>

<style>
  .settings-shell { display: flex; height: 100vh; height: 100dvh; background: var(--background); }
  aside { flex: 0 0 176px; padding: 26px 12px 16px; background: var(--sidebar); border-right: 1px solid var(--separator); display: flex; flex-direction: column; }
  .identity { display: flex; align-items: center; gap: 10px; margin: 0 8px 30px; }
  .app-mark { display: grid; place-items: center; width: 42px; height: 42px; background: var(--accent-soft); color: var(--accent-text); border-radius: 12px; }
  .identity strong { display: block; font-size: 21px; font-weight: 650; letter-spacing: -.7px; }
  .identity div > span { color: var(--secondary); font-size: 11px; }
  nav { display: flex; flex-direction: column; gap: 5px; }
  nav button { display: flex; align-items: center; gap: 10px; padding: 10px 11px; border: 0; border-radius: 8px; background: transparent; text-align: left; color: var(--secondary); font-size: 13px; }
  nav button:hover { background: var(--fill); color: var(--label); }
  nav button.active { color: var(--accent-text); background: var(--accent-soft); font-weight: 600; }
  .sidebar-note { display: flex; align-items: center; gap: 7px; margin: auto 10px 0; padding-top: 24px; color: var(--tertiary); font-size: 11px; }
  .settings-main { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  header { padding: 27px 28px 20px; flex-shrink: 0; }
  h1 { font-size: 25px; letter-spacing: -.8px; font-weight: 650; line-height: 1.2; margin: 0 0 8px; }
  header p { color: var(--secondary); margin: 0; font-size: 12px; }
  main { flex: 1; min-height: 0; padding: 0 28px 24px; overflow-y: auto; }
  .page { display: flex; flex-direction: column; gap: 16px; max-width: 760px; }
  .page[hidden] { display: none; }
  .group { background: var(--surface); border: 1px solid var(--separator); border-radius: 12px; padding: 18px; display: flex; flex-direction: column; gap: 16px; }
  h2 { font-size: 13px; font-weight: 600; margin: 0; }
  .section-title p, .switch-row p, .integration-heading p { font-size: 12px; color: var(--secondary); margin: 5px 0 0; line-height: 1.5; }
  label { display: flex; flex-direction: column; gap: 6px; font-size: 12px; color: var(--secondary); min-width: 0; }
  label :global(input), label select { color: var(--label); font-size: 13px; }
  .profile-fields { display: grid; grid-template-columns: 1.2fr 1fr; gap: 14px; }
  .shortcut-controls { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
  .kbd { display: flex; align-items: center; justify-content: center; gap: 12px; font-size: 16px; font-weight: 500; padding: 9px 14px; border: 1px solid var(--separator); border-bottom-width: 3px; border-radius: 8px; background: var(--field); min-width: 132px; min-height: 44px; }
  .kbd :global(svg) { color: var(--tertiary); }
  .kbd.recording { color: var(--accent-text); border-color: var(--accent); background: var(--accent-soft); }
  .hint { font-size: 11px; color: var(--tertiary); line-height: 1.5; margin: 0; }
  .note { font-size: 12px; margin: 0; color: var(--accent-text); }
  .error { color: var(--red); font-size: 12px; margin: 0; overflow-wrap: anywhere; }
  .switch-row { display: flex; align-items: center; gap: 20px; justify-content: space-between; }
  .switch { display: block; position: relative; width: 34px; height: 20px; flex-shrink: 0; }
  .switch input { position: absolute; inset: 0; width: 100%; height: 100%; margin: 0; opacity: 0; z-index: 1; }
  .switch span { display: block; height: 20px; border-radius: 12px; background: var(--fill); border: 1px solid var(--tertiary); transition: background 120ms; }
  .switch span::after { content: ""; display: block; width: 14px; height: 14px; margin: 2px; border-radius: 50%; background: var(--secondary); transition: transform 120ms; }
  .switch input:checked + span { background: var(--accent); border-color: var(--accent); }
  .switch input:checked + span::after { transform: translateX(14px); background: white; }
  .switch input:focus-visible + span { outline: 2px solid var(--accent-text); outline-offset: 3px; }
  .segmented { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 3px; padding: 3px; max-width: 380px; background: var(--fill); border-radius: 9px; }
  .segmented label { position: relative; }
  .segmented input { position: absolute; inset: 0; width: 100%; height: 100%; margin: 0; opacity: 0; }
  .segmented span { display: grid; place-items: center; min-height: 30px; border-radius: 7px; font-size: 12px; color: var(--secondary); }
  .segmented input:checked + span { background: var(--surface); color: var(--label); font-weight: 600; box-shadow: 0 1px 2px #00000026; }
  .segmented input:focus-visible + span { outline: 2px solid var(--accent-text); outline-offset: 1px; }
  .two-fields { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }
  .workdays { border: 0; padding: 0; margin: 0; min-width: 0; }
  legend { font-size: 12px; color: var(--secondary); padding: 0; margin-bottom: 8px; }
  .days { display: grid; grid-template-columns: repeat(7, minmax(0, 1fr)); gap: 5px; }
  .day { position: relative; }
  .day input { position: absolute; width: 100%; height: 100%; margin: 0; opacity: 0; }
  .day span { display: grid; place-items: center; min-height: 36px; font-size: 11px; border: 1px solid var(--separator); border-radius: 7px; color: var(--secondary); background: var(--field); }
  .day input:checked + span { color: var(--accent-text); border-color: var(--accent); background: var(--accent-soft); font-weight: 600; }
  .day input:focus-visible + span { outline: 2px solid var(--accent-text); outline-offset: 2px; }
  .integration-heading { display: flex; gap: 12px; align-items: flex-start; }
  .integration-icon { display: grid; place-items: center; width: 40px; height: 40px; background: var(--accent-soft); color: var(--accent-text); border-radius: 10px; flex-shrink: 0; }
  pre { margin: 0; padding: 14px; border: 1px solid var(--separator); border-radius: 8px; background: var(--field); font-size: 11px; line-height: 1.65; white-space: pre-wrap; overflow-wrap: anywhere; user-select: text; -webkit-user-select: text; }
  .copy-row { display: flex; justify-content: flex-end; }
  footer { min-height: 45px; padding: 12px 28px; border-top: 1px solid var(--separator); flex-shrink: 0; }
  footer > span { display: flex; align-items: center; gap: 6px; font-size: 11px; color: var(--tertiary); }
  footer > span.saved { color: var(--accent-text); }
  @media (max-width: 680px) {
    aside { flex-basis: 150px; padding-left: 8px; padding-right: 8px; }
    header { padding: 24px 20px 18px; }
    main { padding-left: 20px; padding-right: 20px; }
    footer { padding-left: 20px; padding-right: 20px; }
    .group { padding: 15px; }
    .profile-fields { grid-template-columns: 1fr; }
  }
  @media (max-width: 500px) {
    .settings-shell { flex-direction: column; }
    aside { flex: 0 0 auto; padding: 8px; border-right: 0; border-bottom: 1px solid var(--separator); }
    .identity, .sidebar-note { display: none; }
    nav { flex-direction: row; gap: 3px; }
    nav button { flex: 1; justify-content: center; padding: 9px 5px; font-size: 11px; gap: 5px; }
    nav button :global(svg) { width: 14px; }
    .days { gap: 3px; }
  }
</style>
