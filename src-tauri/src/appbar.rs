/// AppBar API wrapper for Windows.
/// Registers a window as an application desktop toolbar (appbar) using SHAppBarNotify.
/// This reserves screen space so other maximized windows won't overlap.
#[cfg(windows)]
pub mod platform {
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
    };
    use windows::Win32::UI::Shell::{
        SHAppBarMessage, ABM_NEW, ABM_QUERYPOS, ABM_REMOVE, ABM_SETPOS, APPBARDATA,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CallWindowProcW, GetWindowLongPtrW, GetWindowRect, MoveWindow, SetWindowLongPtrW,
        SetWindowPos, ShowWindow, HWND_TOPMOST, SWP_FRAMECHANGED, SWP_NOACTIVATE, SW_HIDE,
        SW_SHOWNOACTIVATE, WNDPROC, GWL_WNDPROC,
    };

    use std::mem;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;

    /// Custom message ID for appbar notifications (WM_USER + 100).
    const WM_APPBAR_CALLBACK: u32 = 0x0464;
    /// ABN_FULLSCREENAPP notification code.
    const ABN_FULLSCREENAPP: u32 = 2;

    static REGISTERED: AtomicBool = AtomicBool::new(false);
    /// Whether the window is currently hidden due to a fullscreen app.
    static HIDDEN_FOR_FULLSCREEN: AtomicBool = AtomicBool::new(false);
    /// Whether auto-hide on fullscreen is enabled.
    static AUTO_HIDE_ENABLED: AtomicBool = AtomicBool::new(true);
    /// Original window procedure before subclassing.
    static ORIGINAL_WNDPROC: Mutex<isize> = Mutex::new(0);
    /// The appbar window handle, stored for use by set_auto_hide.
    static APPBAR_HWND: Mutex<isize> = Mutex::new(0);

    /// The expected physical-pixel rect for the appbar window.
    /// Set during registration, used by correct_position to snap back.
    static EXPECTED_RECT: Mutex<Option<RECT>> = Mutex::new(None);

    /// Window procedure that intercepts appbar callback messages.
    unsafe extern "system" fn appbar_wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if msg == WM_APPBAR_CALLBACK && wparam.0 as u32 == ABN_FULLSCREENAPP {
            if AUTO_HIDE_ENABLED.load(Ordering::SeqCst) {
                let entering_fullscreen = lparam.0 != 0;
                if entering_fullscreen {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                    HIDDEN_FOR_FULLSCREEN.store(true, Ordering::SeqCst);
                } else if HIDDEN_FOR_FULLSCREEN.load(Ordering::SeqCst) {
                    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                    HIDDEN_FOR_FULLSCREEN.store(false, Ordering::SeqCst);
                    // Re-ensure topmost position
                    correct_position(hwnd.0 as isize);
                }
            }
            return LRESULT(0);
        }

        let original = ORIGINAL_WNDPROC.lock().ok().map(|w| *w).unwrap_or(0);
        if original != 0 {
            let proc: WNDPROC = mem::transmute(original);
            return CallWindowProcW(proc, hwnd, msg, wparam, lparam);
        }
        windows::Win32::UI::WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam)
    }

    /// Set whether auto-hide on fullscreen is enabled.
    pub fn set_auto_hide(enabled: bool) {
        AUTO_HIDE_ENABLED.store(enabled, Ordering::SeqCst);
        // If disabling and currently hidden, show the window back
        if !enabled && HIDDEN_FOR_FULLSCREEN.load(Ordering::SeqCst) {
            HIDDEN_FOR_FULLSCREEN.store(false, Ordering::SeqCst);
            let hwnd_val = APPBAR_HWND.lock().ok().map(|h| *h).unwrap_or(0);
            if hwnd_val != 0 {
                let hwnd = HWND(hwnd_val as *mut _);
                unsafe {
                    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                }
                correct_position(hwnd_val);
            }
        }
    }

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
            abd.uCallbackMessage = WM_APPBAR_CALLBACK;

            // Register new appbar
            let result = SHAppBarMessage(ABM_NEW, &mut abd);
            if result == 0 {
                log::error!("ABM_NEW failed");
                return false;
            }
            REGISTERED.store(true, Ordering::SeqCst);

            // Store hwnd for set_auto_hide
            if let Ok(mut h) = APPBAR_HWND.lock() {
                *h = hwnd.0 as isize;
            }

            // Subclass the window to receive appbar notifications
            {
                let mut orig = ORIGINAL_WNDPROC.lock().unwrap();
                if *orig == 0 {
                    let prev = GetWindowLongPtrW(hwnd, GWL_WNDPROC);
                    *orig = prev;
                    SetWindowLongPtrW(hwnd, GWL_WNDPROC, appbar_wndproc as *const () as isize);
                }
            }

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
            // Restore original window procedure
            {
                let mut orig = ORIGINAL_WNDPROC.lock().unwrap();
                if *orig != 0 {
                    SetWindowLongPtrW(hwnd, GWL_WNDPROC, *orig);
                    *orig = 0;
                }
            }

            let mut abd: APPBARDATA = mem::zeroed();
            abd.cbSize = mem::size_of::<APPBARDATA>() as u32;
            abd.hWnd = hwnd;
            SHAppBarMessage(ABM_REMOVE, &mut abd);
        }
        REGISTERED.store(false, Ordering::SeqCst);
        HIDDEN_FOR_FULLSCREEN.store(false, Ordering::SeqCst);

        if let Ok(mut expected) = EXPECTED_RECT.lock() {
            *expected = None;
        }
        if let Ok(mut h) = APPBAR_HWND.lock() {
            *h = 0;
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

    pub fn set_auto_hide(_enabled: bool) {}
}
