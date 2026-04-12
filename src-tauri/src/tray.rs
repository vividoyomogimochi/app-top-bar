use std::sync::Mutex;
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, Submenu},
    tray::TrayIconBuilder,
    AppHandle, Manager, WebviewUrl, WebviewWindowBuilder,
};

use crate::appbar::platform;
use crate::config::{self, ConfigState};
use crate::server;

const HEIGHTS: &[u32] = &[40, 60, 80, 100, 120];

/// Stored references to tray menu items for updating check states.
pub struct TrayMenuItems {
    monitor_items: Vec<CheckMenuItem<tauri::Wry>>,
    height_items: Vec<CheckMenuItem<tauri::Wry>>,
    autostart_item: CheckMenuItem<tauri::Wry>,
    auto_hide_item: CheckMenuItem<tauri::Wry>,
}

/// Update check marks on monitor and height items to reflect current config.
fn update_check_states(app: &AppHandle) {
    let config = app.state::<ConfigState>().0.lock().unwrap().clone();
    let items = app.state::<Mutex<TrayMenuItems>>();
    let items = items.lock().unwrap();

    for (i, item) in items.monitor_items.iter().enumerate() {
        let _ = item.set_checked(i as u32 == config.monitor);
    }
    for (i, item) in items.height_items.iter().enumerate() {
        let _ = item.set_checked(HEIGHTS[i] == config.bar_height);
    }
    let _ = items.autostart_item.set_checked(config.auto_start);
    let _ = items.auto_hide_item.set_checked(config.auto_hide_fullscreen);
}

/// Open or focus a settings dialog window.
fn open_settings_window(app: &AppHandle, label: &str, html: &str, title: &str) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.set_focus();
        return;
    }
    let main_window = app.get_webview_window("main");
    let mut builder = WebviewWindowBuilder::new(
        app,
        label,
        WebviewUrl::App(html.into()),
    )
    .title(title)
    .inner_size(460.0, 300.0)
    .resizable(false)
    .minimizable(false)
    .maximizable(false)
    .center();

    if let Some(ref parent) = main_window {
        builder = builder.parent(parent).expect("failed to set parent window");
    }

    let _ = builder.build();
}

pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let config = app.state::<ConfigState>().0.lock().unwrap().clone();

    // Monitor submenu
    let monitors = platform::enumerate_monitors();
    let monitor_sub = Submenu::with_id(app, "monitors", "Monitor", true)?;
    let mut monitor_items = Vec::new();
    for i in 0..monitors.len() {
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
        monitor_items.push(item);
    }

    // Height submenu
    let height_sub = Submenu::with_id(app, "heights", "Bar Height", true)?;
    let mut height_items = Vec::new();
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
        height_items.push(item);
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

    // Auto-hide on fullscreen toggle
    let auto_hide_item = CheckMenuItem::with_id(
        app,
        "auto_hide_fullscreen",
        "Auto-hide on Fullscreen",
        true,
        config.auto_hide_fullscreen,
        None::<&str>,
    )?;

    let settings_item = MenuItem::with_id(app, "settings", "Settings...", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[&monitor_sub, &height_sub, &autostart_item, &auto_hide_item, &settings_item, &quit_item],
    )?;

    // Store item references for later check-state updates
    // Sync auto-hide setting to appbar module
    platform::set_auto_hide(config.auto_hide_fullscreen);

    app.manage(Mutex::new(TrayMenuItems {
        monitor_items,
        height_items,
        autostart_item,
        auto_hide_item,
    }));

    TrayIconBuilder::with_id("main-tray")
        .icon(tauri::include_image!("icons/32x32.png"))
        .menu(&menu)
        .tooltip("LED AppBar")
        .on_menu_event(move |app, event| {
            let id = event.id().as_ref();

            if id == "quit" {
                server::stop(app);
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

            if id == "settings" {
                open_settings_window(app, "settings", "settings.html", "Settings");
                return;
            }

            if id == "autostart" {
                let new_state = {
                    let state = app.state::<ConfigState>();
                    let mut cfg = state.0.lock().unwrap();
                    cfg.auto_start = !cfg.auto_start;
                    config::save_config(app, &cfg);
                    cfg.auto_start
                };
                // Sync registry
                if let Some(autostart) =
                    app.try_state::<tauri_plugin_autostart::AutoLaunchManager>()
                {
                    if new_state {
                        let _ = autostart.enable();
                    } else {
                        let _ = autostart.disable();
                    }
                }
                update_check_states(app);
                return;
            }

            if id == "auto_hide_fullscreen" {
                let new_state = {
                    let state = app.state::<ConfigState>();
                    let mut cfg = state.0.lock().unwrap();
                    cfg.auto_hide_fullscreen = !cfg.auto_hide_fullscreen;
                    config::save_config(app, &cfg);
                    cfg.auto_hide_fullscreen
                };
                platform::set_auto_hide(new_state);
                update_check_states(app);
                return;
            }

            if let Some(idx_str) = id.strip_prefix("monitor_") {
                if let Ok(idx) = idx_str.parse::<u32>() {
                    let should_register = {
                        let state = app.state::<ConfigState>();
                        let mut cfg = state.0.lock().unwrap();

                        if cfg.monitor == idx {
                            false
                        } else {
                            cfg.monitor = idx;
                            config::save_config(app, &cfg);
                            true
                        }
                    }; // lock dropped

                    if should_register {
                        #[cfg(windows)]
                        {
                            let state = app.state::<ConfigState>();
                            let cfg = state.0.lock().unwrap();
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
                    }

                    update_check_states(app);
                }
                return;
            }

            if let Some(h_str) = id.strip_prefix("height_") {
                if let Ok(h) = h_str.parse::<u32>() {
                    let should_register = {
                        let state = app.state::<ConfigState>();
                        let mut cfg = state.0.lock().unwrap();

                        if cfg.bar_height == h {
                            false
                        } else {
                            cfg.bar_height = h;
                            config::save_config(app, &cfg);
                            true
                        }
                    }; // lock dropped

                    if should_register {
                        #[cfg(windows)]
                        {
                            let state = app.state::<ConfigState>();
                            let cfg = state.0.lock().unwrap();
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
                    }

                    update_check_states(app);
                }
            }
        })
        .build(app)?;

    Ok(())
}
