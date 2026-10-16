use std::path::{Path, PathBuf};

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use rusqlite::{Connection, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};

use crate::{KalaError, Month, Project};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: i64,
    pub project_id: i64,
    pub start: NaiveDateTime,
    pub end: Option<NaiveDateTime>,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInput {
    pub id: Option<i64>,
    pub name: String,
    pub aliases: Vec<String>,
    pub color: String,
    pub hourly_rate: f64,
    pub currency: String,
    pub rounding_min: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryPatch {
    pub project_id: Option<i64>,
    pub start: Option<NaiveDateTime>,
    pub end: Option<NaiveDateTime>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub user_name: String,
    pub shortcut: Option<String>,
    pub reminder_enabled: bool,
    pub reminder_time: NaiveTime,
    pub work_days: Vec<u32>,
    pub work_start: NaiveTime,
    pub language: String,
    pub last_reminder_date: Option<NaiveDate>,
    pub onboarded: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            user_name: String::new(),
            shortcut: None,
            reminder_enabled: true,
            reminder_time: NaiveTime::from_hms_opt(17, 0, 0).unwrap(),
            work_days: vec![1, 2, 3, 4, 5],
            work_start: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            language: "en".into(),
            last_reminder_date: None,
            onboarded: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Planned {
    Log { project: Project, start: NaiveDateTime, end: NaiveDateTime, description: String, overlaps: bool },
    StartTimer { project: Project, description: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum LogOutcome {
    Logged { entry: Entry, overlaps: bool },
    TimerStarted { started: Entry, stopped: Option<Entry> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub project: Project,
    pub minutes: i64,
    pub amount: f64,
}

pub struct Store {
    conn: Connection,
}

const TS: &str = "%Y-%m-%dT%H:%M";

const SCHEMA: &str = "
CREATE TABLE projects(
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE COLLATE NOCASE,
  aliases TEXT NOT NULL DEFAULT '',
  color TEXT NOT NULL DEFAULT '#0a84ff',
  hourly_rate REAL NOT NULL DEFAULT 0,
  currency TEXT NOT NULL DEFAULT 'IDR',
  rounding_min INTEGER NOT NULL DEFAULT 60 CHECK(rounding_min IN (0,15,30,60)),
  archived INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE entries(
  id INTEGER PRIMARY KEY,
  project_id INTEGER NOT NULL REFERENCES projects(id),
  start_at TEXT NOT NULL,
  end_at TEXT,
  description TEXT NOT NULL DEFAULT '',
  CHECK(end_at IS NULL OR end_at > start_at)
);
CREATE INDEX entries_start ON entries(start_at);
CREATE UNIQUE INDEX one_running_timer ON entries((end_at IS NULL)) WHERE end_at IS NULL;
CREATE TABLE settings(id INTEGER PRIMARY KEY CHECK(id = 1), json TEXT NOT NULL);
";

const PROJECT_COLS: &str = "id, name, aliases, color, hourly_rate, currency, rounding_min, archived";
const ENTRY_COLS: &str = "id, project_id, start_at, end_at, description";

fn ts(t: NaiveDateTime) -> String {
    t.format(TS).to_string()
}

/// Descriptions end up as one CSV/Excel cell; newlines would let a caller smuggle extra lines into files.
fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn day_key(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

fn parse_ts(s: &str) -> rusqlite::Result<NaiveDateTime> {
    NaiveDateTime::parse_from_str(s, TS)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))
}

fn project_row(r: &rusqlite::Row) -> rusqlite::Result<Project> {
    let aliases: String = r.get(2)?;
    Ok(Project {
        id: r.get(0)?,
        name: r.get(1)?,
        aliases: aliases.split(',').map(str::trim).filter(|a| !a.is_empty()).map(String::from).collect(),
        color: r.get(3)?,
        hourly_rate: r.get(4)?,
        currency: r.get(5)?,
        rounding_min: r.get(6)?,
        archived: r.get(7)?,
    })
}

fn entry_row(r: &rusqlite::Row) -> rusqlite::Result<Entry> {
    let end: Option<String> = r.get(3)?;
    Ok(Entry {
        id: r.get(0)?,
        project_id: r.get(1)?,
        start: parse_ts(&r.get::<_, String>(2)?)?,
        end: end.as_deref().map(parse_ts).transpose()?,
        description: r.get(4)?,
    })
}

impl Store {
    pub fn default_path() -> PathBuf {
        match std::env::var_os("KALA_DB") {
            Some(p) => PathBuf::from(p),
            None => dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("com.aghyksa.kala").join("kala.db"),
        }
    }

    pub fn open(path: &Path) -> Result<Store, KalaError> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.query_row("PRAGMA journal_mode=WAL", [], |_| Ok(()))?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let tx = Transaction::new_unchecked(&conn, TransactionBehavior::Immediate)?;
        let version: i64 = tx.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version < 1 {
            tx.execute_batch(SCHEMA)?;
            tx.pragma_update(None, "user_version", 1)?;
        }
        tx.commit()?;
        Ok(Store { conn })
    }

    pub fn projects(&self, include_archived: bool) -> Result<Vec<Project>, KalaError> {
        let sql = format!("SELECT {PROJECT_COLS} FROM projects WHERE archived = 0 OR ?1 ORDER BY id");
        let mut st = self.conn.prepare_cached(&sql)?;
        let rows = st.query_map([include_archived], project_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    fn project(&self, id: i64) -> Result<Project, KalaError> {
        let sql = format!("SELECT {PROJECT_COLS} FROM projects WHERE id = ?1");
        Ok(self.conn.query_row(&sql, [id], project_row)?)
    }

    pub fn save_project(&self, p: &ProjectInput) -> Result<Project, KalaError> {
        let name = p.name.trim();
        if name.is_empty() {
            return Err(KalaError::InvalidEntry { reason: "project_name_empty".into() });
        }
        if ![0, 15, 30, 60].contains(&p.rounding_min) {
            return Err(KalaError::InvalidEntry { reason: "rounding_invalid".into() });
        }
        let aliases = p.aliases.iter().map(|a| a.trim()).filter(|a| !a.is_empty()).collect::<Vec<_>>().join(",");
        let duplicate = |e: rusqlite::Error| match e {
            rusqlite::Error::SqliteFailure(f, _) if f.code == rusqlite::ErrorCode::ConstraintViolation => {
                KalaError::InvalidEntry { reason: "duplicate_name".into() }
            }
            other => other.into(),
        };
        let id = match p.id {
            Some(id) => {
                self.conn.execute(
                    "UPDATE projects SET name=?1, aliases=?2, color=?3, hourly_rate=?4, currency=?5, rounding_min=?6 WHERE id=?7",
                    rusqlite::params![name, aliases, p.color, p.hourly_rate, p.currency, p.rounding_min, id],
                ).map_err(duplicate)?;
                id
            }
            None => {
                self.conn.execute(
                    "INSERT INTO projects(name, aliases, color, hourly_rate, currency, rounding_min) VALUES (?1,?2,?3,?4,?5,?6)",
                    rusqlite::params![name, aliases, p.color, p.hourly_rate, p.currency, p.rounding_min],
                ).map_err(duplicate)?;
                self.conn.last_insert_rowid()
            }
        };
        self.project(id)
    }

    pub fn set_archived(&self, id: i64, archived: bool) -> Result<(), KalaError> {
        self.conn.execute("UPDATE projects SET archived = ?2 WHERE id = ?1", rusqlite::params![id, archived])?;
        Ok(())
    }

    pub fn plan_text(&self, text: &str, now: NaiveDateTime) -> Result<Planned, KalaError> {
        let parsed = crate::parse::parse(text, now.date())?;
        let projects = self.projects(false)?;
        let project = crate::parse::resolve_project(&parsed.project, &projects, false)?.clone();
        let (start, end) = match parsed.time {
            crate::TimeSpec::None => {
                return Ok(Planned::StartTimer { project, description: parsed.description });
            }
            crate::TimeSpec::Range(a, b) => {
                let end_day = if b <= a { parsed.date.succ_opt().unwrap_or(parsed.date) } else { parsed.date };
                (parsed.date.and_time(a), end_day.and_time(b))
            }
            crate::TimeSpec::Duration(min) => {
                let start = if parsed.date == now.date() {
                    now - chrono::Duration::minutes(min as i64)
                } else {
                    match self.last_end_on(parsed.date)? {
                        Some(end) => end,
                        None => parsed.date.and_time(self.settings()?.work_start),
                    }
                };
                crate::round::auto_span(start, min as i64, project.rounding_min)
            }
        };
        let overlaps = self.overlaps(start, end, None)?;
        Ok(Planned::Log { project, start, end, description: parsed.description, overlaps })
    }

    fn last_end_on(&self, date: NaiveDate) -> Result<Option<NaiveDateTime>, KalaError> {
        let end: Option<String> = self.conn.query_row(
            "SELECT MAX(end_at) FROM entries WHERE end_at IS NOT NULL AND start_at >= ?1 AND start_at < ?2",
            [day_key(date), day_key(date.succ_opt().unwrap_or(date))],
            |r| r.get(0),
        )?;
        Ok(end.as_deref().map(parse_ts).transpose()?)
    }

    fn overlaps(&self, start: NaiveDateTime, end: NaiveDateTime, except: Option<i64>) -> Result<bool, KalaError> {
        Ok(self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM entries WHERE end_at IS NOT NULL AND start_at < ?2 AND end_at > ?1 AND id IS NOT ?3)",
            rusqlite::params![ts(start), ts(end), except],
            |r| r.get(0),
        )?)
    }

    fn insert_entry(&self, project_id: i64, start: NaiveDateTime, end: Option<NaiveDateTime>, desc: &str) -> Result<Entry, KalaError> {
        self.conn.execute(
            "INSERT INTO entries(project_id, start_at, end_at, description) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![project_id, ts(start), end.map(ts), one_line(desc)],
        )?;
        self.entry(self.conn.last_insert_rowid())
    }

    pub fn entry(&self, id: i64) -> Result<Entry, KalaError> {
        let sql = format!("SELECT {ENTRY_COLS} FROM entries WHERE id = ?1");
        Ok(self.conn.query_row(&sql, [id], entry_row)?)
    }

    pub fn log_text(&self, text: &str, now: NaiveDateTime) -> Result<LogOutcome, KalaError> {
        match self.plan_text(text, now)? {
            Planned::Log { project, start, end, description, overlaps } => {
                let entry = self.insert_entry(project.id, start, Some(end), &description)?;
                Ok(LogOutcome::Logged { entry, overlaps })
            }
            Planned::StartTimer { project, description } => self.timer_start(project.id, &description, now),
        }
    }

    pub fn entries_between(&self, from: NaiveDate, to: NaiveDate, project_id: Option<i64>) -> Result<Vec<Entry>, KalaError> {
        let sql = format!(
            "SELECT {ENTRY_COLS} FROM entries WHERE end_at IS NOT NULL AND start_at >= ?1 AND start_at < ?2 \
             AND (?3 IS NULL OR project_id = ?3) ORDER BY start_at, id"
        );
        let mut st = self.conn.prepare_cached(&sql)?;
        let rows = st.query_map(rusqlite::params![day_key(from), day_key(to.succ_opt().unwrap_or(to)), project_id], entry_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn update_entry(&self, id: i64, patch: &EntryPatch) -> Result<Entry, KalaError> {
        let cur = self.entry(id)?;
        let next = Entry {
            id,
            project_id: patch.project_id.unwrap_or(cur.project_id),
            start: patch.start.unwrap_or(cur.start),
            end: patch.end.or(cur.end),
            description: patch.description.clone().unwrap_or(cur.description),
        };
        if next.end.is_some_and(|e| e <= next.start) {
            return Err(KalaError::InvalidEntry { reason: "end_before_start".into() });
        }
        self.project(next.project_id)?;
        self.conn.execute(
            "UPDATE entries SET project_id=?1, start_at=?2, end_at=?3, description=?4 WHERE id=?5",
            rusqlite::params![next.project_id, ts(next.start), next.end.map(ts), one_line(&next.description), id],
        )?;
        self.entry(id)
    }

    pub fn delete_entry(&self, id: i64) -> Result<(), KalaError> {
        match self.conn.execute("DELETE FROM entries WHERE id = ?1", [id])? {
            0 => Err(KalaError::InvalidEntry { reason: "not_found".into() }),
            _ => Ok(()),
        }
    }

    pub fn running(&self) -> Result<Option<Entry>, KalaError> {
        let sql = format!("SELECT {ENTRY_COLS} FROM entries WHERE end_at IS NULL");
        match self.conn.query_row(&sql, [], entry_row) {
            Ok(e) => Ok(Some(e)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Closes the running timer with project rounding; under a minute it was a misclick and is dropped.
    fn close_running(&self, now: NaiveDateTime, description: Option<&str>, keep_short: bool) -> Result<Option<Entry>, KalaError> {
        let Some(run) = self.running()? else { return Ok(None) };
        let worked = (now - run.start).num_minutes();
        if worked < 1 && !keep_short {
            self.delete_entry(run.id)?;
            return Ok(None);
        }
        let unit = self.project(run.project_id)?.rounding_min;
        let (start, end) = crate::round::auto_span(run.start, worked.max(1), unit);
        let desc = one_line(description.unwrap_or(&run.description));
        self.conn.execute(
            "UPDATE entries SET start_at=?1, end_at=?2, description=?3 WHERE id=?4",
            rusqlite::params![ts(start), ts(end), desc, run.id],
        )?;
        Ok(Some(self.entry(run.id)?))
    }

    pub fn timer_start(&self, project_id: i64, description: &str, now: NaiveDateTime) -> Result<LogOutcome, KalaError> {
        self.project(project_id)?;
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        let stopped = self.close_running(now, None, false)?;
        let started = self.insert_entry(project_id, now, None, description.trim())?;
        tx.commit()?;
        Ok(LogOutcome::TimerStarted { started, stopped })
    }

    pub fn timer_stop(&self, description: Option<&str>, now: NaiveDateTime) -> Result<Entry, KalaError> {
        // Stop never blocks on a missing description; the UI opens the saved entry for editing instead.
        let run = self.running()?.ok_or(KalaError::NoRunningTimer)?;
        let desc = description.map(str::trim).filter(|d| !d.is_empty()).unwrap_or(run.description.trim()).to_string();
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        let stopped = self.close_running(now, Some(&desc), true)?.ok_or(KalaError::NoRunningTimer)?;
        tx.commit()?;
        Ok(stopped)
    }

    pub fn summary(&self, month: Month, project_id: Option<i64>) -> Result<Vec<ProjectSummary>, KalaError> {
        let sql = format!(
            "SELECT {}, CAST(ROUND(SUM((julianday(e.end_at) - julianday(e.start_at)) * 1440)) AS INTEGER) \
             FROM entries e JOIN projects p ON p.id = e.project_id \
             WHERE e.end_at IS NOT NULL AND e.start_at >= ?1 AND e.start_at < ?2 AND (?3 IS NULL OR e.project_id = ?3) \
             GROUP BY p.id ORDER BY p.id",
            PROJECT_COLS.split(", ").map(|c| format!("p.{c}")).collect::<Vec<_>>().join(", ")
        );
        let mut st = self.conn.prepare_cached(&sql)?;
        let rows = st.query_map(
            rusqlite::params![day_key(month.first_day()), day_key(month.last_day().succ_opt().unwrap()), project_id],
            |r| {
                let project = project_row(r)?;
                let minutes: i64 = r.get(8)?;
                let amount = minutes as f64 / 60.0 * project.hourly_rate;
                Ok(ProjectSummary { project, minutes, amount })
            },
        )?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn settings(&self) -> Result<Settings, KalaError> {
        let json: Option<String> = match self.conn.query_row("SELECT json FROM settings WHERE id = 1", [], |r| r.get(0)) {
            Ok(j) => Some(j),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(e.into()),
        };
        Ok(json.and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default())
    }

    pub fn save_settings(&self, s: &Settings) -> Result<(), KalaError> {
        let json = serde_json::to_string(s).map_err(|e| KalaError::InvalidEntry { reason: e.to_string() })?;
        self.conn.execute("INSERT INTO settings(id, json) VALUES (1, ?1) ON CONFLICT(id) DO UPDATE SET json = ?1", [json])?;
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    pub fn dt(day: u32, h: u32, m: u32) -> NaiveDateTime {
        d(2026, 10, day).and_hms_opt(h, m, 0).unwrap()
    }

    pub fn now() -> NaiveDateTime {
        dt(16, 15, 40)
    }

    pub fn seed(s: &Store) {
        for (name, aliases, color) in [("Acme", vec!["portal".to_string()], "#30d158"), ("Globex", vec![], "#bf5af2")] {
            let input = ProjectInput {
                id: None,
                name: name.into(),
                aliases,
                color: color.into(),
                hourly_rate: 0.0,
                currency: "IDR".into(),
                rounding_min: 60,
            };
            s.save_project(&input).unwrap();
        }
    }

    pub fn temp_store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("kala.db")).unwrap();
        seed(&store);
        (dir, store)
    }

    pub fn id_of(s: &Store, name: &str) -> i64 {
        s.projects(true).unwrap().into_iter().find(|p| p.name == name).unwrap().id
    }

    fn logged(o: LogOutcome) -> (Entry, bool) {
        match o {
            LogOutcome::Logged { entry, overlaps } => (entry, overlaps),
            other => panic!("expected Logged, got {other:?}"),
        }
    }

    fn span(e: &Entry) -> (NaiveDateTime, NaiveDateTime) {
        (e.start, e.end.unwrap())
    }

    #[test]
    fn fresh_store_has_no_projects_and_default_settings() {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::open(&dir.path().join("kala.db")).unwrap();
        assert!(s.projects(true).unwrap().is_empty());
        let st = s.settings().unwrap();
        assert_eq!(st.reminder_time, NaiveTime::from_hms_opt(17, 0, 0).unwrap());
        assert_eq!(st.work_days, [1, 2, 3, 4, 5]);
        assert_eq!(st.shortcut, None);
        assert!(!st.onboarded);
    }

    #[test]
    fn midnight_range_ends_next_day() {
        let (_d, s) = temp_store();
        let (e, _) = logged(s.log_text("acme 23-24 late deploy", now()).unwrap());
        assert_eq!(span(&e), (dt(16, 23, 0), dt(17, 0, 0)));
    }

    #[test]
    fn delete_missing_entry_errors() {
        let (_d, s) = temp_store();
        assert_eq!(s.delete_entry(999), Err(KalaError::InvalidEntry { reason: "not_found".into() }));
    }

    #[test]
    fn duplicate_project_name_is_a_clear_error() {
        let (_d, s) = temp_store();
        let dup = ProjectInput {
            id: None,
            name: "acme".into(),
            aliases: vec![],
            color: "#000000".into(),
            hourly_rate: 0.0,
            currency: "IDR".into(),
            rounding_min: 60,
        };
        assert_eq!(s.save_project(&dup), Err(KalaError::InvalidEntry { reason: "duplicate_name".into() }));
    }

    #[test]
    fn spec_log_examples() {
        let (_d, s) = temp_store();
        let (e, overlaps) = logged(s.log_text("globex 13-15 meeting sprint", now()).unwrap());
        assert_eq!((e.project_id, span(&e), overlaps), (id_of(&s, "Globex"), (dt(16, 13, 0), dt(16, 15, 0)), false));
        let (e, _) = logged(s.log_text("acme 2j fix role", now()).unwrap());
        assert_eq!((e.project_id, span(&e), e.description.as_str()), (id_of(&s, "Acme"), (dt(16, 13, 0), dt(16, 15, 0)), "fix role"));
        let (e, _) = logged(s.log_text("kemarin acme 1j review", now()).unwrap());
        assert_eq!(span(&e), (dt(15, 9, 0), dt(15, 10, 0)));
        let (e, _) = logged(s.log_text("kemarin acme 1j lagi", now()).unwrap());
        assert_eq!(span(&e), (dt(15, 10, 0), dt(15, 11, 0)));
        match s.log_text("acme benerin tema", now()).unwrap() {
            LogOutcome::TimerStarted { started, stopped } => {
                assert_eq!((started.description.as_str(), started.end, stopped), ("benerin tema", None, None));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn overlap_is_flagged_not_blocked() {
        let (_d, s) = temp_store();
        s.log_text("acme 13-15 a", now()).unwrap();
        let (_, overlaps) = logged(s.log_text("globex 14-16 b", now()).unwrap());
        assert!(overlaps);
        assert_eq!(s.entries_between(d(2026, 10, 16), d(2026, 10, 16), None).unwrap().len(), 2);
    }

    #[test]
    fn timer_switch_stops_previous_with_rounding() {
        let (_d, s) = temp_store();
        let (acme, globex) = (id_of(&s, "Acme"), id_of(&s, "Globex"));
        s.timer_start(acme, "a", dt(16, 13, 35)).unwrap();
        match s.timer_start(globex, "b", dt(16, 14, 25)).unwrap() {
            LogOutcome::TimerStarted { stopped: Some(prev), .. } => {
                assert_eq!((prev.project_id, span(&prev)), (acme, (dt(16, 13, 0), dt(16, 14, 0))));
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(s.running().unwrap().unwrap().project_id, globex);
    }

    #[test]
    fn timer_switch_under_a_minute_discards_previous() {
        let (_d, s) = temp_store();
        let (acme, globex) = (id_of(&s, "Acme"), id_of(&s, "Globex"));
        s.timer_start(acme, "misclick", dt(16, 13, 35)).unwrap();
        match s.timer_start(globex, "b", dt(16, 13, 35)).unwrap() {
            LogOutcome::TimerStarted { stopped, .. } => assert_eq!(stopped, None),
            other => panic!("{other:?}"),
        }
        assert!(s.entries_between(d(2026, 10, 16), d(2026, 10, 16), None).unwrap().is_empty());
    }

    #[test]
    fn running_timer_survives_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kala.db");
        {
            let s = Store::open(&path).unwrap();
            seed(&s);
            s.timer_start(id_of(&s, "Acme"), "x", now()).unwrap();
        }
        let s = Store::open(&path).unwrap();
        assert_eq!(s.running().unwrap().unwrap().description, "x");
    }

    #[test]
    fn second_running_timer_rejected_by_db() {
        let (_d, s) = temp_store();
        let acme = id_of(&s, "Acme");
        s.timer_start(acme, "x", now()).unwrap();
        let r = s.conn.execute(
            "INSERT INTO entries(project_id, start_at, end_at, description) VALUES (?1, '2026-10-16T16:00', NULL, 'y')",
            [acme],
        );
        assert!(r.is_err());
    }

    #[test]
    fn timer_stop_saves_even_without_description() {
        let (_d, s) = temp_store();
        s.timer_start(id_of(&s, "Acme"), "", dt(16, 13, 0)).unwrap();
        let e = s.timer_stop(None, dt(16, 14, 0)).unwrap();
        assert_eq!((e.description.as_str(), span(&e)), ("", (dt(16, 13, 0), dt(16, 14, 0))));
        assert_eq!(s.running().unwrap(), None);
        s.timer_start(id_of(&s, "Acme"), "", dt(16, 15, 0)).unwrap();
        assert_eq!(s.timer_stop(Some("x"), dt(16, 16, 0)).unwrap().description, "x");
    }

    #[test]
    fn two_stores_write_concurrently() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kala.db");
        seed(&Store::open(&path).unwrap());
        let handles: Vec<_> = (0..2)
            .map(|t| {
                let path = path.clone();
                std::thread::spawn(move || {
                    let s = Store::open(&path).unwrap();
                    for i in 0..50 {
                        s.log_text(&format!("acme 1j t{t}-{i}"), now()).unwrap();
                    }
                })
            })
            .collect();
        handles.into_iter().for_each(|h| h.join().unwrap());
        let s = Store::open(&path).unwrap();
        assert_eq!(s.entries_between(d(2026, 10, 16), d(2026, 10, 16), None).unwrap().len(), 100);
    }

    #[test]
    fn update_keeps_exact_times_and_rejects_inverted() {
        let (_d, s) = temp_store();
        let (e, _) = logged(s.log_text("acme 1j a", now()).unwrap());
        let patch = EntryPatch { start: Some(dt(16, 13, 20)), end: Some(dt(16, 14, 5)), ..Default::default() };
        assert_eq!(span(&s.update_entry(e.id, &patch).unwrap()), (dt(16, 13, 20), dt(16, 14, 5)));
        let bad = EntryPatch { end: Some(dt(16, 13, 20)), ..Default::default() };
        assert!(matches!(s.update_entry(e.id, &bad), Err(KalaError::InvalidEntry { .. })));
    }

    #[test]
    fn descriptions_are_single_line() {
        let (_d, s) = temp_store();
        let (e, _) = logged(s.log_text("acme 1j a", now()).unwrap());
        let patch = EntryPatch { description: Some("x\nssh-ed25519 AAA\r\ny".into()), ..Default::default() };
        assert_eq!(s.update_entry(e.id, &patch).unwrap().description, "x ssh-ed25519 AAA y");
        s.timer_start(id_of(&s, "Acme"), "a\nb", now()).unwrap();
        assert_eq!(s.running().unwrap().unwrap().description, "a b");
    }

    #[test]
    fn delete_entry_removes_finished_and_running_entries() {
        let (_d, s) = temp_store();
        let (e, _) = logged(s.log_text("acme 1j a", now()).unwrap());
        s.delete_entry(e.id).unwrap();
        assert!(s.entries_between(d(2026, 10, 16), d(2026, 10, 16), None).unwrap().is_empty());
        s.timer_start(id_of(&s, "Acme"), "x", now()).unwrap();
        let run = s.running().unwrap().unwrap();
        s.delete_entry(run.id).unwrap();
        assert_eq!(s.running().unwrap(), None);
        s.timer_start(id_of(&s, "Globex"), "y", now()).unwrap();
        assert_eq!(s.running().unwrap().unwrap().description, "y");
    }

    #[test]
    fn archive_and_restore_project() {
        let (_d, s) = temp_store();
        let globex = id_of(&s, "Globex");
        s.set_archived(globex, true).unwrap();
        assert_eq!(s.projects(false).unwrap().len(), 1);
        s.set_archived(globex, false).unwrap();
        assert_eq!(s.projects(false).unwrap().len(), 2);
    }

    #[test]
    fn summary_october() {
        let (_d, s) = temp_store();
        let acme = s.projects(false).unwrap().remove(0);
        s.save_project(&ProjectInput {
            id: Some(acme.id),
            name: acme.name,
            aliases: acme.aliases,
            color: acme.color,
            hourly_rate: 100000.0,
            currency: acme.currency,
            rounding_min: acme.rounding_min,
        })
        .unwrap();
        s.log_text("acme 2j a", now()).unwrap();
        s.log_text("kemarin acme 1j b", now()).unwrap();
        let sum = s.summary(Month { year: 2026, month: 10 }, Some(acme.id)).unwrap();
        assert_eq!((sum.len(), sum[0].minutes, sum[0].amount), (1, 180, 300000.0));
    }
}
