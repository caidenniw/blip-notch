#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai;
mod app;
mod bus;
mod conf;
mod dropzone;
mod gfx;
mod hover;
mod keys;
mod logic;
mod media;
mod rec;
mod rt;
mod server;
mod store;
mod sys;
mod time;
mod ui;
mod win;

use app::App;
use gfx::Gfx;
use std::cell::RefCell;
use win::tray::{Tray, WM_APP_TRAY};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Ole::OleInitialize;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
};
use windows::Win32::UI::WindowsAndMessaging::*;

thread_local! {
    static GFX: RefCell<Option<Gfx>> = const { RefCell::new(None) };
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
    static TRAY: RefCell<Option<Tray>> = const { RefCell::new(None) };
}

fn render_frame(hwnd: HWND) {
    let (_, _, _, _, scale) = win::window::primary_monitor(Some(hwnd));
    let (_, _, pw, ph) = win::window::top_centre(scale);

    GFX.with(|g| {
        let mut g = g.borrow_mut();
        let Some(gfx) = g.as_mut() else { return };
        let _ = gfx.resize(pw as u32, ph as u32);

        APP.with(|a| {
            let mut a = a.borrow_mut();
            let Some(app) = a.as_mut() else { return };
            app.render(hwnd, gfx, scale as f32);
        });
    });
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_TIMER => {
            render_frame(hwnd);
            LRESULT(0)
        }
        bus::WM_APP_EVENT => {
            render_frame(hwnd);
            LRESULT(0)
        }
        bus::WM_APP_TOGGLE => {
            bus::post(bus::Event::Hotkey);
            render_frame(hwnd);
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
        WM_LBUTTONDOWN => {
            let x = (lp.0 & 0xffff) as i16 as f32;
            let y = ((lp.0 >> 16) & 0xffff) as i16 as f32;
            APP.with(|a| {
                if let Some(app) = a.borrow_mut().as_mut() {
                    let mut frame = ui::Frame::default();
                    let hit = frame.hit_test(x, y);
                    app.ui.active = hit.map(|h| h.id);
                }
            });
            render_frame(hwnd);
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            APP.with(|a| {
                if let Some(app) = a.borrow_mut().as_mut() {
                    app.ui.active = None;
                }
            });
            render_frame(hwnd);
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let x = (lp.0 & 0xffff) as i16 as f32;
            let y = ((lp.0 >> 16) & 0xffff) as i16 as f32;
            APP.with(|a| {
                if let Some(app) = a.borrow_mut().as_mut() {
                    app.ui.pointer_pos = (x, y);
                }
            });
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

fn main() {
    let _single = match win::single::SingleInstance::acquire() {
        Some(s) => s,
        None => return, // other instance already triggered
    };

    conf::log("blip start");

    // Initialize tokio background runtime
    let _ = rt::rt();

    unsafe {
        let _ = OleInitialize(None);

        let hwnd = win::window::create(wndproc).expect("window");
        bus::init(hwnd);

        let (_, _, _, _, scale) = win::window::primary_monitor(Some(hwnd));
        let (_, _, pw, ph) = win::window::top_centre(scale);
        GFX.with(|g| *g.borrow_mut() = Some(Gfx::new(pw as u32, ph as u32).expect("gfx")));
        APP.with(|a| *a.borrow_mut() = Some(App::new()));

        let tray = Tray::new(hwnd);
        TRAY.with(|t| *t.borrow_mut() = Some(tray));

        // Background services
        hover::start();
        dropzone::start();
        keys::start();
        media::start();
        sys::start();
        server::start();

        // Hotkeys: Ctrl+Alt+B (id 1) and Ctrl+Alt+Q (id 2)
        let _ = RegisterHotKey(Some(hwnd), 1, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, 0x42);
        let _ = RegisterHotKey(Some(hwnd), 2, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, 0x51);

        // 16ms animation and tick timer
        SetTimer(Some(hwnd), 1, 16, None);

        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        render_frame(hwnd);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
