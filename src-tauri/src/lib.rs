mod appbar;
mod config;
mod tray;

use config::{ConfigState, load_config};
use std::sync::Mutex;
use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let cfg = load_config();

    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(ConfigState(Mutex::new(cfg)))
        .setup(|app| {
            let handle = app.handle().clone();

            // Setup system tray
            tray::setup_tray(&handle)?;

            // Register appbar after window is created
            let config = {
                let state = handle.state::<ConfigState>();
                let cfg = state.0.lock().unwrap().clone();
                cfg
            };

            if let Some(window) = app.get_webview_window("main") {
                // On Windows, register the appbar
                #[cfg(windows)]
                {
                    if let Ok(hwnd) = window.hwnd() {
                        appbar::platform::register_appbar(
                            hwnd.0 as isize,
                            config.bar_height,
                            config.monitor,
                        );
                    }
                }

                // Handle autostart
                if config.auto_start {
                    if let Some(autostart) = app.try_state::<tauri_plugin_autostart::AutoLaunchManager>() {
                        let _ = autostart.enable();
                    }
                }

                // Unregister appbar on window close
                let handle_clone = handle.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { .. } = event {
                        if let Some(win) = handle_clone.get_webview_window("main") {
                            #[cfg(windows)]
                            {
                                if let Ok(hwnd) = win.hwnd() {
                                    appbar::platform::unregister_appbar(hwnd.0 as isize);
                                }
                            }
                            let _ = win;
                        }
                    }
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
