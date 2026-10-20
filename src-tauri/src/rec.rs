use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

use crate::{ai, conf};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

static REC: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

const SPEECH_RMS: f32 = 0.018;
const SILENCE_MS: u64 = 1300;

pub fn start(app: AppHandle, mode: &str) -> Result<(), String> {
    let mut guard = REC.lock().unwrap();
    if guard.is_some() {
        return Err("sudah merekam".into());
    }
    let flag = Arc::new(AtomicBool::new(false));
    *guard = Some(flag.clone());
    drop(guard);
    let mode = mode.to_string();
    std::thread::spawn(move || capture(app, mode, flag));
    Ok(())
}

pub fn stop() {
    if let Some(f) = REC.lock().unwrap().as_ref() {
        f.store(true, Ordering::Relaxed);
    }
}

#[tauri::command]
pub fn rec_start(app: AppHandle, mode: String) -> Result<(), String> {
    start(app, &mode)
}

#[tauri::command]
pub fn rec_stop() {
    stop();
}

trait ToF32 {
    fn to_f32(self) -> f32;
}
impl ToF32 for f32 {
    fn to_f32(self) -> f32 {
        self
    }
}
impl ToF32 for i16 {
    fn to_f32(self) -> f32 {
        self as f32 / 32768.0
    }
}
impl ToF32 for u16 {
    fn to_f32(self) -> f32 {
        (self as f32 - 32768.0) / 32768.0
    }
}

fn build<T>(
    device: &cpal::Device,
    cfg: &cpal::StreamConfig,
    buf: Arc<Mutex<Vec<f32>>>,
    level: Arc<AtomicU32>,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: cpal::SizedSample + ToF32 + Send + 'static,
{
    let ch = cfg.channels as usize;
    device.build_input_stream(
        cfg,
        move |data: &[T], _| {
            let mut b = buf.lock().unwrap();
            let mut sum = 0f64;
            let frames = data.len() / ch.max(1);
            for frame in data.chunks(ch.max(1)) {
                let mut acc = 0f32;
                for s in frame {
                    acc += (*s).to_f32();
                }
                let v = acc / ch.max(1) as f32;
                b.push(v);
                sum += (v as f64) * (v as f64);
            }
            let rms = (sum / frames.max(1) as f64).sqrt() as f32;
            level.store(rms.to_bits(), Ordering::Relaxed);
        },
        |e| conf::log(&format!("mic stream error: {e}")),
        None,
    )
}

fn capture(app: AppHandle, mode: String, stop_flag: Arc<AtomicBool>) {
    let host = cpal::default_host();
    let Some(device) = host.default_input_device() else {
        finish_error(&app, "mic tidak bisa dipakai: tidak ada perangkat input");
        return;
    };
    let supported = match device.default_input_config() {
        Ok(c) => c,
        Err(e) => {
            finish_error(&app, &format!("mic tidak bisa dipakai: {e}"));
            return;
        }
    };
    let cfg: cpal::StreamConfig = supported.config();
    let rate = cfg.sample_rate.0;
    let buf = Arc::new(Mutex::new(Vec::<f32>::new()));
    let level = Arc::new(AtomicU32::new(0));

    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => build::<f32>(&device, &cfg, buf.clone(), level.clone()),
        cpal::SampleFormat::I16 => build::<i16>(&device, &cfg, buf.clone(), level.clone()),
        cpal::SampleFormat::U16 => build::<u16>(&device, &cfg, buf.clone(), level.clone()),
        other => {
            finish_error(&app, &format!("format mic tidak didukung: {other:?}"));
            return;
        }
    };
    let stream = match stream {
        Ok(s) => s,
        Err(e) => {
            finish_error(&app, &format!("mic tidak bisa dipakai: {e}"));
            return;
        }
    };
    if let Err(e) = stream.play() {
        finish_error(&app, &format!("mic tidak bisa dipakai: {e}"));
        return;
    }

    let started = Instant::now();
    let mut onset = false;
    let mut silence_since: u64 = 0;
    loop {
        if stop_flag.load(Ordering::Relaxed) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
        let rms = f32::from_bits(level.load(Ordering::Relaxed));
        let _ = app.emit("rec:level", rms);
        let elapsed = started.elapsed().as_millis() as u64;
        if mode == "chat" {
            if rms > SPEECH_RMS {
                onset = true;
                silence_since = 0;
            } else if onset {
                if silence_since == 0 {
                    silence_since = elapsed;
                } else if elapsed - silence_since > SILENCE_MS {
                    break;
                }
            }
        }
        let limit_ms = if mode == "chat" { 30_000 } else { 120_000 };
        if elapsed > limit_ms {
            break;
        }
    }
    drop(stream);

    let samples = std::mem::take(&mut *buf.lock().unwrap());
    *REC.lock().unwrap() = None;

    let duration = samples.len() as f32 / rate.max(1) as f32;
    if duration < 0.4 || (mode == "chat" && !onset) {
        let _ = app.emit("stt:empty", ());
        return;
    }

    let pcm = resample(&samples, rate, 16_000);
    let wav = wav_bytes(&pcm, 16_000);
    conf::log(&format!("rec {mode} {} samples {duration:.1}s", samples.len()));

    match tauri::async_runtime::block_on(ai::stt(wav)) {
        Ok(text) if !text.trim().is_empty() => {
            let text = text.trim().to_string();
            if mode == "chat" {
                let _ = app.emit("stt:result", serde_json::json!({ "text": text }));
            } else {
                paste(&text);
                let _ = app.emit("dictate", "done");
            }
        }
        Ok(_) => {
            let _ = app.emit("stt:empty", ());
        }
        Err(e) => {
            let _ = app.emit("stt:error", serde_json::json!({ "error": e }));
        }
    }
}

fn finish_error(app: &AppHandle, msg: &str) {
    *REC.lock().unwrap() = None;
    conf::log(msg);
    let _ = app.emit("stt:error", serde_json::json!({ "error": msg }));
}

fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to {
        return input.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let n = (input.len() as f64 / ratio) as usize;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let pos = i as f64 * ratio;
        let idx = pos.floor() as usize;
        let frac = (pos - idx as f64) as f32;
        let a = input.get(idx).copied().unwrap_or(0.0);
        let b = input.get(idx + 1).copied().unwrap_or(a);
        out.push(a + (b - a) * frac);
    }
    out
}

fn wav_bytes(pcm: &[f32], rate: u32) -> Vec<u8> {
    let data_len = pcm.len() * 2;
    let mut v = Vec::with_capacity(44 + data_len);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&((36 + data_len) as u32).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&rate.to_le_bytes());
    v.extend_from_slice(&(rate * 2).to_le_bytes());
    v.extend_from_slice(&2u16.to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&(data_len as u32).to_le_bytes());
    for s in pcm {
        let x = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        v.extend_from_slice(&x.to_le_bytes());
    }
    v
}

fn paste(text: &str) {
    let mut clip = match arboard::Clipboard::new() {
        Ok(c) => c,
        Err(e) => {
            conf::log(&format!("clipboard gagal: {e}"));
            return;
        }
    };
    let old = clip.get_text().ok();
    if clip.set_text(text.to_string()).is_err() {
        conf::log("clipboard set gagal");
        return;
    }
    send_ctrl_v();
    std::thread::sleep(Duration::from_millis(350));
    if let Some(old) = old {
        let _ = clip.set_text(old);
    }
}

fn send_ctrl_v() {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY,
        VK_CONTROL, VK_V,
    };
    let key = |vk: VIRTUAL_KEY, up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if up { KEYEVENTF_KEYUP } else { Default::default() },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let inputs = [
        key(VK_CONTROL, false),
        key(VK_V, false),
        key(VK_V, true),
        key(VK_CONTROL, true),
    ];
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}
