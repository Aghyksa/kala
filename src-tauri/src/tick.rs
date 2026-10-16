use std::collections::BTreeMap;
use std::time::Duration;

use chrono::{Local, Timelike};
use kala_core::i18n::{Lang, duration_label, label};
use kala_core::reminder::reminder_due;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::{AppState, commands::now, tray};

pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let wait = 60 - Local::now().second() as u64;
            tokio::time::sleep(Duration::from_secs(wait.max(1))).await;
            tray::refresh_title(&app);
            if let Some((title, body)) = due_reminder(&app) {
                let _ = app.notification().builder().title(title).body(body).show();
            }
        }
    });
}

pub fn pretty_shortcut(acc: &str) -> String {
    acc.split('+')
        .map(|p| match p {
            "Command" | "Cmd" | "Super" => "⌘",
            "Control" | "Ctrl" => "⌃",
            "Alt" | "Option" => "⌥",
            "Shift" => "⇧",
            other => other,
        })
        .collect()
}

/// Builds the reminder text and marks today as reminded, in one lock.
fn due_reminder(app: &AppHandle) -> Option<(String, String)> {
    let state = app.state::<AppState>();
    let store = state.store.lock().ok()?;
    let mut settings = store.settings().ok()?;
    let now = now();
    if !reminder_due(&settings, now) {
        return None;
    }
    let lang = Lang::from_code(&settings.language);
    let projects = store.projects(true).ok()?;
    let name = |id: i64| projects.iter().find(|p| p.id == id).map(|p| p.name.clone()).unwrap_or_default();
    let mut per_project: BTreeMap<String, i64> = BTreeMap::new();
    for e in store.entries_between(now.date(), now.date(), None).ok()? {
        *per_project.entry(name(e.project_id)).or_default() += e.end.map(|end| (end - e.start).num_minutes()).unwrap_or(0);
    }
    let mut lines = vec![if per_project.is_empty() {
        label(lang, "reminder_empty").to_string()
    } else {
        let parts: Vec<String> = per_project.iter().map(|(p, m)| format!("{p} {}", duration_label(lang, *m))).collect();
        format!("{}: {}", label(lang, "reminder_today"), parts.join(" · "))
    }];
    if let Ok(Some(run)) = store.running() {
        lines.push(label(lang, "reminder_running").replace("{project}", &name(run.project_id)));
    }
    if let Some(sc) = &settings.shortcut {
        lines.push(label(lang, "reminder_hint").replace("{shortcut}", &pretty_shortcut(sc)));
    }
    settings.last_reminder_date = Some(now.date());
    store.save_settings(&settings).ok()?;
    Some((label(lang, "reminder_title").to_string(), lines.join("\n")))
}
