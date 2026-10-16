<script lang="ts">
  import { api, hhmm, minutesBetween, PALETTE, type KalaError, type LogOutcome, type Planned } from "./api";
  import { dayLabel, duration, errorText, t } from "./i18n.svelte";

  let {
    onDone,
    onCancel,
    autofocus = false,
  }: { onDone: (o: LogOutcome) => void; onCancel: () => void; autofocus?: boolean } = $props();

  let text = $state("");
  let preview = $state<Planned | null>(null);
  let error = $state<KalaError | null>(null);
  let busy = $state(false);
  let input: HTMLInputElement;
  let timer: ReturnType<typeof setTimeout> | undefined;

  export function focus() {
    input?.focus();
  }

  function refresh() {
    clearTimeout(timer);
    timer = setTimeout(async () => {
      if (!text.trim()) {
        preview = null;
        error = null;
        return;
      }
      try {
        preview = await api.previewText(text);
        error = null;
      } catch (e) {
        preview = null;
        error = e as KalaError;
      }
    }, 80);
  }

  async function submit() {
    if (busy || !text.trim()) return;
    busy = true;
    try {
      const outcome = await api.submitText(text);
      text = "";
      preview = null;
      error = null;
      onDone(outcome);
    } catch (e) {
      error = e as KalaError;
    } finally {
      busy = false;
    }
  }

  function replaceProject(name: string) {
    const token = String(error?.token ?? "");
    const words = text.split(/(\s+)/);
    const at = words.findIndex((w) => w === token);
    if (at >= 0) words[at] = name.toLowerCase();
    text = words.join("");
    refresh();
    input.focus();
  }

  async function createProject() {
    const name = String(error?.token ?? "");
    try {
      await saveNewProject(name);
    } catch (e) {
      error = e as KalaError;
      return;
    }
    refresh();
    input.focus();
  }

  async function saveNewProject(name: string) {
    const used = (await api.listProjects(true)).length;
    await api.saveProject({
      id: null,
      name: name.toUpperCase() === name ? name : name[0].toUpperCase() + name.slice(1),
      aliases: [],
      color: PALETTE[used % PALETTE.length],
      hourlyRate: 0,
      currency: "IDR",
      roundingMin: 60,
    });
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      submit();
    } else if (e.key === "Escape") {
      e.preventDefault();
      text = "";
      preview = null;
      error = null;
      onCancel();
    }
  }
</script>

<div class="quick">
  <!-- svelte-ignore a11y_autofocus -->
  <input
    bind:this={input}
    bind:value={text}
    oninput={refresh}
    {onkeydown}
    {autofocus}
    placeholder={t("placeholder")}
    spellcheck="false"
    autocomplete="off"
    aria-label={t("placeholder")}
  />
  {#if preview}
    <div class="chips" aria-live="polite">
      <span class="chip"><span class="dot" style:background={preview.project.color}></span>{preview.project.name}</span>
      {#if preview.kind === "log"}
        <span class="chip">{dayLabel(preview.start.slice(0, 10))}</span>
        <span class="chip">{hhmm(preview.start)}–{hhmm(preview.end)} · {duration(minutesBetween(preview.start, preview.end))}</span>
      {:else}
        <span class="chip">▶ {t("startsTimer")}</span>
      {/if}
      {#if preview.description}<span class="chip desc">{preview.description}</span>{/if}
      {#if preview.kind === "log" && preview.overlaps}<span class="warn">⚠ {t("overlap")}</span>{/if}
    </div>
  {:else if error && errorText(error)}
    <div class="error" role="alert">
      {errorText(error)}
      {#if error.code === "ProjectAmbiguous"}
        {#each (error.candidates as string[]) ?? [] as c (c)}
          <button class="link" onclick={() => replaceProject(c)}>{c}</button>
        {/each}
      {:else if error.code === "ProjectNotFound"}
        <button class="link" onclick={createProject}>{t("createProject", { name: error.token })}</button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .quick { display: flex; flex-direction: column; gap: 6px; }
  input {
    width: 100%;
    box-sizing: border-box;
    font: inherit;
    font-size: 15px;
    padding: 8px 10px;
    border-radius: 7px;
    border: 1px solid var(--separator);
    background: var(--field);
    color: var(--label);
    outline: none;
  }
  input:focus { border-color: var(--accent); box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 35%, transparent); }
  .chips { display: flex; flex-wrap: wrap; gap: 5px; align-items: center; }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 2px 8px;
    border-radius: 5px;
    background: var(--fill);
    font-size: 12px;
    color: var(--label);
  }
  .chip.desc { color: var(--secondary); max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
  .warn { font-size: 12px; color: var(--orange); }
  .error { font-size: 12px; color: var(--red); display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
</style>
