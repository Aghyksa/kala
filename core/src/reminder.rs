use chrono::{Datelike, NaiveDateTime};

use crate::store::Settings;

/// `>=` (not `==`) so a Mac that wakes after the reminder time still gets today's reminder once.
pub fn reminder_due(s: &Settings, now: NaiveDateTime) -> bool {
    s.reminder_enabled
        && s.work_days.contains(&now.weekday().number_from_monday())
        && now.time() >= s.reminder_time
        && s.last_reminder_date != Some(now.date())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::{d, dt};

    #[test]
    fn reminder_due_rules() {
        let s = Settings::default();
        assert!(!reminder_due(&s, dt(16, 16, 59)));
        assert!(reminder_due(&s, dt(16, 17, 0)));
        assert!(reminder_due(&s, dt(16, 21, 0)));
        let sent = Settings { last_reminder_date: Some(d(2026, 10, 16)), ..Settings::default() };
        assert!(!reminder_due(&sent, dt(16, 17, 30)));
        assert!(!reminder_due(&s, dt(17, 17, 30)));
        let off = Settings { reminder_enabled: false, ..Settings::default() };
        assert!(!reminder_due(&off, dt(16, 17, 30)));
    }
}
