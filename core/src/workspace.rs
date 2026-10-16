use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params, params_from_iter};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::KalaError;
use crate::store::{Entry, Store};

pub(crate) const SCHEMA: &str = "
CREATE TABLE project_fields(
  id INTEGER PRIMARY KEY,
  project_id INTEGER NOT NULL REFERENCES projects(id),
  name TEXT NOT NULL,
  kind TEXT NOT NULL CHECK(kind IN ('text','number','date','select','checkbox','url')),
  options TEXT NOT NULL DEFAULT '[]',
  show_in_table INTEGER NOT NULL DEFAULT 1,
  show_in_export INTEGER NOT NULL DEFAULT 1,
  position INTEGER NOT NULL DEFAULT 0,
  archived INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX project_fields_project ON project_fields(project_id,archived,position,id);
CREATE TABLE entry_values(
  entry_id INTEGER NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
  field_id INTEGER NOT NULL REFERENCES project_fields(id),
  value_json TEXT NOT NULL,
  value_text TEXT NOT NULL,
  PRIMARY KEY(entry_id,field_id)
);
CREATE INDEX entry_values_filter ON entry_values(field_id,value_text,entry_id);
CREATE INDEX entries_project_start ON entries(project_id,start_at) WHERE end_at IS NOT NULL;
CREATE TABLE workspace_views(id INTEGER PRIMARY KEY,name TEXT NOT NULL,config TEXT NOT NULL);
CREATE TRIGGER entries_reassign_fields AFTER UPDATE OF project_id ON entries
WHEN OLD.project_id <> NEW.project_id BEGIN
  DELETE FROM entry_values WHERE entry_id=NEW.id;
END;
";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldKind { Text, Number, Date, Select, Checkbox, Url }

impl FieldKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text", Self::Number => "number", Self::Date => "date",
            Self::Select => "select", Self::Checkbox => "checkbox", Self::Url => "url",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldDefinition {
    pub id: i64,
    pub project_id: i64,
    pub name: String,
    pub kind: FieldKind,
    pub options: Vec<String>,
    pub show_in_table: bool,
    pub show_in_export: bool,
    pub position: i64,
    pub archived: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldInput {
    pub id: Option<i64>,
    pub project_id: i64,
    pub name: String,
    pub kind: FieldKind,
    pub options: Vec<String>,
    pub show_in_table: bool,
    pub show_in_export: bool,
    pub position: i64,
    pub archived: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceEntry {
    #[serde(flatten)]
    pub entry: Entry,
    pub fields: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEntryInput {
    pub id: Option<i64>,
    pub project_id: i64,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub description: String,
    #[serde(default)]
    pub fields: BTreeMap<String, Value>,
}

fn default_limit() -> u32 { 100 }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceQuery {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub project_id: Option<i64>,
    #[serde(default)]
    pub search: String,
    pub field_id: Option<i64>,
    pub field_value: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTotal { pub project_id: i64, pub minutes: i64, pub entries: i64 }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DayTotal { pub date: NaiveDate, pub minutes: i64, pub entries: i64 }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldTotal { pub value: String, pub minutes: i64, pub entries: i64 }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceData {
    pub entries: Vec<WorkspaceEntry>,
    pub total: i64,
    pub total_minutes: i64,
    pub by_project: Vec<ProjectTotal>,
    pub by_day: Vec<DayTotal>,
    pub field_groups: Vec<FieldTotal>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedView { pub id: i64, pub name: String, pub config: Value }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewInput { pub id: Option<i64>, pub name: String, pub config: Value }

const FIELD_COLS: &str = "id,project_id,name,kind,options,show_in_table,show_in_export,position,archived";
const MINUTES: &str = "CAST(ROUND((julianday(e.end_at)-julianday(e.start_at))*1440) AS INTEGER)";

struct EntryScope {
    from: String,
    conditions: String,
    params: Vec<rusqlite::types::Value>,
}

fn invalid(reason: &str) -> KalaError { KalaError::InvalidEntry { reason: reason.into() } }

fn json_sql<T: serde::de::DeserializeOwned>(json: &str, column: usize) -> rusqlite::Result<T> {
    serde_json::from_str(json).map_err(|e| rusqlite::Error::FromSqlConversionFailure(column, rusqlite::types::Type::Text, Box::new(e)))
}

fn field_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<FieldDefinition> {
    let kind: String = row.get(3)?;
    Ok(FieldDefinition {
        id: row.get(0)?, project_id: row.get(1)?, name: row.get(2)?,
        kind: json_sql(&format!("\"{kind}\""), 3)?, options: json_sql(&row.get::<_, String>(4)?, 4)?,
        show_in_table: row.get(5)?, show_in_export: row.get(6)?, position: row.get(7)?, archived: row.get(8)?,
    })
}

fn valid_date(date: NaiveDate) -> bool { (1900..=9999).contains(&date.year()) }

fn validate_value(field: &FieldDefinition, value: &Value, previous: Option<&Value>) -> Result<(), KalaError> {
    let valid = match field.kind {
        FieldKind::Text => value.as_str().is_some_and(|s| s.len() <= 16000),
        FieldKind::Number => value.is_number(),
        FieldKind::Date => value.as_str().is_some_and(|s| s.len() == 10
            && NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok_and(valid_date)),
        FieldKind::Select => value.as_str().is_some_and(|s| field.options.iter().any(|option| option == s)
            || previous == Some(value)),
        FieldKind::Checkbox => value.is_boolean(),
        FieldKind::Url => value.as_str().is_some_and(|s| {
            let host = s.strip_prefix("https://").or_else(|| s.strip_prefix("http://"));
            s.len() <= 4000 && !s.chars().any(|c| c.is_whitespace() || c.is_control())
                && host.is_some_and(|h| !h.split(['/', '?', '#']).next().unwrap_or_default().is_empty())
        }),
    };
    if valid { Ok(()) } else { Err(invalid("field_value_invalid")) }
}

fn scalar_text(value: &Value) -> String {
    value.as_str().map(str::to_owned).unwrap_or_else(|| value.to_string())
}

fn view_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SavedView> {
    Ok(SavedView { id: row.get(0)?, name: row.get(1)?, config: json_sql(&row.get::<_, String>(2)?, 2)? })
}

impl Store {
    pub fn saved_views(&self) -> Result<Vec<SavedView>, KalaError> {
        Ok(self.conn.prepare_cached("SELECT id,name,config FROM workspace_views ORDER BY name COLLATE NOCASE,id")?
            .query_map([],view_row)?.collect::<Result<_, _>>()?)
    }

    pub fn save_view(&self, input: &ViewInput) -> Result<SavedView, KalaError> {
        let name = input.name.trim();
        if name.is_empty() || name.len() > 512 { return Err(invalid("view_name_invalid")); }
        if !input.config.is_object() { return Err(invalid("view_config_invalid")); }
        let config = input.config.to_string();
        if config.len() > 65536 { return Err(invalid("view_config_too_large")); }
        let id = if let Some(id) = input.id {
            if self.conn.execute("UPDATE workspace_views SET name=?1,config=?2 WHERE id=?3",params![name,config,id])? == 0 {
                return Err(invalid("view_not_found"));
            }
            id
        } else {
            self.conn.execute("INSERT INTO workspace_views(name,config) VALUES(?1,?2)",params![name,config])?;
            self.conn.last_insert_rowid()
        };
        Ok(self.conn.query_row("SELECT id,name,config FROM workspace_views WHERE id=?1",[id],view_row)?)
    }

    pub fn delete_view(&self, id: i64) -> Result<(), KalaError> {
        if self.conn.execute("DELETE FROM workspace_views WHERE id=?1",[id])? == 0 { return Err(invalid("view_not_found")); }
        Ok(())
    }

    pub fn workspace_entries_before(&self, query: &WorkspaceQuery, before: Option<(NaiveDateTime,i64)>) -> Result<Vec<WorkspaceEntry>, KalaError> {
        let tx = self.workspace_snapshot()?;
        let mut scope = self.workspace_scope(query)?;
        if let Some((start,id)) = before {
            scope.params.push(start.format("%Y-%m-%dT%H:%M").to_string().into());
            scope.params.push(id.into());
            scope.conditions.push_str(&format!(" AND (e.start_at,e.id)<(?{},?{})",scope.params.len()-1,scope.params.len()));
        }
        let page_query = WorkspaceQuery { offset:0,..query.clone() };
        let entries = self.workspace_page(&page_query,&scope)?;
        if let Some(tx) = tx { tx.commit()?; }
        Ok(entries)
    }

    fn workspace_snapshot(&self) -> Result<Option<Transaction<'_>>, KalaError> {
        Ok(if self.conn.is_autocommit() { Some(self.conn.unchecked_transaction()?) } else { None })
    }

    fn workspace_scope(&self, query: &WorkspaceQuery) -> Result<EntryScope, KalaError> {
        if query.from > query.to || !valid_date(query.from) || !valid_date(query.to) { return Err(invalid("date_range_invalid")); }
        if query.limit == 0 || query.limit > 200 { return Err(invalid("pagination_invalid")); }
        if query.search.len() > 4000 || query.field_value.as_ref().is_some_and(|s| s.len() > 16000) { return Err(invalid("query_too_large")); }
        if let Some(project) = query.project_id { self.require_project(project)?; }
        if let Some(field) = query.field_id {
            let definition = self.field(field)?;
            if query.project_id.is_some_and(|project| project != definition.project_id) { return Err(invalid("field_project_invalid")); }
        } else if query.field_value.is_some() { return Err(invalid("field_filter_invalid")); }
        let mut scope = EntryScope {
            from: "FROM entries e".into(),
            conditions: "WHERE e.end_at IS NOT NULL AND e.start_at>=?1 AND e.start_at<?2".into(),
            params: vec![query.from.format("%Y-%m-%d").to_string().into(), format!("{}T24:00", query.to).into()],
        };
        if let Some(project) = query.project_id {
            scope.params.push(project.into());
            scope.conditions.push_str(&format!(" AND e.project_id=?{}", scope.params.len()));
        }
        if !query.search.trim().is_empty() {
            scope.from.push_str(" JOIN projects p ON p.id=e.project_id");
            let escaped = query.search.trim().replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
            scope.params.push(format!("%{escaped}%").into());
            scope.conditions.push_str(&format!(" AND (e.description LIKE ?{0} ESCAPE '\\' OR p.name LIKE ?{0} ESCAPE '\\')", scope.params.len()));
        }
        if let (Some(field), Some(value)) = (query.field_id, &query.field_value) {
            scope.params.push(field.into());
            let field_param = scope.params.len();
            scope.params.push(value.clone().into());
            scope.from.push_str(&format!(" JOIN entry_values f ON f.entry_id=e.id AND f.field_id=?{field_param} AND f.value_text=?{}", scope.params.len()));
        }
        Ok(scope)
    }

    fn workspace_page(&self, query: &WorkspaceQuery, scope: &EntryScope) -> Result<Vec<WorkspaceEntry>, KalaError> {
        let mut bound = scope.params.clone();
        bound.push(i64::from(query.limit).into());
        bound.push(i64::from(query.offset).into());
        let sql = format!("SELECT e.id,e.project_id,e.start_at,e.end_at,e.description {} {} ORDER BY e.start_at DESC,e.id DESC LIMIT ?{} OFFSET ?{}",
            scope.from, scope.conditions, bound.len()-1, bound.len());
        let mut stmt = self.conn.prepare_cached(&sql)?;
        let mut entries: Vec<_> = stmt.query_map(params_from_iter(bound), |r| Ok(WorkspaceEntry {
            entry: crate::store::entry_row(r)?, fields: BTreeMap::new(),
        }))?.collect::<Result<_, _>>()?;
        if entries.is_empty() { return Ok(entries); }
        let indexes: HashMap<_, _> = entries.iter().enumerate().map(|(i,e)| (e.entry.id,i)).collect();
        let sql = format!("SELECT entry_id,field_id,value_json FROM entry_values WHERE entry_id IN ({})", vec!["?"; entries.len()].join(","));
        let mut stmt = self.conn.prepare_cached(&sql)?;
        let mut values = stmt.query(params_from_iter(entries.iter().map(|e| e.entry.id)))?;
        while let Some(row) = values.next()? {
            let entry: i64 = row.get(0)?;
            let field: i64 = row.get(1)?;
            let value = json_sql(&row.get::<_, String>(2)?, 2)?;
            if let Some(index) = indexes.get(&entry) { entries[*index].fields.insert(field.to_string(), value); }
        }
        Ok(entries)
    }

    pub fn workspace_entries(&self, query: &WorkspaceQuery) -> Result<Vec<WorkspaceEntry>, KalaError> {
        let tx = self.workspace_snapshot()?;
        let scope = self.workspace_scope(query)?;
        let entries = self.workspace_page(query, &scope)?;
        if let Some(tx) = tx { tx.commit()?; }
        Ok(entries)
    }

    pub fn workspace(&self, query: &WorkspaceQuery) -> Result<WorkspaceData, KalaError> {
        let tx = self.workspace_snapshot()?;
        let scope = self.workspace_scope(query)?;
        let entries = self.workspace_page(query, &scope)?;
        let sql = format!("SELECT COUNT(*),COALESCE(SUM({MINUTES}),0) {} {}", scope.from, scope.conditions);
        let (total,total_minutes) = self.conn.query_row(&sql, params_from_iter(&scope.params), |r| Ok((r.get(0)?,r.get(1)?)))?;
        let sql = format!("SELECT e.project_id,SUM({MINUTES}) AS minutes,COUNT(*) {} {} GROUP BY e.project_id ORDER BY minutes DESC,e.project_id", scope.from, scope.conditions);
        let by_project = self.conn.prepare_cached(&sql)?.query_map(params_from_iter(&scope.params), |r| Ok(ProjectTotal {
            project_id: r.get(0)?, minutes: r.get(1)?, entries: r.get(2)?,
        }))?.collect::<Result<_, _>>()?;
        let sql = format!("SELECT substr(e.start_at,1,10),SUM({MINUTES}),COUNT(*) {} {} GROUP BY substr(e.start_at,1,10) ORDER BY substr(e.start_at,1,10)", scope.from, scope.conditions);
        let by_day = self.conn.prepare_cached(&sql)?.query_map(params_from_iter(&scope.params), |r| {
            let date: String = r.get(0)?;
            Ok(DayTotal { date: NaiveDate::parse_from_str(&date,"%Y-%m-%d")
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0,rusqlite::types::Type::Text,Box::new(e)))?, minutes: r.get(1)?, entries: r.get(2)? })
        })?.collect::<Result<_, _>>()?;
        let field_groups = if let Some(field) = query.field_id {
            let mut bound = scope.params.clone();
            bound.push(field.into());
            let sql = format!("SELECT COALESCE(g.value_text,''),SUM({MINUTES}) AS minutes,COUNT(*) {} LEFT JOIN entry_values g ON g.entry_id=e.id AND g.field_id=?{} {} GROUP BY COALESCE(g.value_text,'') ORDER BY minutes DESC,COALESCE(g.value_text,'')", scope.from, bound.len(), scope.conditions);
            self.conn.prepare_cached(&sql)?.query_map(params_from_iter(bound), |r| Ok(FieldTotal {
                value: r.get(0)?, minutes: r.get(1)?, entries: r.get(2)?,
            }))?.collect::<Result<_, _>>()?
        } else { Vec::new() };
        if let Some(tx) = tx { tx.commit()?; }
        Ok(WorkspaceData { entries,total,total_minutes,by_project,by_day,field_groups })
    }

    fn entry_fields(&self, id: i64) -> Result<BTreeMap<String, Value>, KalaError> {
        let mut stmt = self.conn.prepare_cached("SELECT field_id,value_json FROM entry_values WHERE entry_id=?1")?;
        Ok(stmt.query_map([id], |row| Ok((row.get::<_, i64>(0)?.to_string(), json_sql(&row.get::<_, String>(1)?, 1)?)))?
            .collect::<Result<_, _>>()?)
    }

    pub fn workspace_entry(&self, id: i64) -> Result<WorkspaceEntry, KalaError> {
        Ok(WorkspaceEntry { entry: self.entry(id)?, fields: self.entry_fields(id)? })
    }

    pub fn save_workspace_entry(&self, input: &WorkspaceEntryInput) -> Result<WorkspaceEntry, KalaError> {
        let minutes = (input.end - input.start).num_minutes();
        if !valid_date(input.start.date()) || !valid_date(input.end.date()) { return Err(invalid("date_range_invalid")); }
        if input.start.second() != 0 || input.end.second() != 0 || input.start.nanosecond() != 0 || input.end.nanosecond() != 0 {
            return Err(invalid("time_precision_invalid"));
        }
        if input.description.len() > 32000 || input.fields.len() > 2000 { return Err(invalid("entry_too_large")); }
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        self.require_project(input.project_id)?;
        let previous = input.id.map(|id| self.workspace_entry(id)).transpose()?;
        if previous.as_ref().is_some_and(|e| e.entry.end.is_none()) { return Err(invalid("entry_running")); }
        let unchanged_long_session = minutes > 1440 && previous.as_ref()
            .is_some_and(|e| e.entry.start == input.start && e.entry.end == Some(input.end));
        if !(1..=1440).contains(&minutes) && !unchanged_long_session { return Err(invalid("duration_invalid")); }
        let definitions: BTreeMap<_, _> = self.fields(Some(input.project_id), true)?.into_iter().map(|f| (f.id, f)).collect();
        let mut values = Vec::with_capacity(input.fields.len());
        for (key, value) in &input.fields {
            let id = key.parse::<i64>().map_err(|_| invalid("field_not_found"))?;
            if id <= 0 || id.to_string() != *key { return Err(invalid("field_not_found")); }
            let field = definitions.get(&id).ok_or_else(|| invalid("field_project_invalid"))?;
            let old = previous.as_ref().filter(|e| e.entry.project_id == input.project_id).and_then(|e| e.fields.get(key));
            if field.archived {
                if old != Some(value) { return Err(invalid("field_archived")); }
                continue;
            }
            if value.is_null() { continue; }
            validate_value(field, value, old)?;
            values.push((id, value.to_string(), scalar_text(value)));
        }
        let start = input.start.format("%Y-%m-%dT%H:%M").to_string();
        let end = input.end.format("%Y-%m-%dT%H:%M").to_string();
        let description = input.description.split_whitespace().collect::<Vec<_>>().join(" ");
        let id = if let Some(id) = input.id {
            self.conn.execute("UPDATE entries SET project_id=?1,start_at=?2,end_at=?3,description=?4 WHERE id=?5",
                params![input.project_id, start, end, description, id])?;
            id
        } else {
            self.conn.execute("INSERT INTO entries(project_id,start_at,end_at,description) VALUES(?1,?2,?3,?4)",
                params![input.project_id, start, end, description])?;
            self.conn.last_insert_rowid()
        };
        self.conn.execute("DELETE FROM entry_values WHERE entry_id=?1 AND field_id IN (SELECT id FROM project_fields WHERE project_id=?2 AND archived=0)", params![id, input.project_id])?;
        if !values.is_empty() {
            let sql = format!("INSERT INTO entry_values(entry_id,field_id,value_json,value_text) VALUES {}",
                vec!["(?,?,?,?)"; values.len()].join(","));
            let bound = values.into_iter().flat_map(|(field, json, text)| [
                rusqlite::types::Value::Integer(id), rusqlite::types::Value::Integer(field),
                rusqlite::types::Value::Text(json), rusqlite::types::Value::Text(text),
            ]);
            self.conn.execute(&sql, params_from_iter(bound))?;
        }
        let saved = self.workspace_entry(id)?;
        tx.commit()?;
        Ok(saved)
    }

    pub fn fields(&self, project_id: Option<i64>, include_archived: bool) -> Result<Vec<FieldDefinition>, KalaError> {
        let mut sql = format!("SELECT {FIELD_COLS} FROM project_fields WHERE 1=1");
        if project_id.is_some() { sql.push_str(" AND project_id=?1"); }
        if !include_archived { sql.push_str(" AND archived=0"); }
        sql.push_str(" ORDER BY project_id,position,id");
        let mut stmt = self.conn.prepare_cached(&sql)?;
        Ok(stmt.query_map(params_from_iter(project_id), field_row)?.collect::<Result<_, _>>()?)
    }

    fn field(&self, id: i64) -> Result<FieldDefinition, KalaError> {
        self.conn.query_row(&format!("SELECT {FIELD_COLS} FROM project_fields WHERE id=?1"), [id], field_row)
            .optional()?.ok_or_else(|| invalid("field_not_found"))
    }

    fn require_project(&self, id: i64) -> Result<(), KalaError> {
        let exists: bool = self.conn.query_row("SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1)", [id], |r| r.get(0))?;
        if exists { Ok(()) } else { Err(invalid("project_not_found")) }
    }

    pub fn save_field(&self, input: &FieldInput) -> Result<FieldDefinition, KalaError> {
        let name = input.name.trim();
        if name.is_empty() || name.len() > 512 { return Err(invalid("field_name_invalid")); }
        if input.position < 0 { return Err(invalid("field_position_invalid")); }
        let options: Vec<_> = input.options.iter().map(|s| s.trim().to_owned()).collect();
        if options.len() > 200 || options.iter().any(|s| s.is_empty() || s.len() > 1000)
            || options.iter().collect::<HashSet<_>>().len() != options.len()
            || (input.kind == FieldKind::Select && options.is_empty())
            || (input.kind != FieldKind::Select && !options.is_empty()) {
            return Err(invalid("field_options_invalid"));
        }
        self.require_project(input.project_id)?;
        if let Some(id) = input.id {
            let current = self.field(id)?;
            if current.project_id != input.project_id { return Err(invalid("field_project_immutable")); }
            if current.kind != input.kind { return Err(invalid("field_kind_immutable")); }
        }
        let options = serde_json::to_string(&options).map_err(|_| invalid("field_options_invalid"))?;
        let id = if let Some(id) = input.id {
            self.conn.execute("UPDATE project_fields SET name=?1,options=?2,show_in_table=?3,show_in_export=?4,position=?5,archived=?6 WHERE id=?7",
                params![name, options, input.show_in_table, input.show_in_export, input.position, input.archived, id])?;
            id
        } else {
            self.conn.execute("INSERT INTO project_fields(project_id,name,kind,options,show_in_table,show_in_export,position,archived) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![input.project_id, name, input.kind.as_str(), options, input.show_in_table, input.show_in_export, input.position, input.archived])?;
            self.conn.last_insert_rowid()
        };
        self.field(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::{d, dt, id_of, temp_store};
    use serde_json::json;

    fn entry_input(project_id: i64) -> WorkspaceEntryInput {
        WorkspaceEntryInput { id: None, project_id, start: dt(16, 9, 0), end: dt(16, 10, 0),
            description: "Work".into(), fields: BTreeMap::new() }
    }

    fn query() -> WorkspaceQuery {
        WorkspaceQuery { from: d(2026,10,15), to: d(2026,10,16), project_id: None,
            search: String::new(), field_id: None, field_value: None, limit: 100, offset: 0 }
    }

    #[test]
    fn report_keyset_pages_handle_equal_start_times_without_skips() {
        let (_dir,s) = temp_store();
        let project = id_of(&s,"Acme");
        let mut ids = Vec::new();
        for i in 0..7 {
            let mut input = entry_input(project);
            input.description = format!("row {i}");
            ids.push(s.save_workspace_entry(&input).unwrap().entry.id);
        }
        ids.reverse();
        let mut q = query(); q.limit=3; q.offset=500;
        let tx = s.conn.unchecked_transaction().unwrap();
        let mut actual = Vec::new();
        let mut before = None;
        loop {
            let page = s.workspace_entries_before(&q,before).unwrap();
            if page.is_empty() { break; }
            let last = &page.last().unwrap().entry;
            before = Some((last.start,last.id));
            actual.extend(page.into_iter().map(|e|e.entry.id));
        }
        tx.commit().unwrap();
        assert_eq!(actual,ids);
    }

    #[test]
    fn saved_views_persist_edit_and_delete_without_affecting_entries() {
        let (dir,s) = temp_store();
        let mut input = ViewInput { id: None, name: "October".into(), config: json!({"query":query(),"columns":["date","hours"],"groupBy":"day"}) };
        let saved = s.save_view(&input).unwrap();
        input.id = Some(saved.id);
        input.name = "October report".into();
        input.config["columns"] = json!(["project","hours"]);
        s.save_view(&input).unwrap();
        drop(s);
        let s = Store::open(&dir.path().join("kala.db")).unwrap();
        let views = s.saved_views().unwrap();
        assert_eq!(views,[SavedView { id:saved.id,name:input.name,config:input.config }]);
        s.delete_view(saved.id).unwrap();
        assert!(s.saved_views().unwrap().is_empty());
        assert!(s.delete_view(saved.id).is_err());
        assert_eq!(s.projects(true).unwrap().len(),2);
    }

    #[test]
    fn saved_views_reject_nonobjects_oversized_configs_and_missing_ids() {
        let (_dir,s) = temp_store();
        let mut input = ViewInput { id:None,name:"Report".into(),config:json!([]) };
        assert!(s.save_view(&input).is_err());
        input.config = json!({"value":"x".repeat(65536)});
        assert!(s.save_view(&input).is_err());
        input.config = json!({}); input.name = "  ".into();
        assert!(s.save_view(&input).is_err());
        input.name = "Report".into(); input.id=Some(9999);
        assert!(s.save_view(&input).is_err());
        assert!(s.saved_views().unwrap().is_empty());
    }

    #[test]
    fn query_pagination_keeps_all_match_totals_and_scopes_dates_search_projects() {
        let (_dir, s) = temp_store();
        let acme = id_of(&s, "Acme");
        let globex = id_of(&s, "Globex");
        for (project, day, hour, description) in [(acme,14,9,"outside"),(acme,15,9,"100%_done"),
            (globex,16,10,"Review"),(acme,16,23,"Late"),(acme,17,0,"outside")] {
            let mut input = entry_input(project);
            input.start = dt(day,hour,0);
            input.end = input.start + chrono::Duration::hours(1);
            input.description = description.into();
            s.save_workspace_entry(&input).unwrap();
        }
        s.timer_start(acme,"running",dt(16,12,0)).unwrap();
        let mut q = query();
        q.limit = 1;
        let first = s.workspace(&q).unwrap();
        assert_eq!((first.entries.len(),first.total,first.total_minutes), (1,3,180));
        assert_eq!(first.entries[0].entry.description,"Late");
        assert_eq!(first.by_day.iter().map(|d| (d.date,d.minutes,d.entries)).collect::<Vec<_>>(),
            [(d(2026,10,15),60,1),(d(2026,10,16),120,2)]);
        assert_eq!(first.by_project.iter().find(|p| p.project_id==acme).map(|p| (p.minutes,p.entries)),Some((120,2)));
        q.offset = 1;
        assert_eq!(s.workspace_entries(&q).unwrap()[0].entry.description,"Review");
        assert_eq!(s.workspace(&q).unwrap().total,3);
        q.offset = 0;
        q.project_id = Some(acme);
        assert_eq!(s.workspace(&q).unwrap().total_minutes,120);
        q.search = "%_".into();
        assert_eq!(s.workspace(&q).unwrap().total,1);
        q.search = "acme".into();
        assert_eq!(s.workspace(&q).unwrap().total,2);
        q.search = "' OR 1=1 --".into();
        assert_eq!(s.workspace(&q).unwrap().total,0);
    }

    #[test]
    fn custom_filter_and_groups_use_exact_values_including_archived_definitions() {
        let (_dir, s) = temp_store();
        let acme = id_of(&s,"Acme");
        let mut definition = field_input(acme,FieldKind::Number);
        let field = s.save_field(&definition).unwrap();
        for (hour,value) in [(8,Some(json!(2))),(9,Some(json!(20))),(10,Some(json!(2))),(11,None)] {
            let mut input = entry_input(acme);
            input.start = dt(16,hour,0);
            input.end = input.start + chrono::Duration::hours(1);
            if let Some(value) = value { input.fields.insert(field.id.to_string(),value); }
            s.save_workspace_entry(&input).unwrap();
        }
        let mut q = query();
        q.field_id = Some(field.id);
        q.limit = 1;
        let data = s.workspace(&q).unwrap();
        assert_eq!(data.total,4);
        assert_eq!(data.field_groups.iter().find(|g| g.value=="2").map(|g| (g.minutes,g.entries)),Some((120,2)));
        assert_eq!(data.field_groups.iter().find(|g| g.value.is_empty()).map(|g| g.entries),Some(1));
        definition.id = Some(field.id);
        definition.archived = true;
        s.save_field(&definition).unwrap();
        q.field_value = Some("2".into());
        let filtered = s.workspace(&q).unwrap();
        assert_eq!((filtered.entries.len(),filtered.total,filtered.total_minutes),(1,2,120));
        assert_eq!(filtered.entries[0].fields.get(&field.id.to_string()),Some(&json!(2)));
        q.project_id = Some(id_of(&s,"Globex"));
        assert!(s.workspace(&q).is_err());
    }

    #[test]
    fn query_rejects_malformed_ranges_pagination_and_field_filters() {
        let (_dir,s) = temp_store();
        let mut q = query();
        q.to = q.from.pred_opt().unwrap();
        assert!(s.workspace(&q).is_err());
        q = query(); q.limit = 201;
        assert!(s.workspace(&q).is_err());
        q.limit = 0;
        assert!(s.workspace(&q).is_err());
        q = query(); q.field_value = Some("abc".into());
        assert!(s.workspace(&q).is_err());
        q.field_id = Some(999999);
        assert!(s.workspace(&q).is_err());
        q = query(); q.project_id = Some(999999);
        assert!(s.workspace(&q).is_err());
        let defaults: WorkspaceQuery = serde_json::from_value(json!({"from":"2026-10-15","to":"2026-10-16"})).unwrap();
        assert_eq!((defaults.limit,defaults.offset,defaults.search.as_str()),(100,0,""));
    }

    #[test]
    fn entry_values_are_typed_atomic_and_cannot_cross_projects() {
        let (_dir, s) = temp_store();
        let acme = id_of(&s, "Acme");
        let globex = id_of(&s, "Globex");
        let text = s.save_field(&field_input(acme, FieldKind::Text)).unwrap();
        let number = s.save_field(&field_input(acme, FieldKind::Number)).unwrap();
        let foreign = s.save_field(&field_input(globex, FieldKind::Text)).unwrap();
        let mut input = entry_input(acme);
        input.fields = BTreeMap::from([(text.id.to_string(), json!("ABC-12")), (number.id.to_string(), json!(3.5))]);
        let saved = s.save_workspace_entry(&input).unwrap();
        input.id = Some(saved.entry.id);
        input.description = "Changed".into();
        input.fields.insert(number.id.to_string(), json!("3.5"));
        assert!(s.save_workspace_entry(&input).is_err());
        assert_eq!(s.workspace_entry(saved.entry.id).unwrap(), saved);
        input.fields.insert(number.id.to_string(), json!(4));
        input.fields.insert(foreign.id.to_string(), json!("wrong project"));
        assert!(s.save_workspace_entry(&input).is_err());
        input.fields.remove(&foreign.id.to_string());
        input.fields.insert("999999".into(), json!(null));
        assert!(s.save_workspace_entry(&input).is_err());
        input.fields.remove("999999");
        input.project_id = globex;
        assert!(s.save_workspace_entry(&input).is_err());
        input.fields.clear();
        let reassigned = s.save_workspace_entry(&input).unwrap();
        assert_eq!(reassigned.entry.project_id, globex);
        assert!(reassigned.fields.is_empty());
    }

    #[test]
    fn database_value_failure_rolls_back_entry_change_and_previous_values() {
        let (_dir, s) = temp_store();
        let project = id_of(&s, "Acme");
        let field = s.save_field(&field_input(project, FieldKind::Text)).unwrap();
        let mut input = entry_input(project);
        input.fields.insert(field.id.to_string(), json!("original"));
        let saved = s.save_workspace_entry(&input).unwrap();
        s.conn.execute_batch("CREATE TRIGGER reject_value BEFORE INSERT ON entry_values
            WHEN NEW.value_text='reject' BEGIN SELECT RAISE(ABORT,'test failure'); END;").unwrap();
        input.id = Some(saved.entry.id);
        input.description = "must roll back".into();
        input.fields.insert(field.id.to_string(), json!("reject"));
        assert!(s.save_workspace_entry(&input).is_err());
        assert_eq!(s.workspace_entry(saved.entry.id).unwrap(), saved);
        input.id = None;
        assert!(s.save_workspace_entry(&input).is_err());
        assert_eq!(s.conn.query_row("SELECT COUNT(*) FROM entries", [], |r| r.get::<_, i64>(0)).unwrap(), 1);
    }

    #[test]
    fn optional_field_types_reject_invalid_values_and_accept_empty_values() {
        let (_dir, s) = temp_store();
        let project = id_of(&s, "Acme");
        let cases = [
            (FieldKind::Text, json!("value"), json!(["value"])),
            (FieldKind::Number, json!(0), json!(false)),
            (FieldKind::Date, json!("2026-10-16"), json!("2026-02-30")),
            (FieldKind::Select, json!("Todo"), json!("Other")),
            (FieldKind::Checkbox, json!(false), json!("false")),
            (FieldKind::Url, json!("https://example.com/task/1"), json!("javascript:alert(1)")),
        ];
        for (kind, good, bad) in cases {
            let mut definition = field_input(project, kind);
            if kind == FieldKind::Select { definition.options = vec!["Todo".into()]; }
            let field = s.save_field(&definition).unwrap();
            let mut input = entry_input(project);
            input.fields.insert(field.id.to_string(), bad);
            assert!(s.save_workspace_entry(&input).is_err(), "{kind:?}");
            input.fields.insert(field.id.to_string(), good.clone());
            let saved = s.save_workspace_entry(&input).unwrap();
            assert_eq!(saved.fields.get(&field.id.to_string()), Some(&good));
            input.id = Some(saved.entry.id);
            input.fields.insert(field.id.to_string(), Value::Null);
            assert!(s.save_workspace_entry(&input).unwrap().fields.is_empty());
        }
    }

    #[test]
    fn historical_options_and_archived_values_survive_entry_edits() {
        let (_dir, s) = temp_store();
        let project = id_of(&s, "Acme");
        let mut definition = field_input(project, FieldKind::Select);
        definition.options = vec!["Old".into(), "New".into()];
        let field = s.save_field(&definition).unwrap();
        let mut input = entry_input(project);
        input.fields.insert(field.id.to_string(), json!("Old"));
        let saved = s.save_workspace_entry(&input).unwrap();
        input.id = Some(saved.entry.id);
        definition.id = Some(field.id);
        definition.options = vec!["New".into()];
        s.save_field(&definition).unwrap();
        input.description = "Edited".into();
        assert_eq!(s.save_workspace_entry(&input).unwrap().fields, saved.fields);
        definition.archived = true;
        s.save_field(&definition).unwrap();
        input.fields.clear();
        assert_eq!(s.save_workspace_entry(&input).unwrap().fields, saved.fields);
        input.fields.insert(field.id.to_string(), json!("New"));
        assert!(s.save_workspace_entry(&input).is_err());
        s.update_entry(saved.entry.id, &crate::store::EntryPatch { project_id: Some(id_of(&s, "Globex")), ..Default::default() }).unwrap();
        assert!(s.workspace_entry(saved.entry.id).unwrap().fields.is_empty());
        s.delete_entry(saved.entry.id).unwrap();
        assert_eq!(s.conn.query_row("SELECT COUNT(*) FROM entry_values", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
    }

    #[test]
    fn legacy_multiday_timer_keeps_times_when_editing_description_and_fields() {
        let (_dir, s) = temp_store();
        let project = id_of(&s, "Acme");
        let field = s.save_field(&field_input(project, FieldKind::Text)).unwrap();
        s.timer_start(project, "Long session", dt(14, 8, 0)).unwrap();
        let original = s.timer_stop(None, dt(16, 10, 0)).unwrap();
        let input = WorkspaceEntryInput {
            id: Some(original.id), project_id: project, start: original.start,
            end: original.end.unwrap(), description: "Updated description".into(),
            fields: BTreeMap::from([(field.id.to_string(), json!("ABC-12"))]),
        };
        let saved = s.save_workspace_entry(&input).unwrap();
        assert_eq!((saved.entry.start, saved.entry.end), (dt(14, 8, 0), Some(dt(16, 10, 0))));
        assert_eq!((saved.entry.end.unwrap() - saved.entry.start).num_minutes(), 3000);
        assert_eq!(saved.entry.description, "Updated description");
        assert_eq!(saved.fields.get(&field.id.to_string()), Some(&json!("ABC-12")));
    }

    #[test]
    fn legacy_multiday_timer_rejects_changed_times_over_one_day() {
        let (_dir, s) = temp_store();
        let project = id_of(&s, "Acme");
        s.timer_start(project, "Long session", dt(14, 8, 0)).unwrap();
        let original = s.timer_stop(None, dt(16, 10, 0)).unwrap();
        let mut input = WorkspaceEntryInput {
            id: Some(original.id), project_id: project, start: dt(14, 9, 0),
            end: dt(16, 11, 0), description: "Changed times".into(), fields: BTreeMap::new(),
        };
        assert!(s.save_workspace_entry(&input).is_err());
        assert_eq!(s.workspace_entry(original.id).unwrap().entry, original);
        input.start = original.start;
        assert!(s.save_workspace_entry(&input).is_err());
        input.end = input.start + chrono::Duration::hours(24);
        assert_eq!(s.save_workspace_entry(&input).unwrap().entry.end, Some(dt(15, 8, 0)));
    }

    #[test]
    fn explicit_entry_rejects_invalid_duration_and_running_timer_edits() {
        let (_dir, s) = temp_store();
        let project = id_of(&s, "Acme");
        let mut input = entry_input(project);
        input.end = input.start;
        assert!(s.save_workspace_entry(&input).is_err());
        input.end = input.start + chrono::Duration::hours(24) + chrono::Duration::minutes(1);
        assert!(s.save_workspace_entry(&input).is_err());
        input.end = input.start + chrono::Duration::hours(24);
        assert!(s.save_workspace_entry(&input).is_ok());
        s.timer_start(project, "running", dt(16, 12, 0)).unwrap();
        input.id = Some(s.running().unwrap().unwrap().id);
        assert!(s.save_workspace_entry(&input).is_err());
        assert!(s.running().unwrap().is_some());
    }

    fn field_input(project_id: i64, kind: FieldKind) -> FieldInput {
        FieldInput {
            id: None, project_id, name: "Ticket".into(), kind, options: vec![],
            show_in_table: true, show_in_export: true, position: 0, archived: false,
        }
    }

    #[test]
    fn field_definition_survives_rename_archive_restore_and_reopen() {
        let (dir, s) = temp_store();
        let project = id_of(&s, "Acme");
        let mut input = field_input(project, FieldKind::Text);
        let original = s.save_field(&input).unwrap();
        input.id = Some(original.id);
        input.name = "Issue".into();
        input.archived = true;
        s.save_field(&input).unwrap();
        assert!(s.fields(Some(project), false).unwrap().is_empty());
        drop(s);
        let s = Store::open(&dir.path().join("kala.db")).unwrap();
        let archived = s.fields(Some(project), true).unwrap();
        assert_eq!((archived[0].id, archived[0].name.as_str(), archived[0].archived), (original.id, "Issue", true));
        input.archived = false;
        s.save_field(&input).unwrap();
        assert_eq!(s.fields(None, false).unwrap().len(), 1);
    }

    #[test]
    fn field_types_projects_and_options_are_validated() {
        let (_dir, s) = temp_store();
        let acme = id_of(&s, "Acme");
        let globex = id_of(&s, "Globex");
        let mut input = field_input(acme, FieldKind::Select);
        input.options = vec!["Todo".into(), "Done".into()];
        let field = s.save_field(&input).unwrap();
        input.id = Some(field.id);
        input.kind = FieldKind::Text;
        assert!(s.save_field(&input).is_err());
        input.kind = FieldKind::Select;
        input.project_id = globex;
        assert!(s.save_field(&input).is_err());
        input.project_id = acme;
        input.options = vec!["Todo".into(), " Todo ".into()];
        assert!(s.save_field(&input).is_err());
        input.options = vec![];
        assert!(s.save_field(&input).is_err());
        input.id = None;
        input.project_id = 9999;
        assert!(s.save_field(&input).is_err());
        assert_eq!(s.fields(None, true).unwrap(), [field]);
    }
}
