#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai;
mod conf;
mod dropzone;
mod island;
mod keys;
mod media;
mod rec;
mod server;
mod sys;

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = app.emit("hotkey", "tap");
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .invoke_handler(tauri::generate_handler![
            conf::info,
            island::set_rect,
            island::set_shape,
            island::focus_input,
            island::release_focus,
            ai::chat_send,
            ai::chat_abort,
            ai::stop_speak,
            ai::beep,
            rec::rec_start,
            rec::rec_stop,
            media::media_cmd,
            media::lyrics,
            media::open_target,
            sys::volume,
            sys::sys_status,
            sys::shelf_info,
            sys::open_path,
            sys::shelf_from_clipboard,
            sys::shelf_copy,
            sys::reveal_path,
            sys::shelf_drag_out,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            conf::log("blip start");
            island::init(&handle);
            dropzone::start(handle.clone());
            keys::start(handle.clone());
            media::start(handle.clone());
            sys::start(handle.clone());
            server::start(handle.clone());
            tray(&handle)?;
            shortcuts(&handle);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn shortcuts(app: &AppHandle) {
    let gs = app.global_shortcut();
    let toggle = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyB);
    let quit = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyQ);
    let app2 = app.clone();
    let _ = gs.on_shortcut(toggle, move |_app, _sc, event| {
        if event.state() == ShortcutState::Pressed {
            let _ = app2.emit("hotkey", "tap");
        }
    });
    let app3 = app.clone();
    let _ = gs.on_shortcut(quit, move |_app, _sc, event| {
        if event.state() == ShortcutState::Pressed {
            app3.exit(0);
        }
    });
}

fn tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Buka/Tutup", true, None::<&str>)?;
    let autostart = CheckMenuItem::with_id(
        app,
        "autostart",
        "Mulai saat login",
        true,
        autostart_enabled(app),
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Keluar", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &autostart, &sep, &quit])?;
    let icon = app.default_window_icon().cloned();
    let mut builder = TrayIconBuilder::with_id("blip")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .tooltip("Blip");
    if let Some(icon) = icon {
        builder = builder.icon(icon);
    }
    builder
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => {
                let _ = app.emit("hotkey", "tap");
            }
            "autostart" => {
                use tauri_plugin_autostart::ManagerExt;
                let manager = app.autolaunch();
                let enabled = manager.is_enabled().unwrap_or(false);
                let _ = if enabled { manager.disable() } else { manager.enable() };
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

fn autostart_enabled(app: &AppHandle) -> bool {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().unwrap_or(false)
}
