use std::path::{Path, PathBuf};

use kala_core::KalaError;
use kala_core::i18n::Lang;
use kala_core::report::{build_report, write_report_csv, write_report_xlsx};
use kala_core::workspace::WorkspaceQuery;
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportResult {
    path: String,
    rows: usize,
    total_hours: f64,
}

#[tauri::command]
pub async fn export_report(app: AppHandle, query: WorkspaceQuery, columns: Vec<String>, format: String) -> Result<ReportResult, KalaError> {
    let ext = match format.as_str() {
        "csv" => "csv", "xlsx" => "xlsx",
        _ => return Err(KalaError::InvalidEntry { reason: "report_format_invalid".into() }),
    };
    tauri::async_runtime::spawn_blocking(move || {
        let doc = {
            let state = app.state::<AppState>();
            let store = state.store.lock().map_err(|_| KalaError::Db { message: "store lock poisoned".into() })?;
            let lang = Lang::from_code(&store.settings()?.language);
            build_report(&store, &query, &columns, lang)?
        };
        let downloads = dirs::download_dir().ok_or_else(|| KalaError::Io { message: "Downloads folder unavailable".into() })?;
        std::fs::create_dir_all(&downloads)?;
        let stem = format!("Kala-report-{}-{}", query.from.format("%Y-%m-%d"), query.to.format("%Y-%m-%d"));
        let path = reserve_path(&downloads, &stem, ext)?;
        let result = if ext == "csv" { write_report_csv(&doc, &path) } else { write_report_xlsx(&doc, &path) };
        if let Err(error) = result {
            let _ = std::fs::remove_file(&path);
            return Err(error);
        }
        Ok(ReportResult { path: path.display().to_string(), rows: doc.rows.len(), total_hours: doc.total_hours })
    }).await.map_err(|error| KalaError::Io { message: error.to_string() })?
}

fn reserve_path(dir: &Path, stem: &str, ext: &str) -> Result<PathBuf, KalaError> {
    for number in 1u64.. {
        let name = if number == 1 { format!("{stem}.{ext}") } else { format!("{stem}-{number}.{ext}") };
        let path = dir.join(name);
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(_) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(KalaError::Io { message: "Report filename space exhausted".into() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_files_are_reserved_without_overwriting_existing_exports() {
        let dir = tempfile::tempdir().unwrap();
        let existing = dir.path().join("Kala-report-2026-10-01-2026-10-31.csv");
        std::fs::write(&existing, "keep previous report").unwrap();
        let next = reserve_path(dir.path(), "Kala-report-2026-10-01-2026-10-31", "csv").unwrap();
        assert_eq!(next.file_name().unwrap(), "Kala-report-2026-10-01-2026-10-31-2.csv");
        assert!(next.is_file());
        assert_eq!(std::fs::read_to_string(existing).unwrap(), "keep previous report");
        let third = reserve_path(dir.path(), "Kala-report-2026-10-01-2026-10-31", "csv").unwrap();
        assert_eq!(third.file_name().unwrap(), "Kala-report-2026-10-01-2026-10-31-3.csv");
    }
}
