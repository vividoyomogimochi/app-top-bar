use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, Submenu},
    tray::TrayIconBuilder,
    AppHandle, Manager,
};

use crate::appbar::platform;
use crate::config::{self, AppConfig, ConfigState};

const TRAY_ID: &str = "main-tray";
const HEIGHTS: &[u32] = &[40, 60, 80, 100, 120];

/// Build the tray menu with correct check states based on current config.
fn build_menu(app: &AppHandle, config: &AppConfig) -> tauri::Result<Menu<tauri::Wry>> {
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

    let height_sub = Submenu::with_id(app, "heights", "Bar Height", true)?;
    for &h in HEIGHTS {
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

    let autostart_item = CheckMenuItem::with_id(
        app,
        "autostart",
        "Auto Start",
        true,
        config.auto_start,
        None::<&str>,
    )?;

    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    Menu::with_items(
        app,
        &[&monitor_sub, &height_sub, &autostart_item, &quit_item],
    )
}

/// Rebuild the tray menu to reflect updated check states.
fn rebuild_tray_menu(app: &AppHandle) {
    let config = {
        let state = app.state::<ConfigState>();
        let cfg = state.0.lock().unwrap().clone();
        cfg
    };
    if let Ok(menu) = build_menu(app, &config) {
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let config = {
        let state = app.state::<ConfigState>();
        let cfg = state.0.lock().unwrap().clone();
        cfg
    };

    let menu = build_menu(app, &config)?;

    TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip("Top Bar")
        .on_menu_event(move |app, event| {
            let id = event.id().as_ref();

            if id == "quit" {
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
                rebuild_tray_menu(app);
                return;
            }

            if let Some(idx_str) = id.strip_prefix("monitor_") {
                if let Ok(idx) = idx_str.parse::<u32>() {
                    let state = app.state::<ConfigState>();
                    let mut cfg = state.0.lock().unwrap();

                    // Skip if same monitor already selected
                    if cfg.monitor == idx {
                        rebuild_tray_menu(app);
                        return;
                    }

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

                    rebuild_tray_menu(app);
                }
                return;
            }

            if let Some(h_str) = id.strip_prefix("height_") {
                if let Ok(h) = h_str.parse::<u32>() {
                    let state = app.state::<ConfigState>();
                    let mut cfg = state.0.lock().unwrap();

                    // Skip if same height already selected
                    if cfg.bar_height == h {
                        rebuild_tray_menu(app);
                        return;
                    }

                    cfg.bar_height = h;
                    config::save_config(&cfg);

                    #[cfg(windows)]
                    if let Some(window) = app.get_webview_window("main") {
                        if let Ok(hwnd) = window.hwnd() {
                            platform::register_appbar(hwnd.0 as isize, h, cfg.monitor);
                        }
                    }

                    rebuild_tray_menu(app);
                }
            }
        })
        .build(app)?;

    Ok(())
}
