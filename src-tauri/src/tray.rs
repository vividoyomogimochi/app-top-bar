use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, Submenu},
    tray::TrayIconBuilder,
    AppHandle, Manager,
};

use crate::appbar::platform;
use crate::config::{self, ConfigState};

pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let config = {
        let state = app.state::<ConfigState>();
        let cfg = state.0.lock().unwrap().clone();
        cfg
    };

    // Monitor submenu
    #[cfg(windows)]
    let monitors = platform::enumerate_monitors();
    #[cfg(not(windows))]
    let monitors = platform::enumerate_monitors();

    let monitor_count = monitors.len();

    let monitor_sub = Submenu::with_id(app, "monitors", "Monitor", true)?;
    for i in 0..monitor_count {
        let label = if i == 0 {
            format!("Monitor {} (Primary)", i + 1)
        } else {
            format!("Monitor {}", i + 1)
        };
        let item = CheckMenuItem::with_id(
            app,
            format!("monitor_{}", i),
            label,
            true,
            i as u32 == config.monitor,
            None::<&str>,
        )?;
        monitor_sub.append(&item)?;
    }

    // Height submenu
    let heights: &[u32] = &[40, 60, 80, 100, 120];
    let height_sub = Submenu::with_id(app, "heights", "Bar Height", true)?;
    for &h in heights {
        let item = CheckMenuItem::with_id(
            app,
            format!("height_{}", h),
            format!("{}px", h),
            true,
            config.bar_height == h,
            None::<&str>,
        )?;
        height_sub.append(&item)?;
    }

    // Auto-start toggle
    let autostart_item = CheckMenuItem::with_id(
        app,
        "autostart",
        "Auto Start",
        true,
        config.auto_start,
        None::<&str>,
    )?;

    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[&monitor_sub, &height_sub, &autostart_item, &quit_item],
    )?;

    TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("Top Bar")
        .on_menu_event(move |app, event| {
            let id = event.id().as_ref();

            if id == "quit" {
                // Unregister appbar before quitting
                if let Some(window) = app.get_webview_window("main") {
                    #[cfg(windows)]
                    {
                        if let Ok(hwnd) = window.hwnd() {
                            platform::unregister_appbar(hwnd.0 as isize);
                        }
                    }
                    let _ = window;
                }
                app.exit(0);
                return;
            }

            if id == "autostart" {
                let state = app.state::<ConfigState>();
                let mut cfg = state.0.lock().unwrap();
                cfg.auto_start = !cfg.auto_start;
                config::save_config(&cfg);
                return;
            }

            if let Some(idx_str) = id.strip_prefix("monitor_") {
                if let Ok(idx) = idx_str.parse::<u32>() {
                    let state = app.state::<ConfigState>();
                    let mut cfg = state.0.lock().unwrap();
                    cfg.monitor = idx;
                    config::save_config(&cfg);

                    #[cfg(windows)]
                    if let Some(window) = app.get_webview_window("main") {
                        if let Ok(hwnd) = window.hwnd() {
                            platform::register_appbar(
                                hwnd.0 as isize,
                                cfg.bar_height,
                                cfg.monitor,
                            );
                        }
                    }
                }
                return;
            }

            if let Some(h_str) = id.strip_prefix("height_") {
                if let Ok(h) = h_str.parse::<u32>() {
                    let state = app.state::<ConfigState>();
                    let mut cfg = state.0.lock().unwrap();
                    cfg.bar_height = h;
                    config::save_config(&cfg);

                    #[cfg(windows)]
                    if let Some(window) = app.get_webview_window("main") {
                        if let Ok(hwnd) = window.hwnd() {
                            platform::register_appbar(hwnd.0 as isize, h, cfg.monitor);
                        }
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}
