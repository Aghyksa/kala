use chrono::{Datelike, NaiveDate, NaiveTime, Weekday};
use serde::{Deserialize, Serialize};

use crate::{KalaError, Project};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum TimeSpec {
    None,
    Duration(u32),
    Range(NaiveTime, NaiveTime),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Parsed {
    pub date: NaiveDate,
    pub date_explicit: bool,
    pub project: String,
    pub time: TimeSpec,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "code")]
pub enum ParseError {
    Empty,
    MissingProject,
    BadTime { token: String },
    RangeEndBeforeStart,
    ZeroDuration,
    TooLong,
    MissingDescription,
}

pub fn parse(text: &str, today: NaiveDate) -> Result<Parsed, ParseError> {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    if tokens.is_empty() {
        return Err(ParseError::Empty);
    }
    let mut i = 0;
    let mut date = today;
    let mut date_explicit = false;
    if let Some(d) = parse_date(tokens[0], today) {
        date = d;
        date_explicit = true;
        i = 1;
    }
    let project = match tokens.get(i) {
        Some(t) if time_token(t).is_none() => t.to_string(),
        _ => return Err(ParseError::MissingProject),
    };
    i += 1;
    let (time, used) = take_time(&tokens[i..])?;
    i += used;
    let description = tokens[i..].join(" ");
    if time != TimeSpec::None && description.is_empty() {
        return Err(ParseError::MissingDescription);
    }
    Ok(Parsed { date, date_explicit, project, time, description })
}

pub fn resolve_project<'a>(token: &str, projects: &'a [Project], include_archived: bool) -> Result<&'a Project, KalaError> {
    let needle = token.to_lowercase();
    let active = || projects.iter().filter(|p| include_archived || !p.archived);
    let names = |p: &'a Project| std::iter::once(&p.name).chain(&p.aliases).map(|n| n.to_lowercase());
    let exact: Vec<&Project> = active().filter(|p| names(p).any(|n| n == needle)).collect();
    let found = if exact.is_empty() {
        active().filter(|p| names(p).any(|n| n.starts_with(&needle))).collect()
    } else {
        exact
    };
    match found.as_slice() {
        [p] => Ok(*p),
        [] => Err(KalaError::ProjectNotFound { token: token.into() }),
        many => Err(KalaError::ProjectAmbiguous {
            token: token.into(),
            candidates: many.iter().map(|p| p.name.clone()).collect(),
        }),
    }
}

fn parse_date(token: &str, today: NaiveDate) -> Option<NaiveDate> {
    let t = token.to_lowercase();
    if t == "kemarin" || t == "yesterday" {
        return today.pred_opt();
    }
    if let Some(wd) = weekday(&t) {
        let back = (today.weekday().num_days_from_monday() + 7 - wd.num_days_from_monday()) % 7;
        return today.checked_sub_days(chrono::Days::new(back as u64));
    }
    if t.len() == 10 {
        if let Ok(d) = NaiveDate::parse_from_str(&t, "%Y-%m-%d") {
            return Some(d);
        }
    }
    let parts: Vec<&str> = t.split('/').collect();
    let num = |x: &str| x.parse::<u32>().ok();
    match parts[..] {
        [dd, mm, yy] if yy.len() == 2 || yy.len() == 4 => {
            let y: i32 = yy.parse().ok()?;
            NaiveDate::from_ymd_opt(if yy.len() == 2 { 2000 + y } else { y }, num(mm)?, num(dd)?)
        }
        // Timesheets look back: a day/month later than today means last year.
        [dd, mm] => {
            let d = NaiveDate::from_ymd_opt(today.year(), num(mm)?, num(dd)?)?;
            if d > today { d.with_year(today.year() - 1) } else { Some(d) }
        }
        _ => None,
    }
}

fn weekday(t: &str) -> Option<Weekday> {
    Some(match t {
        "senin" | "mon" | "monday" => Weekday::Mon,
        "selasa" | "tue" | "tues" | "tuesday" => Weekday::Tue,
        "rabu" | "wed" | "wednesday" => Weekday::Wed,
        "kamis" | "thu" | "thur" | "thurs" | "thursday" => Weekday::Thu,
        "jumat" | "jum'at" | "fri" | "friday" => Weekday::Fri,
        "sabtu" | "sat" | "saturday" => Weekday::Sat,
        "minggu" | "sun" | "sunday" => Weekday::Sun,
        _ => return None,
    })
}

/// Reads the time phrase after the project: "2j", "1h 30m", "1.5 h", "13-15", "13 - 15", "13–15".
/// Returns how many tokens it used. A digit-led token that isn't a valid time is an error,
/// so typos don't silently become a timer description.
fn take_time(tokens: &[&str]) -> Result<(TimeSpec, usize), ParseError> {
    let ts: Vec<String> = tokens.iter().map(|t| t.replace(['\u{2013}', '\u{2014}'], "-")).collect();
    if ts.len() >= 3 && ts[1] == "-" && clock_shape(&ts[0]).is_some() && clock_shape(&ts[2]).is_some() {
        let spec = time_token(&format!("{}-{}", ts[0], ts[2])).expect("clock shapes form a range")?;
        return Ok((spec, 3));
    }
    let (mut minutes, mut used) = (0u32, 0usize);
    loop {
        if let (Some(n), Some(u)) = (ts.get(used), ts.get(used + 1)) {
            if is_number(n) && duration_minutes(&format!("1{u}")).is_some() {
                minutes += duration_minutes(&format!("{n}{u}")).ok_or_else(|| ParseError::BadTime { token: format!("{n} {u}") })?;
                used += 2;
                continue;
            }
        }
        match ts.get(used).map(|t| time_token(t)) {
            Some(Some(Ok(TimeSpec::Duration(m)))) => {
                minutes += m;
                used += 1;
            }
            Some(Some(Ok(range))) if used == 0 => return Ok((range, 1)),
            Some(Some(Err(ParseError::ZeroDuration))) if used == 0 => return Err(ParseError::ZeroDuration),
            Some(Some(Err(e))) if used == 0 => return Err(e),
            _ => break,
        }
    }
    if used > 0 {
        return match minutes {
            0 => Err(ParseError::ZeroDuration),
            m if m > MAX_MINUTES => Err(ParseError::TooLong),
            m => Ok((TimeSpec::Duration(m), used)),
        };
    }
    match ts.first() {
        Some(t) if t.starts_with(|c: char| c.is_ascii_digit()) && t.contains(|c: char| c.is_ascii_alphabetic() || "-:.".contains(c)) => {
            Err(ParseError::BadTime { token: tokens[0].into() })
        }
        _ => Ok((TimeSpec::None, 0)),
    }
}

/// One entry covers at most a day; "30j" is a typo, not a shift.
const MAX_MINUTES: u32 = 24 * 60;

fn is_number(s: &str) -> bool {
    !s.is_empty() && s.replace(',', ".").parse::<f64>().is_ok()
}

/// None = not shaped like a time token; Some(Err) = shaped like one but invalid.
fn time_token(token: &str) -> Option<Result<TimeSpec, ParseError>> {
    let t = token.to_lowercase();
    if let Some((a, b)) = t.split_once('-') {
        let (sa, sb) = (clock_shape(a)?, clock_shape(b)?);
        let bad = || ParseError::BadTime { token: token.into() };
        let start = clock(sa).ok_or_else(bad);
        // "24" ends at midnight: stored as 00:00, the store puts it on the next day.
        let ends_at_midnight = sb == (24, 0);
        let end = if ends_at_midnight { Ok(NaiveTime::MIN) } else { clock(sb).ok_or_else(bad) };
        return Some(match (start, end) {
            (Ok(s), Ok(e)) if e <= s && !ends_at_midnight => Err(ParseError::RangeEndBeforeStart),
            (Ok(s), Ok(e)) => Ok(TimeSpec::Range(s, e)),
            (Err(e), _) | (_, Err(e)) => Err(e),
        });
    }
    let minutes = duration_minutes(&t)?;
    Some(if minutes == 0 { Err(ParseError::ZeroDuration) } else { Ok(TimeSpec::Duration(minutes)) })
}

fn clock_shape(s: &str) -> Option<(u32, u32)> {
    let (h, m) = match s.split_once(['.', ':']) {
        Some((h, m)) if m.len() == 2 => (h, m),
        Some(_) => return None,
        None => (s, "00"),
    };
    let digits = |x: &str| !x.is_empty() && x.len() <= 2 && x.bytes().all(|b| b.is_ascii_digit());
    (digits(h) && digits(m)).then(|| (h.parse().unwrap(), m.parse().unwrap()))
}

fn clock((h, m): (u32, u32)) -> Option<NaiveTime> {
    NaiveTime::from_hms_opt(h, m, 0)
}

fn duration_minutes(t: &str) -> Option<u32> {
    let mut rest = t;
    let mut total = 0.0;
    while !rest.is_empty() {
        let num_len = rest.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == ',')).unwrap_or(rest.len());
        let unit_len = rest[num_len..].find(|c: char| !c.is_ascii_alphabetic()).unwrap_or(rest.len() - num_len);
        let value: f64 = rest[..num_len].replace(',', ".").parse().ok()?;
        total += match &rest[num_len..num_len + unit_len] {
            "j" | "jam" | "h" | "hr" | "hrs" | "hour" | "hours" => value * 60.0,
            "m" | "mnt" | "min" | "mins" | "menit" | "minute" | "minutes" => value,
            _ => return None,
        };
        rest = &rest[num_len + unit_len..];
    }
    Some(total.round() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn hm(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    fn proj(id: i64, name: &str, aliases: &[&str]) -> Project {
        Project {
            id,
            name: name.into(),
            aliases: aliases.iter().map(|a| a.to_string()).collect(),
            color: "#000000".into(),
            hourly_rate: 0.0,
            currency: "IDR".into(),
            rounding_min: 60,
            archived: false,
        }
    }

    #[test]
    fn spec_examples() {
        let t = d(2026, 10, 16);
        let p = parse("acme 2j fix role", t).unwrap();
        assert_eq!(
            (p.project.as_str(), p.time, p.description.as_str(), p.date, p.date_explicit),
            ("acme", TimeSpec::Duration(120), "fix role", t, false)
        );
        assert_eq!(parse("globex 13-15 meeting sprint", t).unwrap().time, TimeSpec::Range(hm(13, 0), hm(15, 0)));
        assert_eq!(parse("kemarin acme 1j review", t).unwrap().date, d(2026, 10, 15));
        let timer = parse("acme benerin tema", t).unwrap();
        assert_eq!((timer.time, timer.description.as_str()), (TimeSpec::None, "benerin tema"));
    }

    #[test]
    fn durations() {
        for (s, m) in [
            ("2j", 120),
            ("2h", 120),
            ("2jam", 120),
            ("1,5j", 90),
            ("1.5h", 90),
            ("30m", 30),
            ("30mnt", 30),
            ("1j30m", 90),
            ("90m", 90),
        ] {
            assert_eq!(parse(&format!("acme {s} x"), d(2026, 10, 16)).unwrap().time, TimeSpec::Duration(m), "{s}");
        }
    }

    #[test]
    fn ranges() {
        for (s, a, b) in [
            ("13.30-15", hm(13, 30), hm(15, 0)),
            ("13:30-15:00", hm(13, 30), hm(15, 0)),
            ("9-11.30", hm(9, 0), hm(11, 30)),
        ] {
            assert_eq!(parse(&format!("acme {s} x"), d(2026, 10, 16)).unwrap().time, TimeSpec::Range(a, b), "{s}");
        }
    }

    #[test]
    fn dates() {
        let t = d(2026, 10, 16);
        for (s, want) in [
            ("senin", d(2026, 10, 12)),
            ("mon", d(2026, 10, 12)),
            ("kamis", d(2026, 10, 15)),
            ("jumat", t),
            ("yesterday", d(2026, 10, 15)),
            ("16/10", t),
            ("2026-10-01", d(2026, 10, 1)),
        ] {
            let p = parse(&format!("{s} acme 1j x"), t).unwrap();
            assert_eq!((p.date, p.date_explicit), (want, true), "{s}");
        }
    }

    #[test]
    fn errors_and_edge_inputs() {
        let t = d(2026, 10, 16);
        assert_eq!(parse("   ", t), Err(ParseError::Empty));
        assert_eq!(parse("2j fix", t), Err(ParseError::MissingProject));
        assert_eq!(parse("acme 15-13 x", t), Err(ParseError::RangeEndBeforeStart));
        assert!(matches!(parse("acme 25-26 x", t), Err(ParseError::BadTime { .. })));
        assert_eq!(parse("acme 0j x", t), Err(ParseError::ZeroDuration));
        assert_eq!(parse("acme 2j", t), Err(ParseError::MissingDescription));
        let p = parse("acme 2 bug", t).unwrap();
        assert_eq!((p.time, p.description.as_str()), (TimeSpec::None, "2 bug"));
    }

    #[test]
    fn two_digit_years_and_past_day_month() {
        assert_eq!(parse("16/10/26 acme 1j x", d(2026, 10, 16)).unwrap().date, d(2026, 10, 16));
        assert_eq!(parse("16/10/2026 acme 1j x", d(2026, 10, 16)).unwrap().date, d(2026, 10, 16));
        assert_eq!(parse("28/12 acme 1j x", d(2027, 1, 2)).unwrap().date, d(2026, 12, 28));
    }

    #[test]
    fn spaced_and_combined_time_tokens() {
        let t = d(2026, 10, 16);
        let p = parse("acme 1h 30m fix", t).unwrap();
        assert_eq!((p.time, p.description.as_str()), (TimeSpec::Duration(90), "fix"));
        for (s, m) in [("2hrs", 120), ("1 hour", 60), ("45 mins", 45), ("1.5 h", 90), ("2 jam", 120)] {
            assert_eq!(parse(&format!("acme {s} x"), t).unwrap().time, TimeSpec::Duration(m), "{s}");
        }
        for s in ["13 - 15", "13–15", "13 – 15"] {
            assert_eq!(parse(&format!("acme {s} x"), t).unwrap().time, TimeSpec::Range(hm(13, 0), hm(15, 0)), "{s}");
        }
        assert!(matches!(parse("acme 2jj x", t), Err(ParseError::BadTime { .. })));
        assert_eq!(parse("acme 2 bug", t).unwrap().time, TimeSpec::None);
    }

    #[test]
    fn durations_over_a_day_are_rejected() {
        let t = d(2026, 10, 16);
        assert_eq!(parse("acme 30j x", t), Err(ParseError::TooLong));
        assert_eq!(parse("acme 24j x", t).unwrap().time, TimeSpec::Duration(1440));
    }

    #[test]
    fn range_can_end_at_midnight() {
        let t = d(2026, 10, 16);
        assert_eq!(parse("acme 23-24 x", t).unwrap().time, TimeSpec::Range(hm(23, 0), hm(0, 0)));
        assert_eq!(parse("acme 22.30-24.00 x", t).unwrap().time, TimeSpec::Range(hm(22, 30), hm(0, 0)));
        assert_eq!(parse("acme 23-00 x", t), Err(ParseError::RangeEndBeforeStart));
    }

    #[test]
    fn archived_projects_resolve_only_when_asked() {
        let mut old = proj(4, "Legacy", &[]);
        old.archived = true;
        let ps = vec![proj(1, "Acme", &[]), old];
        assert!(matches!(resolve_project("legacy", &ps, false), Err(KalaError::ProjectNotFound { .. })));
        assert_eq!(resolve_project("legacy", &ps, true).unwrap().id, 4);
    }

    #[test]
    fn resolve() {
        let ps = vec![proj(1, "Acme", &["portal"]), proj(2, "Globex", &[]), proj(3, "AcmeX", &[])];
        assert_eq!(resolve_project("acme", &ps, false).unwrap().id, 1);
        assert_eq!(resolve_project("PORTAL", &ps, false).unwrap().id, 1);
        assert_eq!(resolve_project("g", &ps, false).unwrap().id, 2);
        assert!(matches!(resolve_project("x", &ps, false), Err(KalaError::ProjectNotFound { .. })));
        assert!(matches!(resolve_project("ac", &ps, false), Err(KalaError::ProjectAmbiguous { candidates, .. }) if candidates.len() == 2));
    }
}
