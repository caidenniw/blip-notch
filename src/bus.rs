//! Service threads -> UI thread event bus.
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{LazyLock, Mutex};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

pub const WM_APP_EVENT: u32 = WM_APP + 1;
pub const WM_APP_TOGGLE: u32 = WM_APP + 2;

static CH: LazyLock<(Sender<Event>, Mutex<Receiver<Event>>)> = LazyLock::new(|| {
    let (tx, rx) = std::sync::mpsc::channel();
    (tx, Mutex::new(rx))
});
static HWND_RAW: AtomicIsize = AtomicIsize::new(0);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DictatePhase {
    Start,
    Done,
}

#[derive(Clone, Debug)]
pub enum HudEvent {
    Volume { value: u32, muted: bool },
    Caps { on: bool },
    Power { plugged: bool, battery: Option<u32> },
    Lang { value: String },
    Net { online: bool },
    Audio { name: String },
    Brightness { value: u32 },
    Bt { name: String, on: bool },
}

#[derive(Clone, Debug, Default)]
pub struct MediaInfo {
    pub title: String,
    pub artist: String,
    pub playing: bool,
    pub position: f64,
    pub duration: f64,
    pub app: String,
    pub ts: u64,
    /// `None` = cover unchanged (same track); `Some(None)` = track changed, no cover;
    /// `Some(Some(bytes))` = new cover (raw encoded image bytes).
    pub art: Option<Option<Vec<u8>>>,
}

#[derive(Clone, Debug)]
pub enum Event {
    Hotkey,
    Dictate(DictatePhase),
    Hover(bool),
    Cursor { x: f64, y: f64 },
    Compact(bool),
    DragNear(bool),
    ChatChunk { id: String, delta: String },
    ChatDone { id: String, model: Option<String>, aborted: bool },
    ChatError { id: String, error: String },
    TtsStart,
    TtsLevel(f32),
    TtsEnd,
    RecLevel(f32),
    SttResult(String),
    SttEmpty,
    SttError(String),
    Media(MediaInfo),
    Hud(HudEvent),
    Sys(crate::sys::Snap),
    ShelfDrop(Vec<String>),
    ShelfDragEnd(bool),
    DragEnter,
    DragLeave,
    UiShow,
    UiHide,
    UiAsk(String),
    Agent { agent: String, state: String, text: Option<String> },
    Lyrics { key: String, result: Result<serde_json::Value, String> },
    ClipboardText(Result<String, String>),
    ShelfInfo(Vec<serde_json::Value>),
    Toast(String),
}

pub fn init(hwnd: HWND) {
    HWND_RAW.store(hwnd.0 as isize, Ordering::SeqCst);
}

pub fn hwnd() -> HWND {
    HWND(HWND_RAW.load(Ordering::SeqCst) as *mut _)
}

/// Queue an event and wake the UI thread.
pub fn post(e: Event) {
    if CH.0.send(e).is_ok() {
        let h = hwnd();
        if !h.0.is_null() {
            unsafe {
                let _ = PostMessageW(Some(h), WM_APP_EVENT, WPARAM(0), LPARAM(0));
            }
        }
    }
}

/// UI thread only: drain everything queued so far.
pub fn drain() -> Vec<Event> {
    CH.1.lock().unwrap().try_iter().collect()
}
