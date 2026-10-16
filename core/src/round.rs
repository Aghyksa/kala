use chrono::{Duration, NaiveDateTime, Timelike};

/// Start floors to the unit; duration rounds to nearest unit (half up, min one unit).
pub fn auto_span(start: NaiveDateTime, worked_min: i64, unit: u32) -> (NaiveDateTime, NaiveDateTime) {
    if unit == 0 {
        return (start, start + Duration::minutes(worked_min));
    }
    let unit = unit as i64;
    let of_day = (start.hour() * 60 + start.minute()) as i64;
    let start = start.with_second(0).unwrap() - Duration::minutes(of_day % unit);
    let units = ((worked_min + unit / 2) / unit).max(1);
    (start, start + Duration::minutes(units * unit))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn dt(day: u32, h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, day).unwrap().and_hms_opt(h, m, 0).unwrap()
    }

    #[test]
    fn spec_rounding() {
        assert_eq!(auto_span(dt(16, 13, 35), 50, 60), (dt(16, 13, 0), dt(16, 14, 0)));
        assert_eq!(auto_span(dt(16, 14, 25), 75, 60), (dt(16, 14, 0), dt(16, 15, 0)));
        assert_eq!(auto_span(dt(16, 13, 40), 120, 60), (dt(16, 13, 0), dt(16, 15, 0)));
        assert_eq!(auto_span(dt(16, 9, 0), 90, 60), (dt(16, 9, 0), dt(16, 11, 0)));
        assert_eq!(auto_span(dt(16, 9, 7), 7, 15), (dt(16, 9, 0), dt(16, 9, 15)));
        assert_eq!(auto_span(dt(16, 9, 7), 50, 0), (dt(16, 9, 7), dt(16, 9, 57)));
    }
}
