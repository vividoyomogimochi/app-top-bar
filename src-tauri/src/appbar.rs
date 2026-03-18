/// AppBar API wrapper for Windows.
/// Registers a window as an application desktop toolbar (appbar) using SHAppBarNotify.
/// This reserves screen space so other maximized windows won't overlap.
#[cfg(windows)]
pub mod platform {
    use windows::Win32::Foundation::{HWND, LPARAM, RECT};
    use windows::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
    };
    use windows::Win32::UI::Shell::{
        SHAppBarMessage, ABM_NEW, ABM_QUERYPOS, ABM_REMOVE, ABM_SETPOS, APPBARDATA,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClientRect, GetWindowLongW, GetWindowRect, MoveWindow, SetWindowPos, GWL_EXSTYLE,
        GWL_STYLE, HWND_TOPMOST, SWP_FRAMECHANGED, SWP_NOACTIVATE,
    };

    use std::fmt::Write as FmtWrite;
    use std::fs;
    use std::mem;
    use std::sync::atomic::{AtomicBool, Ordering};

    static REGISTERED: AtomicBool = AtomicBool::new(false);

    fn dump_diag(hwnd: HWND, label: &str, diag: &mut String) {
        unsafe {
            let mut wr: RECT = mem::zeroed();
            let _ = GetWindowRect(hwnd, &mut wr);
            let mut cr: RECT = mem::zeroed();
            let _ = GetClientRect(hwnd, &mut cr);
            let style = GetWindowLongW(hwnd, GWL_STYLE);
            let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
            let _ = writeln!(
                diag,
                "[{label}] WindowRect=({},{},{},{}) ClientRect=({},{},{},{}) style=0x{:08X} ex_style=0x{:08X}",
                wr.left, wr.top, wr.right, wr.bottom,
                cr.left, cr.top, cr.right, cr.bottom,
                style, ex_style
            );
        }
    }

    /// Get monitor info sorted by position (left-to-right, then top-to-bottom)
    /// to match Windows Display Settings ordering.
    /// Returns Vec of (monitor_rect, work_rect, is_primary).
    pub fn enumerate_monitors() -> Vec<(RECT, RECT, bool)> {
        let monitors: std::sync::Mutex<Vec<(RECT, RECT, bool)>> =
            std::sync::Mutex::new(Vec::new());

        unsafe {
            let monitors_ptr = &monitors as *const _ as isize;
            let _ = EnumDisplayMonitors(
                None,
                None,
                Some(monitor_enum_proc),
                LPARAM(monitors_ptr),
            );
        }

        let mut result = monitors.into_inner().unwrap_or_default();

        // Sort by left coordinate, then top — matches Windows display settings order
        result.sort_by(|a, b| {
            let a_rect = &a.0;
            let b_rect = &b.0;
            a_rect.left.cmp(&b_rect.left).then(a_rect.top.cmp(&b_rect.top))
        });

        result
    }

    unsafe extern "system" fn monitor_enum_proc(
        hmonitor: HMONITOR,
        _hdc: HDC,
        _lprect: *mut RECT,
        lparam: LPARAM,
    ) -> windows::core::BOOL {
        let monitors =
            &*(lparam.0 as *const std::sync::Mutex<Vec<(RECT, RECT, bool)>>);
        let mut info: MONITORINFO = mem::zeroed();
        info.cbSize = mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(hmonitor, &mut info).as_bool() {
            let is_primary = (info.dwFlags & 1) != 0; // MONITORINFOF_PRIMARY
            if let Ok(mut m) = monitors.lock() {
                m.push((info.rcMonitor, info.rcWork, is_primary));
            }
        }
        windows::core::BOOL(1)
    }

    /// Register the window as an appbar at the top of the specified monitor.
    pub fn register_appbar(hwnd: isize, bar_height: u32, monitor_index: u32) -> bool {
        let mut diag = String::new();

        let monitors = enumerate_monitors();
        for (i, (mon, work, primary)) in monitors.iter().enumerate() {
            let _ = writeln!(
                diag,
                "Monitor[{i}]: rcMonitor=({},{},{},{}) rcWork=({},{},{},{}) primary={}",
                mon.left, mon.top, mon.right, mon.bottom,
                work.left, work.top, work.right, work.bottom,
                primary
            );
        }

        let monitor_rect = match monitors.get(monitor_index as usize) {
            Some((rect, _, _)) => *rect,
            None => match monitors.first() {
                Some((rect, _, _)) => *rect,
                None => return false,
            },
        };

        let _ = writeln!(
            diag,
            "Target: monitor_index={} bar_height={} monitor_rect=({},{},{},{})",
            monitor_index, bar_height,
            monitor_rect.left, monitor_rect.top, monitor_rect.right, monitor_rect.bottom
        );

        let hwnd = HWND(hwnd as *mut _);

        dump_diag(hwnd, "before_register", &mut diag);

        unsafe {
            // Remove previous registration if any
            if REGISTERED.load(Ordering::SeqCst) {
                unregister_appbar(hwnd.0 as isize);
            }

            let mut abd: APPBARDATA = mem::zeroed();
            abd.cbSize = mem::size_of::<APPBARDATA>() as u32;
            abd.hWnd = hwnd;

            // Register new appbar
            let result = SHAppBarMessage(ABM_NEW, &mut abd);
            if result == 0 {
                log::error!("ABM_NEW failed");
                let _ = writeln!(diag, "ABM_NEW FAILED");
                write_diag(&diag);
                return false;
            }
            REGISTERED.store(true, Ordering::SeqCst);
            let _ = writeln!(diag, "ABM_NEW ok");

            // Query and set position
            abd.uEdge = 1; // ABE_TOP
            abd.rc = RECT {
                left: monitor_rect.left,
                top: monitor_rect.top,
                right: monitor_rect.right,
                bottom: monitor_rect.top + bar_height as i32,
            };

            let _ = writeln!(
                diag,
                "Before QUERYPOS: rc=({},{},{},{})",
                abd.rc.left, abd.rc.top, abd.rc.right, abd.rc.bottom
            );

            SHAppBarMessage(ABM_QUERYPOS, &mut abd);

            let _ = writeln!(
                diag,
                "After QUERYPOS: rc=({},{},{},{})",
                abd.rc.left, abd.rc.top, abd.rc.right, abd.rc.bottom
            );

            abd.rc.bottom = abd.rc.top + bar_height as i32;
            SHAppBarMessage(ABM_SETPOS, &mut abd);

            let _ = writeln!(
                diag,
                "After SETPOS: rc=({},{},{},{})",
                abd.rc.left, abd.rc.top, abd.rc.right, abd.rc.bottom
            );

            // Move the actual window to match
            let _ = MoveWindow(
                hwnd,
                abd.rc.left,
                abd.rc.top,
                abd.rc.right - abd.rc.left,
                abd.rc.bottom - abd.rc.top,
                true,
            );

            dump_diag(hwnd, "after_MoveWindow", &mut diag);

            // Ensure topmost
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                abd.rc.left,
                abd.rc.top,
                abd.rc.right - abd.rc.left,
                abd.rc.bottom - abd.rc.top,
                SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );

            dump_diag(hwnd, "after_SetWindowPos", &mut diag);
        }

        write_diag(&diag);

        // Delayed re-check: something may move the window after setup() returns
        let hwnd_raw = hwnd.0 as isize;
        std::thread::spawn(move || {
            for delay_ms in [500, 2000] {
                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                let hwnd = HWND(hwnd_raw as *mut _);
                let mut delayed = String::new();
                dump_diag(hwnd, &format!("delayed_{}ms", delay_ms), &mut delayed);
                append_diag(&delayed);
            }
        });

        true
    }

    fn write_diag(diag: &str) {
        if let Some(dir) = dirs::config_dir() {
            let path = dir.join("app-top-bar").join("diag.log");
            let _ = fs::write(path, diag);
        }
    }

    fn append_diag(diag: &str) {
        if let Some(dir) = dirs::config_dir() {
            let path = dir.join("app-top-bar").join("diag.log");
            use std::io::Write;
            if let Ok(mut f) = fs::OpenOptions::new().append(true).open(&path) {
                let _ = f.write_all(diag.as_bytes());
            }
        }
    }

    /// Unregister the appbar, releasing the reserved screen space.
    pub fn unregister_appbar(hwnd: isize) {
        if !REGISTERED.load(Ordering::SeqCst) {
            return;
        }

        let hwnd = HWND(hwnd as *mut _);
        unsafe {
            let mut abd: APPBARDATA = mem::zeroed();
            abd.cbSize = mem::size_of::<APPBARDATA>() as u32;
            abd.hWnd = hwnd;
            SHAppBarMessage(ABM_REMOVE, &mut abd);
        }
        REGISTERED.store(false, Ordering::SeqCst);
    }
}

#[cfg(not(windows))]
pub mod platform {
    /// Stub for non-Windows platforms.
    pub fn enumerate_monitors() -> Vec<((i32, i32, i32, i32), (i32, i32, i32, i32), bool)> {
        vec![((0, 0, 1920, 1080), (0, 0, 1920, 1040), true)]
    }

    pub fn register_appbar(_hwnd: isize, _bar_height: u32, _monitor_index: u32) -> bool {
        log::warn!("AppBar API is only available on Windows");
        false
    }

    pub fn unregister_appbar(_hwnd: isize) {}
}
