//! Autostart registry helper under HKCU\Software\Microsoft\Windows\CurrentVersion\Run.
use windows::core::{w, PCWSTR};
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE, REG_SZ,
};

const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const APP_NAME: PCWSTR = w!("Blip");

pub fn is_enabled() -> bool {
    unsafe {
        for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
            let mut key = HKEY::default();
            if RegOpenKeyExW(root, RUN_KEY, None, KEY_READ, &mut key).is_ok() {
                let mut data_type = REG_SZ;
                let mut size = 0u32;
                let r = RegQueryValueExW(key, APP_NAME, None, Some(&mut data_type), None, Some(&mut size));
                let _ = RegCloseKey(key);
                if r.is_ok() {
                    return true;
                }
            }
        }
        false
    }
}

pub fn set_enabled(enable: bool) -> Result<(), String> {
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY, None, KEY_WRITE, &mut key).is_err() {
            return Err("gagal buka registry Run".into());
        }
        let res = if enable {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let val = format!("\"{}\"", exe.to_string_lossy());
            let wide: Vec<u16> = val.encode_utf16().chain(std::iter::once(0)).collect();
            let slice = std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2);
            RegSetValueExW(key, APP_NAME, None, REG_SZ, Some(slice))
        } else {
            RegDeleteValueW(key, APP_NAME)
        };
        let _ = RegCloseKey(key);
        res.ok().map_err(|e| e.to_string())?;
        Ok(())
    }
}
