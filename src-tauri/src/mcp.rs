use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use chrono::{Local, NaiveDate, NaiveDateTime, NaiveTime};
use kala_core::export::{build_export, write_csv, write_xlsx};
use kala_core::i18n::Lang;
use kala_core::parse::resolve_project;
use kala_core::store::{Entry, EntryPatch, Store};
use kala_core::{KalaError, Month, Project};
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::{ServerHandler, ServiceExt, handler::server::wrapper::Parameters, schemars, tool, tool_handler, tool_router, transport::stdio};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Clone)]
pub struct KalaMcp {
    store: Arc<Mutex<Store>>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct LogTimeArgs {
    #[schemars(description = "One line: [date] <project> [time] <description>. Examples: 'acme 2j fix role', '16/10 globex 13-15 meeting', 'kemarin acme 1.5h review'. No time = start a timer.")]
    pub text: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListArgs {
    #[schemars(description = "Project name or alias. Omit for all projects.")]
    pub project: Option<String>,
    #[schemars(description = "YYYY-MM-DD, default first day of this month")]
    pub from: Option<String>,
    #[schemars(description = "YYYY-MM-DD inclusive, default last day of this month")]
    pub to: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UpdateArgs {
    pub id: i64,
    #[schemars(description = "Project name or alias")]
    pub project: Option<String>,
    #[schemars(description = "YYYY-MM-DD")]
    pub date: Option<String>,
    #[schemars(description = "HH:MM")]
    pub start: Option<String>,
    #[schemars(description = "HH:MM, on the entry's date")]
    pub end: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct IdArgs {
    pub id: i64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SummaryArgs {
    #[schemars(description = "YYYY-MM, default this month")]
    pub month: Option<String>,
    pub project: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ExportArgs {
    pub project: String,
    #[schemars(description = "YYYY-MM")]
    pub month: String,
    #[schemars(description = "Absolute output file path")]
    pub path: String,
    #[schemars(description = "xlsx (default) or csv")]
    pub format: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TimerStartArgs {
    pub project: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TimerStopArgs {
    #[schemars(description = "Optional; replaces the timer's description")]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct Empty {}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EntryView {
    id: i64,
    project: String,
    start: NaiveDateTime,
    end: Option<NaiveDateTime>,
    description: String,
    hours: Option<f64>,
}

fn now() -> NaiveDateTime {
    Local::now().naive_local()
}

fn bad(reason: &str) -> KalaError {
    KalaError::InvalidEntry { reason: reason.into() }
}

fn date_arg(s: &str) -> Result<NaiveDate, KalaError> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| bad("date_format_yyyy_mm_dd"))
}

fn time_arg(s: &str) -> Result<NaiveTime, KalaError> {
    NaiveTime::parse_from_str(s, "%H:%M").map_err(|_| bad("time_format_hh_mm"))
}

fn month_arg(s: Option<&str>) -> Result<Month, KalaError> {
    match s {
        Some(m) => Month::parse(m).ok_or_else(|| bad("month_format_yyyy_mm")),
        None => Ok(Month::of(now().date())),
    }
}

fn reply<T: Serialize>(r: Result<T, KalaError>) -> String {
    match r {
        Ok(v) => serde_json::to_string(&v).unwrap_or_default(),
        Err(e) => json!({ "error": e }).to_string(),
    }
}

impl KalaMcp {
    pub fn new(store: Store) -> Self {
        KalaMcp { store: Arc::new(Mutex::new(store)) }
    }

    fn with<T>(&self, f: impl FnOnce(&Store) -> Result<T, KalaError>) -> Result<T, KalaError> {
        let store = self.store.lock().map_err(|_| KalaError::Db { message: "store lock poisoned".into() })?;
        f(&store)
    }

    /// Logging and timers only see active projects; reads and exports also reach archived ones.
    fn project(store: &Store, token: &str, include_archived: bool) -> Result<Project, KalaError> {
        let projects = store.projects(include_archived)?;
        resolve_project(token, &projects, include_archived).cloned()
    }

    fn views(store: &Store, entries: Vec<Entry>) -> Result<Vec<EntryView>, KalaError> {
        let projects = store.projects(true)?;
        Ok(entries
            .into_iter()
            .map(|e| EntryView {
                id: e.id,
                project: projects.iter().find(|p| p.id == e.project_id).map(|p| p.name.clone()).unwrap_or_default(),
                hours: e.end.map(|end| ((end - e.start).num_minutes() as f64 / 60.0 * 100.0).round() / 100.0),
                start: e.start,
                end: e.end,
                description: e.description,
            })
            .collect())
    }
}

#[tool_router]
impl KalaMcp {
    #[tool(description = "Log work time in Kala from one line of text, the same syntax as Kala's quick entry: [date] <project> [time] <description>. Dates: kemarin/yesterday, weekday names (senin/mon), 16/10, 2026-10-16. Time: duration (2j, 2h, 1,5j, 30m, 1j30m) or range (13-15, 13.30-15). Without a time it starts a timer.")]
    fn log_time(&self, Parameters(args): Parameters<LogTimeArgs>) -> String {
        reply(self.with(|s| s.log_text(&args.text, now())))
    }

    #[tool(description = "List finished time entries, oldest first. Defaults to this month, all projects.")]
    fn list_entries(&self, Parameters(args): Parameters<ListArgs>) -> String {
        reply(self.with(|s| {
            let month = Month::of(now().date());
            let from = args.from.as_deref().map(date_arg).transpose()?.unwrap_or(month.first_day());
            let to = args.to.as_deref().map(date_arg).transpose()?.unwrap_or(month.last_day());
            let pid = args.project.as_deref().map(|p| Self::project(s, p, true)).transpose()?.map(|p| p.id);
            Self::views(s, s.entries_between(from, to, pid)?)
        }))
    }

    #[tool(description = "Edit a time entry by id. Only the given fields change; times are stored exactly as given.")]
    fn update_entry(&self, Parameters(args): Parameters<UpdateArgs>) -> String {
        reply(self.with(|s| {
            let cur = s.entry(args.id)?;
            let date = args.date.as_deref().map(date_arg).transpose()?.unwrap_or(cur.start.date());
            let start_t = args.start.as_deref().map(time_arg).transpose()?;
            let end_t = args.end.as_deref().map(time_arg).transpose()?;
            let moved = args.date.is_some();
            let patch = EntryPatch {
                project_id: args.project.as_deref().map(|p| Self::project(s, p, false)).transpose()?.map(|p| p.id),
                start: (moved || start_t.is_some()).then(|| date.and_time(start_t.unwrap_or(cur.start.time()))),
                end: match (end_t, cur.end) {
                    (Some(t), _) => Some(date.and_time(t)),
                    (None, Some(e)) if moved => Some(date.and_time(e.time())),
                    _ => None,
                },
                description: args.description,
            };
            Self::views(s, vec![s.update_entry(args.id, &patch)?]).map(|mut v| v.remove(0))
        }))
    }

    #[tool(description = "Delete a time entry by id.")]
    fn delete_entry(&self, Parameters(args): Parameters<IdArgs>) -> String {
        reply(self.with(|s| s.delete_entry(args.id).map(|_| json!({ "deleted": args.id }))))
    }

    #[tool(description = "Total minutes and amount (hours x hourly rate) per project for a month.")]
    fn summary(&self, Parameters(args): Parameters<SummaryArgs>) -> String {
        reply(self.with(|s| {
            let month = month_arg(args.month.as_deref())?;
            let pid = args.project.as_deref().map(|p| Self::project(s, p, true)).transpose()?.map(|p| p.id);
            s.summary(month, pid)
        }))
    }

    #[tool(description = "Export one project's month to an .xlsx (default) or .csv file at an absolute path.")]
    fn export(&self, Parameters(args): Parameters<ExportArgs>) -> String {
        reply(self.with(|s| {
            let project = Self::project(s, &args.project, true)?;
            let month = month_arg(Some(&args.month))?;
            let format = args.format.as_deref().unwrap_or("xlsx");
            if format != "xlsx" && format != "csv" {
                return Err(bad("format_xlsx_or_csv"));
            }
            let path = PathBuf::from(&args.path);
            // Never overwrite, and only write files that look like exports: an agent can't clobber dotfiles.
            if path.extension().and_then(|e| e.to_str()) != Some(format) {
                return Err(bad("path_extension_must_match_format"));
            }
            if path.exists() {
                return Err(bad("file_exists"));
            }
            let doc = build_export(s, project.id, month, Lang::from_code(&s.settings()?.language))?;
            if format == "csv" { write_csv(&doc, &path)? } else { write_xlsx(&doc, &path)? }
            Ok(json!({ "path": args.path, "rows": doc.rows.len(), "totalHours": doc.total_hours, "totalPrice": doc.total_price }))
        }))
    }

    #[tool(description = "Start a timer for a project. A running timer is stopped and saved first.")]
    fn timer_start(&self, Parameters(args): Parameters<TimerStartArgs>) -> String {
        reply(self.with(|s| {
            let project = Self::project(s, &args.project, false)?;
            s.timer_start(project.id, args.description.as_deref().unwrap_or(""), now())
        }))
    }

    #[tool(description = "Stop the running timer and save it as an entry (rounded per project).")]
    fn timer_stop(&self, Parameters(args): Parameters<TimerStopArgs>) -> String {
        reply(self.with(|s| {
            let e = s.timer_stop(args.description.as_deref(), now())?;
            Self::views(s, vec![e]).map(|mut v| v.remove(0))
        }))
    }

    #[tool(description = "The running timer, or null.")]
    fn timer_status(&self, Parameters(_): Parameters<Empty>) -> String {
        reply(self.with(|s| match s.running()? {
            Some(e) => {
                let minutes = (now() - e.start).num_minutes();
                let view = Self::views(s, vec![e])?.remove(0);
                Ok(json!({ "running": view, "minutes": minutes }))
            }
            None => Ok(json!({ "running": null })),
        }))
    }
}

#[tool_handler]
impl ServerHandler for KalaMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("kala", env!("CARGO_PKG_VERSION")).with_title("Kala timesheet"))
            .with_instructions(
                "Kala is the user's local timesheet. Use log_time with one line like 'acme 2j fix role' or \
                 '16/10 globex 13-15 meeting'; list_entries and summary to answer hour questions; export writes \
                 a new .xlsx/.csv file (never overwrites)."
                    .to_string(),
            )
    }
}

pub fn run_blocking() -> anyhow::Result<()> {
    tokio::runtime::Builder::new_multi_thread().enable_all().build()?.block_on(async {
        let store = Store::open(&Store::default_path())?;
        let service = KalaMcp::new(store).serve(stdio()).await?;
        service.waiting().await?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mcp() -> (tempfile::TempDir, KalaMcp) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("kala.db")).unwrap();
        for name in ["Acme", "Globex"] {
            let input = kala_core::store::ProjectInput {
                id: None,
                name: name.into(),
                aliases: vec![],
                color: "#0a84ff".into(),
                hourly_rate: 0.0,
                currency: "IDR".into(),
                rounding_min: 60,
            };
            store.save_project(&input).unwrap();
        }
        (dir, KalaMcp::new(store))
    }

    #[test]
    fn server_reports_its_name_as_kala() {
        use rmcp::ServerHandler;
        let (_d, m) = mcp();
        let info = m.get_info();
        assert_eq!(info.server_info.name, "kala");
        assert!(info.instructions.unwrap_or_default().contains("log_time"));
    }

    #[test]
    fn log_time_tool_writes_entry() {
        let (_d, m) = mcp();
        let out = m.log_time(Parameters(LogTimeArgs { text: "2026-10-16 acme 13-15 fix role".into() }));
        assert!(out.contains("\"description\":\"fix role\""), "{out}");
        assert!(out.contains("2026-10-16T13:00"), "{out}");
    }

    #[test]
    fn errors_are_returned_as_text() {
        let (_d, m) = mcp();
        let out = m.log_time(Parameters(LogTimeArgs { text: "zzz 1j x".into() }));
        assert!(out.contains("ProjectNotFound"), "{out}");
    }

    #[test]
    fn export_refuses_existing_files_and_wrong_extensions() {
        let (dir, m) = mcp();
        let existing = dir.path().join("authorized_keys");
        std::fs::write(&existing, "keep").unwrap();
        let call = |path: &std::path::Path, format: Option<&str>| {
            m.export(Parameters(ExportArgs {
                project: "acme".into(),
                month: "2026-10".into(),
                path: path.display().to_string(),
                format: format.map(String::from),
            }))
        };
        assert!(call(&existing, Some("csv")).contains("error"));
        assert_eq!(std::fs::read_to_string(&existing).unwrap(), "keep");
        assert!(call(&dir.path().join("a.txt"), Some("csv")).contains("error"));
        let ok = dir.path().join("a.csv");
        assert!(!call(&ok, Some("csv")).contains("error"));
        assert!(call(&ok, Some("csv")).contains("error"));
    }

    #[test]
    fn update_entry_moves_date_and_keeps_times() {
        let (_d, m) = mcp();
        m.log_time(Parameters(LogTimeArgs { text: "2026-10-16 acme 13-15 a".into() }));
        let out = m.update_entry(Parameters(UpdateArgs {
            id: 1,
            project: Some("globex".into()),
            date: Some("2026-10-15".into()),
            start: None,
            end: None,
            description: None,
        }));
        assert!(out.contains("\"project\":\"Globex\""), "{out}");
        assert!(out.contains("\"start\":\"2026-10-15T13:00:00\""), "{out}");
        assert!(out.contains("\"end\":\"2026-10-15T15:00:00\""), "{out}");
    }
}
