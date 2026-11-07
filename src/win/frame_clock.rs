//! High-resolution one-shot timer wakes the UI; all JS and drawing stay on the UI thread.
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::time::{Duration, Instant};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, WPARAM, WAIT_OBJECT_0};
use windows::Win32::System::Threading::*;
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

pub const WM_FRAME: u32 = WM_APP + 5;
struct State { deadline: Option<Instant>, stop: bool }
pub struct FrameClock { event: isize, state: Arc<Mutex<State>>, animating: Arc<AtomicBool>, posted: Arc<AtomicBool> }

impl FrameClock {
    pub fn new(hwnd: HWND) -> Self {
        let event = unsafe { CreateEventW(None, false, false, PCWSTR::null()).expect("frame event") };
        let raw = event.0 as isize;
        let window = hwnd.0 as isize;
        let state = Arc::new(Mutex::new(State { deadline: None, stop: false }));
        let shared = state.clone();
        let animating = Arc::new(AtomicBool::new(false));
        let posted = Arc::new(AtomicBool::new(false));
        let active = animating.clone();
        let pending = posted.clone();
        std::thread::spawn(move || unsafe {
            let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_ABOVE_NORMAL);
            let event = HANDLE(raw as *mut _);
            let timer = CreateWaitableTimerExW(None, PCWSTR::null(), CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, TIMER_ALL_ACCESS.0)
                .or_else(|_| CreateWaitableTimerExW(None, PCWSTR::null(), 0, TIMER_ALL_ACCESS.0))
                .expect("frame timer");
            loop {
                let deadline = {
                    let s = shared.lock().unwrap();
                    if s.stop { break; }
                    s.deadline
                };
                if active.load(SeqCst) {
                    if windows::Win32::Graphics::Dwm::DwmFlush().is_err() {
                        std::thread::sleep(Duration::from_millis(16));
                    }
                    if shared.lock().unwrap().stop { break; }
                    if !pending.swap(true, SeqCst) {
                        if PostMessageW(Some(HWND(window as *mut _)), WM_FRAME, WPARAM(0), LPARAM(0)).is_err() {
                            pending.store(false, SeqCst);
                        }
                    }
                    continue;
                }
                let result = match deadline {
                    Some(at) => {
                        let ticks = (at.saturating_duration_since(Instant::now()).as_nanos() / 100).max(1).min(i64::MAX as u128) as i64;
                        if SetWaitableTimer(timer, &-ticks, 0, None, None, false).is_err() { break; }
                        WaitForMultipleObjects(&[event, timer], false, INFINITE)
                    }
                    None => WaitForMultipleObjects(&[event], false, INFINITE),
                };
                if result.0 == WAIT_OBJECT_0.0 + 1 {
                    let mut s = shared.lock().unwrap();
                    if s.deadline == deadline {
                        s.deadline = None;
                        if !pending.swap(true, SeqCst) {
                            if PostMessageW(Some(HWND(window as *mut _)), WM_FRAME, WPARAM(0), LPARAM(0)).is_err() {
                                pending.store(false, SeqCst);
                            }
                        }
                    }
                } else if result != WAIT_OBJECT_0 { break; }
            }
            let _ = CloseHandle(timer);
            let _ = CloseHandle(event);
        });
        Self { event: raw, state, animating, posted }
    }
    pub fn set_animating(&self, on: bool) {
        if on && !self.animating.swap(on, SeqCst) {
            unsafe { let _ = SetEvent(HANDLE(self.event as *mut _)); }
        } else if !on {
            self.animating.store(false, SeqCst);
        }
    }
    pub fn frame_consumed(&self) {
        self.posted.store(false, SeqCst);
    }
    pub fn schedule(&self, delay_ms: Option<f64>) {
        self.state.lock().unwrap().deadline = delay_ms.map(|ms| Instant::now() + Duration::from_secs_f64(ms.max(0.1) / 1000.0));
        unsafe { let _ = SetEvent(HANDLE(self.event as *mut _)); }
    }
}
impl Drop for FrameClock {
    fn drop(&mut self) {
        self.state.lock().unwrap().stop = true;
        unsafe { let _ = SetEvent(HANDLE(self.event as *mut _)); }
    }
}
