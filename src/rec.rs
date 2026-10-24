use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use crate::bus::{self, DictatePhase, Event};

use crate::{ai, conf};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

static REC: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

const SPEECH_RMS: f32 = 0.018;
const SILENCE_MS: u64 = 1300;
const NO_SPEECH_MS: u64 = 8000;

const WARMUP_FRAMES: u32 = 2; // mic start-up frames read ~0, ignore
const CALIB_FRAMES: u32 = 8; // frames 3..=8 (300 ms) set the noise floor
const ONSET_FRAMES: u32 = 3; // 150 ms above threshold = speech, so clicks/taps do not count

#[derive(Default)]
struct Vad {
    n: u32,
    floor: f32,
    peak: f32,
    run: u32,
    onset: bool,
    quiet_since: Option<u64>,
}

impl Vad {
    /// Feed one 50 ms RMS frame. True when the utterance is over.
    fn feed(&mut self, rms: f32, now_ms: u64) -> bool {
        self.n += 1;
        if self.n <= WARMUP_FRAMES {
            return false;
        }
        if self.n <= CALIB_FRAMES {
            self.floor = if self.n == WARMUP_FRAMES + 1 { rms } else { self.floor.min(rms) };
            self.floor = self.floor.clamp(0.002, 0.04);
            return false;
        }
        let start = SPEECH_RMS.max(self.floor * 2.5);
        if !self.onset {
            if rms > start {
                self.run += 1;
            } else {
                self.run = 0;
                // follow slow noise drift (fan spinning up/down) while nobody talks
                self.floor = (self.floor + (rms - self.floor) * 0.05).clamp(0.002, 0.04);
            }
            if self.run >= ONSET_FRAMES {
                self.onset = true;
                self.peak = rms;
            }
            return false;
        }
        self.peak = self.peak.max(rms);
        // speech ends when the signal falls back near the noise floor or well under the loudest speech
        let end = (self.floor * 1.8).max(0.012).max(self.peak * 0.25);
        if rms > end {
            self.quiet_since = None;
            return false;
        }
        let t = *self.quiet_since.get_or_insert(now_ms);
        now_ms - t > SILENCE_MS
    }
}

pub fn start(mode: &str) -> Result<(), String> {
    let mut guard = REC.lock().unwrap();
    if guard.is_some() {
        return Err("sudah merekam".into());
    }
    let flag = Arc::new(AtomicBool::new(false));
    *guard = Some(flag.clone());
    drop(guard);
    let mode = mode.to_string();
    std::thread::spawn(move || capture(mode, flag));
    Ok(())
}

pub fn stop() {
    if let Some(f) = REC.lock().unwrap().as_ref() {
        f.store(true, Ordering::Relaxed);
    }
}

pub fn rec_start(mode: String) -> Result<(), String> {
    start(&mode)
}

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

fn capture(mode: String, stop_flag: Arc<AtomicBool>) {
    let host = cpal::default_host();
    let Some(device) = host.default_input_device() else {
        finish_error("mic tidak bisa dipakai: tidak ada perangkat input");
        return;
    };
    let supported = match device.default_input_config() {
        Ok(c) => c,
        Err(e) => {
            finish_error(&format!("mic tidak bisa dipakai: {e}"));
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
            finish_error(&format!("format mic tidak didukung: {other:?}"));
            return;
        }
    };
    let stream = match stream {
        Ok(s) => s,
        Err(e) => {
            finish_error(&format!("mic tidak bisa dipakai: {e}"));
            return;
        }
    };
    if let Err(e) = stream.play() {
        finish_error(&format!("mic tidak bisa dipakai: {e}"));
        return;
    }

    let started = Instant::now();
    let mut vad = Vad::default();
    loop {
        if stop_flag.load(Ordering::Relaxed) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
        let rms = f32::from_bits(level.load(Ordering::Relaxed));
        bus::post(Event::RecLevel(rms));
        let elapsed = started.elapsed().as_millis() as u64;
        if mode == "chat" && (vad.feed(rms, elapsed) || (!vad.onset && elapsed > NO_SPEECH_MS)) {
            break;
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
    if duration < 0.4 || (mode == "chat" && !vad.onset) {
        bus::post(Event::SttEmpty);
        return;
    }

    let pcm = resample(&samples, rate, 16_000);
    let wav = wav_bytes(&pcm, 16_000);
    conf::log(&format!("rec {mode} {} samples {duration:.1}s floor {:.3}", samples.len(), vad.floor));

    match crate::rt::block_on(ai::stt(wav)) {
        Ok(text) if !text.trim().is_empty() => {
            let text = text.trim().to_string();
            if mode == "chat" {
                bus::post(Event::SttResult(text));
            } else {
                paste(&text);
                bus::post(Event::Dictate(DictatePhase::Done));
            }
        }
        Ok(_) => {
            bus::post(Event::SttEmpty);
        }
        Err(e) => {
            bus::post(Event::SttError(e));
        }
    }
}

fn finish_error(msg: &str) {
    *REC.lock().unwrap() = None;
    conf::log(msg);
    bus::post(Event::SttError(msg.to_string()));
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

#[cfg(test)]
mod tests {
    use super::Vad;

    fn run(v: &mut Vad, rms: f32, ms: u64, t: &mut u64) -> bool {
        let mut end = false;
        for _ in 0..ms / 50 {
            *t += 50;
            end |= v.feed(rms, *t);
        }
        end
    }

    #[test]
    fn steady_fan_never_starts() {
        let (mut v, mut t) = (Vad::default(), 0);
        assert!(!run(&mut v, 0.03, 10_000, &mut t));
        assert!(!v.onset);
    }

    #[test]
    fn speech_over_fan_ends_after_silence() {
        let (mut v, mut t) = (Vad::default(), 0);
        run(&mut v, 0.03, 1000, &mut t);
        assert!(!v.onset);
        run(&mut v, 0.12, 1000, &mut t);
        assert!(v.onset);
        assert!(!run(&mut v, 0.03, 1000, &mut t));
        assert!(run(&mut v, 0.03, 500, &mut t));
    }

    #[test]
    fn click_is_not_speech() {
        let (mut v, mut t) = (Vad::default(), 0);
        run(&mut v, 0.004, 1000, &mut t);
        run(&mut v, 0.2, 50, &mut t);
        run(&mut v, 0.004, 1000, &mut t);
        assert!(!v.onset);
    }

    #[test]
    fn quiet_room_speech_starts() {
        let (mut v, mut t) = (Vad::default(), 0);
        run(&mut v, 0.004, 1000, &mut t);
        run(&mut v, 0.05, 500, &mut t);
        assert!(v.onset);
    }
}
