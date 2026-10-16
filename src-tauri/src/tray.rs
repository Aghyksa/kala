use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::image::Image;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, LogicalPosition, Manager, Rect, WebviewWindow};

use crate::AppState;

const TRAY_ID: &str = "kala";

/// Blur hides the popover before the tray click arrives; this keeps that click from reopening it.
static LAST_BLUR_HIDE: Mutex<Option<Instant>> = Mutex::new(None);

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(include_bytes!("../icons/tray-template@2x.png"))?)
        .icon_as_template(true)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, rect, .. } = event {
                toggle_popover(tray.app_handle(), rect);
            }
        })
        .build(app)?;
    refresh_title(app);
    Ok(())
}

pub fn note_blur_hide() {
    *LAST_BLUR_HIDE.lock().unwrap() = Some(Instant::now());
}

pub fn toggle_popover(app: &AppHandle, tray_rect: Rect) {
    let Some(w) = app.get_webview_window("popover") else { return };
    if w.is_visible().unwrap_or(false) {
        let _ = w.hide();
        return;
    }
    if LAST_BLUR_HIDE.lock().unwrap().is_some_and(|t| t.elapsed() < Duration::from_millis(300)) {
        return;
    }
    place_under(&w, tray_rect);
    show_focused(app, &w);
    let _ = w.emit("kala://refresh", ());
}

pub fn show_focused(app: &AppHandle, w: &WebviewWindow) {
    #[cfg(target_os = "macos")]
    crate::panel::show_key(app, w.label());
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Works in logical points per monitor: tray rects arrive in that monitor's physical pixels,
/// and mixed-DPI setups (2x laptop + 1x external) break a single global scale.
fn place_under(w: &WebviewWindow, tray: Rect) {
    let pos = tray.position.to_physical::<f64>(1.0);
    let size = tray.size.to_physical::<f64>(1.0);
    let Ok(monitors) = w.available_monitors() else { return };
    let Some(m) = monitors.iter().find(|m| {
        let (mp, ms) = (m.position(), m.size());
        pos.x >= mp.x as f64 && pos.x < (mp.x + ms.width as i32) as f64 && pos.y >= mp.y as f64 && pos.y < (mp.y + ms.height as i32) as f64
    }) else {
        return;
    };
    let sf = m.scale_factor();
    let (left, right) = (m.position().x as f64 / sf, (m.position().x as f64 + m.size().width as f64) / sf);
    let width = w.outer_size().map(|s| s.width as f64 / w.scale_factor().unwrap_or(1.0)).unwrap_or(360.0);
    let x = ((pos.x + size.width / 2.0) / sf - width / 2.0).clamp(left + 8.0, right - width - 8.0);
    let y = (pos.y + size.height) / sf + 4.0;
    let _ = w.set_position(LogicalPosition::new(x, y));
}

pub fn set_timer_title(app: &AppHandle, running: Option<(&str, i64)>) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    let title = running.map(|(name, min)| format!("● {name} {}:{:02}", min / 60, min % 60));
    let _ = tray.set_title(title);
}

pub fn refresh_title(app: &AppHandle) {
    let running = {
        let state = app.state::<AppState>();
        let Ok(store) = state.store.lock() else { return };
        store.running().ok().flatten().and_then(|e| {
            let name = store.projects(true).ok()?.into_iter().find(|p| p.id == e.project_id)?.name;
            Some((name, (crate::commands::now() - e.start).num_minutes().max(0)))
        })
    };
    set_timer_title(app, running.as_ref().map(|(n, m)| (n.as_str(), *m)));
}
