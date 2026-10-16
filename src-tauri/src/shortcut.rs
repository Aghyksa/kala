use kala_core::KalaError;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// Shift alone would hijack plain typing (Shift+K is a capital K in every app).
pub fn has_primary_modifier(accelerator: &str) -> bool {
    let parts: Vec<String> = accelerator.split('+').map(|p| p.trim().to_lowercase()).collect();
    let Some((_, mods)) = parts.split_last() else { return false };
    mods.iter().any(|m| {
        matches!(
            m.as_str(),
            "command" | "cmd" | "super" | "commandorcontrol" | "cmdorctrl" | "control" | "ctrl" | "alt" | "option"
        )
    })
}

/// Registers the new shortcut before releasing the old one, so a failed change keeps the old one working.
pub fn apply(app: &AppHandle, new: &str, old: Option<&str>) -> Result<(), KalaError> {
    if !has_primary_modifier(new) {
        return Err(KalaError::ShortcutInvalid { accelerator: new.into() });
    }
    let shortcut: Shortcut = new.parse().map_err(|_| KalaError::ShortcutInvalid { accelerator: new.into() })?;
    let old = old.and_then(|o| o.parse::<Shortcut>().ok());
    if old == Some(shortcut) {
        return Ok(());
    }
    let gs = app.global_shortcut();
    gs.on_shortcut(shortcut, |app, _, event| {
        if event.state == ShortcutState::Pressed {
            show_quick(app);
        }
    })
    .map_err(|_| KalaError::ShortcutTaken { accelerator: new.into() })?;
    if let Some(o) = old {
        let _ = gs.unregister(o);
    }
    Ok(())
}

pub fn show_quick(app: &AppHandle) {
    let Some(w) = app.get_webview_window("quick") else { return };
    if w.is_visible().unwrap_or(false) {
        let _ = w.hide();
        return;
    }
    let _ = w.center();
    crate::tray::show_focused(app, &w);
    let _ = w.emit("kala://refresh", ());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_need_command_control_or_option() {
        assert!(has_primary_modifier("Control+Alt+K"));
        assert!(has_primary_modifier("Command+Shift+K"));
        assert!(has_primary_modifier("Alt+Space"));
        assert!(!has_primary_modifier("Shift+K"));
        assert!(!has_primary_modifier("K"));
    }
}
