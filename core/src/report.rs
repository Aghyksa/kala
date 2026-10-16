use std::io::{BufWriter, Write};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use chrono::{NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};
use rust_xlsxwriter::{Format, FormatBorder, Formula, Workbook, XlsxError};

use crate::i18n::{label, Lang};
use crate::store::Store;
use crate::workspace::{FieldDefinition, FieldKind, WorkspaceEntry, WorkspaceQuery};
use crate::KalaError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ReportValue {
    Empty,
    Text(String),
    Date(NaiveDate),
    Time(NaiveTime),
    Number(f64),
    Boolean(bool),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportColumn {
    pub key: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportRow {
    pub cells: Vec<ReportValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportDoc {
    pub title: String,
    pub period: String,
    pub user_name: String,
    pub lang: Lang,
    pub columns: Vec<ReportColumn>,
    pub rows: Vec<ReportRow>,
    pub total_minutes: i64,
    pub total_hours: f64,
}

enum ColumnSource { No, Date, Project, Start, End, Description, Hours, Field(FieldDefinition) }

fn invalid(reason: &str) -> KalaError { KalaError::InvalidEntry { reason: reason.into() } }

pub fn build_report(store: &Store, query: &WorkspaceQuery, columns: &[String], lang: Lang) -> Result<ReportDoc, KalaError> {
    let snapshot = store.conn.unchecked_transaction()?;
    let projects: BTreeMap<_, _> = store.projects(true)?.into_iter().map(|p| (p.id, p.name)).collect();
    let project_name = query.project_id.map(|id| projects.get(&id).ok_or_else(|| invalid("project_not_found"))).transpose()?;
    let fields: BTreeMap<_, _> = store.fields(query.project_id, true)?.into_iter().map(|f| (f.id, f)).collect();
    let defaults = ["no", "date", "project", "start", "end", "description", "hours"].map(str::to_owned);
    let keys = if columns.is_empty() { &defaults[..] } else { columns };
    if keys.len() > 16_384 { return Err(invalid("report_columns_invalid")); }
    let mut seen = HashSet::new();
    let mut selected = Vec::new();
    let mut headers = Vec::new();
    for key in keys {
        if !seen.insert(key) { return Err(invalid("report_columns_invalid")); }
        let (source, title) = match key.as_str() {
            "no" => (ColumnSource::No, label(lang, "col_no").into()),
            "date" => (ColumnSource::Date, label(lang, "col_date").into()),
            "project" => (ColumnSource::Project, if lang == Lang::Id { "Proyek" } else { "Project" }.into()),
            "start" => (ColumnSource::Start, label(lang, "col_start").into()),
            "end" => (ColumnSource::End, label(lang, "col_end").into()),
            "description" => (ColumnSource::Description, label(lang, "col_desc").into()),
            "hours" => (ColumnSource::Hours, label(lang, "col_hours").into()),
            _ => {
                let id = key.strip_prefix("field:").and_then(|s| s.parse::<i64>().ok())
                    .filter(|id| *id > 0 && key == &format!("field:{id}"))
                    .ok_or_else(|| invalid("report_columns_invalid"))?;
                let field = fields.get(&id).ok_or_else(|| invalid("report_columns_invalid"))?;
                (ColumnSource::Field(field.clone()), field.name.clone())
            },
        };
        headers.push(ReportColumn { key: key.clone(), title });
        selected.push(source);
    }
    let mut page_query = query.clone();
    page_query.limit = 200;
    page_query.offset = 0;
    let mut page = store.workspace_entries_before(&page_query, None)?;
    let mut total_minutes = 0i64;
    let mut rows = Vec::new();
    loop {
        let count = page.len();
        let cursor = page.last().map(|row| (row.entry.start, row.entry.id));
        for entry in page {
            let end = entry.entry.end.ok_or_else(|| invalid("entry_unfinished"))?;
            let minutes = (end - entry.entry.start).num_minutes();
            total_minutes += minutes;
            let cells = selected.iter().map(|column| cell(column, &entry, rows.len() + 1, minutes, &projects)).collect::<Result<_, _>>()?;
            rows.push(ReportRow { cells });
        }
        if count < page_query.limit as usize { break; }
        page = store.workspace_entries_before(&page_query, cursor)?;
    }
    let user_name = store.settings()?.user_name;
    snapshot.commit()?;
    Ok(ReportDoc {
        title: project_name.map(|name| format!("{} {name}", label(lang, "timesheet"))).unwrap_or_else(|| label(lang, "timesheet").into()),
        period: format!("{} – {}", query.from.format("%d/%m/%Y"), query.to.format("%d/%m/%Y")),
        user_name,
        lang, columns: headers, rows, total_minutes, total_hours: total_minutes as f64 / 60.0,
    })
}

fn cell(column: &ColumnSource, row: &WorkspaceEntry, no: usize, minutes: i64, projects: &BTreeMap<i64, String>) -> Result<ReportValue, KalaError> {
    let entry = &row.entry;
    let end = entry.end.ok_or_else(|| invalid("entry_unfinished"))?;
    Ok(match column {
        ColumnSource::No => ReportValue::Number(no as f64),
        ColumnSource::Date => ReportValue::Date(entry.start.date()),
        ColumnSource::Start => ReportValue::Time(entry.start.time()),
        ColumnSource::End => ReportValue::Time(end.time()),
        ColumnSource::Project => ReportValue::Text(projects.get(&entry.project_id).cloned().ok_or_else(|| invalid("project_not_found"))?),
        ColumnSource::Description => ReportValue::Text(entry.description.clone()),
        ColumnSource::Hours => ReportValue::Number(minutes as f64 / 60.0),
        ColumnSource::Field(field) if field.project_id != entry.project_id => ReportValue::Empty,
        ColumnSource::Field(field) => match row.fields.get(&field.id.to_string()) {
            None | Some(serde_json::Value::Null) => ReportValue::Empty,
            Some(value) => match field.kind {
                FieldKind::Number => ReportValue::Number(value.as_f64().ok_or_else(|| invalid("field_value_invalid"))?),
                FieldKind::Checkbox => ReportValue::Boolean(value.as_bool().ok_or_else(|| invalid("field_value_invalid"))?),
                FieldKind::Date => ReportValue::Date(NaiveDate::parse_from_str(value.as_str().ok_or_else(|| invalid("field_value_invalid"))?, "%Y-%m-%d")
                    .map_err(|_| invalid("field_value_invalid"))?),
                _ => ReportValue::Text(value.as_str().ok_or_else(|| invalid("field_value_invalid"))?.to_owned()),
            },
        },
    })
}

fn csv_text(value: &str) -> String {
    let first = value.trim_start().chars().next();
    let value = if matches!(first, Some('=' | '+' | '-' | '@')) || value.starts_with(['\t', '\r']) {
        format!("'{value}")
    } else {
        value.to_string()
    };
    if value.contains([',', '"', '\n', '\r']) { format!("\"{}\"", value.replace('"', "\"\"")) } else { value }
}

pub fn write_report_csv(doc: &ReportDoc, path: &Path) -> Result<(), KalaError> {
    let mut out = BufWriter::new(std::fs::File::create(path)?);
    writeln!(out, "{}", doc.columns.iter().map(|c| csv_text(&c.title)).collect::<Vec<_>>().join(","))?;
    for row in &doc.rows {
        let values: Vec<_> = row.cells.iter().zip(&doc.columns).map(|(value, column)| match value {
            ReportValue::Empty => String::new(),
            ReportValue::Text(s) => csv_text(s),
            ReportValue::Date(d) => d.format("%d/%m/%Y").to_string(),
            ReportValue::Time(t) => t.format("%H:%M").to_string(),
            ReportValue::Number(n) if column.key == "hours" => format!("{n:.2}"),
            ReportValue::Number(n) => n.to_string(),
            ReportValue::Boolean(b) => b.to_string(),
        }).collect();
        writeln!(out, "{}", values.join(","))?;
    }
    let label_column = doc.columns.iter().position(|c| c.key != "hours");
    let totals: Vec<_> = doc.columns.iter().enumerate().map(|(i, c)| {
        if c.key == "hours" { format!("{:.2}", doc.total_hours) }
        else if Some(i) == label_column { label(doc.lang, "total").to_string() }
        else { String::new() }
    }).collect();
    writeln!(out, "{}", totals.join(","))?;
    Ok(out.flush()?)
}

pub fn write_report_xlsx(doc: &ReportDoc, path: &Path) -> Result<(), KalaError> {
    xlsx(doc, path).map_err(|e| KalaError::Io { message: e.to_string() })
}

fn xlsx(doc: &ReportDoc, path: &Path) -> Result<(), XlsxError> {
    let mut book = Workbook::new();
    let sheet = book.add_worksheet();
    sheet.set_name("Timesheet")?;
    let title = Format::new().set_bold().set_font_size(14);
    let header = Format::new().set_bold().set_border_bottom(FormatBorder::Thin);
    let date = Format::new().set_num_format("dd/mm/yyyy");
    let time = Format::new().set_num_format("hh:mm");
    let hours = Format::new().set_num_format("0.00");
    let wrapped = Format::new().set_text_wrap();
    sheet.write_with_format(0, 0, &doc.title, &title)?;
    sheet.write(1, 0, &doc.period)?;
    sheet.write(2, 0, &doc.user_name)?;
    sheet.write(3, 0, format!("{} ({})", label(doc.lang, "total"), label(doc.lang, "col_hours")))?;
    sheet.write_with_format(3, 1, doc.total_hours, &hours)?;
    for (column, info) in doc.columns.iter().enumerate() {
        let column = column as u16;
        sheet.write_with_format(4, column, &info.title, &header)?;
        let width = match info.key.as_str() {
            "no" => 6, "date" => 13, "start" | "end" => 10, "hours" => 12,
            "description" => 50, _ => 24,
        };
        sheet.set_column_width(column, width)?;
    }
    let first = 5u32;
    for (index, entry) in doc.rows.iter().enumerate() {
        let row = first + index as u32;
        for (column, value) in entry.cells.iter().enumerate() {
            let col = column as u16;
            match value {
                ReportValue::Empty => {},
                ReportValue::Text(s) => { sheet.write_string_with_format(row, col, s, &wrapped)?; },
                ReportValue::Date(d) => { sheet.write_with_format(row, col, d, &date)?; },
                ReportValue::Time(t) => { sheet.write_with_format(row, col, t, &time)?; },
                ReportValue::Number(n) if doc.columns[column].key == "hours" => { sheet.write_with_format(row, col, *n, &hours)?; },
                ReportValue::Number(n) => { sheet.write_number(row, col, *n)?; },
                ReportValue::Boolean(b) => { sheet.write_boolean(row, col, *b)?; },
            }
        }
    }
    let total_row = first + doc.rows.len() as u32;
    if let Some(column) = doc.columns.iter().position(|c| c.key != "hours") {
        sheet.write_with_format(total_row, column as u16, label(doc.lang, "total"), &header)?;
    }
    if let Some(column) = doc.columns.iter().position(|c| c.key == "hours") {
        let column = column as u16;
        let formula = if doc.rows.is_empty() { "=0".to_string() } else {
            let col = rust_xlsxwriter::utility::column_number_to_name(column);
            format!("=SUM({col}{}:{col}{})", first + 1, total_row)
        };
        sheet.write_formula_with_format(total_row, column, Formula::new(formula).set_result(doc.total_hours.to_string()), &hours.set_bold())?;
    }
    sheet.set_freeze_panes(first, 0)?;
    if !doc.columns.is_empty() {
        sheet.autofilter(4, 0, total_row.saturating_sub(1).max(4), doc.columns.len() as u16 - 1)?;
    }
    book.save(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::{d, id_of, now, temp_store};
    use crate::workspace::{FieldInput, WorkspaceEntryInput};

    fn query(project_id: Option<i64>) -> WorkspaceQuery {
        WorkspaceQuery {
            from: d(2026, 10, 16), to: d(2026, 10, 16), project_id,
            search: String::new(), field_id: None, field_value: None, limit: 1, offset: 200,
        }
    }

    #[test]
    fn report_exports_all_matches_across_pages_with_exact_minutes() {
        let (_dir, store) = temp_store();
        for _ in 0..205 {
            store.log_text("16/10 acme 9-9.20 issue", now()).unwrap();
        }
        for text in ["16/10 globex 9-10 issue", "15/10 acme 9-10 issue", "16/10 acme 9-10 other"] {
            store.log_text(text, now()).unwrap();
        }
        let mut query = query(Some(id_of(&store, "Acme")));
        query.search = "issue".into();
        let doc = build_report(&store, &query, &[], Lang::En).unwrap();
        assert_eq!((doc.rows.len(), doc.total_minutes), (205, 4100));
        assert_eq!(doc.total_hours, 68.33333333333333);
        assert_eq!(doc.columns.iter().map(|c| c.key.as_str()).collect::<Vec<_>>(),
            ["no", "date", "project", "start", "end", "description", "hours"]);
        assert_eq!(doc.rows[204].cells[0], ReportValue::Number(205.0));
        assert_eq!(doc.rows[0].cells[2], ReportValue::Text("Acme".into()));
        assert_eq!(doc.rows[0].cells[6], ReportValue::Number(1.0 / 3.0));
    }

    #[test]
    fn report_rejects_unknown_duplicate_and_noncanonical_columns() {
        let (_dir, store) = temp_store();
        let query = query(None);
        for keys in [vec!["rate"], vec!["currency"], vec!["amount"], vec!["hours", "hours"], vec!["field:999"], vec!["field:01"], vec!["field:-1"]] {
            let keys: Vec<_> = keys.into_iter().map(str::to_owned).collect();
            assert_eq!(build_report(&store, &query, &keys, Lang::En),
                Err(KalaError::InvalidEntry { reason: "report_columns_invalid".into() }));
        }
    }

    #[test]
    fn report_filters_custom_values_and_keeps_renamed_archived_field_types() {
        let (_dir, store) = temp_store();
        let project_id = id_of(&store, "Acme");
        let mut inputs: Vec<_> = [("Ticket", FieldKind::Text), ("Due", FieldKind::Date), ("Points", FieldKind::Number), ("Reviewed", FieldKind::Checkbox)]
            .into_iter().map(|(name, kind)| FieldInput {
                id: None, project_id, name: name.into(), kind, options: vec![],
                show_in_table: true, show_in_export: true, position: 0, archived: false,
            }).collect();
        let definitions: Vec<_> = inputs.iter().map(|input| store.save_field(input).unwrap()).collect();
        let values = [serde_json::json!("ABC"), serde_json::json!("2026-11-05"), serde_json::json!(2.5), serde_json::json!(true)];
        let fields = definitions.iter().zip(values).map(|(field, value)| (field.id.to_string(), value)).collect();
        store.save_workspace_entry(&WorkspaceEntryInput {
            id: None, project_id, start: d(2026, 10, 16).and_hms_opt(9, 0, 0).unwrap(),
            end: d(2026, 10, 16).and_hms_opt(9, 20, 0).unwrap(), description: "review".into(), fields,
        }).unwrap();
        store.log_text("16/10 acme 10-11 review", now()).unwrap();
        inputs[0].id = Some(definitions[0].id);
        inputs[0].name = "Issue".into();
        inputs[0].archived = true;
        store.save_field(&inputs[0]).unwrap();
        let mut query = query(Some(project_id));
        query.search = "review".into();
        query.field_id = Some(definitions[0].id);
        query.field_value = Some("ABC".into());
        let columns = vec![format!("field:{}", definitions[1].id), format!("field:{}", definitions[0].id),
            "hours".into(), format!("field:{}", definitions[2].id), format!("field:{}", definitions[3].id)];
        let doc = build_report(&store, &query, &columns, Lang::Id).unwrap();
        assert_eq!((doc.rows.len(), doc.total_minutes), (1, 20));
        assert_eq!(doc.columns.iter().map(|c| c.title.as_str()).collect::<Vec<_>>(), ["Due", "Issue", "Jam", "Points", "Reviewed"]);
        assert_eq!(doc.rows[0].cells, [ReportValue::Date(d(2026, 11, 5)), ReportValue::Text("ABC".into()),
            ReportValue::Number(1.0 / 3.0), ReportValue::Number(2.5), ReportValue::Boolean(true)]);
        query.project_id = Some(id_of(&store, "Globex"));
        assert_eq!(build_report(&store, &query, &columns, Lang::En),
            Err(KalaError::InvalidEntry { reason: "report_columns_invalid".into() }));
    }

    #[test]
    fn empty_report_keeps_requested_headers_and_zero_total() {
        let (dir, store) = temp_store();
        let doc = build_report(&store, &query(None), &["date".into(), "hours".into()], Lang::Id).unwrap();
        assert_eq!((doc.rows.len(), doc.total_minutes, doc.total_hours), (0, 0, 0.0));
        let path = dir.path().join("empty.csv");
        write_report_csv(&doc, &path).unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), "Tanggal,Jam\nTotal,0.00\n");
        write_report_xlsx(&doc, &dir.path().join("empty.xlsx")).unwrap();
    }

    fn document() -> ReportDoc {
        ReportDoc {
            title: "Timesheet Acme".into(),
            period: "01/10/2026 – 31/10/2026".into(),
            user_name: "A user".into(),
            lang: Lang::En,
            columns: [
                ("description", "Description"), ("date", "Date"), ("start", "Start"),
                ("hours", "Hours"), ("field:3", "Reviewed"), ("field:4", "Points"),
            ].into_iter().map(|(key, title)| ReportColumn { key: key.into(), title: title.into() }).collect(),
            rows: vec![ReportRow { cells: vec![
                ReportValue::Text("=review, \"quoted\"\nline".into()),
                ReportValue::Date(d(2026, 10, 16)),
                ReportValue::Time(NaiveTime::from_hms_opt(9, 20, 0).unwrap()),
                ReportValue::Number(1.0 / 3.0),
                ReportValue::Boolean(true),
                ReportValue::Number(2.5),
            ] }],
            total_minutes: 20,
            total_hours: 1.0 / 3.0,
        }
    }

    #[test]
    fn csv_escapes_text_formats_hours_and_preserves_custom_numbers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.csv");
        write_report_csv(&document(), &path).unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), concat!(
            "Description,Date,Start,Hours,Reviewed,Points\n",
            "\"'=review, \"\"quoted\"\"\nline\",16/10/2026,09:20,0.33,true,2.5\n",
            "Total,,,0.33,,\n",
        ));
    }

    #[test]
    fn xlsx_writes_numeric_date_time_hours_and_boolean_cells() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.xlsx");
        write_report_xlsx(&document(), &path).unwrap();
        let output = std::process::Command::new("unzip")
            .args(["-p", path.to_str().unwrap(), "xl/worksheets/sheet1.xml"]).output().unwrap();
        assert!(output.status.success());
        let sheet = String::from_utf8(output.stdout).unwrap();
        assert!(sheet.contains("<c r=\"B6\" s=\""), "{sheet}");
        assert!(sheet.contains("<v>46311</v>"), "date must be an Excel serial: {sheet}");
        assert!(sheet.contains("<v>0.3888888888888889</v>"), "time must be numeric: {sheet}");
        assert!(sheet.contains("<v>0.3333333333333333</v>"), "hours must be numeric: {sheet}");
        assert!(sheet.contains("t=\"b\"><v>1</v>"), "checkbox must be boolean: {sheet}");
        assert!(sheet.contains("<f>SUM(D6:D6)</f>"), "total must sum unrounded hours: {sheet}");
    }

    #[test]
    fn xlsx_keeps_exact_total_when_hours_column_is_omitted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("description.xlsx");
        let mut doc = document();
        doc.columns.truncate(1);
        doc.rows[0].cells.truncate(1);
        write_report_xlsx(&doc, &path).unwrap();
        let output = std::process::Command::new("unzip")
            .args(["-p", path.to_str().unwrap(), "xl/worksheets/sheet1.xml"]).output().unwrap();
        assert!(output.status.success());
        let sheet = String::from_utf8(output.stdout).unwrap();
        assert!(sheet.contains("<c r=\"B4\" s=\""), "exact report total must exist above headers: {sheet}");
        assert!(sheet.contains("<v>0.3333333333333333</v>"), "total must stay numeric: {sheet}");
    }
}
