mod appbar;
mod config;
mod tray;

use config::{ConfigState, load_config};
use std::sync::Mutex;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_autostart::MacosLauncher;

#[tauri::command]
fn get_url(state: tauri::State<'_, ConfigState>) -> String {
    state.0.lock().unwrap().url.clone()
}

#[tauri::command]
fn set_url(app: tauri::AppHandle, state: tauri::State<'_, ConfigState>, url: String) {
    {
        let mut cfg = state.0.lock().unwrap();
        cfg.url = url.clone();
        config::save_config(&cfg);
    }
    if let Some(window) = app.get_webview_window("main") {
        if let Ok(parsed) = url.parse::<url::Url>() {
            let _ = window.navigate(parsed);
        }
    }
}

#[tauri::command]
fn close_settings_window(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.close();
    }
}

const SCROLLBAR_HIDE_SCRIPT: &str = r#"
(function() {
    const style = document.createElement('style');
    style.textContent = `
        ::-webkit-scrollbar { display: none !important; }
        html, body {
            overflow: hidden !important;
            margin: 0 !important;
            padding: 0 !important;
        }
    `;
    (document.head || document.documentElement).appendChild(style);
})();
"#;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let cfg = load_config();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // Focus existing window when a second instance is launched
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.set_focus();
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(tauri::generate_handler![get_url, set_url, close_settings_window])
        .manage(ConfigState(Mutex::new(cfg)))
        .setup(|app| {
            let handle = app.handle().clone();

            let config = {
                let state = handle.state::<ConfigState>();
                let cfg = state.0.lock().unwrap().clone();
                cfg
            };

            // Create the window programmatically for initialization_script support
            let external_url: url::Url = config
                .url
                .parse()
                .expect("invalid ticker URL in config");

            let window = WebviewWindowBuilder::new(
                app,
                "main",
                WebviewUrl::External(external_url),
            )
            .initialization_script(SCROLLBAR_HIDE_SCRIPT)
            .title("LED AppBar")
            .inner_size(1920.0, config.bar_height as f64)
            .position(0.0, 0.0)
            .decorations(false)
            .resizable(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .shadow(false)
            .build()?;

            // Setup system tray
            tray::setup_tray(&handle)?;

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

            // Sync autostart registry with config
            if let Some(autostart) =
                app.try_state::<tauri_plugin_autostart::AutoLaunchManager>()
            {
                if config.auto_start {
                    let _ = autostart.enable();
                } else {
                    let _ = autostart.disable();
                }
            }

            // Handle window events
            let handle_clone = handle.clone();
            window.on_window_event(move |event| {
                match event {
                    // After appbar registration the work area shifts down, and Tauri's
                    // event loop re-applies position(0,0) relative to the new work area,
                    // pushing the window down by bar_height.  Catch the spurious move
                    // and snap the window back via Win32 API.
                    tauri::WindowEvent::Moved(_pos) => {
                        #[cfg(windows)]
                        {
                            if let Some(win) = handle_clone.get_webview_window("main") {
                                if let Ok(hwnd) = win.hwnd() {
                                    appbar::platform::correct_position(hwnd.0 as isize);
                                }
                            }
                        }
                    }
                    tauri::WindowEvent::CloseRequested { .. } => {
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
                    _ => {}
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
