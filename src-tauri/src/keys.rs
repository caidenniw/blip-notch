use crate::{conf, rec};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage,
    KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

const VK_RCONTROL: u32 = 0xA3;
const LLKHF_INJECTED: u32 = 0x10;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum KeyOut {
    Tap,
    HoldStart,
    HoldEnd,
}

pub struct KeyMachine {
    down: bool,
    other: bool,
    hold: bool,
    t0: Option<Instant>,
}

impl Default for KeyMachine {
    fn default() -> Self {
        Self { down: false, other: false, hold: false, t0: None }
    }
}

impl KeyMachine {
    pub fn feed(&mut self, vk: u32, down: bool, now: Instant, _hold_ms: u64) -> Option<KeyOut> {
        if vk == VK_RCONTROL {
            if down {
                if self.down {
                    return None; // auto-repeat
                }
                self.down = true;
                self.other = false;
                self.hold = false;
                self.t0 = Some(now);
                return None;
            }
            if !self.down {
                return None;
            }
            let out = if self.hold {
                Some(KeyOut::HoldEnd)
            } else if !self.other {
                Some(KeyOut::Tap)
            } else {
                None
            };
            self.down = false;
            self.hold = false;
            self.t0 = None;
            return out;
        }
        if self.down {
            self.other = true;
        }
        None
    }

    pub fn tick(&mut self, now: Instant, hold_ms: u64) -> Option<KeyOut> {
        if !self.down || self.other || self.hold {
            return None;
        }
        let t0 = self.t0?;
        if now.duration_since(t0).as_millis() as u64 >= hold_ms {
            self.hold = true;
            return Some(KeyOut::HoldStart);
        }
        None
    }
}

static MACHINE: Mutex<KeyMachine> = Mutex::new(KeyMachine {
    down: false,
    other: false,
    hold: false,
    t0: None,
});
static TX: OnceLock<Sender<KeyOut>> = OnceLock::new();

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == 0 {
        let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        if kb.flags.0 & LLKHF_INJECTED == 0 {
            let msg = wparam.0 as u32;
            let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
            let up = msg == WM_KEYUP || msg == WM_SYSKEYUP;
            if down || up {
                let out = MACHINE
                    .lock()
                    .unwrap()
                    .feed(kb.vkCode, down, Instant::now(), conf::get().ui.hold_ms);
                if let Some(o) = out {
                    if let Some(tx) = TX.get() {
                        let _ = tx.send(o);
                    }
                }
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

pub fn start(app: AppHandle) {
    let (tx, rx) = mpsc::channel::<KeyOut>();
    let _ = TX.set(tx);

    std::thread::spawn(move || consume(app, rx));

    std::thread::spawn(|| unsafe {
        let hmod = GetModuleHandleW(None).ok().map(|h| windows::Win32::Foundation::HINSTANCE(h.0));
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), hmod, 0);
        match hook {
            Ok(_) => conf::log("keyboard hook terpasang"),
            Err(e) => {
                conf::log(&format!("keyboard hook gagal: {e}"));
                return;
            }
        }
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });

    std::thread::spawn(|| loop {
        std::thread::sleep(Duration::from_millis(25));
        let out = MACHINE
            .lock()
            .unwrap()
            .tick(Instant::now(), conf::get().ui.hold_ms);
        if let Some(o) = out {
            if let Some(tx) = TX.get() {
                let _ = tx.send(o);
            }
        }
    });
}

fn consume(app: AppHandle, rx: Receiver<KeyOut>) {
    for out in rx {
        match out {
            KeyOut::Tap => {
                let _ = app.emit("hotkey", "tap");
            }
            KeyOut::HoldStart => {
                if rec::start(app.clone(), "dictate").is_ok() {
                    let _ = app.emit("dictate", "start");
                }
            }
            KeyOut::HoldEnd => rec::stop(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn tap_is_tap() {
        let mut m = KeyMachine::default();
        let t0 = Instant::now();
        assert_eq!(m.feed(VK_RCONTROL, true, t0, 300), None);
        assert_eq!(m.feed(VK_RCONTROL, false, t0 + Duration::from_millis(100), 300), Some(KeyOut::Tap));
    }

    #[test]
    fn hold_requires_tick_and_ends() {
        let mut m = KeyMachine::default();
        let t0 = Instant::now();
        assert_eq!(m.feed(VK_RCONTROL, true, t0, 300), None);
        assert_eq!(m.tick(t0 + Duration::from_millis(100), 300), None);
        assert_eq!(m.tick(t0 + Duration::from_millis(500), 300), Some(KeyOut::HoldStart));
        assert_eq!(m.feed(VK_RCONTROL, false, t0 + Duration::from_millis(600), 300), Some(KeyOut::HoldEnd));
    }

    #[test]
    fn other_key_cancels() {
        let mut m = KeyMachine::default();
        let t0 = Instant::now();
        assert_eq!(m.feed(VK_RCONTROL, true, t0, 300), None);
        assert_eq!(m.feed(0x43, true, t0 + Duration::from_millis(30), 300), None); // 'C'
        assert_eq!(m.feed(0x43, false, t0 + Duration::from_millis(50), 300), None);
        assert_eq!(m.tick(t0 + Duration::from_millis(500), 300), None);
        assert_eq!(m.feed(VK_RCONTROL, false, t0 + Duration::from_millis(600), 300), None);
    }

    #[test]
    fn auto_repeat_is_ignored() {
        let mut m = KeyMachine::default();
        let t0 = Instant::now();
        assert_eq!(m.feed(VK_RCONTROL, true, t0, 300), None);
        assert_eq!(m.feed(VK_RCONTROL, true, t0 + Duration::from_millis(40), 300), None);
        assert_eq!(m.feed(VK_RCONTROL, false, t0 + Duration::from_millis(80), 300), Some(KeyOut::Tap));
        assert_eq!(m.feed(VK_RCONTROL, false, t0 + Duration::from_millis(90), 300), None);
    }
}
