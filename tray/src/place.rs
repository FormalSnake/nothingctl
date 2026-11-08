//! Where the flyout opens: the corner of the work area nearest the taskbar, on the monitor under
//! the pointer, as Windows' own flyouts do. The tray icon's rectangle is not exposed, and the
//! pointer is on it when it was clicked.

#[cfg(windows)]
pub fn origin(width: f32, height: f32) -> Option<nd::core::Point> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint};
    use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    const MARGIN: f32 = 12.;

    unsafe {
        let mut cursor = POINT::default();
        GetCursorPos(&mut cursor).ok()?;
        let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
        GetMonitorInfoW(monitor, &mut info).ok().ok()?;
        let (mut dpi_x, mut dpi_y) = (96, 96);
        GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y).ok()?;
        let scale = dpi_x as f32 / 96.;
        let work = info.rcWork;
        let full = info.rcMonitor;
        // The taskbar sits on the edge where the work area stops short of the monitor.
        let x = if work.left > full.left {
            work.left as f32 / scale + MARGIN
        } else {
            work.right as f32 / scale - width - MARGIN
        };
        let y = if work.top > full.top {
            work.top as f32 / scale + MARGIN
        } else {
            work.bottom as f32 / scale - height - MARGIN
        };
        Some(nd::core::Point { x, y })
    }
}

#[cfg(not(windows))]
pub fn origin(_width: f32, _height: f32) -> Option<nd::core::Point> {
    None
}
