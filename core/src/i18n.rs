use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Lang {
    Id,
    #[default]
    En,
}

impl Lang {
    pub fn from_code(code: &str) -> Lang {
        if code.to_lowercase().starts_with("id") { Lang::Id } else { Lang::En }
    }
}

pub fn label(lang: Lang, key: &str) -> &'static str {
    match (lang, key) {
        (_, "col_no") => "No",
        (Lang::Id, "col_date") => "Tanggal",
        (Lang::En, "col_date") => "Date",
        (Lang::Id, "col_start") => "Mulai",
        (Lang::En, "col_start") => "Start",
        (Lang::Id, "col_end") => "Selesai",
        (Lang::En, "col_end") => "End",
        (Lang::Id, "col_desc") => "Deskripsi",
        (Lang::En, "col_desc") => "Description",
        (Lang::Id, "col_hours") => "Jam",
        (Lang::En, "col_hours") => "Hours",
        (_, "col_price") => "Price",
        (_, "total") => "Total",
        (_, "timesheet") => "Timesheet",
        (Lang::Id, "reminder_title") => "Kala: cek jam hari ini",
        (Lang::En, "reminder_title") => "Kala: check today's hours",
        (Lang::Id, "reminder_running") => "Timer masih jalan: {project}",
        (Lang::En, "reminder_running") => "Timer still running: {project}",
        (Lang::Id, "reminder_hint") => "Tekan {shortcut} buat nyatet",
        (Lang::En, "reminder_hint") => "Press {shortcut} to log",
        (Lang::Id, "reminder_empty") => "Belum ada jam yang dicatat",
        (Lang::En, "reminder_empty") => "No hours logged yet",
        (Lang::Id, "reminder_today") => "Hari ini",
        (Lang::En, "reminder_today") => "Today",
        _ => "",
    }
}

pub fn month_name(lang: Lang, month: u32) -> &'static str {
    const ID: [&str; 12] = ["Januari", "Februari", "Maret", "April", "Mei", "Juni", "Juli", "Agustus", "September", "Oktober", "November", "Desember"];
    const EN: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    let names = if lang == Lang::Id { ID } else { EN };
    names.get(month.saturating_sub(1) as usize).copied().unwrap_or("")
}

pub fn duration_label(lang: Lang, minutes: i64) -> String {
    let (h, m) = (minutes / 60, minutes % 60);
    let hu = if lang == Lang::Id { "j" } else { "h" };
    match (h, m) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}{hu}"),
        (h, m) => format!("{h}{hu} {m}m"),
    }
}
