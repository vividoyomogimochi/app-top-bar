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
        GetWindowRect, MoveWindow, SetWindowPos, HWND_TOPMOST, SWP_FRAMECHANGED, SWP_NOACTIVATE,
    };

    use std::mem;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;

    static REGISTERED: AtomicBool = AtomicBool::new(false);

    /// The expected physical-pixel rect for the appbar window.
    /// Set during registration, used by correct_position to snap back.
    static EXPECTED_RECT: Mutex<Option<RECT>> = Mutex::new(None);

    /// Get monitor info sorted by position (left-to-right, then top-to-bottom)
    /// to match Windows Display Settings ordering.
    /// Returns Vec of (monitor_rect, is_primary).
    pub fn enumerate_monitors() -> Vec<(RECT, bool)> {
        let monitors: std::sync::Mutex<Vec<(RECT, bool)>> = std::sync::Mutex::new(Vec::new());

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
            &*(lparam.0 as *const std::sync::Mutex<Vec<(RECT, bool)>>);
        let mut info: MONITORINFO = mem::zeroed();
        info.cbSize = mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(hmonitor, &mut info).as_bool() {
            let is_primary = (info.dwFlags & 1) != 0; // MONITORINFOF_PRIMARY
            if let Ok(mut m) = monitors.lock() {
                m.push((info.rcMonitor, is_primary));
            }
        }
        windows::core::BOOL(1)
    }

    /// Register the window as an appbar at the top of the specified monitor.
    pub fn register_appbar(hwnd: isize, bar_height: u32, monitor_index: u32) -> bool {
        let monitors = enumerate_monitors();
        let monitor_rect = match monitors.get(monitor_index as usize) {
            Some((rect, _)) => *rect,
            None => match monitors.first() {
                Some((rect, _)) => *rect,
                None => return false,
            },
        };

        let hwnd = HWND(hwnd as *mut _);

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
                return false;
            }
            REGISTERED.store(true, Ordering::SeqCst);

            // Query and set position
            abd.uEdge = 1; // ABE_TOP
            abd.rc = RECT {
                left: monitor_rect.left,
                top: monitor_rect.top,
                right: monitor_rect.right,
                bottom: monitor_rect.top + bar_height as i32,
            };

            SHAppBarMessage(ABM_QUERYPOS, &mut abd);
            abd.rc.bottom = abd.rc.top + bar_height as i32;
            SHAppBarMessage(ABM_SETPOS, &mut abd);

            // Store the expected rect for correct_position
            if let Ok(mut expected) = EXPECTED_RECT.lock() {
                *expected = Some(abd.rc);
            }

            // Move the actual window to match
            let _ = MoveWindow(
                hwnd,
                abd.rc.left,
                abd.rc.top,
                abd.rc.right - abd.rc.left,
                abd.rc.bottom - abd.rc.top,
                true,
            );

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
        }

        true
    }

    /// If the window has drifted from the expected appbar rect, snap it back.
    /// Called from the Moved window event to counteract Tauri re-applying
    /// work-area-relative coordinates after the appbar shifts the work area.
    pub fn correct_position(hwnd: isize) {
        let expected = match EXPECTED_RECT.lock().ok().and_then(|e| *e) {
            Some(r) => r,
            None => return,
        };

        let hwnd = HWND(hwnd as *mut _);
        unsafe {
            let mut current: RECT = mem::zeroed();
            if GetWindowRect(hwnd, &mut current).is_err() {
                return;
            }

            if current.left == expected.left
                && current.top == expected.top
                && current.right == expected.right
                && current.bottom == expected.bottom
            {
                return; // already correct
            }

            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                expected.left,
                expected.top,
                expected.right - expected.left,
                expected.bottom - expected.top,
                SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
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

        if let Ok(mut expected) = EXPECTED_RECT.lock() {
            *expected = None;
        }
    }
}

#[cfg(not(windows))]
pub mod platform {
    /// Stub for non-Windows platforms.
    pub fn enumerate_monitors() -> Vec<((i32, i32, i32, i32), bool)> {
        vec![((0, 0, 1920, 1080), true)]
    }

    pub fn register_appbar(_hwnd: isize, _bar_height: u32, _monitor_index: u32) -> bool {
        log::warn!("AppBar API is only available on Windows");
        false
    }

    pub fn correct_position(_hwnd: isize) {}

    pub fn unregister_appbar(_hwnd: isize) {}
}
