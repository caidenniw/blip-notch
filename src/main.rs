#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai;
mod bus;
mod conf;
mod dropzone;
mod engine;
mod css;
mod dom;
mod js;
mod layout;
mod gfx;
mod hover;
mod input;
mod keys;
mod media;
mod paint;
mod rec;
mod rt;
mod server;
#[cfg(debug_assertions)]
mod snapshot;
mod sys;
mod win;

use engine::Engine;
use gfx::Gfx;
use input::{Input, Mods};
use std::cell::RefCell;
use std::time::Instant;
use win::tray::{Tray, WM_APP_TRAY};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::Ole::OleInitialize;
use windows::Win32::UI::Input::Ime::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
/// `WM_MOUSELEAVE` (winuser.h; lives in the Controls feature of the `windows` crate).
const WM_MOUSELEAVE: u32 = 0x02A3;
use windows::Win32::UI::WindowsAndMessaging::*;


/// Everything the window procedure drives.
struct App {
    /// Island (layered, dirty-rect present) vs settings (normal window, full blit).
    island: bool,
    engine: Engine,
    hwnd: HWND,
    input: Input,
    gfx: Gfx,
    t0: Instant,
    /// Last painted state signature; repaint only when it changes.
    painted: Option<(u64, u64, crate::paint::Overlay, bool)>,
    clock: win::frame_clock::FrameClock,
    /// Island bounds painted last frame (logical px); unioned into the next dirty rect.
    last_dirty: Option<(f32, f32, f32, f32)>,
    high_surrogate: Option<u16>,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
    static TRAY: RefCell<Option<Tray>> = const { RefCell::new(None) };
    static SETTINGS: RefCell<Option<App>> = const { RefCell::new(None) };
}

fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|a| a.try_borrow_mut().ok().and_then(|mut a| a.as_mut().map(f)))
}

fn with_settings<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    SETTINGS.with(|a| a.try_borrow_mut().ok().and_then(|mut a| a.as_mut().map(f)))
}

/// The `App` that owns `hwnd`.
fn with_win<R>(hwnd: HWND, f: impl FnOnce(&mut App) -> R) -> Option<R> {
    if hwnd == bus::hwnd() { with_app(f) } else { with_settings(f) }
}

/// Opens the settings window, or focuses it when already open.
unsafe fn open_settings() {
    let existing = SETTINGS.with(|s| s.borrow().as_ref().map(|a| a.hwnd));
    if let Some(h) = existing {
        let _ = ShowWindow(h, SW_RESTORE);
        let _ = SetForegroundWindow(h);
        return;
    }
    let hwnd = match win::window::create_settings(wndproc) {
        Ok(h) => h,
        Err(e) => {
            conf::log(&format!("settings window gagal: {e}"));
            return;
        }
    };
    bus::init_settings(hwnd);
    let k = scale(hwnd);
    let gfx = match Gfx::new((win::window::SET_W * k as f64).round() as u32, (win::window::SET_H * k as f64).round() as u32) {
        Ok(g) => g,
        Err(e) => {
            conf::log(&format!("settings gfx gagal: {e}"));
            let _ = DestroyWindow(hwnd);
            return;
        }
    };
    let engine = Engine::boot(&engine::Doc::SETTINGS, js::storage::Storage::memory(), 0.0, false, &[]);
    let (r, s) = conf::motion(&conf::get());
    engine.set_motion(r, s);
    SETTINGS.with(|s| {
        *s.borrow_mut() = Some(App { island: false, hwnd, engine, input: Input::default(), gfx, t0: Instant::now(), painted: None, clock: win::frame_clock::FrameClock::new(hwnd), last_dirty: None, high_surrogate: None })
    });
    let _ = ShowWindow(hwnd, SW_SHOW);
    let _ = SetForegroundWindow(hwnd);
    with_win(hwnd, |a| a.pump(hwnd, true));
}

fn scale(hwnd: HWND) -> f32 {
    unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd).max(96) as f32 / 96.0 }
}

fn lp_xy(hwnd: HWND, lp: LPARAM) -> (f32, f32) {
    let k = scale(hwnd);
    ((lp.0 & 0xffff) as i16 as f32 / k, ((lp.0 >> 16) & 0xffff) as i16 as f32 / k)
}

fn mods() -> Mods {
    unsafe {
        Mods {
            ctrl: GetKeyState(VK_CONTROL.0 as i32) < 0,
            shift: GetKeyState(VK_SHIFT.0 as i32) < 0,
            alt: GetKeyState(VK_MENU.0 as i32) < 0,
        }
    }
}

impl App {
    fn now(&self) -> f64 {
        self.t0.elapsed().as_secs_f64() * 1000.0
    }

    /// Bus → JS, timers, rAF, style/layout, paint; then reschedules the timer.
    fn pump(&mut self, hwnd: HWND, frame: bool) {
        // Re-arm WM_FRAME only after this frame's work: posted messages outrank input in
        // GetMessage, so arming early lets frames starve clicks/wheel while animating.
        let now = self.now();
        let profile_start = Instant::now();
        let events = if self.island { bus::drain() } else { bus::drain_settings() };
        let last_cursor = events.iter().rposition(|ev| matches!(ev, bus::Event::Cursor { .. }));
        for (i, ev) in events.into_iter().enumerate() {
            if matches!(ev, bus::Event::Cursor { .. }) && Some(i) != last_cursor { continue; }
            if matches!(ev, bus::Event::Config) {
                let (r, s) = conf::motion(&conf::get());
                self.engine.set_motion(r, s);
            }
            self.engine.deliver(ev);
        }
        self.engine.pump(now, frame);
        self.input.apply_focus_requests(&self.engine);
        self.input.blink(&self.engine);
        self.engine.page.borrow_mut().storage.flush(false);
        let profile_update = profile_start.elapsed();
        if frame || !self.engine.animating() { self.present(hwnd); }
        if frame && std::env::var_os("BLIP_PROFILE_FRAMES").is_some() {
            conf::log(&format!("frame now={now:.3} update_ms={:.3} total_ms={:.3} anim={}", profile_update.as_secs_f64() * 1000.0, profile_start.elapsed().as_secs_f64() * 1000.0, self.engine.animating()));
        }
        self.schedule();
        if frame { self.clock.frame_consumed(); }
    }

    fn present(&mut self, hwnd: HWND) {
        self.engine.overlay = self.input.sync_overlay(&self.engine);
        let sig = {
            let p = self.engine.page.borrow();
            (p.dom.style_gen, p.dom.text_gen, self.engine.overlay.clone(), self.engine.animating() || p.dirty)
        };
        if self.painted.as_ref() == Some(&sig) && !sig.3 {
            return;
        }
        self.engine.page.borrow_mut().dirty = false;
        let full = self.painted.is_none();
        self.painted = Some(sig);
        let k = scale(hwnd);
        if !self.island {
            let (w, h) = ((win::window::SET_W * k as f64).round() as u32, (win::window::SET_H * k as f64).round() as u32);
            if self.gfx.resize(w, h).is_err() {
                return;
            }
            self.gfx.begin(k, None);
            self.engine.paint(&self.gfx, k);
            if self.gfx.present_blit(hwnd).is_err() {
                conf::log("present gagal, rebuild device");
                let _ = self.gfx.rebuild();
                self.engine.painter.borrow_mut().reset_device();
                self.painted = None;
            }
            return;
        }
        let (x, y, pw, ph) = win::window::top_centre(k as f64);
        if self.gfx.resize(pw as u32, ph as u32).is_err() {
            return;
        }
        // Everything visible lives inside #island; repaint only its old ∪ new bounds.
        let cur = self.engine.rect_of("island").map(|r| (r.0 - 4.0, r.1 - 4.0, r.2 + 4.0, r.3 + 4.0));
        let dirty = match (cur, self.last_dirty) {
            (Some(_), _) if full => None,
            (Some(c), Some(l)) => Some((c.0.min(l.0), c.1.min(l.1), c.2.max(l.2), c.3.max(l.3))),
            (Some(c), None) => Some(c),
            (None, _) => None,
        }
        .map(|r| {
            let (w, h) = (self.gfx.w as f32, self.gfx.h as f32);
            [(r.0 * k).floor().clamp(0.0, w) as u32, (r.1 * k).floor().clamp(0.0, h) as u32, (r.2 * k).ceil().clamp(0.0, w) as u32, (r.3 * k).ceil().clamp(0.0, h) as u32]
        });
        self.last_dirty = cur;
        self.gfx.begin(k, dirty);
        self.engine.paint(&self.gfx, k);
        if self.gfx.present(hwnd, x, y, dirty).is_err() {
            conf::log("present gagal, rebuild device");
            let _ = self.gfx.rebuild();
            self.engine.painter.borrow_mut().reset_device();
            self.painted = None;
        }
    }

    fn schedule(&self) {
        let now = self.now();
        let mut due: Option<f64> = self.engine.next_timer().map(|t| t - now);
        if self.input.caret_active(&self.engine) {
            let to_flip = 500.0 - (now - self.input.caret_t0).rem_euclid(500.0);
            due = Some(due.map_or(to_flip, |d| d.min(to_flip)));
        }
        if let Some(ms) = self.engine.page.borrow().storage.due_in() {
            due = Some(due.map_or(ms as f64, |d| d.min(ms as f64)));
        }
        self.clock.set_animating(self.engine.animating());
        self.clock.schedule(due);
    }
}

fn pump(hwnd: HWND, frame: bool) {
    with_win(hwnd, |a| a.pump(hwnd, frame));
}

fn key_name(vk: u16, lp: LPARAM) -> Option<String> {
    let k = VIRTUAL_KEY(vk);
    Some(match k {
        VK_RETURN => "Enter".into(),
        VK_ESCAPE => "Escape".into(),
        VK_BACK => "Backspace".into(),
        VK_DELETE => "Delete".into(),
        VK_LEFT => "ArrowLeft".into(),
        VK_RIGHT => "ArrowRight".into(),
        VK_UP => "ArrowUp".into(),
        VK_DOWN => "ArrowDown".into(),
        VK_HOME => "Home".into(),
        VK_END => "End".into(),
        VK_TAB => "Tab".into(),
        VK_SPACE => " ".into(),
        _ => unsafe {
            let mut state = [0u8; 256];
            let _ = GetKeyboardState(&mut state);
            state[VK_CONTROL.0 as usize] = 0; // keep the printable key under Ctrl
            let mut buf = [0u16; 4];
            let scan = ((lp.0 >> 16) & 0xff) as u32;
            let n = ToUnicode(vk as u32, scan, Some(&state), &mut buf, 0);
            if n <= 0 {
                return None;
            }
            String::from_utf16_lossy(&buf[..n as usize])
        },
    })
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_MOUSEACTIVATE if hwnd == bus::hwnd() => LRESULT(MA_NOACTIVATE as isize),
        WM_PAINT if hwnd != bus::hwnd() => {
            let mut ps = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
            windows::Win32::Graphics::Gdi::BeginPaint(hwnd, &mut ps);
            let _ = windows::Win32::Graphics::Gdi::EndPaint(hwnd, &ps);
            with_win(hwnd, |a| { a.painted = None; a.pump(hwnd, false) });
            LRESULT(0)
        }
        WM_ERASEBKGND if hwnd != bus::hwnd() => LRESULT(1),
        bus::WM_APP_SETTINGS => {
            open_settings();
            LRESULT(0)
        }
        win::frame_clock::WM_FRAME => {
            pump(hwnd, true);
            LRESULT(0)
        }
        WM_TIMER | bus::WM_APP_EVENT => {
            with_win(hwnd, |a| { let frame = !a.engine.animating(); a.pump(hwnd, frame); });
            LRESULT(0)
        }
        bus::WM_APP_TOGGLE => {
            bus::post(bus::Event::Hotkey);
            pump(hwnd, false);
            LRESULT(0)
        }
        win::window::WM_APP_DRAG => {
            with_win(hwnd, |a| a.input.cancel(&a.engine));
            if let Some((id, paths)) = engine::DRAG.with(|d| d.borrow_mut().pop_front()) {
                let r = sys::shelf_drag_out(paths).map(|_| serde_json::Value::Null);
                bus::post(bus::Event::Resolve { id, result: r });
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let (x, y) = lp_xy(hwnd, lp);
            let buttons = if wp.0 & 1 != 0 { 1 } else { 0 };
            with_win(hwnd, |a| {
                a.input.move_to(&a.engine, x, y, buttons, mods());
                a.pump(hwnd, false);
            });
            let mut t = TRACKMOUSEEVENT { cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32, dwFlags: TME_LEAVE, hwndTrack: hwnd, dwHoverTime: 0 };
            let _ = TrackMouseEvent(&mut t);
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            with_win(hwnd, |a| {
                a.input.leave(&a.engine);
                a.pump(hwnd, false);
            });
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let (x, y) = lp_xy(hwnd, lp);
            SetCapture(hwnd);
            with_win(hwnd, |a| {
                a.input.down(&a.engine, x, y, mods());
                a.pump(hwnd, false);
            });
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let (x, y) = lp_xy(hwnd, lp);
            let dbl = GetDoubleClickTime() as f64;
            with_win(hwnd, |a| {
                a.input.up(&a.engine, x, y, mods(), dbl);
                a.pump(hwnd, false);
            });
            // After `up`: ReleaseCapture sends WM_CAPTURECHANGED, which cancels any pending press.
            let _ = ReleaseCapture();
            LRESULT(0)
        }
        WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
            let mut pt = POINT { x: (lp.0 & 0xffff) as i16 as i32, y: ((lp.0 >> 16) & 0xffff) as i16 as i32 };
            let _ = windows::Win32::Graphics::Gdi::ScreenToClient(hwnd, &mut pt);
            let k = scale(hwnd);
            let delta = ((wp.0 >> 16) & 0xffff) as i16 as f32;
            let (dx, dy) = if msg == WM_MOUSEHWHEEL { (delta / 120.0 * 100.0, 0.0) } else {
                let v = -delta / 120.0 * 100.0;
                if mods().shift { (v, 0.0) } else { (0.0, v) }
            };
            with_win(hwnd, |a| {
                a.input.wheel(&a.engine, pt.x as f32 / k, pt.y as f32 / k, dx, dy);
                a.pump(hwnd, false);
            });
            LRESULT(0)
        }
        WM_RBUTTONUP => {
            let (x, y) = lp_xy(hwnd, lp);
            let state = with_win(hwnd, |a| a.input.context_target(&a.engine, x, y).then(|| a.input.context_state(&a.engine))).flatten();
            if let Some(state) = state {
                if let Ok(menu) = CreatePopupMenu() {
                    for (i, label) in [windows::core::w!("Potong"), windows::core::w!("Salin"), windows::core::w!("Tempel"), windows::core::w!("Pilih semua")].into_iter().enumerate() {
                        let flags = if state[i] { MF_STRING } else { MF_STRING | MF_GRAYED };
                        let _ = AppendMenuW(menu, flags, i + 1, label);
                    }
                    let mut pt = POINT::default();
                    let _ = GetCursorPos(&mut pt);
                    let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, pt.x, pt.y, None, hwnd, None);
                    let _ = DestroyMenu(menu);
                    with_win(hwnd, |a| { a.input.context_command(&a.engine, cmd.0 as u32); a.pump(hwnd, false); });
                }
            }
            LRESULT(0)
        }
        WM_CAPTURECHANGED if HWND(lp.0 as _) != hwnd => {
            with_win(hwnd, |a| { a.input.cancel(&a.engine); a.pump(hwnd, false); });
            LRESULT(0)
        }
        WM_CANCELMODE => {
            with_win(hwnd, |a| { a.input.cancel(&a.engine); a.pump(hwnd, false); });
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_IME_STARTCOMPOSITION => {
            let himc = ImmGetContext(hwnd);
            if !himc.is_invalid() {
                let k = scale(hwnd);
                if let Some(Some((x, y, h))) = with_win(hwnd, |a| a.input.caret_screen(&a.engine)) {
                    let form = COMPOSITIONFORM { dwStyle: CFS_POINT, ptCurrentPos: POINT { x: (x * k) as i32, y: ((y + h) * k) as i32 }, rcArea: Default::default() };
                    let _ = ImmSetCompositionWindow(himc, &form);
                }
                let _ = ImmReleaseContext(hwnd, himc);
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_IME_COMPOSITION if lp.0 as u32 & GCS_RESULTSTR.0 != 0 => {
            let himc = ImmGetContext(hwnd);
            if himc.is_invalid() { return DefWindowProcW(hwnd, msg, wp, lp); }
            let bytes = ImmGetCompositionStringW(himc, GCS_RESULTSTR, None, 0);
            if bytes > 0 {
                let mut buf = vec![0u16; bytes as usize / 2];
                ImmGetCompositionStringW(himc, GCS_RESULTSTR, Some(buf.as_mut_ptr().cast()), bytes as u32);
                let text = String::from_utf16_lossy(&buf);
                with_win(hwnd, |a| { a.input.insert_text(&a.engine, &text); a.pump(hwnd, false); });
            }
            let _ = ImmReleaseContext(hwnd, himc);
            LRESULT(0)
        }
        WM_SETCURSOR if (lp.0 & 0xffff) as u32 == HTCLIENT => {
            let kind = with_win(hwnd, |a| a.input.cursor(&a.engine)).unwrap_or(0);
            let id = match kind {
                1 => IDC_HAND,
                2 => IDC_IBEAM,
                _ => IDC_ARROW,
            };
            if let Ok(c) = LoadCursorW(None, id) {
                SetCursor(Some(c));
            }
            LRESULT(1)
        }
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            if let Some(k) = key_name(wp.0 as u16, lp) {
                with_win(hwnd, |a| {
                    a.input.key(&a.engine, &k, mods());
                    a.pump(hwnd, false);
                });
            }
            if msg == WM_SYSKEYDOWN {
                return DefWindowProcW(hwnd, msg, wp, lp);
            }
            LRESULT(0)
        }
        WM_CHAR => {
            if !mods().ctrl || mods().alt {
                with_win(hwnd, |a| {
                    if let Some(c) = input::utf16_unit(&mut a.high_surrogate, wp.0 as u16).filter(|c| *c >= ' ' && *c != '\u{7f}') {
                        a.input.char(&a.engine, c);
                        a.pump(hwnd, false);
                    }
                });
            }
            LRESULT(0)
        }
        WM_KILLFOCUS => {
            with_win(hwnd, |a| {
                a.input.set_focus(&a.engine, None, false);
                a.pump(hwnd, false);
            });
            LRESULT(0)
        }
        WM_DPICHANGED => {
            let r = &*(lp.0 as *const windows::Win32::Foundation::RECT);
            let _ = SetWindowPos(hwnd, None, r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOZORDER | SWP_NOACTIVATE);
            with_win(hwnd, |a| {
                a.painted = None;
                a.pump(hwnd, false);
            });
            LRESULT(0)
        }
        WM_APP_TRAY => {
            let evt = lp.0 as u32;
            if evt == WM_LBUTTONUP || evt == WM_RBUTTONUP {
                TRAY.with(|t| {
                    if let Some(tray) = t.borrow().as_ref() {
                        tray.show_menu();
                    }
                });
            }
            LRESULT(0)
        }
        WM_HOTKEY => {
            let id = wp.0;
            if id == 1 {
                bus::post(bus::Event::Hotkey);
            } else if id == 2 {
                PostQuitMessage(0);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            if hwnd == bus::hwnd() {
                PostQuitMessage(0);
            } else {
                bus::init_settings(HWND::default());
                let _ = bus::drain_settings();
                // Deferred drop: WM_DESTROY may arrive while the settings App is borrowed.
                SETTINGS.with(|s| if let Ok(mut s) = s.try_borrow_mut() { *s = None; });
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

fn main() {
    #[cfg(debug_assertions)]
    {
        let args: Vec<String> = std::env::args().collect();
        if args.len() == 4 && args[1] == "--snapshot" {
            std::process::exit(snapshot::run(&args[2], &args[3]));
        }
    }
    let _single = match win::single::SingleInstance::acquire() {
        Some(s) => s,
        None => return, // other instance already triggered
    };

    conf::log("blip start");

    // Initialize tokio background runtime
    let _ = rt::rt();

    unsafe {
        let _ = OleInitialize(None);
        // Blip is never foreground: opt out of EcoQoS so frames keep full CPU speed.
        {
            use windows::Win32::System::Threading::*;
            let state = PROCESS_POWER_THROTTLING_STATE {
                Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
                ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED | PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION,
                StateMask: 0,
            };
            if let Err(e) = SetProcessInformation(GetCurrentProcess(), ProcessPowerThrottling, &state as *const _ as *const _, std::mem::size_of_val(&state) as u32) {
                conf::log(&format!("power throttling opt-out gagal: {e}"));
            }
            let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_ABOVE_NORMAL);
        }

        let hwnd = win::window::create(wndproc).expect("window");
        bus::init(hwnd);

        let k = scale(hwnd);
        let (_, _, pw, ph) = win::window::top_centre(k as f64);
        let gfx = Gfx::new(pw as u32, ph as u32).expect("gfx");
        let storage = js::storage::Storage::load(conf::dir().join("localstorage.json"));
        let engine = Engine::boot(&engine::Doc::ISLAND, storage, 0.0, false, &[]);
        let (r, s) = conf::motion(&conf::get());
        engine.set_motion(r, s);
        APP.with(|a| {
            *a.borrow_mut() = Some(App { island: true, hwnd, engine, input: Input::default(), gfx, t0: Instant::now(), painted: None, clock: win::frame_clock::FrameClock::new(hwnd), last_dirty: None, high_surrogate: None })
        });

        let tray = Tray::new(hwnd);
        TRAY.with(|t| *t.borrow_mut() = Some(tray));

        // Background services
        conf::apply(&conf::get());
        hover::start();
        dropzone::start();
        keys::start();
        media::start();
        sys::start();
        server::start();

        // Hotkeys: open/close from config (id 1) and Ctrl+Alt+Q (id 2)
        if let Err(e) = win::window::set_toggle_hotkey(hwnd, &conf::get().ui.toggle_hotkey) {
            conf::log(&format!("hotkey {}: {e}", conf::get().ui.toggle_hotkey));
            let _ = win::window::set_toggle_hotkey(hwnd, "ctrl+alt+b");
        }
        let _ = RegisterHotKey(Some(hwnd), 2, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, 0x51);

        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        pump(hwnd, true);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        with_app(|a| a.engine.page.borrow_mut().storage.flush(true));
        conf::log("blip exit");
    }
}

