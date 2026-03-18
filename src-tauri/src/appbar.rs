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
        MoveWindow, SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE,
    };

    use std::mem;
    use std::sync::atomic::{AtomicBool, Ordering};

    static REGISTERED: AtomicBool = AtomicBool::new(false);

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

    /// Query and set appbar position. Called twice on first registration
    /// to work around the initial offset when a taskbar is present.
    unsafe fn set_appbar_pos(abd: &mut APPBARDATA, monitor_rect: &RECT, bar_height: u32) {
        abd.uEdge = 1; // ABE_TOP
        abd.rc = RECT {
            left: monitor_rect.left,
            top: monitor_rect.top,
            right: monitor_rect.right,
            bottom: monitor_rect.top + bar_height as i32,
        };

        SHAppBarMessage(ABM_QUERYPOS, abd);

        // Force position to the monitor's absolute top edge.
        // QUERYPOS may shift rc.top down on monitors with a taskbar,
        // but we always want to sit at the very top.
        abd.rc.left = monitor_rect.left;
        abd.rc.top = monitor_rect.top;
        abd.rc.right = monitor_rect.right;
        abd.rc.bottom = monitor_rect.top + bar_height as i32;

        SHAppBarMessage(ABM_SETPOS, abd);
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

            // First register with 1px height, then resize to actual height.
            // On monitors with a taskbar, the initial work area calculation
            // can cause an offset; a size change forces Windows to recalculate.
            set_appbar_pos(&mut abd, &monitor_rect, 1);
            set_appbar_pos(&mut abd, &monitor_rect, bar_height);

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
                SWP_NOACTIVATE,
            );
        }

        true
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
    pub fn enumerate_monitors() -> Vec<((i32, i32, i32, i32), bool)> {
        vec![((0, 0, 1920, 1080), true)]
    }

    pub fn register_appbar(_hwnd: isize, _bar_height: u32, _monitor_index: u32) -> bool {
        log::warn!("AppBar API is only available on Windows");
        false
    }

    pub fn unregister_appbar(_hwnd: isize) {}
}
