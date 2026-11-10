//! Select the monitor containing the foreground Windows application, before NEXUS
//! takes focus. If the OS cannot identify it, the caller uses the primary monitor.

#[cfg(windows)]
pub fn center() -> Option<(f64, f64)> {
    use std::ffi::c_void;

    #[repr(C)]
    #[derive(Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    struct MonitorInfo {
        size: u32,
        monitor: Rect,
        work: Rect,
        flags: u32,
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetForegroundWindow() -> *mut c_void;
        fn MonitorFromWindow(window: *mut c_void, flags: u32) -> *mut c_void;
        fn GetMonitorInfoW(monitor: *mut c_void, info: *mut MonitorInfo) -> i32;
    }

    // SAFETY: these handles are borrowed from Windows and never dereferenced or
    // freed here. info is a correctly sized, writable Win32 MONITORINFO structure.
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground.is_null() {
            return None;
        }
        let monitor = MonitorFromWindow(foreground, 0); // MONITOR_DEFAULTTONULL
        if monitor.is_null() {
            return None;
        }
        let mut info = MonitorInfo {
            size: std::mem::size_of::<MonitorInfo>() as u32,
            monitor: Rect::default(),
            work: Rect::default(),
            flags: 0,
        };
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            return None;
        }
        Some((
            (f64::from(info.monitor.left) + f64::from(info.monitor.right)) / 2.0,
            (f64::from(info.monitor.top) + f64::from(info.monitor.bottom)) / 2.0,
        ))
    }
}

#[cfg(not(windows))]
pub fn center() -> Option<(f64, f64)> {
    None
}
