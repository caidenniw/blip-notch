//! Single-instance guard via a named system mutex.
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, LPARAM, WPARAM};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW};

const MUTEX_NAME: PCWSTR = w!("dev.deni.blip-sim");

pub struct SingleInstance {
    handle: HANDLE,
}

impl SingleInstance {
    /// Attempts to acquire the single-instance mutex. If another instance is running,
    /// sends `WM_APP_TOGGLE` to its window and returns `None`.
    pub fn acquire() -> Option<Self> {
        unsafe {
            let handle = CreateMutexW(None, true, MUTEX_NAME).ok()?;
            if GetLastError() == ERROR_ALREADY_EXISTS {
                if let Ok(hwnd) = FindWindowW(crate::win::window::CLASS, None) {
                    if !hwnd.0.is_null() {
                        let _ = PostMessageW(Some(hwnd), crate::bus::WM_APP_TOGGLE, WPARAM(0), LPARAM(0));
                    }
                }
                None
            } else {
                Some(SingleInstance { handle })
            }
        }
    }
}

impl Drop for SingleInstance {
    fn drop(&mut self) {
        if !self.handle.0.is_null() {
            unsafe {
                let _ = CloseHandle(self.handle);
            }
        }
    }
}
