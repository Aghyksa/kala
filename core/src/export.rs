use std::path::Path;

use chrono::{NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};

use rust_xlsxwriter::{Format, FormatBorder, Formula, Workbook, XlsxError};

use crate::i18n::{label, month_name, Lang};
use crate::store::Store;
use crate::{KalaError, Month};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRow {
    pub no: u32,
    pub date: NaiveDate,
    pub start: NaiveTime,
    pub end: NaiveTime,
    pub description: String,
    pub hours: f64,
    pub price: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportDoc {
    pub title: String,
    pub period: String,
    pub user_name: String,
    pub currency: String,
    pub show_price: bool,
    pub lang: Lang,
    pub rows: Vec<ExportRow>,
    pub total_hours: f64,
    pub total_price: Option<f64>,
}

pub fn build_export(store: &Store, project_id: i64, month: Month, lang: Lang) -> Result<ExportDoc, KalaError> {
    let project = store
        .projects(true)?
        .into_iter()
        .find(|p| p.id == project_id)
        .ok_or(KalaError::InvalidEntry { reason: "project_not_found".into() })?;
    let show_price = project.hourly_rate > 0.0;
    let rows: Vec<ExportRow> = store
        .entries_between(month.first_day(), month.last_day(), Some(project_id))?
        .into_iter()
        .enumerate()
        .map(|(i, e)| {
            let end = e.end.expect("entries_between returns finished entries");
            let hours = (end - e.start).num_minutes() as f64 / 60.0;
            ExportRow {
                no: i as u32 + 1,
                date: e.start.date(),
                start: e.start.time(),
                end: end.time(),
                description: e.description,
                hours,
                price: show_price.then(|| hours * project.hourly_rate),
            }
        })
        .collect();
    // Totals from exact minutes so they match Store::summary; rounding is display-only.
    let total_minutes: f64 = rows.iter().map(|r| r.hours * 60.0).sum::<f64>().round();
    let total_hours = round2(total_minutes / 60.0);
    let total_price = show_price.then(|| round2(total_minutes / 60.0 * project.hourly_rate));
    Ok(ExportDoc {
        title: format!("{} {}", label(lang, "timesheet"), project.name),
        period: format!("{} {}", month_name(lang, month.month), month.year),
        user_name: store.settings()?.user_name,
        currency: project.currency,
        show_price,
        lang,
        rows,
        total_hours,
        total_price,
    })
}

/// `+ 0.0` turns the -0.0 that an empty f64 sum yields into 0.0.
fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0 + 0.0
}

/// ISO 4217 currencies without minor units show no decimals; everything else keeps cents.
pub fn price_format(currency: &str) -> &'static str {
    const ZERO_DECIMAL: [&str; 9] = ["IDR", "JPY", "KRW", "VND", "CLP", "ISK", "PYG", "UGX", "XAF"];
    if ZERO_DECIMAL.contains(&currency.to_uppercase().as_str()) { "#,##0" } else { "#,##0.00" }
}

fn money(x: f64) -> String {
    let r = round2(x);
    if r.fract() == 0.0 { format!("{}", r as i64) } else { format!("{r:.2}") }
}

fn headers(doc: &ExportDoc) -> Vec<String> {
    let mut h: Vec<String> = ["col_no", "col_date", "col_start", "col_end", "col_desc", "col_hours"]
        .iter()
        .map(|k| label(doc.lang, k).to_string())
        .collect();
    if doc.show_price {
        h.push(format!("{} ({})", label(doc.lang, "col_price"), doc.currency));
    }
    h
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() }
}

pub fn write_csv(doc: &ExportDoc, path: &Path) -> Result<(), KalaError> {
    let mut out = headers(doc).join(",") + "\n";
    for r in &doc.rows {
        let mut cols = vec![
            r.no.to_string(),
            r.date.format("%d/%m/%Y").to_string(),
            r.start.format("%H:%M").to_string(),
            r.end.format("%H:%M").to_string(),
            csv_field(&r.description),
            format!("{:.2}", r.hours),
        ];
        cols.extend(r.price.map(money));
        out += &(cols.join(",") + "\n");
    }
    out += &format!(",,,,{},{:.2}", label(doc.lang, "total"), doc.total_hours);
    if let Some(p) = doc.total_price {
        out += &format!(",{}", money(p));
    }
    out += "\n";
    Ok(std::fs::write(path, out)?)
}

pub fn write_xlsx(doc: &ExportDoc, path: &Path) -> Result<(), KalaError> {
    xlsx(doc, path).map_err(|e| KalaError::Io { message: e.to_string() })
}

fn xlsx(doc: &ExportDoc, path: &Path) -> Result<(), XlsxError> {
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.set_name("Timesheet")?;
    let bold = Format::new().set_bold();
    let header = Format::new().set_bold().set_border_bottom(FormatBorder::Thin);
    let date_f = Format::new().set_num_format("dd/mm/yyyy");
    let time_f = Format::new().set_num_format("hh:mm");
    let hours_f = Format::new().set_num_format("0.00");
    let price_f = Format::new().set_num_format(price_format(&doc.currency));
    let wrap = Format::new().set_text_wrap();

    ws.write_with_format(0, 0, &doc.title, &Format::new().set_bold().set_font_size(14))?;
    ws.write(1, 0, &doc.period)?;
    if !doc.user_name.is_empty() {
        ws.write(2, 0, &doc.user_name)?;
    }
    for (c, h) in headers(doc).iter().enumerate() {
        ws.write_with_format(4, c as u16, h, &header)?;
    }
    let first = 5u32;
    for (i, r) in doc.rows.iter().enumerate() {
        let row = first + i as u32;
        ws.write(row, 0, r.no)?;
        ws.write_with_format(row, 1, &r.date, &date_f)?;
        ws.write_with_format(row, 2, &r.start, &time_f)?;
        ws.write_with_format(row, 3, &r.end, &time_f)?;
        ws.write_with_format(row, 4, &r.description, &wrap)?;
        ws.write_with_format(row, 5, r.hours, &hours_f)?;
        if let Some(p) = r.price {
            ws.write_with_format(row, 6, p, &price_f)?;
        }
    }
    let total_row = first + doc.rows.len() as u32;
    let sum = |col: char, value: f64| {
        let f = if doc.rows.is_empty() { "=0".to_string() } else { format!("=SUM({col}{}:{col}{})", first + 1, total_row) };
        Formula::new(f).set_result(value.to_string())
    };
    ws.write_with_format(total_row, 4, label(doc.lang, "total"), &bold)?;
    ws.write_formula_with_format(total_row, 5, sum('F', doc.total_hours), &hours_f.clone().set_bold())?;
    if let Some(p) = doc.total_price {
        ws.write_formula_with_format(total_row, 6, sum('G', p), &price_f.clone().set_bold())?;
    }
    ws.set_column_width(0, 5)?;
    ws.set_column_width(1, 12)?;
    ws.set_column_width(4, 50)?;
    ws.set_column_width(6, 14)?;
    wb.save(path)
}

pub fn default_file_name(project: &str, month: Month, ext: &str) -> String {
    let safe: String = project.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    format!("Kala-{safe}-{}-{:02}.{ext}", month.year, month.month)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::{d, dt, id_of, now, temp_store};
    use crate::store::ProjectInput;

    const OCT: Month = Month { year: 2026, month: 10 };

    fn set_rate(s: &Store, name: &str, rate: f64) {
        let p = s.projects(true).unwrap().into_iter().find(|p| p.name == name).unwrap();
        s.save_project(&ProjectInput {
            id: Some(p.id),
            name: p.name,
            aliases: p.aliases,
            color: p.color,
            hourly_rate: rate,
            currency: p.currency,
            rounding_min: p.rounding_min,
        })
        .unwrap();
    }

    #[test]
    fn rows_totals_and_price() {
        let (_d, s) = temp_store();
        set_rate(&s, "Acme", 100000.0);
        s.log_text("16/10 acme 13-15 fix role", now()).unwrap();
        s.log_text("15/10 acme 9-10 review", now()).unwrap();
        let doc = build_export(&s, id_of(&s, "Acme"), OCT, Lang::Id).unwrap();
        assert_eq!((doc.title.as_str(), doc.period.as_str()), ("Timesheet Acme", "Oktober 2026"));
        let got: Vec<_> = doc.rows.iter().map(|r| (r.no, r.date, r.hours, r.price)).collect();
        assert_eq!(got, [(1, d(2026, 10, 15), 1.0, Some(100000.0)), (2, d(2026, 10, 16), 2.0, Some(200000.0))]);
        assert_eq!((doc.total_hours, doc.total_price, doc.show_price), (3.0, Some(300000.0), true));
    }

    #[test]
    fn totals_come_from_exact_minutes_and_match_summary() {
        let (_d, s) = temp_store();
        set_rate(&s, "Acme", 100000.0);
        for t in ["16/10 acme 9-9.20 a", "16/10 acme 9.20-9.40 b", "16/10 acme 9.40-10 c"] {
            s.log_text(t, now()).unwrap();
        }
        let acme = id_of(&s, "Acme");
        let doc = build_export(&s, acme, OCT, Lang::En).unwrap();
        let sum = &s.summary(OCT, Some(acme)).unwrap()[0];
        assert_eq!((doc.total_hours, doc.total_price), (1.0, Some(100000.0)));
        assert_eq!((sum.minutes as f64 / 60.0, sum.amount), (doc.total_hours, doc.total_price.unwrap()));
    }

    #[test]
    fn month_boundaries() {
        let (_d, s) = temp_store();
        let acme = id_of(&s, "Acme");
        s.log_text("31/10/2026 acme 22-23 a", now()).unwrap();
        s.log_text("01/11/2026 acme 9-10 b", now()).unwrap();
        s.timer_start(acme, "midnight", dt(31, 23, 0)).unwrap();
        s.timer_stop(None, d(2026, 11, 1).and_hms_opt(1, 0, 0).unwrap()).unwrap();
        s.timer_start(acme, "still running", dt(30, 9, 0)).unwrap();
        let doc = build_export(&s, acme, OCT, Lang::En).unwrap();
        let got: Vec<_> = doc.rows.iter().map(|r| (r.date, r.description.as_str(), r.hours)).collect();
        assert_eq!(got, [(d(2026, 10, 31), "a", 1.0), (d(2026, 10, 31), "midnight", 2.0)]);
        assert_eq!((doc.total_hours, doc.show_price, doc.total_price), (3.0, false, None));
    }

    #[test]
    fn empty_month_still_writes_valid_files() {
        let (dir, s) = temp_store();
        let doc = build_export(&s, id_of(&s, "Globex"), OCT, Lang::Id).unwrap();
        assert_eq!((doc.rows.len(), doc.total_hours), (0, 0.0));
        let x = dir.path().join("e.xlsx");
        write_xlsx(&doc, &x).unwrap();
        assert_eq!(&std::fs::read(&x).unwrap()[..2], b"PK");
        let c = dir.path().join("e.csv");
        write_csv(&doc, &c).unwrap();
        assert_eq!(std::fs::read_to_string(&c).unwrap(), "No,Tanggal,Mulai,Selesai,Deskripsi,Jam\n,,,,Total,0.00\n");
    }

    #[test]
    fn csv_exact_text_and_quoting() {
        let (dir, s) = temp_store();
        s.log_text("16/10 acme 13-15 fix \"role\", lagi", now()).unwrap();
        let doc = build_export(&s, id_of(&s, "Acme"), OCT, Lang::Id).unwrap();
        let c = dir.path().join("x.csv");
        write_csv(&doc, &c).unwrap();
        assert_eq!(
            std::fs::read_to_string(&c).unwrap(),
            "No,Tanggal,Mulai,Selesai,Deskripsi,Jam\n1,16/10/2026,13:00,15:00,\"fix \"\"role\"\", lagi\",2.00\n,,,,Total,2.00\n"
        );
    }

    #[test]
    fn csv_includes_price_column_when_rate_set() {
        let (dir, s) = temp_store();
        set_rate(&s, "Acme", 150000.0);
        s.log_text("16/10 acme 13-14.30 a", now()).unwrap();
        let doc = build_export(&s, id_of(&s, "Acme"), OCT, Lang::En).unwrap();
        let c = dir.path().join("p.csv");
        write_csv(&doc, &c).unwrap();
        assert_eq!(
            std::fs::read_to_string(&c).unwrap(),
            "No,Date,Start,End,Description,Hours,Price (IDR)\n1,16/10/2026,13:00,14:30,a,1.50,225000\n,,,,Total,1.50,225000\n"
        );
    }

    #[test]
    fn price_format_keeps_cents_except_zero_decimal_currencies() {
        assert_eq!(price_format("IDR"), "#,##0");
        assert_eq!(price_format("jpy"), "#,##0");
        assert_eq!(price_format("USD"), "#,##0.00");
        assert_eq!(price_format("SGD"), "#,##0.00");
    }

    #[test]
    fn default_file_name_format() {
        assert_eq!(default_file_name("Acme", OCT, "xlsx"), "Kala-Acme-2026-10.xlsx");
    }
}
