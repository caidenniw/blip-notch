//! System tray icon and popup menu.
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

pub const WM_APP_TRAY: u32 = WM_APP + 3;

pub const CMD_TOGGLE: usize = 101;
pub const CMD_AUTOSTART: usize = 102;
pub const CMD_CONFIG: usize = 103;
pub const CMD_QUIT: usize = 104;

pub struct Tray {
    hwnd: HWND,
}

impl Tray {
    pub fn new(hwnd: HWND) -> Self {
        unsafe {
            let hinst = GetModuleHandleW(None).unwrap_or_default();
            let hicon = LoadIconW(Some(hinst.into()), PCWSTR(1 as *const u16))
                .unwrap_or_else(|_| LoadIconW(None, IDI_APPLICATION).unwrap());

            let mut tip = [0u16; 128];
            let name = "Blip";
            for (i, c) in name.encode_utf16().enumerate() {
                if i < 127 {
                    tip[i] = c;
                }
            }

            let nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: 1,
                uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
                uCallbackMessage: WM_APP_TRAY,
                hIcon: hicon,
                szTip: tip,
                ..Default::default()
            };
            let _ = Shell_NotifyIconW(NIM_ADD, &nid);
        }
        Tray { hwnd }
    }

    pub fn show_menu(&self) {
        unsafe {
            let menu = match CreatePopupMenu() {
                Ok(m) => m,
                Err(_) => return,
            };

            let _ = AppendMenuW(menu, MF_STRING, CMD_TOGGLE, w!("Buka/Tutup"));

            let autostart_flags = if crate::win::autostart::is_enabled() {
                MF_STRING | MF_CHECKED
            } else {
                MF_STRING | MF_UNCHECKED
            };
            let _ = AppendMenuW(menu, autostart_flags, CMD_AUTOSTART, w!("Mulai saat login"));
            let _ = AppendMenuW(menu, MF_STRING, CMD_CONFIG, w!("Buka config"));
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
            let _ = AppendMenuW(menu, MF_STRING, CMD_QUIT, w!("Keluar"));

            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);

            let _ = SetForegroundWindow(self.hwnd);
            let cmd = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_RIGHTBUTTON,
                pt.x,
                pt.y,
                None,
                self.hwnd,
                None,
            );
            let _ = DestroyMenu(menu);

            match cmd.0 as usize {
                CMD_TOGGLE => {
                    crate::bus::post(crate::bus::Event::Hotkey);
                }
                CMD_AUTOSTART => {
                    let cur = crate::win::autostart::is_enabled();
                    let _ = crate::win::autostart::set_enabled(!cur);
                }
                CMD_CONFIG => {
                    let _ = std::process::Command::new("notepad.exe")
                        .arg(crate::conf::config_path())
                        .spawn();
                }
                CMD_QUIT => {
                    PostQuitMessage(0);
                }
                _ => {}
            }
        }
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        unsafe {
            let nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: 1,
                ..Default::default()
            };
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
        }
    }
}
