pub mod commands;
pub mod mcp;
#[cfg(target_os = "macos")]
pub mod panel;
pub mod shortcut;
pub mod tick;
pub mod tray;

use std::sync::Mutex;

use kala_core::store::Store;
use tauri::{Manager, RunEvent, WindowEvent};

pub struct AppState {
    pub store: Mutex<Store>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let store = Store::open(&Store::default_path()).expect("open Kala database");
    // Must be the first plugin: a second launch exits and the running Kala opens Settings instead.
    // The callback arrives on an async worker; AppKit windows must only be touched on the main thread.
    let builder = tauri::Builder::default().plugin(tauri_plugin_single_instance::init(|app, _, _| {
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || commands::open_settings(handle));
    }));
    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());
    builder
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .manage(AppState { store: Mutex::new(store) })
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            for label in ["popover", "quick"] {
                #[cfg(target_os = "macos")]
                panel::convert(app.handle(), label, true);
                #[cfg(not(target_os = "macos"))]
                if let Some(w) = app.get_webview_window(label) {
                    let handle = w.clone();
                    w.on_window_event(move |e| {
                        if let WindowEvent::Focused(false) = e {
                            if handle.is_visible().unwrap_or(false) {
                                tray::note_blur_hide();
                                let _ = handle.hide();
                            }
                        }
                    });
                }
            }
            #[cfg(target_os = "macos")]
            panel::convert(app.handle(), "settings", false);
            if let Some(w) = app.get_webview_window("settings") {
                let handle = w.clone();
                w.on_window_event(move |e| {
                    if let WindowEvent::CloseRequested { api, .. } = e {
                        api.prevent_close();
                        let _ = handle.hide();
                    }
                });
            }
            tray::create(app.handle())?;
            tick::spawn(app.handle().clone());
            let shortcut = app.state::<AppState>().store.lock().ok().and_then(|s| s.settings().ok()).and_then(|s| s.shortcut);
            match shortcut {
                Some(sc) => {
                    if let Err(e) = shortcut::apply(app.handle(), &sc, None) {
                        eprintln!("kala: shortcut {sc} not registered: {e}");
                    }
                }
                None => commands::open_settings(app.handle().clone()),
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::preview_text,
            commands::submit_text,
            commands::list_day,
            commands::update_entry,
            commands::delete_entry,
            commands::timer_status,
            commands::timer_start,
            commands::timer_stop,
            commands::list_projects,
            commands::save_project,
            commands::archive_project,
            commands::get_settings,
            commands::save_settings,
            commands::set_shortcut,
            commands::mcp_config_snippet,
            commands::shortcut_registered,
            commands::export_month,
            commands::reveal_path,
            commands::hide_window,
            commands::open_settings,
            commands::quit_app,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_, e| {
            if let RunEvent::ExitRequested { code: None, api, .. } = e {
                api.prevent_exit();
            }
        });
}
