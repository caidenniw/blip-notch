use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;

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
}

impl Default for Ui {
    fn default() -> Self {
        Self { port: 51321, hold_ms: 300, auto_collapse_ms: 5000 }
    }
}

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

static CONF: LazyLock<Conf> = LazyLock::new(Conf::read);

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

pub fn get() -> &'static Conf {
    &CONF
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
