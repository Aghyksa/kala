pub mod export;
pub mod i18n;
pub mod parse;
pub mod reminder;
pub mod round;
pub mod store;

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

pub use parse::{ParseError, Parsed, TimeSpec};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub aliases: Vec<String>,
    pub color: String,
    pub hourly_rate: f64,
    pub currency: String,
    pub rounding_min: u32,
    pub archived: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "code")]
pub enum KalaError {
    Parse { error: ParseError },
    ProjectNotFound { token: String },
    ProjectAmbiguous { token: String, candidates: Vec<String> },
    NoRunningTimer,
    DescriptionRequired,
    InvalidEntry { reason: String },
    ShortcutInvalid { accelerator: String },
    ShortcutTaken { accelerator: String },
    Db { message: String },
    Io { message: String },
}

impl std::fmt::Display for KalaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", serde_json::to_string(self).unwrap_or_default())
    }
}

impl std::error::Error for KalaError {}

impl From<ParseError> for KalaError {
    fn from(error: ParseError) -> Self {
        KalaError::Parse { error }
    }
}

impl From<rusqlite::Error> for KalaError {
    fn from(e: rusqlite::Error) -> Self {
        KalaError::Db { message: e.to_string() }
    }
}

impl From<std::io::Error> for KalaError {
    fn from(e: std::io::Error) -> Self {
        KalaError::Io { message: e.to_string() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Month {
    pub year: i32,
    pub month: u32,
}

impl Month {
    pub fn parse(s: &str) -> Option<Month> {
        let (y, m) = s.trim().split_once('-')?;
        let month = Month { year: y.parse().ok()?, month: m.parse().ok()? };
        ((1900..=9999).contains(&month.year) && (1..=12).contains(&month.month)).then_some(month)
    }

    pub fn of(d: NaiveDate) -> Month {
        Month { year: d.year(), month: d.month() }
    }

    pub fn first_day(&self) -> NaiveDate {
        NaiveDate::from_ymd_opt(self.year, self.month, 1).expect("valid month")
    }

    pub fn last_day(&self) -> NaiveDate {
        let (y, m) = if self.month == 12 { (self.year + 1, 1) } else { (self.year, self.month + 1) };
        NaiveDate::from_ymd_opt(y, m, 1).expect("valid month").pred_opt().expect("valid date")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn month_parse_rejects_out_of_range_values() {
        assert_eq!(Month::parse("2026-10"), Some(Month { year: 2026, month: 10 }));
        assert_eq!(Month::parse("300000-10"), None);
        assert_eq!(Month::parse("2026-13"), None);
        assert_eq!(Month::parse("-5-10"), None);
    }
}
