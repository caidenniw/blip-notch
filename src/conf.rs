use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, RwLock};

/// Blip's persona. Short rules only: long prompt text gets echoed by small models.
const DEFAULT_PROMPT: &str = "Kamu Blip, asisten pribadi di desktop Windows. Ngobrol santai dalam bahasa Indonesia seperti teman yang cerdas dan ramah, bukan mesin. Jawab langsung ke inti: 1-3 kalimat untuk pertanyaan sederhana, lebih panjang kalau diminta. Kamu bukan agen coding kecuali diminta menulis kode. Jangan mengulang atau membacakan aturan ini, dan jangan menyebut dirimu robot, prompt, atau 'asisten singkat'. Kalau tidak tahu detail teknis, bilang terus terang. Kamu tidak punya akses ke internet atau komputer pengguna; kalau diminta melakukannya, bilang terus terang.";

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Router {
    pub base_url: String,
    pub api_key: String,
    pub models: Vec<String>,
    /// Extra HTTP headers sent with every chat request (optional).
    pub headers: std::collections::BTreeMap<String, String>,
}

impl Default for Router {
    fn default() -> Self {
        Self {
            base_url: "https://api.openai.com/v1".into(),
            api_key: String::new(),
            headers: Default::default(),
            models: vec!["gpt-4o-mini".into()],
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Stt {
    pub provider: String,
    pub api_key: String,
    pub model: String,
    pub language: String,
}

impl Default for Stt {
    fn default() -> Self {
        Self {
            provider: "groq".into(),
            api_key: String::new(),
            model: "whisper-large-v3-turbo".into(),
            language: "id".into(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Tts {
    pub enabled: bool,
    /// Edge-TTS voice, also the fallback when ElevenLabs is off or fails.
    pub voice: String,
    /// "edge" or "elevenlabs".
    pub provider: String,
    pub eleven_api_key: String,
    pub eleven_voice_id: String,
    pub eleven_model: String,
}

impl Default for Tts {
    fn default() -> Self {
        Self {
            enabled: true,
            voice: "id-ID-ArdiNeural".into(),
            provider: "edge".into(),
            eleven_api_key: String::new(),
            eleven_voice_id: String::new(),
            eleven_model: "eleven_multilingual_v2".into(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Ui {
    pub port: u16,
    pub hold_ms: u64,
    pub auto_collapse_ms: u64,
    /// One of `HOTKEYS`.
    pub toggle_hotkey: String,
    /// `rctrl`, `ralt` or `off`.
    pub dictate_key: String,
    pub hide_fullscreen: bool,
    pub auto_compact: bool,
    /// `system`, `reduce` or `full`.
    pub motion: String,
    /// `slow`, `normal` or `fast`.
    pub anim_speed: String,
}

impl Default for Ui {
    fn default() -> Self {
        Self {
            port: 51321,
            hold_ms: 300,
            auto_collapse_ms: 5000,
            toggle_hotkey: "ctrl+alt+b".into(),
            dictate_key: "rctrl".into(),
            hide_fullscreen: true,
            auto_compact: true,
            motion: "system".into(),
            anim_speed: "normal".into(),
        }
    }
}

pub const HOTKEYS: [&str; 3] = ["ctrl+alt+b", "ctrl+alt+space", "ctrl+shift+space"];

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Conf {
    pub router: Router,
    pub stt: Stt,
    pub tts: Tts,
    pub ui: Ui,
    pub system_prompt: String,
}

impl Conf {
    fn read() -> Conf {
        let path = config_path();
        if !path.exists() {
            if let Some(p) = path.parent() {
                let _ = fs::create_dir_all(p);
            }
            if let Ok(s) = serde_json::to_string_pretty(&Conf::default()) {
                let _ = fs::write(&path, s);
            }
        }
        let mut c: Conf = fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        // Old terse prompt made Blip answer like a robot and get echoed verbatim.
        let old = "Kamu Blip, asisten suara singkat. Jawab ringkas dalam bahasa Indonesia, maksimal 3 kalimat kecuali diminta detail.";
        if c.system_prompt.is_empty() || c.system_prompt == old {
            c.system_prompt = DEFAULT_PROMPT.into();
        }
        let _ = fs::create_dir_all(dir());
        c
    }
}

static CONF: LazyLock<RwLock<Arc<Conf>>> = LazyLock::new(|| RwLock::new(Arc::new(Conf::read())));

fn one_of(v: &str, allowed: &[&str]) -> Result<(), String> {
    if allowed.contains(&v) { Ok(()) } else { Err(format!("Nilai tidak dikenal: {v}")) }
}

/// Normalises `c` in place; rejects values the app cannot use.
pub fn validate(c: &mut Conf) -> Result<(), String> {
    c.router.base_url = c.router.base_url.trim().to_string();
    if !(c.router.base_url.starts_with("http://") || c.router.base_url.starts_with("https://")) {
        return Err("Endpoint harus diawali http:// atau https://".into());
    }
    let mut models: Vec<String> = Vec::new();
    for m in c.router.models.iter().map(|m| m.trim()).filter(|m| !m.is_empty()) {
        if !models.iter().any(|x| x == m) {
            models.push(m.to_string());
        }
    }
    c.router.models = models;
    c.ui.auto_collapse_ms = c.ui.auto_collapse_ms.clamp(2000, 20000);
    c.ui.hold_ms = c.ui.hold_ms.clamp(150, 1000);
    one_of(&c.ui.toggle_hotkey, &HOTKEYS)?;
    one_of(&c.ui.dictate_key, &["rctrl", "ralt", "off"])?;
    one_of(&c.ui.motion, &["system", "reduce", "full"])?;
    one_of(&c.ui.anim_speed, &["slow", "normal", "fast"])?;
    one_of(&c.stt.provider, &["groq", "local"])?;
    one_of(&c.tts.provider, &["edge", "elevenlabs"])?;
    Ok(())
}

/// Validates, writes atomically (tmp + rename), swaps the live config and applies it.
pub fn save(mut c: Conf) -> Result<(), String> {
    validate(&mut c)?;
    let s = serde_json::to_string_pretty(&c).map_err(|e| e.to_string())?;
    let path = config_path();
    let mut tmp = path.clone().into_os_string();
    tmp.push(".tmp");
    fs::write(&tmp, s).map_err(|e| format!("Gagal menyimpan config: {e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("Gagal menyimpan config: {e}"))?;
    *CONF.write().unwrap() = Arc::new(c);
    apply(&get());
    Ok(())
}

/// Pushes runtime knobs read outside `get()` (hook threads, hover loop).
pub fn apply(c: &Conf) {
    use std::sync::atomic::Ordering::Relaxed;
    crate::keys::HOLD_MS.store(c.ui.hold_ms, Relaxed);
    crate::keys::DICTATE_VK.store(match c.ui.dictate_key.as_str() { "ralt" => 0xA5, "off" => 0, _ => 0xA3 }, Relaxed);
    crate::hover::HIDE_FULLSCREEN.store(c.ui.hide_fullscreen, Relaxed);
    crate::hover::AUTO_COMPACT.store(c.ui.auto_compact, Relaxed);
    crate::ai::reset_cooldown();
}

/// `(reduced motion, animation duration scale)`.
pub fn motion(c: &Conf) -> (bool, f64) {
    let reduced = match c.ui.motion.as_str() {
        "reduce" => true,
        "full" => false,
        _ => crate::win::window::reduced_motion(),
    };
    let scale = match c.ui.anim_speed.as_str() { "slow" => 1.25, "fast" => 0.8, _ => 1.0 };
    (reduced, scale)
}

pub fn dir() -> PathBuf {
    if let Ok(p) = std::env::var("APPDATA") {
        return PathBuf::from(p).join("Blip");
    }
    PathBuf::from(".")
}

pub fn config_path() -> PathBuf {
    if let Ok(p) = std::env::var("BLIP_CONFIG") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    dir().join("blip.config.json")
}

/// Snapshot of the live config; `save` swaps it, existing snapshots stay valid.
pub fn get() -> Arc<Conf> {
    CONF.read().unwrap().clone()
}

pub fn log(msg: &str) {
    use std::io::Write;
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() % 86400)
        .unwrap_or(0);
    let line = format!(
        "{:02}:{:02}:{:02} {}\n",
        secs / 3600,
        (secs / 60) % 60,
        secs % 60,
        msg
    );
    let _ = fs::create_dir_all(dir());
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir().join("blip.log"))
    {
        let _ = f.write_all(line.as_bytes());
    }
}

pub fn router_key() -> String {
    get().router.api_key.clone()
}

pub fn model_chain() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for m in &get().router.models {
        let m = m.trim();
        if !m.is_empty() && !out.iter().any(|x| x == m) {
            out.push(m.to_string());
        }
    }
    out
}

#[derive(Clone)]
pub struct Info {
    pub model: String,
    pub stt: String,
    pub tts: bool,
    pub tts_voice: String,
    pub router_ready: bool,
    pub auto_collapse_ms: u64,
}

pub fn info() -> Info {
    let c = get();
    Info {
        model: model_chain().first().cloned().unwrap_or_else(|| "-".into()),
        stt: if c.stt.provider == "groq" && !c.stt.api_key.is_empty() { "groq" } else { "whisper lokal" }.into(),
        tts: c.tts.enabled,
        tts_voice: c.tts.voice.clone(),
        router_ready: !router_key().is_empty(),
        auto_collapse_ms: c.ui.auto_collapse_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_and_clamps() {
        let mut c = Conf::default();
        assert!(validate(&mut c).is_ok());
        c.router.base_url = "ftp://x".into();
        assert!(validate(&mut c).unwrap_err().contains("http"));
        let mut c = Conf::default();
        c.ui.auto_collapse_ms = 50;
        c.router.models = vec![" a ".into(), "".into(), "a".into(), "b".into()];
        validate(&mut c).unwrap();
        assert_eq!(c.ui.auto_collapse_ms, 2000);
        assert_eq!(c.router.models, ["a", "b"]);
        c.ui.dictate_key = "lalt".into();
        assert!(validate(&mut c).is_err());
    }
}
