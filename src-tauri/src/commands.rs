use std::sync::MutexGuard;

use chrono::{Local, NaiveDate, NaiveDateTime};
use kala_core::export::{build_export, default_file_name, write_csv, write_xlsx};
use kala_core::i18n::Lang;
use kala_core::store::{Entry, EntryPatch, LogOutcome, Planned, ProjectInput, Settings, Store};
use kala_core::{KalaError, Month, Project};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{AppState, tray};

type Res<T> = Result<T, KalaError>;

pub fn now() -> NaiveDateTime {
    Local::now().naive_local()
}

fn store<'a>(state: &'a State<AppState>) -> Res<MutexGuard<'a, Store>> {
    state.store.lock().map_err(|_| KalaError::Db { message: "store lock poisoned".into() })
}

fn day(s: &str) -> Res<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| KalaError::InvalidEntry { reason: "date_format".into() })
}

/// Runs a store call, then refreshes the menu bar timer title once the lock is released.
fn touching_timer<T>(app: &AppHandle, state: &State<AppState>, f: impl FnOnce(&Store) -> Res<T>) -> Res<T> {
    let out = f(&*store(state)?);
    tray::refresh_title(app);
    out
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayView {
    date: NaiveDate,
    entries: Vec<Entry>,
    total_minutes: i64,
}

#[tauri::command]
pub fn preview_text(state: State<AppState>, text: String) -> Res<Planned> {
    store(&state)?.plan_text(&text, now())
}

#[tauri::command]
pub fn submit_text(app: AppHandle, state: State<AppState>, text: String) -> Res<LogOutcome> {
    touching_timer(&app, &state, |s| s.log_text(&text, now()))
}

#[tauri::command]
pub fn list_day(state: State<AppState>, date: String) -> Res<DayView> {
    let date = day(&date)?;
    let entries = store(&state)?.entries_between(date, date, None)?;
    let total_minutes = entries.iter().filter_map(|e| e.end.map(|end| (end - e.start).num_minutes())).sum();
    Ok(DayView { date, entries, total_minutes })
}

#[tauri::command]
pub fn update_entry(app: AppHandle, state: State<AppState>, id: i64, patch: EntryPatch) -> Res<Entry> {
    touching_timer(&app, &state, |s| s.update_entry(id, &patch))
}

#[tauri::command]
pub fn delete_entry(app: AppHandle, state: State<AppState>, id: i64) -> Res<()> {
    touching_timer(&app, &state, |s| s.delete_entry(id))
}

#[tauri::command]
pub fn timer_status(state: State<AppState>) -> Res<Option<Entry>> {
    store(&state)?.running()
}

#[tauri::command]
pub fn timer_start(app: AppHandle, state: State<AppState>, project_id: i64, description: String) -> Res<LogOutcome> {
    touching_timer(&app, &state, |s| s.timer_start(project_id, &description, now()))
}

#[tauri::command]
pub fn timer_stop(app: AppHandle, state: State<AppState>, description: Option<String>) -> Res<Entry> {
    touching_timer(&app, &state, |s| s.timer_stop(description.as_deref(), now()))
}

#[tauri::command]
pub fn list_projects(state: State<AppState>, include_archived: bool) -> Res<Vec<Project>> {
    store(&state)?.projects(include_archived)
}

#[tauri::command]
pub fn save_project(state: State<AppState>, input: ProjectInput) -> Res<Project> {
    store(&state)?.save_project(&input)
}

#[tauri::command]
pub fn archive_project(state: State<AppState>, id: i64, archived: bool) -> Res<()> {
    store(&state)?.set_archived(id, archived)
}

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Res<Settings> {
    store(&state)?.settings()
}

/// Shortcut and last reminder date belong to the backend; a stale UI copy must not roll them back.
#[tauri::command]
pub fn save_settings(state: State<AppState>, mut settings: Settings) -> Res<()> {
    let s = store(&state)?;
    let cur = s.settings()?;
    settings.shortcut = cur.shortcut;
    settings.last_reminder_date = cur.last_reminder_date;
    s.save_settings(&settings)
}

#[tauri::command]
pub fn set_shortcut(app: AppHandle, state: State<AppState>, accelerator: String) -> Res<Settings> {
    let mut settings = store(&state)?.settings()?;
    crate::shortcut::apply(&app, &accelerator, settings.shortcut.as_deref())?;
    settings.shortcut = Some(accelerator);
    store(&state)?.save_settings(&settings)?;
    Ok(settings)
}

#[tauri::command]
pub fn mcp_config_snippet() -> String {
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "/Applications/Kala.app/Contents/MacOS/kala".into());
    let json = serde_json::json!({ "mcpServers": { "kala": { "command": exe, "args": ["mcp"] } } });
    format!("claude mcp add --scope user kala -- \"{exe}\" mcp\n\n{}", serde_json::to_string_pretty(&json).unwrap_or_default())
}

#[tauri::command]
pub fn hide_window(app: AppHandle, label: String) {
    if let Some(w) = app.get_webview_window(&label) {
        let _ = w.hide();
    }
}

#[tauri::command]
pub fn open_settings(app: AppHandle) {
    if let Some(w) = app.get_webview_window("popover") {
        let _ = w.hide();
    }
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.center();
        tray::show_focused(&app, &w);
        let _ = w.emit("kala://refresh", ());
    }
}

/// macOS hotkeys are non-exclusive, so a conflict can't always be detected up front;
/// this tells Settings whether the saved shortcut is actually live.
#[tauri::command]
pub fn shortcut_registered(app: AppHandle, state: State<AppState>) -> Res<bool> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
    let saved = store(&state)?.settings()?.shortcut;
    Ok(saved.and_then(|a| a.parse::<Shortcut>().ok()).is_some_and(|sc| app.global_shortcut().is_registered(sc)))
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    path: String,
    rows: usize,
    total_hours: f64,
}

/// Writes straight to Downloads: a save panel can't come to the front from a non-activating panel.
#[tauri::command]
pub fn export_month(state: State<AppState>, project_id: i64, month: String, format: String) -> Res<ExportResult> {
    let month = Month::parse(&month).ok_or(KalaError::InvalidEntry { reason: "month_format".into() })?;
    let s = store(&state)?;
    let project = s.projects(true)?.into_iter().find(|p| p.id == project_id).ok_or(KalaError::InvalidEntry { reason: "project_not_found".into() })?;
    let doc = build_export(&s, project_id, month, Lang::from_code(&s.settings()?.language))?;
    let ext = if format == "csv" { "csv" } else { "xlsx" };
    let dir = dirs::download_dir().unwrap_or_else(std::env::temp_dir);
    let base = default_file_name(&project.name, month, ext);
    let stem = base.trim_end_matches(&format!(".{ext}")).to_string();
    let path = (1..)
        .map(|n| if n == 1 { dir.join(&base) } else { dir.join(format!("{stem}-{n}.{ext}")) })
        .find(|p| !p.exists())
        .expect("unbounded range");
    if ext == "csv" { write_csv(&doc, &path)? } else { write_xlsx(&doc, &path)? }
    Ok(ExportResult { path: path.display().to_string(), rows: doc.rows.len(), total_hours: doc.total_hours })
}

/// Only exports Kala wrote: an existing file inside Downloads (after resolving `..` and symlinks).
pub fn revealable(path: &std::path::Path, downloads: &std::path::Path) -> bool {
    match (path.is_absolute(), path.canonicalize(), downloads.canonicalize()) {
        (true, Ok(p), Ok(d)) => p.is_file() && p.starts_with(d),
        _ => false,
    }
}

#[tauri::command]
pub fn reveal_path(path: String) {
    let path = std::path::PathBuf::from(path);
    let Some(downloads) = dirs::download_dir() else { return };
    if !revealable(&path, &downloads) {
        return;
    }
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg("-R").arg(&path).spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_existing_files_inside_downloads_are_revealed() {
        let dir = tempfile::tempdir().unwrap();
        let downloads = dir.path().join("Downloads");
        std::fs::create_dir(&downloads).unwrap();
        let file = downloads.join("Kala-Acme-2026-10.xlsx");
        std::fs::write(&file, "x").unwrap();
        assert!(revealable(&file, &downloads));
        assert!(!revealable(&downloads.join("missing.xlsx"), &downloads));
        assert!(!revealable(std::path::Path::new("Kala.xlsx"), &downloads));
        let outside = dir.path().join("secret.txt");
        std::fs::write(&outside, "x").unwrap();
        assert!(!revealable(&outside, &downloads));
        assert!(!revealable(&downloads.join("../secret.txt"), &downloads));
    }
}
