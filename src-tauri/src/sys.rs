use crate::conf;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    eMultimedia, eRender, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
};
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CAPITAL};

#[derive(Clone, PartialEq)]
struct Snap {
    volume: u32,
    muted: bool,
    caps: bool,
    battery: Option<u32>,
    plugged: bool,
    lang: String,
    net: bool,
    audio: String,
}

impl Snap {
    fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "volume": self.volume,
            "muted": self.muted,
            "caps": self.caps,
            "battery": self.battery,
            "plugged": self.plugged,
            "lang": self.lang,
            "net": self.net,
            "audio": self.audio,
        })
    }
}

fn com_init() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
}

fn endpoint() -> Option<(IMMDevice, IAudioEndpointVolume)> {
    unsafe {
        let de: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        let dev = de.GetDefaultAudioEndpoint(eRender, eMultimedia).ok()?;
        let vol = dev.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None).ok()?;
        Some((dev, vol))
    }
}

fn device_name(dev: &IMMDevice) -> String {
    use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
    use windows::Win32::System::Com::StructuredStorage::{
        PropVariantClear, PropVariantToStringAlloc,
    };
    use windows::Win32::System::Com::{CoTaskMemFree, STGM_READ};
    unsafe {
        let Ok(store) = dev.OpenPropertyStore(STGM_READ) else {
            return String::new();
        };
        let Ok(mut prop) = store.GetValue(&PKEY_Device_FriendlyName) else {
            return String::new();
        };
        let Ok(pwstr) = PropVariantToStringAlloc(&prop) else {
            let _ = PropVariantClear(&mut prop);
            return String::new();
        };
        let s = pwstr.to_string().unwrap_or_default();
        CoTaskMemFree(Some(pwstr.0 as _));
        let _ = PropVariantClear(&mut prop);
        s
    }
}

fn current_lang() -> String {
    use windows::Win32::Globalization::LCIDToLocaleName;
    use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyboardLayout;
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    unsafe {
        let fg = GetForegroundWindow();
        let tid = GetWindowThreadProcessId(fg, None);
        let hkl = GetKeyboardLayout(tid);
        let langid = (hkl.0 as usize & 0xFFFF) as u32;
        let mut buf = [0u16; 85];
        let len = LCIDToLocaleName(langid, Some(&mut buf[..85]), 0);
        if len > 1 {
            String::from_utf16_lossy(&buf[..(len as usize - 1)])
        } else {
            String::new()
        }
    }
}

fn current_net() -> bool {
    use windows::Networking::Connectivity::{NetworkConnectivityLevel, NetworkInformation};
    NetworkInformation::GetInternetConnectionProfile()
        .and_then(|p| p.GetNetworkConnectivityLevel())
        .map(|lvl| lvl == NetworkConnectivityLevel::InternetAccess)
        .unwrap_or(false)
}

fn snapshot(ep: Option<&IAudioEndpointVolume>, lang: String, net: bool, audio: String) -> Snap {
    let (volume, muted) = ep
        .and_then(|e| unsafe {
            let v = e.GetMasterVolumeLevelScalar().ok()?;
            let m = e.GetMute().ok()?;
            Some(((v * 100.0).round() as u32, m.as_bool()))
        })
        .unwrap_or((0, false));

    let caps = unsafe { GetKeyState(VK_CAPITAL.0 as i32) & 1 == 1 };

    let mut ps = unsafe { std::mem::zeroed::<SYSTEM_POWER_STATUS>() };
    let mut battery = None;
    let mut plugged = false;
    if unsafe { GetSystemPowerStatus(&mut ps) }.is_ok() {
        plugged = ps.ACLineStatus == 1;
        // BatteryFlag 128 = no system battery, 255 = unknown percentage.
        if ps.BatteryFlag & 128 == 0 && ps.BatteryLifePercent != 255 {
            battery = Some(ps.BatteryLifePercent as u32);
        }
    }

    Snap {
        volume,
        muted,
        caps,
        battery,
        plugged,
        lang,
        net,
        audio,
    }
}

struct Brightness {
    svc: Option<windows::Win32::System::Wmi::IWbemServices>,
    dead: bool,
}

impl Brightness {
    fn connect() -> Option<windows::Win32::System::Wmi::IWbemServices> {
        use windows::core::BSTR;
        use windows::Win32::System::Com::{
            CoSetProxyBlanket, EOAC_NONE, RPC_C_AUTHN_LEVEL_CALL, RPC_C_IMP_LEVEL_IMPERSONATE,
        };
        use windows::Win32::System::Wmi::{IWbemContext, IWbemLocator, WbemLocator};
        unsafe {
            let loc: IWbemLocator = CoCreateInstance(&WbemLocator, None, CLSCTX_ALL).ok()?;
            let svc = loc
                .ConnectServer(
                    &BSTR::from("ROOT\\WMI"),
                    &BSTR::new(),
                    &BSTR::new(),
                    &BSTR::new(),
                    0,
                    &BSTR::new(),
                    None::<&IWbemContext>,
                )
                .ok()?;
            let _ = CoSetProxyBlanket(
                &svc,
                10, // RPC_C_AUTHN_WINNT
                0,  // RPC_C_AUTHZ_NONE
                windows::core::PCWSTR::null(),
                RPC_C_AUTHN_LEVEL_CALL,
                RPC_C_IMP_LEVEL_IMPERSONATE,
                None,
                EOAC_NONE,
            );
            Some(svc)
        }
    }

    /// Current panel brightness 0..=100, or None when the display has no WMI brightness
    /// (desktop monitors): then it stays disabled for the rest of the session.
    fn read(&mut self) -> Option<u32> {
        use windows::core::BSTR;
        use windows::Win32::System::Variant::{VariantClear, VariantToUInt32, VARIANT};
        use windows::Win32::System::Wmi::{
            IWbemClassObject, IWbemContext, WBEM_FLAG_FORWARD_ONLY, WBEM_FLAG_RETURN_IMMEDIATELY,
        };
        if self.dead {
            return None;
        }
        if self.svc.is_none() {
            self.svc = Self::connect();
        }
        let svc = self.svc.as_ref()?;
        let got = unsafe {
            (|| {
                let en = svc
                    .ExecQuery(
                        &BSTR::from("WQL"),
                        &BSTR::from(
                            "SELECT CurrentBrightness FROM WmiMonitorBrightness WHERE Active=TRUE",
                        ),
                        WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY,
                        None::<&IWbemContext>,
                    )
                    .ok()?;
                let mut objs: [Option<IWbemClassObject>; 1] = [None];
                let mut n = 0u32;
                let _ = en.Next(2000, &mut objs, &mut n);
                if n == 0 {
                    return None;
                }
                let obj = objs[0].take()?;
                let mut v = VARIANT::default();
                obj.Get(windows::core::w!("CurrentBrightness"), 0, &mut v, None, None)
                    .ok()?;
                let out = VariantToUInt32(&v).ok();
                let _ = VariantClear(&mut v);
                out
            })()
        };
        if got.is_none() {
            self.dead = true;
            conf::log("brightness tidak tersedia");
        }
        got.map(|b| b.min(100))
    }
}

fn bt_connected() -> Option<Vec<String>> {
    use windows::Devices::Bluetooth::{BluetoothConnectionStatus, BluetoothDevice};
    use windows::Devices::Enumeration::DeviceInformation;
    let sel =
        BluetoothDevice::GetDeviceSelectorFromConnectionStatus(BluetoothConnectionStatus::Connected)
            .ok()?;
    let list = DeviceInformation::FindAllAsyncAqsFilter(&sel).ok()?.join().ok()?;
    let mut names = Vec::new();
    for i in 0..list.Size().ok()? {
        if let Ok(n) = list.GetAt(i).and_then(|d| d.Name()) {
            let n = n.to_string();
            if !n.is_empty() {
                names.push(n);
            }
        }
    }
    names.sort();
    names.dedup();
    Some(names)
}

pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        com_init();
        let mut ep_pair = endpoint();
        let mut last_ep = Instant::now();
        let mut last_sys = Instant::now();
        let mut cached_audio = ep_pair
            .as_ref()
            .map(|(dev, _)| device_name(dev))
            .unwrap_or_default();
        let mut cached_net = current_net();
        let mut bright = Brightness { svc: None, dead: false };
        let mut prev_bright: Option<u32> = None;
        let mut bt_prev: Option<Vec<String>> = None;
        let mut iter: u64 = 0;
        let mut prev: Option<Snap> = None;
        loop {
            iter += 1;
            if ep_pair.is_none() || last_ep.elapsed() >= Duration::from_secs(5) {
                ep_pair = endpoint();
                last_ep = Instant::now();
                cached_audio = ep_pair
                    .as_ref()
                    .map(|(dev, _)| device_name(dev))
                    .unwrap_or_default();
            }
            if iter % 10 == 0 {
                cached_net = current_net();
            }
            if iter % 3 == 0 {
                let now = bright.read();
                if let (Some(a), Some(b)) = (prev_bright, now) {
                    if a != b {
                        let _ = app.emit(
                            "hud",
                            serde_json::json!({ "kind": "brightness", "value": b }),
                        );
                    }
                }
                if now.is_some() {
                    prev_bright = now;
                }
            }
            if iter % 25 == 0 {
                if let Some(now) = bt_connected() {
                    if let Some(old) = &bt_prev {
                        for n in now.iter().filter(|n| !old.contains(n)) {
                            let _ = app.emit(
                                "hud",
                                serde_json::json!({ "kind": "bt", "name": n, "on": true }),
                            );
                        }
                        for n in old.iter().filter(|n| !now.contains(n)) {
                            let _ = app.emit(
                                "hud",
                                serde_json::json!({ "kind": "bt", "name": n, "on": false }),
                            );
                        }
                    }
                    bt_prev = Some(now);
                }
            }
            let lang = current_lang();
            let s = snapshot(
                ep_pair.as_ref().map(|(_, vol)| vol),
                lang,
                cached_net,
                cached_audio.clone(),
            );

            if let Some(p) = &prev {
                if p.volume != s.volume || p.muted != s.muted {
                    let _ = app.emit(
                        "hud",
                        serde_json::json!({ "kind": "volume", "value": s.volume, "muted": s.muted }),
                    );
                }
                if p.caps != s.caps {
                    let _ = app.emit("hud", serde_json::json!({ "kind": "caps", "on": s.caps }));
                }
                let crossed = |a: Option<u32>, b: Option<u32>| match (a, b) {
                    (Some(a), Some(b)) => a > b && ((a > 20 && b <= 20) || (a > 10 && b <= 10)),
                    _ => false,
                };
                if p.plugged != s.plugged || crossed(p.battery, s.battery) {
                    let _ = app.emit(
                        "hud",
                        serde_json::json!({
                            "kind": "power", "plugged": s.plugged, "battery": s.battery,
                        }),
                    );
                }
                if p.lang != s.lang && !p.lang.is_empty() {
                    let _ = app.emit(
                        "hud",
                        serde_json::json!({ "kind": "lang", "value": s.lang }),
                    );
                }
                if p.net != s.net {
                    let _ = app.emit(
                        "hud",
                        serde_json::json!({ "kind": "net", "online": s.net }),
                    );
                }
                if p.audio != s.audio && !p.audio.is_empty() && !s.audio.is_empty() {
                    let _ = app.emit(
                        "hud",
                        serde_json::json!({ "kind": "audio", "name": s.audio }),
                    );
                }
            }

            if last_sys.elapsed() >= Duration::from_secs(30) {
                last_sys = Instant::now();
                let _ = app.emit("sys", s.json());
            }
            prev = Some(s);
            std::thread::sleep(Duration::from_millis(200));
        }
    });
}

#[tauri::command]
pub fn sys_status() -> serde_json::Value {
    com_init();
    let pair = endpoint();
    let audio = pair
        .as_ref()
        .map(|(dev, _)| device_name(dev))
        .unwrap_or_default();
    snapshot(
        pair.as_ref().map(|(_, vol)| vol),
        current_lang(),
        current_net(),
        audio,
    )
    .json()
}

#[tauri::command]
pub fn volume(dir: String, steps: Option<u32>, level: Option<u32>) -> Result<(), String> {
    com_init();
    let pair = endpoint().ok_or("audio tidak tersedia")?;
    let ep = &pair.1;
    let nil = std::ptr::null();
    unsafe {
        match dir.as_str() {
            "up" | "down" => {
                let cur = ep.GetMasterVolumeLevelScalar().unwrap_or(0.0);
                let step = steps.unwrap_or(5).clamp(1, 50) as f32 * 0.02;
                let v = if dir == "up" {
                    (cur + step).min(1.0)
                } else {
                    (cur - step).max(0.0)
                };
                ep.SetMasterVolumeLevelScalar(v, nil)
                    .map_err(|e| e.to_string())?;
                let _ = ep.SetMute(false, nil);
            }
            "mute" => {
                let muted = ep.GetMute().map_err(|e| e.to_string())?.as_bool();
                ep.SetMute(!muted, nil).map_err(|e| e.to_string())?;
            }
            "set" => {
                let v = level.unwrap_or(0).min(100) as f32 / 100.0;
                ep.SetMasterVolumeLevelScalar(v, nil)
                    .map_err(|e| e.to_string())?;
                let _ = ep.SetMute(false, nil);
            }
            _ => return Err("perintah volume tidak dikenal".into()),
        }
    }
    Ok(())
}

fn valid_file(p: &str) -> bool {
    let p = Path::new(p);
    p.is_absolute() && p.exists()
}

#[tauri::command]
pub fn shelf_info(paths: Vec<String>) -> Vec<serde_json::Value> {
    paths
        .iter()
        .filter_map(|p| {
            let md = std::fs::metadata(p).ok()?;
            let path = Path::new(p);
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| p.clone());
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            Some(serde_json::json!({
                "path": p,
                "name": name,
                "size": if md.is_dir() { 0 } else { md.len() },
                "dir": md.is_dir(),
                "ext": ext,
            }))
        })
        .collect()
}

#[tauri::command]
pub fn open_path(path: String) -> Result<(), String> {
    if !valid_file(&path) {
        return Err("file tidak ditemukan".into());
    }
    std::process::Command::new("explorer")
        .arg(&path)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn shelf_from_clipboard() -> Result<Vec<serde_json::Value>, String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    let paths = cb
        .get()
        .file_list()
        .map_err(|_| "clipboard tidak berisi file".to_string())?;
    if paths.is_empty() {
        return Err("clipboard tidak berisi file".into());
    }
    let paths_str: Vec<String> = paths
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    Ok(shelf_info(paths_str))
}

#[tauri::command]
pub fn clipboard_text() -> Result<String, String> {
    let t = arboard::Clipboard::new()
        .map_err(|e| e.to_string())?
        .get_text()
        .map_err(|_| "clipboard tidak berisi teks".to_string())?;
    let t = t.trim();
    if t.is_empty() {
        return Err("clipboard kosong".into());
    }
    Ok(t.chars().take(4000).collect())
}

#[tauri::command]
pub fn shelf_copy(paths: Vec<String>) -> Result<(), String> {
    if paths.is_empty() {
        return Err("tidak ada file untuk disalin".into());
    }
    for p in &paths {
        if !valid_file(p) {
            return Err("file tidak valid".into());
        }
    }
    let path_bufs: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    cb.set().file_list(&path_bufs).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reveal_path(path: String) -> Result<(), String> {
    if !valid_file(&path) {
        return Err("file tidak ditemukan".into());
    }
    std::process::Command::new("explorer")
        .arg(format!("/select,{path}"))
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn shelf_drag_out(app: AppHandle, paths: Vec<String>) -> Result<(), String> {
    if paths.is_empty() {
        return Err("tidak ada file untuk ditarik".into());
    }
    for p in &paths {
        if !valid_file(p) {
            return Err(format!("file tidak valid: {p}"));
        }
    }

    let icon_path = std::env::temp_dir().join("blip-drag.png");
    if !icon_path.exists() {
        let _ = std::fs::write(&icon_path, include_bytes!("../icons/64x64.png"));
    }

    let win = app.get_webview_window("main").ok_or("window main tidak ada")?;
    let app2 = app.clone();
    let app3 = app.clone();

    app.run_on_main_thread(move || {
        let file_paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
        let r = drag::start_drag(
            &win,
            drag::DragItem::Files(file_paths),
            drag::Image::File(icon_path),
            move |res, _| {
                conf::log(if matches!(res, drag::DragResult::Dropped) {
                    "drag keluar: dilepas"
                } else {
                    "drag keluar: batal"
                });
                let _ = app2.emit("shelf:drag-end", matches!(res, drag::DragResult::Dropped));
            },
            drag::Options::default(),
        );
        if let Err(e) = r {
            let _ = app3.emit("shelf:drag-end", false);
            conf::log(&format!("drag keluar gagal: {e}"));
        }
    })
    .map_err(|e| e.to_string())
}
