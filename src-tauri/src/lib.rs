mod appbar;
mod config;
mod tray;

use config::{ConfigState, load_config};
use std::sync::Mutex;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_autostart::MacosLauncher;

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
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
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
            .title("Top Bar")
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

            // Handle autostart
            if config.auto_start {
                if let Some(autostart) =
                    app.try_state::<tauri_plugin_autostart::AutoLaunchManager>()
                {
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

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
