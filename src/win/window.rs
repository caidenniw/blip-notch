//! Layered, topmost, non-activating island window.
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTOPRIMARY};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::*;

pub const WIN_W: f64 = 640.0;
pub const WIN_H: f64 = 460.0;
pub const CLASS: PCWSTR = w!("BlipIsland");

pub type Proc = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

/// Primary monitor rectangle (physical px) and its DPI scale.
pub fn primary_monitor(hwnd: Option<HWND>) -> (i32, i32, i32, i32, f64) {
    unsafe {
        let m = MonitorFromPoint(windows::Win32::Foundation::POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        let _ = GetMonitorInfoW(m, &mut mi);
        let r = mi.rcMonitor;
        let mut dx = 96u32;
        let mut dy = 96u32;
        let _ = windows::Win32::UI::HiDpi::GetDpiForMonitor(m, windows::Win32::UI::HiDpi::MDT_EFFECTIVE_DPI, &mut dx, &mut dy);
        let dpi = match hwnd {
            Some(h) if !h.0.is_null() => GetDpiForWindow(h).max(96),
            _ => dx.max(96),
        };
        (r.left, r.top, r.right, r.bottom, dpi as f64 / 96.0)
    }
}

/// Top-centre origin (physical px) for a window of `WIN_W` logical px.
pub fn top_centre(scale: f64) -> (i32, i32, i32, i32) {
    let (l, t, r, _, _) = primary_monitor(None);
    let pw = (WIN_W * scale).round() as i32;
    let ph = (WIN_H * scale).round() as i32;
    let x = l + ((r - l) - pw) / 2;
    (x, t, pw, ph)
}

pub fn create(proc: Proc) -> windows::core::Result<HWND> {
    unsafe {
        let hinst = GetModuleHandleW(None)?;
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(proc),
            hInstance: hinst.into(),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            lpszClassName: CLASS,
            ..Default::default()
        };
        RegisterClassExW(&wc);
        let (_, _, _, _, scale) = primary_monitor(None);
        let (x, y, pw, ph) = top_centre(scale);
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT,
            CLASS,
            w!("Blip"),
            WS_POPUP,
            x,
            y,
            pw,
            ph,
            None,
            None,
            Some(hinst.into()),
            None,
        )
    }
}

/// Toggle click-through. `true` = pointer events reach the island.
pub fn set_interactive(hwnd: HWND, on: bool) {
    unsafe {
        let mut ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let t = WS_EX_TRANSPARENT.0 as isize;
        let next = if on { ex & !t } else { ex | t };
        if next != ex {
            ex = next;
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex);
        }
    }
}
