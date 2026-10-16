use std::sync::MutexGuard;

use kala_core::KalaError;
use kala_core::store::Store;
use kala_core::workspace::{
    FieldDefinition, FieldInput, SavedView, ViewInput, WorkspaceData, WorkspaceEntry,
    WorkspaceEntryInput, WorkspaceQuery,
};
use tauri::{AppHandle, Emitter, State};

use crate::AppState;

type Res<T> = Result<T, KalaError>;

fn store<'a>(state: &'a State<AppState>) -> Res<MutexGuard<'a, Store>> {
    state.store.lock().map_err(|_| KalaError::Db { message: "store lock poisoned".into() })
}

#[tauri::command]
pub fn list_fields(state: State<AppState>, project_id: Option<i64>, include_archived: Option<bool>) -> Res<Vec<FieldDefinition>> {
    store(&state)?.fields(project_id, include_archived.unwrap_or(false))
}

#[tauri::command]
pub fn save_field(app: AppHandle, state: State<AppState>, input: FieldInput) -> Res<FieldDefinition> {
    let saved = store(&state)?.save_field(&input)?;
    let _ = app.emit("kala://data-changed", ());
    Ok(saved)
}

#[tauri::command]
pub fn workspace_data(state: State<AppState>, query: WorkspaceQuery) -> Res<WorkspaceData> {
    store(&state)?.workspace(&query)
}

#[tauri::command]
pub fn save_workspace_entry(app: AppHandle, state: State<AppState>, input: WorkspaceEntryInput) -> Res<WorkspaceEntry> {
    let saved = store(&state)?.save_workspace_entry(&input)?;
    let _ = app.emit("kala://data-changed", ());
    Ok(saved)
}

#[tauri::command]
pub fn list_saved_views(state: State<AppState>) -> Res<Vec<SavedView>> {
    store(&state)?.saved_views()
}

#[tauri::command]
pub fn save_saved_view(app: AppHandle, state: State<AppState>, input: ViewInput) -> Res<SavedView> {
    let saved = store(&state)?.save_view(&input)?;
    let _ = app.emit("kala://data-changed", ());
    Ok(saved)
}

#[tauri::command]
pub fn delete_saved_view(app: AppHandle, state: State<AppState>, id: i64) -> Res<()> {
    store(&state)?.delete_view(id)?;
    let _ = app.emit("kala://data-changed", ());
    Ok(())
}
