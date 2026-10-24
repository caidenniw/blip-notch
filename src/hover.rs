use crate::conf;
use crate::dropzone;
use std::sync::Mutex;
use std::time::Duration;
use crate::bus::{self, post, Event};
use crate::win::window::set_interactive;

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
/// Half-width of the top-edge strip that a pointer must dwell on while compact.
const STRIP: f64 = 100.0;

pub fn start() {
    conf::log("island tampil");
    std::thread::spawn(hover_loop);
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
        Some((r, m)) => {
            use windows::Win32::UI::WindowsAndMessaging::{
                GetForegroundWindow, GetWindowLongW, GWL_STYLE, WS_CAPTION,
            };
            let fg = unsafe { GetForegroundWindow() };
            // Maximized normal windows also cover the monitor when the taskbar auto-hides;
            // true fullscreen (F11, borderless games) has no caption.
            let titled = (unsafe { GetWindowLongW(fg, GWL_STYLE) } as u32) & WS_CAPTION.0
                == WS_CAPTION.0;
            !titled && r.left <= m.left && r.top <= m.top && r.right >= m.right && r.bottom >= m.bottom
        }
        None => false,
    }
}

#[cfg(not(windows))]
fn fullscreen_app(_blip: isize) -> bool {
    false
}

/// Physical left-button state: an OLE drag from Explorer starts outside the window, so the UI cannot see it.
#[cfg(windows)]
fn lbutton_down() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    unsafe { GetAsyncKeyState(0x01) as u16 & 0x8000 != 0 }
}

#[cfg(not(windows))]
fn lbutton_down() -> bool {
    false
}

/// True while any app runs an OLE drag (DoDragDrop): the capture window is then OLE's hidden
/// CLIPBRDWNDCLASS window. Window moves, rubber-band selection and browser tab drags capture
/// other classes, so they do not match.
#[cfg(windows)]
fn ole_drag() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetAncestor, GetGUIThreadInfo, GA_ROOT, GUITHREADINFO,
    };
    let mut info = GUITHREADINFO {
        cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    if unsafe { GetGUIThreadInfo(0, &mut info) }.is_err() || info.hwndCapture.0.is_null() {
        return false;
    }
    let root = unsafe { GetAncestor(info.hwndCapture, GA_ROOT) };
    if root.0.is_null() {
        return false;
    }
    let cls = class_of(root);
    cls == "CLIPBRDWNDCLASS"
}

#[cfg(not(windows))]
fn ole_drag() -> bool {
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

/// Taskbar, its flyout hosts and the Start/search core window. While one is foreground
/// (auto-hidden taskbar revealed, tray opened) the previous fullscreen/compact decision holds.
#[cfg(windows)]
fn taskbar_foreground() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    matches!(
        class_of(unsafe { GetForegroundWindow() }).as_str(),
        "Shell_TrayWnd"
            | "Shell_SecondaryTrayWnd"
            | "TopLevelWindowForOverflowXamlIsland"
            | "NotifyIconOverflowWindow"
            | "Windows.UI.Core.CoreWindow"
            | "XamlExplorerHostIslandWindow"
    )
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
            | "XamlExplorerHostIslandWindow"
            | "ForegroundStaging"
            | "MultitaskingViewFrame"
            | "TaskSwitcherWnd"
            | "TopLevelWindowForOverflowXamlIsland"
            | "NotifyIconOverflowWindow"
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

fn hover_loop() {
    use windows::Win32::Foundation::{POINT, RECT as WinRect};
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetWindowRect};
    let hwnd = bus::hwnd();
    let mut last_cursor = (f64::MIN, f64::MIN);
    let mut prev_lbtn = false;
    let mut press_outside = false;
    let mut press_at = std::time::Instant::now();
    let mut press_pt = (0.0f64, 0.0f64);
    let mut hidden = false;
    let mut full_hits: u32 = 0;
    let mut compact = false;
    let mut off_hits: u32 = 0;
    let mut dwell: u32 = 0;
    let mut catcher_on = false;
    let mut catcher_rect = (0, 0, 0, 0);
    let mut drag_near = false;
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
                full_hits = if full {
                    full_hits + 1
                } else if taskbar_foreground() {
                    full_hits
                } else {
                    0
                };
                if full_hits >= 2 && !hidden {
                    // Raw ShowWindowAsync hide; restore below uses the same call.
                    unsafe {
                        let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindowAsync(
                            hwnd,
                            windows::Win32::UI::WindowsAndMessaging::SW_HIDE,
                        );
                    }
                    conf::log("fullscreen: sembunyi");
                    hidden = true;
                    if catcher_on {
                        dropzone::hide();
                        catcher_on = false;
                        catcher_rect = (0, 0, 0, 0);
                    }
                    if drag_near {
                        drag_near = false;
                        post(Event::DragNear(false));
                    }
                    *HOVERING.lock().unwrap() = false;
                    post(Event::Hover(false));
                } else if full_hits == 0 && hidden {
                    {
                        use windows::Win32::UI::WindowsAndMessaging::{
                            ShowWindowAsync, SW_SHOWNOACTIVATE,
                        };
                        unsafe {
                            let _ = ShowWindowAsync(hwnd, SW_SHOWNOACTIVATE);
                        }
                    }
                    assert_topmost(hwnd);
                    set_interactive(hwnd, false);
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
                        set_interactive(hwnd, false);
                        *HOVERING.lock().unwrap() = false;
                        post(Event::Hover(false));
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
            {
                assert_topmost(hwnd);
                // The drop catcher must stay above the Blip window, otherwise
                // Explorer hit-tests the window (which refuses external drops) and
                // the cursor shows a "prohibited" sign instead of a copy cursor.
                if catcher_on {
                    dropzone::raise();
                }
            }
        }
        let mut pt = POINT::default();
        if unsafe { GetCursorPos(&mut pt) }.is_err() {
            continue;
        }
        let mut rc = WinRect::default();
        if unsafe { GetWindowRect(hwnd, &mut rc) }.is_err() {
            continue;
        }
        let cursor = (pt.x as f64, pt.y as f64);
        let pos = POINT { x: rc.left, y: rc.top };
        let dpi = unsafe { GetDpiForWindow(hwnd) };
        let scale = if dpi == 0 { 1.0 } else { dpi as f64 / 96.0 };
        let cx = (cursor.0 - pos.x as f64) / scale;
        let cy = (cursor.1 - pos.y as f64) / scale;

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
                use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
                let fg_is_blip = unsafe { GetForegroundWindow() } == hwnd;
                if touch {
                    off_hits = 0;
                    if !compact {
                        compact = true;
                        post(Event::Compact(true));
                        conf::log("compact: on");
                    }
                } else if !fg_is_blip && !taskbar_foreground() {
                    // Leave compact only after 3 consecutive misses (~720 ms): window/tab switches
                    // pass through states where no window touches the notch.
                    off_hits += 1;
                    if compact && off_hits >= 3 {
                        compact = false;
                        post(Event::Compact(false));
                        conf::log("compact: off");
                    }
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
            press_at = std::time::Instant::now();
            press_pt = (cx, cy);
        }
        if !lbtn {
            press_outside = false;
        }
        prev_lbtn = lbtn;
        let dragging = lbtn && press_outside;
        // A drag, not a click: held >= 150 ms and moved >= 8 px since the press.
        let drag_gesture = dragging
            && press_at.elapsed() >= Duration::from_millis(150)
            && (cx - press_pt.0).abs() + (cy - press_pt.1).abs() >= 8.0;
        // A press that started inside (text selection, scrollbar, seek drag) keeps the panel open.
        let held_inside = lbtn && !press_outside && hovering;
        // A real OLE drag is in flight: the pointer entered the catcher (OLE DragEnter)
        // or any app owns an OLE drag.
        let ole = drag_gesture && ole_drag();
        let filedrag = dropzone::is_active() || ole;

        // Drop catcher: an Explorer file drag must land on a window that accepts
        // drops. The catcher is an invisible window (alpha 1) parked exactly over the
        // visible Blip, so every pixel of the notch or the open panel is a legal
        // drop target. It must stay up for the whole drag and stay above the Blip
        // window, otherwise Explorer hit-tests the window and shows a "prohibited"
        // cursor.
        // `ole` implies `drag_gesture` (press started outside Blip, held and moved), so a plain
        // click on the panel never raises the catcher and swallows the click.
        let near_blip = cx >= rx - 60.0
            && cx <= rx + rw + 60.0
            && cy >= ry - 30.0
            && cy <= ry + rh + 60.0;
        if ole && near_blip && !catcher_on {
            catcher_on = true;
        }
        if catcher_on {
            // Follow the island: the panel grows when it opens, and the pointer must
            // never leave the drop target mid-drag.
            let m = (40.0 * scale) as i32; // CATCH_MARGIN 40 logical px
            let rect = (
                pos.x + (rx * scale) as i32 - m,
                pos.y + (ry * scale) as i32,
                (rw * scale) as i32 + 2 * m,
                (rh * scale) as i32 + m,
            );
            if rect != catcher_rect {
                catcher_rect = rect;
                dropzone::show(rect.0, rect.1, rect.2, rect.3);
            }
            // The Blip window is re-asserted topmost every ~2 s, which would push it
            // above the catcher; Explorer would then hit-test the window, which has no
            // drop target, and show a "prohibited" cursor. Keep the catcher on top.
            dropzone::raise();
        }
        if dragging || filedrag {
            // An OLE drag is in flight: keep the catcher up until the drop lands.
            catcher_release = None;
        } else if catcher_on {
            // The drag is over (no button, no OLE drag): let the UI stop highlighting.
            let deadline = *catcher_release.get_or_insert_with(|| {
                std::time::Instant::now() + Duration::from_millis(250)
            });
            if std::time::Instant::now() >= deadline {
                dropzone::hide();
                catcher_on = false;
                catcher_release = None;
                catcher_rect = (0, 0, 0, 0);
            }
        }
        // "A file drag is over Blip": the UI switches to the File page so the drop
        // lands where the user expects. Mouse state alone cannot tell a file drag from
        // a browser tab drag, so `filedrag` needs OLE or a file-manager drag source.
        if filedrag != drag_near {
            drag_near = filedrag;
            post(Event::DragNear(filedrag));
        }

        let inside = if compact && !hovering {
            // The notch is a sliver: only a deliberate dwell on the very top edge wakes it,
            // so clicks land in the browser tab underneath.
            let strip = cy <= 3.0 && (cx - WIN_W / 2.0).abs() <= STRIP;
            dwell = if strip { dwell + 1 } else { 0 };
            dwell >= 16 || filedrag // ~256 ms, or a file drag
        } else {
            dwell = 0;
            inside_rect || held_inside || filedrag
        };

        {
            let mut h = HOVERING.lock().unwrap();
            if *h != inside {
                *h = inside;
                set_interactive(hwnd, inside);
                post(Event::Hover(inside));
            }
        }
        if (cx - last_cursor.0).abs() >= 1.0 || (cy - last_cursor.1).abs() >= 1.0 {
            last_cursor = (cx, cy);
            if cx.abs() < 700.0 && cy.abs() < 700.0 {
                post(Event::Cursor { x: cx, y: cy });
            }
        }
    }
}

pub fn set_rect(w: f64, h: f64) {
    let w = w.clamp(60.0, WIN_W);
    let h = h.clamp(24.0, WIN_H);
    let mut r = RECT.lock().unwrap();
    if h < r.3 {
        *GRACE.lock().unwrap() = Some((*r, std::time::Instant::now() + Duration::from_millis(900)));
    }
    *r = ((WIN_W - w) / 2.0, 0.0, w, h);
}

/// Remembers the current foreground window. The UI raises its own window itself.
pub fn focus_input() {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    unsafe {
        *PREV_FG.lock().unwrap() = GetForegroundWindow().0 as isize;
    }
}

pub fn release_focus() {
    use windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow;
    let prev = *PREV_FG.lock().unwrap();
    if prev != 0 {
        unsafe {
            let _ = SetForegroundWindow(windows::Win32::Foundation::HWND(prev as *mut _));
        }
    }
}
