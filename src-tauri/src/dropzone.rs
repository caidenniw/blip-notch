//! Native OLE drop target parked above the notch while a file is dragged.
//!
//! wry installs its own `IDropTarget` on the WebView2 child HWND and calls
//! `SetAllowExternalDrop(false)`, so drops over the webview never reach a handler we
//! control. This module creates a separate layered popup window and registers a drop
//! target on it, so Blip owns the OLE path end to end.

use crate::conf;
use std::sync::atomic::{AtomicIsize, Ordering};
use tauri::AppHandle;

static CATCHER: AtomicIsize = AtomicIsize::new(0);
/// Pending geometry requested before the catcher window was ready.
static PENDING: std::sync::Mutex<Option<(i32, i32, i32, i32)>> = std::sync::Mutex::new(None);

/// Window message used to move the catcher from its own thread.
const MOVE_MSG: u32 = 0x8000 + 1; // WM_APP + 1

#[cfg(windows)]
mod imp {
    use super::*;
    use std::cell::Cell;
    use std::ffi::c_void;
    use windows::core::{implement, PCWSTR};
    use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINTL, WPARAM};
    use windows::Win32::System::Com::{
        IDataObject, DVASPECT_CONTENT, FORMATETC, STGMEDIUM, TYMED_HGLOBAL,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Ole::{
        IDropTarget, IDropTarget_Impl, OleInitialize, RegisterDragDrop, ReleaseStgMedium, CF_HDROP,
        DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE,
    };
    use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
    use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, PostMessageW,
        RegisterClassW, SetLayeredWindowAttributes, SetWindowPos, ShowWindowAsync,
        TranslateMessage, HWND_TOPMOST, LWA_ALPHA, MSG, SWP_NOACTIVATE, SWP_SHOWWINDOW, SW_HIDE,
        WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
    };
    use tauri::Emitter;

    const CLASS_NAME: &str = "BlipDropCatcher";

    fn hwnd_from_raw(raw: isize) -> HWND {
        HWND(raw as *mut c_void)
    }

    unsafe extern "system" fn wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if msg == MOVE_MSG {
            let xy = wparam.0;
            let wh = lparam.0 as usize;
            let x = (xy & 0xFFFF) as u16 as i16 as i32;
            let y = ((xy >> 16) & 0xFFFF) as u16 as i16 as i32;
            let w = (wh & 0xFFFF) as u16 as i32;
            let h = ((wh >> 16) & 0xFFFF) as u16 as i32;
            unsafe {
                let _ = SetWindowPos(
                    hwnd,
                    Some(HWND_TOPMOST),
                    x,
                    y,
                    w,
                    h,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
            return LRESULT(0);
        }
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    #[implement(IDropTarget)]
    struct Catcher {
        app: AppHandle,
        valid: Cell<bool>,
    }

    impl Catcher_Impl {
        /// Hide from our own message thread: hiding while the OLE drag is still unwinding
        /// (i.e. from the hover poller right after the button release) cancels the drop.
        fn hide_self(&self) {
            let raw = CATCHER.load(Ordering::SeqCst);
            if raw == 0 {
                return;
            }
            unsafe {
                let _ = ShowWindowAsync(hwnd_from_raw(raw), SW_HIDE);
            }
        }
    }

    fn file_paths(data: Option<&IDataObject>) -> Vec<String> {
        let Some(data) = data else {
            return Vec::new();
        };
        let format = FORMATETC {
            cfFormat: CF_HDROP.0,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        };
        let Ok(mut medium) = (unsafe { data.GetData(&format) }) else {
            return Vec::new();
        };
        let hdrop = HDROP(unsafe { medium.u.hGlobal }.0 as _);
        let mut out = Vec::new();
        unsafe {
            let count = DragQueryFileW(hdrop, u32::MAX, None);
            for i in 0..count {
                let len = DragQueryFileW(hdrop, i, None) as usize;
                if len == 0 {
                    continue;
                }
                let mut buf = vec![0u16; len + 1];
                DragQueryFileW(hdrop, i, Some(&mut buf));
                out.push(String::from_utf16_lossy(&buf[..len]));
            }
            ReleaseStgMedium(&mut medium as *mut STGMEDIUM);
        }
        out
    }

    #[allow(non_snake_case)]
    impl IDropTarget_Impl for Catcher_Impl {
        fn DragEnter(
            &self,
            pdataobj: windows_core::Ref<'_, IDataObject>,
            _grfkeystate: MODIFIERKEYS_FLAGS,
            _pt: &POINTL,
            pdweffect: *mut DROPEFFECT,
        ) -> windows::core::Result<()> {
            let paths = file_paths(pdataobj.as_ref());
            let valid = !paths.is_empty();
            self.valid.set(valid);
            if valid {
                conf::log(&format!("drag masuk {} file", paths.len()));
                let _ = self.app.emit("drag-near", true);
            }
            unsafe {
                *pdweffect = if valid {
                    DROPEFFECT_COPY
                } else {
                    DROPEFFECT_NONE
                };
            }
            Ok(())
        }

        fn DragOver(
            &self,
            _grfkeystate: MODIFIERKEYS_FLAGS,
            _pt: &POINTL,
            pdweffect: *mut DROPEFFECT,
        ) -> windows::core::Result<()> {
            unsafe {
                *pdweffect = if self.valid.get() {
                    DROPEFFECT_COPY
                } else {
                    DROPEFFECT_NONE
                };
            }
            Ok(())
        }

        fn DragLeave(&self) -> windows::core::Result<()> {
            if self.valid.get() {
                let _ = self.app.emit("drag-near", false);
            }
            self.valid.set(false);
            self.hide_self();
            Ok(())
        }

        fn Drop(
            &self,
            pdataobj: windows_core::Ref<'_, IDataObject>,
            _grfkeystate: MODIFIERKEYS_FLAGS,
            _pt: &POINTL,
            pdweffect: *mut DROPEFFECT,
        ) -> windows::core::Result<()> {
            let paths = file_paths(pdataobj.as_ref());
            let _ = self.app.emit("shelf:drop", paths.clone());
            let _ = self.app.emit("drag-near", false);
            conf::log(&format!("drop masuk {} file", paths.len()));
            unsafe { *pdweffect = DROPEFFECT_COPY; }
            self.valid.set(false);
            self.hide_self();
            Ok(())
        }
    }

    pub fn start(app: AppHandle) {
        std::thread::spawn(move || unsafe {
            if let Err(e) = run(app) {
                conf::log(&format!("dropzone gagal: {e}"));
            }
        });
    }

    unsafe fn run(app: AppHandle) -> windows::core::Result<()> {
        unsafe {
            OleInitialize(None)?;
            let hinst = GetModuleHandleW(None)?;
            let class_name: Vec<u16> =
                CLASS_NAME.encode_utf16().chain(std::iter::once(0)).collect();
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: hinst.into(),
                lpszClassName: PCWSTR(class_name.as_ptr()),
                ..Default::default()
            };
            RegisterClassW(&wc);
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                PCWSTR(class_name.as_ptr()),
                PCWSTR(class_name.as_ptr()),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                Some(hinst.into()),
                None,
            )?;
            // Alpha 1: invisible in practice but still hit-tested, unlike alpha 0.
            SetLayeredWindowAttributes(hwnd, COLORREF(0), 1, LWA_ALPHA)?;
            let target: IDropTarget = Catcher {
                app,
                valid: Cell::new(false),
            }
            .into();
            RegisterDragDrop(hwnd, &target)?;
            CATCHER.store(hwnd.0 as isize, Ordering::SeqCst);
            // Apply any geometry that was requested before this thread was ready.
            if let Some((x, y, w, h)) = PENDING.lock().unwrap().take() {
                let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), x, y, w, h, SWP_NOACTIVATE | SWP_SHOWWINDOW);
            }
            conf::log("dropzone siap");

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        Ok(())
    }

    /// Moves the catcher from its own message thread: `SetWindowPos` must run there,
    /// and `SWP_ASYNCWINDOWPOS` was unreliable while an OLE drag was in flight.
    pub fn show(x: i32, y: i32, w: i32, h: i32) {
        let raw = CATCHER.load(Ordering::SeqCst);
        if raw == 0 {
            // Catcher thread not ready yet: store for application at startup.
            *PENDING.lock().unwrap() = Some((x, y, w, h));
            return;
        }
        let packed_xy = (x as u16 as usize) | ((y as u16 as usize) << 16);
        let packed_wh = (w as u16 as usize) | ((h as u16 as usize) << 16);
        unsafe {
            let _ = PostMessageW(
                Some(hwnd_from_raw(raw)),
                MOVE_MSG,
                WPARAM(packed_xy),
                LPARAM(packed_wh as isize),
            );
        }
    }

    pub fn hide() {
        let raw = CATCHER.load(Ordering::SeqCst);
        if raw == 0 {
            return;
        }
        unsafe {
            let _ = ShowWindowAsync(hwnd_from_raw(raw), SW_HIDE);
        }
    }
}

#[cfg(windows)]
pub use imp::{hide, show, start};

#[cfg(not(windows))]
pub fn start(_app: AppHandle) {}
#[cfg(not(windows))]
pub fn show(_x: i32, _y: i32, _w: i32, _h: i32) {}
#[cfg(not(windows))]
pub fn hide() {}
