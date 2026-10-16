<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
  import { onMount } from "svelte";
  import { api, PALETTE, type KalaError, type Project, type Settings } from "../lib/api";
  import { errorText, setLang, t } from "../lib/i18n.svelte";

  type Row = Project & { aliasText: string; dirty: boolean };

  let settings = $state<Settings | null>(null);
  let rows = $state<Row[]>([]);
  let recording = $state(false);
  let shortcutError = $state<KalaError | null>(null);
  let error = $state<KalaError | null>(null);
  let autostart = $state(false);
  let snippet = $state("");
  let copied = $state(false);
  let status = $state("");
  let archivedRows = $state<Project[]>([]);
  let shortcutLive = $state(true);

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
    const all = await api.listProjects(true);
    rows = all.filter((p) => !p.archived).map((p) => ({ ...p, aliasText: p.aliases.join(", "), dirty: false }));
    archivedRows = all.filter((p) => p.archived);
    shortcutLive = !s.shortcut || (await api.shortcutRegistered());
    autostart = await isEnabled().catch(() => false);
    snippet = await api.mcpConfigSnippet();
  }

  async function saveSettings() {
    if (!settings) return;
    try {
      await api.saveSettings(settings);
      setLang(settings.language);
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

  async function saveRow(r: Row) {
    try {
      const saved = await api.saveProject({
        id: r.id > 0 ? r.id : null,
        name: r.name,
        aliases: r.aliasText.split(",").map((a) => a.trim()).filter(Boolean),
        color: r.color,
        hourlyRate: Number(r.hourlyRate) || 0,
        currency: r.currency || "IDR",
        roundingMin: Number(r.roundingMin),
      });
      Object.assign(r, saved, { aliasText: saved.aliases.join(", "), dirty: false });
      error = null;
      flash();
    } catch (e) {
      error = e as KalaError;
    }
  }

  function addRow() {
    rows.push({
      id: -Date.now(),
      name: "",
      aliases: [],
      aliasText: "",
      color: PALETTE[rows.length % PALETTE.length],
      hourlyRate: 0,
      currency: "IDR",
      roundingMin: 60,
      archived: false,
      dirty: true,
    });
  }

  async function archive(r: Row) {
    if (r.id > 0) await api.archiveProject(r.id, true);
    await load();
  }

  async function restore(p: Project) {
    await api.archiveProject(p.id, false);
    await load();
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

  onMount(() => {
    load();
    const un = listen("kala://refresh", load);
    return () => {
      un.then((f) => f());
    };
  });
</script>

<svelte:window onkeydown={onRecordKey} />

{#if settings}
  <main>
    <section>
      <h2>{t("shortcut")}</h2>
      {#if !settings.shortcut}<p class="note">{t("shortcutFirstRun")}</p>{/if}
      <div class="line">
        <span class="kbd">{recording ? t("shortcutPress") : pretty(settings.shortcut)}</span>
        <button class="btn primary" onclick={() => { recording = true; shortcutError = null; }}>{t("shortcutRecord")}</button>
      </div>
      <p class="hint">{t("shortcutHint")}</p>
      {#if shortcutError}<p class="error" role="alert">{errorText(shortcutError)}</p>{/if}
      {#if !shortcutLive}<p class="error" role="alert">{t("shortcutInactive")}</p>{/if}
    </section>

    <section>
      <h2>{t("profile")}</h2>
      <label>{t("yourName")}<input bind:value={settings.userName} onchange={saveSettings} /></label>
      <label>{t("language")}
        <select bind:value={settings.language} onchange={saveSettings}>
          <option value="id">Bahasa Indonesia</option>
          <option value="en">English</option>
        </select>
      </label>
    </section>

    <section>
      <h2>{t("projects")}</h2>
      {#each rows as r (r.id)}
        <div class="project">
          <input type="color" class="swatch" bind:value={r.color} onchange={() => saveRow(r)} aria-label="color" />
          <input class="grow" placeholder={t("projectName")} bind:value={r.name} onchange={() => saveRow(r)} />
          <input class="grow" placeholder={t("aliases")} bind:value={r.aliasText} onchange={() => saveRow(r)} />
          <input class="num" type="number" min="0" step="1000" title={t("rate")} bind:value={r.hourlyRate} onchange={() => saveRow(r)} />
          <input class="cur" title={t("currency")} bind:value={r.currency} onchange={() => saveRow(r)} />
          <select title={t("rounding")} bind:value={r.roundingMin} onchange={() => saveRow(r)}>
            <option value={0}>{t("noRounding")}</option>
            <option value={15}>15m</option>
            <option value={30}>30m</option>
            <option value={60}>60m</option>
          </select>
          <button class="btn small" onclick={() => archive(r)}>{t("archive")}</button>
        </div>
      {/each}
      <p class="hint">{t("projectName")} · {t("aliases")} · {t("rate")} · {t("currency")} · {t("rounding")}</p>
      <button class="btn" onclick={addRow}>+ {t("addProject")}</button>
      {#if archivedRows.length}
        <h3>{t("archived")}</h3>
        {#each archivedRows as p (p.id)}
          <div class="project">
            <span class="dot" style:background={p.color}></span>
            <span class="grow">{p.name}</span>
            <button class="btn small" onclick={() => restore(p)}>{t("restore")}</button>
          </div>
        {/each}
      {/if}
    </section>

    <section>
      <h2>{t("reminder")}</h2>
      <label class="check"><input type="checkbox" bind:checked={settings.reminderEnabled} onchange={saveSettings} />{t("reminderOn")}</label>
      <div class="line">
        <label>{t("reminderTime")}<input type="time" value={settings.reminderTime.slice(0, 5)} onchange={(e) => { settings!.reminderTime = `${e.currentTarget.value}:00`; saveSettings(); }} /></label>
        <label>{t("workStart")}<input type="time" value={settings.workStart.slice(0, 5)} onchange={(e) => { settings!.workStart = `${e.currentTarget.value}:00`; saveSettings(); }} /></label>
      </div>
      <div class="line days" aria-label={t("workDays")}>
        {#each t("days").split(",") as label, i (i)}
          <label class="check"><input type="checkbox" checked={settings.workDays.includes(i + 1)} onchange={() => toggleDay(i + 1)} />{label}</label>
        {/each}
      </div>
    </section>

    <section>
      <h2>{t("general")}</h2>
      <label class="check"><input type="checkbox" checked={autostart} onchange={toggleAutostart} />{t("launchAtLogin")}</label>
    </section>

    <section>
      <h2>{t("mcp")}</h2>
      <p class="hint">{t("mcpHint")}</p>
      <pre>{snippet}</pre>
      <button class="btn" onclick={copy}>{copied ? t("copied") : t("copy")}</button>
    </section>

    {#if error}<p class="error" role="alert">{errorText(error)}</p>{/if}
    <p class="status" aria-live="polite">{status}</p>
  </main>
{/if}

<style>
  main { padding: 18px 22px 30px; height: 100vh; box-sizing: border-box; overflow-y: auto; }
  :global(html), :global(body) { background: Canvas; }
  section { padding: 12px 0; border-bottom: 1px solid var(--separator); display: flex; flex-direction: column; gap: 8px; }
  h2 { font-size: 13px; font-weight: 600; margin: 0; }
  label { display: flex; flex-direction: column; gap: 4px; font-size: 12px; color: var(--secondary); }
  label.check { flex-direction: row; align-items: center; gap: 6px; color: var(--label); font-size: 13px; }
  .line { display: flex; gap: 12px; align-items: center; flex-wrap: wrap; }
  .days { gap: 10px; }
  .kbd { font-size: 15px; padding: 4px 10px; border-radius: 6px; background: var(--fill); min-width: 90px; text-align: center; }
  .hint { font-size: 11px; color: var(--tertiary); margin: 0; }
  .note { font-size: 12px; margin: 0; color: var(--accent); }
  .error { color: var(--red); font-size: 12px; margin: 0; }
  .status { color: var(--secondary); font-size: 11px; min-height: 14px; margin: 8px 0 0; }
  .project { display: flex; gap: 6px; align-items: center; }
  .project .grow { flex: 1; min-width: 0; }
  .project .num { width: 90px; }
  .project .cur { width: 52px; }
  h3 { font-size: 12px; font-weight: 600; margin: 6px 0 0; color: var(--secondary); }
  .dot { width: 10px; height: 10px; border-radius: 50%; }
  .swatch { width: 26px; height: 24px; padding: 0; border: 0; background: none; }
  pre { margin: 0; padding: 8px; border-radius: 6px; background: var(--fill); font-size: 11px; white-space: pre-wrap; word-break: break-all; -webkit-user-select: text; user-select: text; }
</style>
