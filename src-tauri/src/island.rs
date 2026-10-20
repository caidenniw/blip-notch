use crate::conf;
use crate::dropzone;
use std::sync::Mutex;
use std::time::Duration;
use tauri::window::Color;
use tauri::{AppHandle, Emitter, Manager};

static RECT: Mutex<(f64, f64, f64, f64)> = Mutex::new((80.0, 0.0, 360.0, 76.0));
static HOVERING: Mutex<bool> = Mutex::new(false);
static PREV_FG: Mutex<isize> = Mutex::new(0);
/// Previous (larger) rect kept briefly after a shrink so a pointer left in the vacated area
/// does not close the panel.
static GRACE: Mutex<Option<((f64, f64, f64, f64), std::time::Instant)>> = Mutex::new(None);

const PAD_IN: f64 = 28.0;
const PAD_OUT: f64 = 6.0;
const WIN_W: f64 = 640.0;
const WIN_H: f64 = 460.0;
/// Half-width of the region where the native drop catcher appears while a file is dragged.
const MAGNET: f64 = 260.0;
/// Half-width of the top-edge strip that a pointer must dwell on while compact.
const STRIP: f64 = 100.0;

pub fn init(app: &AppHandle) {
    let Some(win) = app.get_webview_window("main") else {
        conf::log("window main tidak ada");
        return;
    };
    if let Some(monitor) = win.primary_monitor().ok().flatten() {
        let scale = monitor.scale_factor();
        let pos = monitor.position();
        let size = monitor.size();
        let w = WIN_W;
        let x = pos.x as f64 / scale + (size.width as f64 / scale - w) / 2.0;
        let y = pos.y as f64 / scale;
        let _ = win.set_position(tauri::LogicalPosition::new(x.round(), y.round()));
    }
    if let Err(e) = win.set_background_color(Some(Color(0, 0, 0, 255))) {
        conf::log(&format!("background Blip gagal: {e}"));
    }
    apply_shape(&win, 168.0, 36.0, 16.0, 11.0);
    let _ = win.set_ignore_cursor_events(true);
    let _ = win.show();
    conf::log("island tampil");

    let app2 = app.clone();
    std::thread::spawn(move || hover_loop(app2));
}

#[cfg(windows)]
fn fullscreen_app(blip: windows::Win32::Foundation::HWND) -> bool {
    use windows::Win32::UI::Shell::{
        SHQueryUserNotificationState, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN,
    };
    // QUNS_BUSY also fires for ordinary foreground work (it hides the desktop), so it is
    // not a full-screen signal; geometry decides everything else.
    let quirk = matches!(
        unsafe { SHQueryUserNotificationState() },
        Ok(windows::Win32::UI::Shell::QUERY_USER_NOTIFICATION_STATE(state))
            if state == QUNS_RUNNING_D3D_FULL_SCREEN.0 || state == QUNS_PRESENTATION_MODE.0
    );
    if quirk {
        return true;
    }
    match foreground_rect(blip) {
        Some((r, m)) => r.left <= m.left && r.top <= m.top && r.right >= m.right && r.bottom >= m.bottom,
        None => false,
    }
}

#[cfg(not(windows))]
fn fullscreen_app(_blip: isize) -> bool {
    false
}

/// Physical left-button state: an OLE drag from Explorer starts outside the webview, so the UI cannot see it.
#[cfg(windows)]
fn lbutton_down() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    unsafe { GetAsyncKeyState(0x01) as u16 & 0x8000 != 0 }
}

#[cfg(not(windows))]
fn lbutton_down() -> bool {
    false
}

#[cfg(windows)]
fn class_of(h: windows::Win32::Foundation::HWND) -> String {
    use windows::Win32::UI::WindowsAndMessaging::GetClassNameW;
    let mut buf = [0u16; 256];
    let n = unsafe { GetClassNameW(h, &mut buf) };
    if n <= 0 {
        String::new()
    } else {
        String::from_utf16_lossy(&buf[..n as usize])
    }
}

#[cfg(windows)]
/// Re-inserts the window at the top of the z-order. `set_always_on_top` is a no-op once
/// tao believes the window is already topmost, so it cannot recover from Win+D, where the
/// shell reorders windows instead of hiding them.
#[cfg(windows)]
fn assert_topmost(h: windows::Win32::Foundation::HWND) {
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOOWNERZORDER,
    };
    unsafe {
        let _ = SetWindowPos(
            h,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER,
        );
    }
}

#[cfg(windows)]
fn is_shell(h: windows::Win32::Foundation::HWND) -> bool {
    matches!(
        class_of(h).as_str(),
        "Progman"
            | "WorkerW"
            | "Shell_TrayWnd"
            | "Shell_SecondaryTrayWnd"
            | "Windows.UI.Core.CoreWindow"
    )
}

/// Window rect of the foreground window plus the monitor it lives on, or `None` when the
/// foreground window is not a normal app window on Blip's own monitor.
#[cfg(windows)]
fn foreground_rect(
    blip: windows::Win32::Foundation::HWND,
) -> Option<(
    windows::Win32::Foundation::RECT,
    windows::Win32::Foundation::RECT,
)> {
    use windows::Win32::Foundation::RECT as WinRect;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowRect, IsIconic, IsWindowVisible,
    };
    unsafe {
        let fg = GetForegroundWindow();
        if fg.0.is_null() || fg == blip || is_shell(fg) {
            return None;
        }
        if IsIconic(fg).as_bool() || !IsWindowVisible(fg).as_bool() {
            return None;
        }
        let mon_fg = MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST);
        let mon_blip = MonitorFromWindow(blip, MONITOR_DEFAULTTONEAREST);
        if mon_fg.0 != mon_blip.0 {
            return None;
        }
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(mon_fg, &mut info).as_bool() {
            return None;
        }
        let mut rect = WinRect::default();
        if GetWindowRect(fg, &mut rect).is_err() {
            return None;
        }
        Some((rect, info.rcMonitor))
    }
}

fn hover_loop(app: AppHandle) {
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    #[cfg(windows)]
    let Ok(hwnd) = win.hwnd() else { return };
    let mut last_cursor = (f64::MIN, f64::MIN);
    let mut prev_lbtn = false;
    let mut press_outside = false;
    let mut hidden = false;
    let mut full_hits: u32 = 0;
    let mut compact = false;
    let mut dwell: u32 = 0;
    let mut catcher_on = false;
    let mut catcher_release: Option<std::time::Instant> = None;
    let mut iter: u64 = 0;
    loop {
        std::thread::sleep(Duration::from_millis(16));
        iter += 1;

        if iter % 60 == 0 {
            #[cfg(windows)]
            {
                let full = fullscreen_app(hwnd);
                // Two consecutive hits (~2 s): a single one is the shell re-laying the desktop
                // during Win+D, which would otherwise flicker the notch away.
                full_hits = if full { full_hits + 1 } else { 0 };
                if full_hits >= 2 && !hidden {
                    let _ = win.hide();
                    conf::log("fullscreen: sembunyi");
                    hidden = true;
                    *HOVERING.lock().unwrap() = false;
                    let _ = app.emit("hover", false);
                } else if full_hits == 0 && hidden {
                    let _ = win.show();
                    let _ = win.set_ignore_cursor_events(true);
                    conf::log("fullscreen: tampil");
                    hidden = false;
                } else if !hidden {
                    use windows::Win32::UI::WindowsAndMessaging::{
                        IsIconic, IsWindowVisible, ShowWindowAsync, SW_SHOWNOACTIVATE,
                    };
                    if unsafe { !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() } {
                        unsafe {
                            let _ = ShowWindowAsync(hwnd, SW_SHOWNOACTIVATE);
                        }
                        assert_topmost(hwnd);
                        let _ = win.set_ignore_cursor_events(true);
                        *HOVERING.lock().unwrap() = false;
                        let _ = app.emit("hover", false);
                        conf::log("pulih: tampil lagi");
                    }
                }
            }
            #[cfg(not(windows))]
            let _ = fullscreen_app(0);
        }
        if hidden {
            continue;
        }
        if iter % 120 == 0 {
            #[cfg(windows)]
            assert_topmost(hwnd);
        }

        let Ok(cursor) = win.cursor_position() else { continue };
        let Ok(pos) = win.outer_position() else { continue };
        let scale = win.scale_factor().unwrap_or(1.0);
        let cx = (cursor.x - pos.x as f64) / scale;
        let cy = (cursor.y - pos.y as f64) / scale;

        if iter % 15 == 0 {
            #[cfg(windows)]
            {
                // Win+D hands the desktop the top z-order slot; take it back.
                assert_topmost(hwnd);
                let nl = pos.x + ((WIN_W / 2.0 - STRIP) * scale) as i32;
                let nr = pos.x + ((WIN_W / 2.0 + STRIP) * scale) as i32;
                let touch = matches!(foreground_rect(hwnd), Some((r, m))
                    if r.top <= m.top + 4
                        && r.bottom > m.top + 100
                        && r.left < nl
                        && r.right > nr);
                if touch != compact {
                    compact = touch;
                    let _ = app.emit("compact", touch);
                    conf::log(if touch { "compact: on" } else { "compact: off" });
                }
            }
        }

        let (rx, ry, rw, rh) = *RECT.lock().unwrap();
        let hovering = *HOVERING.lock().unwrap();
        // Hysteresis: a wide pad keeps a shaky pointer inside, a tight one lets it leave.
        let near = |pad: f64| {
            cx >= rx - pad && cx <= rx + rw + pad && cy >= ry - pad && cy <= ry + rh + pad
        };
        let in_grace = hovering
            && match *GRACE.lock().unwrap() {
                Some(((gx, gy, gw, gh), until)) if std::time::Instant::now() < until => {
                    cx >= gx - PAD_IN
                        && cx <= gx + gw + PAD_IN
                        && cy >= gy - PAD_IN
                        && cy <= gy + gh + PAD_IN
                }
                _ => false,
            };
        let inside_rect = if hovering { near(PAD_IN) || in_grace } else { near(PAD_OUT) };

        let lbtn = lbutton_down();
        if lbtn && !prev_lbtn {
            press_outside = !near(PAD_OUT);
        }
        if !lbtn {
            press_outside = false;
        }
        prev_lbtn = lbtn;
        let dragging = lbtn && press_outside;
        // A press that started inside (text selection, scrollbar, seek drag) keeps the panel open.
        let held_inside = lbtn && !press_outside && hovering;

        if dragging {
            catcher_release = None;
        }
        if dragging
            && !catcher_on
            && cx >= WIN_W / 2.0 - MAGNET
            && cx <= WIN_W / 2.0 + MAGNET
            && cy >= -4.0
            && cy <= 120.0
        {
            dropzone::show(
                pos.x + ((WIN_W / 2.0 - MAGNET) * scale) as i32,
                pos.y,
                (2.0 * MAGNET * scale) as i32,
                (200.0 * scale) as i32,
            );
            catcher_on = true;
        }
        if !lbtn && catcher_on {
            // OLE still needs the window for the release itself: give Drop/DragLeave time to
            // arrive, then fall back to hiding it ourselves.
            let deadline = *catcher_release.get_or_insert_with(|| {
                std::time::Instant::now() + Duration::from_millis(600)
            });
            if std::time::Instant::now() >= deadline {
                dropzone::hide();
                catcher_on = false;
                catcher_release = None;
            }
        }

        let inside = if compact && !hovering {
            // The notch is a sliver: only a deliberate dwell on the very top edge wakes it,
            // so clicks land in the browser tab underneath.
            let strip = cy <= 3.0 && (cx - WIN_W / 2.0).abs() <= STRIP;
            dwell = if strip { dwell + 1 } else { 0 };
            dwell >= 16 // ~256 ms
        } else {
            dwell = 0;
            inside_rect || held_inside
        };

        {
            let mut h = HOVERING.lock().unwrap();
            if *h != inside {
                *h = inside;
                let _ = win.set_ignore_cursor_events(!inside);
                let _ = app.emit("hover", inside);
            }
        }
        if (cx - last_cursor.0).abs() >= 1.0 || (cy - last_cursor.1).abs() >= 1.0 {
            last_cursor = (cx, cy);
            if cx.abs() < 700.0 && cy.abs() < 700.0 {
                let _ = app.emit("cursor", serde_json::json!({ "x": cx, "y": cy }));
            }
        }
    }
}

#[tauri::command]
pub fn set_rect(w: f64, h: f64) {
    let w = w.clamp(60.0, WIN_W);
    let h = h.clamp(24.0, WIN_H);
    let mut r = RECT.lock().unwrap();
    if h < r.3 {
        *GRACE.lock().unwrap() = Some((*r, std::time::Instant::now() + Duration::from_millis(900)));
    }
    *r = ((WIN_W - w) / 2.0, 0.0, w, h);
}

#[cfg(windows)]
fn apply_shape(win: &tauri::WebviewWindow, w: f64, h: f64, r: f64, e: f64) {
    use windows::Win32::Graphics::Gdi::{
        CombineRgn, CreateEllipticRgn, CreateRectRgn, CreateRoundRectRgn, DeleteObject,
        SetWindowRgn, HGDIOBJ, RGN_DIFF, RGN_OR,
    };
    let Ok(hwnd) = win.hwnd() else { return };
    let s = win.scale_factor().unwrap_or(1.0);
    let left = ((WIN_W - w) / 2.0 * s).round() as i32;
    let right = left + (w * s).round() as i32;
    let bottom = (h * s).round() as i32;
    let rad = (r * s).round().max(0.0) as i32;
    let ear = (e * s).round().max(0.0) as i32;
    unsafe {
        // Flat top, rounded bottom: round-rect plus a rect covering the top corners.
        let rgn = CreateRoundRectRgn(left, 0, right + 1, bottom + 1, rad * 2, rad * 2);
        let top = CreateRectRgn(left, 0, right, (bottom - rad).max(1));
        CombineRgn(Some(rgn), Some(rgn), Some(top), RGN_OR);
        let _ = DeleteObject(HGDIOBJ(top.0));
        if ear > 0 {
            // Inverted-corner ears, mirroring #island::before/::after radial gradients.
            for (sq, hole) in [
                (
                    CreateRectRgn(left - ear, 0, left, ear),
                    CreateEllipticRgn(left - 2 * ear, 0, left, 2 * ear),
                ),
                (
                    CreateRectRgn(right, 0, right + ear, ear),
                    CreateEllipticRgn(right, 0, right + 2 * ear, 2 * ear),
                ),
            ] {
                CombineRgn(Some(sq), Some(sq), Some(hole), RGN_DIFF);
                CombineRgn(Some(rgn), Some(rgn), Some(sq), RGN_OR);
                let _ = DeleteObject(HGDIOBJ(sq.0));
                let _ = DeleteObject(HGDIOBJ(hole.0));
            }
        }
        // Nonzero: Windows now owns rgn.
        if SetWindowRgn(hwnd, Some(rgn), true) == 0 {
            let _ = DeleteObject(HGDIOBJ(rgn.0));
            conf::log("region Blip gagal");
        }
    }
}

#[cfg(not(windows))]
fn apply_shape(_: &tauri::WebviewWindow, _: f64, _: f64, _: f64, _: f64) {}

/// Clips the opaque native surface to the island's live CSS shape.
#[tauri::command]
pub fn set_shape(app: AppHandle, w: f64, h: f64, r: f64, e: f64) {
    let w = w.clamp(1.0, WIN_W);
    let h = h.clamp(1.0, WIN_H);
    if let Some(win) = app.get_webview_window("main") {
        apply_shape(&win, w, h, r.clamp(0.0, h), e.clamp(0.0, 40.0));
    }
}

#[tauri::command]
pub fn focus_input(app: AppHandle) {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    unsafe {
        *PREV_FG.lock().unwrap() = GetForegroundWindow().0 as isize;
    }
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.set_focus();
    }
}

#[tauri::command]
pub fn release_focus() {
    use windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow;
    let prev = *PREV_FG.lock().unwrap();
    if prev != 0 {
        unsafe {
            let _ = SetForegroundWindow(windows::Win32::Foundation::HWND(prev as *mut _));
        }
    }
}
