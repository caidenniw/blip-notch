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
/// Settings window client size (logical px).
pub const SET_W: f64 = 600.0;
pub const SET_H: f64 = 540.0;
pub const SET_CLASS: PCWSTR = w!("BlipSettings");

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
        if RegisterClassExW(&wc) == 0 {
            crate::conf::log(&format!("RegisterClassExW gagal: {:?}", windows::core::Error::from_thread()));
        }
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

/// Gives the island keyboard focus (old Tauri `win.set_focus()`): drops `WS_EX_NOACTIVATE`
/// so the foreground switch is allowed, then activates.
pub fn take_focus(hwnd: HWND) {
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex & !(WS_EX_NOACTIVATE.0 as isize));
        let _ = SetForegroundWindow(hwnd);
        let _ = windows::Win32::UI::Input::KeyboardAndMouse::SetFocus(Some(hwnd));
    }
}

/// Restores the non-activating style after `take_focus`.
pub fn drop_focus(hwnd: HWND) {
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | WS_EX_NOACTIVATE.0 as isize);
    }
}

/// Posted to run `sys::shelf_drag_out` from the window procedure, outside any JS call.
pub const WM_APP_DRAG: u32 = WM_APP + 4;

pub fn post_drag(hwnd: HWND) {
    unsafe {
        let _ = PostMessageW(Some(hwnd), WM_APP_DRAG, WPARAM(0), LPARAM(0));
    }
}

/// `prefers-reduced-motion: reduce` when Windows animations are switched off.
pub fn reduced_motion() -> bool {
    let mut on = windows::core::BOOL(1);
    unsafe {
        let _ = SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, Some(&mut on as *mut _ as *mut _), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0));
    }
    !on.as_bool()
}

/// (Re)registers hotkey id 1 (open/close) to one of `conf::HOTKEYS`.
pub fn set_toggle_hotkey(hwnd: HWND, name: &str) -> Result<(), String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::*;
    let (mods, vk) = match name {
        "ctrl+alt+b" => (MOD_CONTROL | MOD_ALT, 0x42),
        "ctrl+alt+space" => (MOD_CONTROL | MOD_ALT, 0x20),
        "ctrl+shift+space" => (MOD_CONTROL | MOD_SHIFT, 0x20),
        _ => return Err(format!("Unknown value: {name}")),
    };
    unsafe {
        let _ = UnregisterHotKey(Some(hwnd), 1);
        RegisterHotKey(Some(hwnd), 1, mods | MOD_NOREPEAT, vk).map_err(|_| "Hotkey is used by another app".to_string())
    }
}

pub fn settings_title() -> &'static str {
    if crate::conf::english() { "Blip Settings" } else { "Pengaturan Blip" }
}

/// Normal (activating) dark-titled window that hosts the settings document.
pub fn create_settings(proc: Proc) -> windows::core::Result<HWND> {
    unsafe {
        let hinst = GetModuleHandleW(None)?;
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(proc),
            hInstance: hinst.into(),
            hIcon: LoadIconW(Some(hinst.into()), PCWSTR(1 as _)).unwrap_or_default(),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            lpszClassName: SET_CLASS,
            ..Default::default()
        };
        let _ = RegisterClassExW(&wc); // already registered on the second open
        let scale = primary_monitor(None).4;
        let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
        let mut r = windows::Win32::Foundation::RECT { left: 0, top: 0, right: (SET_W * scale).round() as i32, bottom: (SET_H * scale).round() as i32 };
        let _ = windows::Win32::UI::HiDpi::AdjustWindowRectExForDpi(&mut r, style, false, WINDOW_EX_STYLE(0), (scale * 96.0) as u32);
        let (w, h) = (r.right - r.left, r.bottom - r.top);
        let m = MonitorFromPoint(windows::Win32::Foundation::POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        let _ = GetMonitorInfoW(m, &mut mi);
        let wa = mi.rcWork;
        let x = wa.left + ((wa.right - wa.left) - w).max(0) / 2;
        let y = wa.top + ((wa.bottom - wa.top) - h).max(0) / 2;
        let title = windows::core::HSTRING::from(settings_title());
        let hwnd = CreateWindowExW(WINDOW_EX_STYLE(0), SET_CLASS, &title, style, x, y, w, h, None, None, Some(hinst.into()), None)?;
        let dark = windows::core::BOOL(1);
        let _ = windows::Win32::Graphics::Dwm::DwmSetWindowAttribute(
            hwnd,
            windows::Win32::Graphics::Dwm::DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark as *const _ as *const _,
            std::mem::size_of_val(&dark) as u32,
        );
        Ok(hwnd)
    }
}
