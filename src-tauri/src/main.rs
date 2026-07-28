#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod state;

use state::AppState;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WindowEvent,
};
use tracing_subscriber::EnvFilter;

fn main() {
    // Acquire this before initializing audio or creating a second tray icon.
    // If another process owns the mutex, restore its window and stop here.
    let Some(_single_instance) = acquire_single_instance_or_restore() else {
        return;
    };

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let engine = match audio_core::protection::shared_engine() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("No se pudo iniciar el motor de audio: {e}");
            std::process::exit(1);
        }
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState { engine })
        .invoke_handler(tauri::generate_handler![
            commands::list_app_groups,
            commands::list_devices,
            commands::get_status,
            commands::get_setup_status,
            commands::prepare_shared_audio,
            commands::get_config,
            commands::update_config,
            commands::activate_protection,
            commands::deactivate_protection,
            commands::refresh_routes,
            commands::copy_diagnostic_report,
        ])
        .setup(|app| {
            let show_i = MenuItem::with_id(app, "show", "Abrir", true, None::<&str>)?;
            let activate_i =
                MenuItem::with_id(app, "activate", "Activar protecciÃ³n", true, None::<&str>)?;
            let deactivate_i =
                MenuItem::with_id(app, "deactivate", "Desactivar protecciÃ³n", true, None::<&str>)?;
            let restore_i =
                MenuItem::with_id(app, "restore", "Restaurar audio normal", true, None::<&str>)?;
            let quit_i =
                MenuItem::with_id(app, "quit", "Salir y restaurar", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[&show_i, &activate_i, &deactivate_i, &restore_i, &quit_i],
            )?;

            let _tray = TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("NoEcho\nSin exclusiones")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "activate" => {
                        let state = app.state::<AppState>();
                        let apps = state.engine.config().excluded_apps;
                        let _ = state.engine.activate(Some(apps));
                        update_tray_tooltip(app);
                    }
                    "deactivate" | "restore" => {
                        let state = app.state::<AppState>();
                        let _ = state.engine.deactivate();
                        update_tray_tooltip(app);
                    }
                    "quit" => {
                        let state = app.state::<AppState>();
                        let _ = state.engine.deactivate();
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            update_tray_tooltip(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<AppState>();
                let close_to_tray = state.engine.config().close_to_tray;
                if close_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                } else {
                    let _ = state.engine.deactivate();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building NoEcho")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                let state = app_handle.state::<AppState>();
                let _ = state.engine.deactivate();
            }
        });
}

#[cfg(windows)]
struct SingleInstanceGuard {
    _file: std::fs::File,
}

#[cfg(windows)]
fn acquire_single_instance_or_restore() -> Option<SingleInstanceGuard> {
    use std::fs::OpenOptions;
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE,
    };

    let lock_dir = dirs::data_local_dir()?.join("NoEcho");
    if std::fs::create_dir_all(&lock_dir).is_err() {
        // Starting is preferable to failing completely if the profile folder
        // is temporarily unavailable.
        return None;
    }
    let lock_path = lock_dir.join("instance.lock");
    match OpenOptions::new()
        .create(true)
        .write(true)
        // Deny read/write/delete sharing while this File remains alive.
        .share_mode(0)
        .open(lock_path)
    {
        Ok(file) => Some(SingleInstanceGuard { _file: file }),
        Err(_) => {
            let title: Vec<u16> = "NoEcho"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let window = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
            if !window.is_null() {
                unsafe {
                    ShowWindow(window, SW_RESTORE);
                    SetForegroundWindow(window);
                }
            }
            None
        }
    }
}

#[cfg(not(windows))]
struct SingleInstanceGuard;

#[cfg(not(windows))]
fn acquire_single_instance_or_restore() -> Option<SingleInstanceGuard> {
    Some(SingleInstanceGuard)
}

fn update_tray_tooltip(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    let status = state.engine.status();
    let text = if status.active {
        format!(
            "ProtecciÃ³n activa\n{} aplicaciÃ³n{}",
            status.excluded_count,
            if status.excluded_count == 1 { "" } else { "es" }
        )
    } else {
        "NoEcho\nSin exclusiones".into()
    };
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_tooltip(Some(text));
    } else {
        // default tray may not use id "main"; ignore.
        let _ = text;
    }
}
