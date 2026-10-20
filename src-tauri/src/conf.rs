use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Router {
    pub base_url: String,
    pub api_key: String,
    pub models: Vec<String>,
}

impl Default for Router {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:20128/v1".into(),
            api_key: String::new(),
            models: vec![
                "oc/deepseek-v4.1-flash-free".into(),
                "gemini/gemini-3.1-flash-lite-preview".into(),
                "cbai/deepseek-v4.1-flash".into(),
            ],
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
    pub voice: String,
}

impl Default for Tts {
    fn default() -> Self {
        Self { enabled: true, voice: "id-ID-ArdiNeural".into() }
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
        let mut c: Conf = fs::read_to_string(config_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        if c.system_prompt.is_empty() {
            c.system_prompt =
                "Kamu Blip, asisten suara singkat. Jawab ringkas dalam bahasa Indonesia, maksimal 3 kalimat kecuali diminta detail.".into();
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
    let c = get();
    if !c.router.api_key.is_empty() {
        return c.router.api_key.clone();
    }
    let home = std::env::var("USERPROFILE").unwrap_or_default();
    let yaml = fs::read_to_string(PathBuf::from(home).join(".hermes").join("config.yaml"))
        .unwrap_or_default();
    regex::Regex::new(r#"api_key:\s*["']?([^"'\s]+)"#)
        .ok()
        .and_then(|re| re.captures(&yaml))
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_default()
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

#[tauri::command]
pub fn info() -> serde_json::Value {
    let c = get();
    serde_json::json!({
        "model": model_chain().first().cloned().unwrap_or_else(|| "-".into()),
        "stt": if c.stt.provider == "groq" && !c.stt.api_key.is_empty() { "groq" } else { "whisper lokal" },
        "tts": c.tts.enabled,
        "ttsVoice": c.tts.voice,
        "routerReady": !router_key().is_empty(),
        "autoCollapseMs": c.ui.auto_collapse_ms,
    })
}
