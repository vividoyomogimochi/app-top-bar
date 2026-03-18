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
        GetWindowLongW, MoveWindow, SetWindowLongW, SetWindowPos, GWL_EXSTYLE, GWL_STYLE,
        HWND_TOPMOST, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
        WINDOW_EX_STYLE, WINDOW_STYLE, WS_CAPTION, WS_EX_CLIENTEDGE, WS_EX_DLGMODALFRAME,
        WS_EX_WINDOWEDGE, WS_POPUP, WS_THICKFRAME,
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

    /// Register the window as an appbar at the top of the specified monitor.
    /// Returns the final physical-pixel rect (x, y, width, height) on success.
    pub fn register_appbar(
        hwnd: isize,
        bar_height: u32,
        monitor_index: u32,
    ) -> Option<(i32, i32, i32, i32)> {
        let monitors = enumerate_monitors();
        let monitor_rect = match monitors.get(monitor_index as usize) {
            Some((rect, _)) => *rect,
            None => match monitors.first() {
                Some((rect, _)) => *rect,
                None => return None,
            },
        };

        let hwnd = HWND(hwnd as *mut _);

        unsafe {
            // Strip frame styles that WRY leaves on even with decorations(false).
            // WS_THICKFRAME / WS_CAPTION cause an invisible border that offsets
            // the window position on first show.
            let style = WINDOW_STYLE(GetWindowLongW(hwnd, GWL_STYLE) as u32);
            let new_style = (style & !(WS_THICKFRAME | WS_CAPTION)) | WS_POPUP;
            SetWindowLongW(hwnd, GWL_STYLE, new_style.0 as i32);

            let ex_style = WINDOW_EX_STYLE(GetWindowLongW(hwnd, GWL_EXSTYLE) as u32);
            let new_ex_style =
                ex_style & !(WS_EX_CLIENTEDGE | WS_EX_WINDOWEDGE | WS_EX_DLGMODALFRAME);
            SetWindowLongW(hwnd, GWL_EXSTYLE, new_ex_style.0 as i32);

            // Apply style changes before positioning
            let _ = SetWindowPos(
                hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );

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
                return None;
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

            let x = abd.rc.left;
            let y = abd.rc.top;
            let w = abd.rc.right - abd.rc.left;
            let h = abd.rc.bottom - abd.rc.top;

            // Move the actual window to match
            let _ = MoveWindow(hwnd, x, y, w, h, true);

            // Ensure topmost
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                x,
                y,
                w,
                h,
                SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );

            Some((x, y, w, h))
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
    pub fn enumerate_monitors() -> Vec<((i32, i32, i32, i32), bool)> {
        vec![((0, 0, 1920, 1080), true)]
    }

    pub fn register_appbar(
        _hwnd: isize,
        _bar_height: u32,
        _monitor_index: u32,
    ) -> Option<(i32, i32, i32, i32)> {
        log::warn!("AppBar API is only available on Windows");
        None
    }

    pub fn unregister_appbar(_hwnd: isize) {}
}
