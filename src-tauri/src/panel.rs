use tauri::{AppHandle, Manager};
use tauri_nspanel::{CollectionBehavior, ManagerExt, PanelLevel, StyleMask, WebviewWindowExt, tauri_panel};

tauri_panel! {
    panel!(KalaPanel {
        config: {
            can_become_key_window: true,
            is_floating_panel: true
        }
    })

    panel_event!(KalaPanelEvents {
        window_did_resign_key(notification: &NSNotification) -> (),
    })
}

/// Non-activating panel: takes keyboard input without stealing activation from the frontmost app
/// (macOS 14+ refuses activateIgnoringOtherApps for menu bar apps).
pub fn convert(app: &AppHandle, label: &'static str, hide_on_blur: bool) {
    let Some(w) = app.get_webview_window(label) else { return };
    let Ok(panel) = w.to_panel::<KalaPanel>() else { return };
    let level = if hide_on_blur { PanelLevel::PopUpMenu } else { PanelLevel::Floating };
    panel.set_level(level.into());
    let _ = panel.add_style_mask(StyleMask::empty().nonactivating_panel().into());
    panel.set_collection_behavior(CollectionBehavior::new().can_join_all_spaces().full_screen_auxiliary().into());
    if !hide_on_blur {
        return;
    }
    let handler = KalaPanelEvents::new();
    let handle = app.clone();
    handler.window_did_resign_key(move |_| {
        if let Ok(p) = handle.get_webview_panel(label) {
            if p.is_visible() {
                crate::tray::note_blur_hide();
                p.hide();
            }
        }
    });
    panel.set_event_handler(Some(handler.as_ref()));
}

pub fn show_key(app: &AppHandle, label: &str) {
    if let Ok(p) = app.get_webview_panel(label) {
        p.show_and_make_key();
    }
}
