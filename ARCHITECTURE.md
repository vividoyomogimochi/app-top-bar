# Architecture

## Overview

LED AppBar is a Tauri v2 application that creates a desktop toolbar (AppBar) at the top of the screen. It uses WebView2 to render a web page and the Windows Shell AppBar API to reserve screen real estate.

```
┌─────────────────────────────────────────────────┐
│ Tauri v2 Application                            │
│                                                 │
│  ┌──────────────────────────────────────────┐   │
│  │ WebView2 (frameless, always-on-top)      │   │
│  │ Loads external URL (ticker page)         │   │
│  │ CSS injected via initialization_script   │   │
│  └──────────────────────────────────────────┘   │
│                                                 │
│  ┌──────────────────────────────────────────┐   │
│  │ Rust Backend                             │   │
│  │  appbar.rs  ← Win32 AppBar API           │   │
│  │  config.rs  ← JSON config persistence    │   │
│  │  tray.rs    ← System tray menu           │   │
│  │  lib.rs     ← App setup & orchestration  │   │
│  └──────────────────────────────────────────┘   │
│                                                 │
│  Plugins: autostart (registry), opener          │
└─────────────────────────────────────────────────┘
```

## Source Files

### `src-tauri/src/lib.rs` — Entry Point

Orchestrates the application lifecycle:

1. Loads config from disk
2. Creates the WebView window programmatically (not declaratively) to inject CSS via `initialization_script` that hides scrollbars and forces `overflow: hidden`
3. Sets up the system tray
4. Registers the AppBar via Win32 API
5. Syncs autostart state with the Windows registry
6. Hooks window close to unregister the AppBar

### `src-tauri/src/appbar.rs` — Windows AppBar API

Core module that interfaces with `SHAppBarNotify` from `Win32::UI::Shell`.

**AppBar lifecycle:**
```
ABM_NEW       → Register window as an AppBar
ABM_QUERYPOS  → Ask Windows for available space at the top edge
ABM_SETPOS    → Commit the position, reserving work area
ABM_REMOVE    → Unregister on exit (restores work area)
```

After `ABM_SETPOS`, the window is moved with `MoveWindow` and pinned with `SetWindowPos(HWND_TOPMOST)`.

**Monitor enumeration:**
- Uses `EnumDisplayMonitors` + `GetMonitorInfoW` to list all displays
- Sorted by left coordinate (then top) to match Windows Display Settings ordering
- Returns `(RECT, is_primary)` tuples

**Non-Windows:** Provides stub implementations that log warnings, allowing the crate to compile on Linux/macOS for development.

### `src-tauri/src/config.rs` — Configuration

JSON config stored at `%APPDATA%/app-top-bar/config.json`.

```rust
struct AppConfig {
    bar_height: u32,    // Default: 80
    monitor: u32,       // Default: 0 (primary)
    auto_start: bool,   // Default: true
    url: String,        // Default: "https://ticker.samoyed.moe/ticker/"
}
```

Config is loaded once at startup into `ConfigState(Mutex<AppConfig>)` managed by Tauri. Tray menu actions mutate this state and call `save_config()` to persist.

### `src-tauri/src/tray.rs` — System Tray

Builds the tray icon and context menu. Menu items are `CheckMenuItem`s with radio-button behavior.

**Key design decisions:**

- Menu items are created once and stored in `TrayMenuItems` (managed state). Check states are updated via `set_checked()` rather than rebuilding the menu, which avoids Tauri menu ID conflicts.
- All `ConfigState` mutex locks are scoped to drop before calling `update_check_states()`, preventing deadlocks since both the event handler and the update function need the config lock.
- Monitor/height changes that match the current value are skipped to avoid unnecessary AppBar re-registration.

## State Management

```
┌─────────────────────────────────────┐
│ Tauri Managed State                 │
│                                     │
│  ConfigState(Mutex<AppConfig>)      │  ← shared config
│  Mutex<TrayMenuItems>               │  ← menu item refs
│  AutoLaunchManager                  │  ← from plugin
└─────────────────────────────────────┘
```

All state access follows the pattern: lock → read/mutate → drop lock → side effects. This prevents deadlocks in the tray event handler.

## Build & Packaging

- **Tauri v2** with `tauri-plugin-autostart` (registry-based) and `tauri-plugin-opener`
- **windows-rs 0.61** for Win32 API bindings (`Shell`, `Gdi`, `WindowsAndMessaging`)
- **CI**: GitHub Actions builds MSI/NSIS installers on tag push; artifacts uploaded on manual dispatch
- **Cross-check**: `cargo check --target x86_64-pc-windows-msvc` works from WSL2 (requires `llvm-rc`)

## Known Limitations

- AppBar API only supports one appbar per screen edge per application. The Windows taskbar (itself an AppBar) at the bottom does not conflict with our top-edge registration, but another top-edge AppBar would.
- On monitors with a taskbar, the initial `ABM_QUERYPOS` may return a slightly offset position on first launch. Changing the bar height corrects it. Root cause is under investigation.
