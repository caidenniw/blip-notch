use crate::conf;
use futures_util::future::{AbortHandle, Abortable};
use futures_util::StreamExt;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, LazyLock, Mutex};
use tauri::{AppHandle, Emitter};

static ABORTS: Mutex<Option<HashMap<String, AbortHandle>>> = Mutex::new(None);
static SPEAK_STOP: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);
static COOLDOWN: Mutex<Option<HashMap<String, std::time::Instant>>> = Mutex::new(None);

fn aborts() -> std::sync::MutexGuard<'static, Option<HashMap<String, AbortHandle>>> {
    ABORTS.lock().unwrap()
}

#[tauri::command]
pub async fn chat_send(
    app: AppHandle,
    id: String,
    messages: Vec<serde_json::Value>,
    speak: Option<bool>,
) {
    let (handle, reg) = AbortHandle::new_pair();
    aborts().get_or_insert_with(HashMap::new).insert(id.clone(), handle);
    let app2 = app.clone();
    let id2 = id.clone();
    let fut = Abortable::new(run_chat(app2, id2, messages, speak.unwrap_or(false)), reg);
    let _ = fut.await;
    aborts().get_or_insert_with(HashMap::new).remove(&id);
}

#[tauri::command]
pub fn chat_abort(app: AppHandle, id: String) {
    stop_current();
    if let Some(h) = aborts().get_or_insert_with(HashMap::new).remove(&id) {
        h.abort();
        let _ = app.emit("chat:done", serde_json::json!({ "id": id, "aborted": true }));
    }
}

async fn run_chat(app: AppHandle, id: String, messages: Vec<serde_json::Value>, speak: bool) {
    let raw_chain = conf::model_chain();
    let mut last_err = "tidak ada model di config".to_string();
    if raw_chain.is_empty() {
        let _ = app.emit("chat:error", serde_json::json!({ "id": id, "error": last_err }));
        return;
    }
    let now = std::time::Instant::now();
    let mut filtered = Vec::new();
    {
        let mut guard = COOLDOWN.lock().unwrap();
        let map = guard.get_or_insert_with(HashMap::new);
        map.retain(|_, until| *until > now);
        for m in &raw_chain {
            if let Some(until) = map.get(m) {
                if *until > now {
                    conf::log(&format!("model dilewati {m}"));
                    continue;
                }
            }
            filtered.push(m.clone());
        }
    }
    let chain = if filtered.is_empty() { raw_chain } else { filtered };
    let mut speaker = if speak && conf::get().tts.enabled {
        Some(Speaker::new(app.clone()))
    } else {
        None
    };
    for model in chain {
        match stream_once(&app, &id, &model, &messages, &mut speaker).await {
            Ok(n) if n > 0 => {
                conf::log(&format!("chat done {id} model {model} chars {n}"));
                let _ = app.emit("chat:done", serde_json::json!({ "id": id, "model": model }));
                // After chat:done, so the player's final tts:end never beats it to the UI.
                if let Some(s) = speaker.as_mut() {
                    s.finish();
                }
                return;
            }
            Ok(_) => {
                last_err = format!("{model} balas kosong");
                conf::log(&last_err);
            } 
            Err((sent, err)) => {
                last_err = err;
                conf::log(&format!("model gagal {model}: {}", last_err.chars().take(140).collect::<String>()));
                if sent == 0 {
                    let cooldown_dur = if last_err.starts_with("HTTP 401")
                        || last_err.starts_with("HTTP 403")
                        || last_err.starts_with("HTTP 404")
                    {
                        Some(std::time::Duration::from_secs(30 * 60))
                    } else if last_err.starts_with("HTTP 429") {
                        Some(std::time::Duration::from_secs(2 * 60))
                    } else {
                        None
                    };
                    if let Some(dur) = cooldown_dur {
                        COOLDOWN
                            .lock()
                            .unwrap()
                            .get_or_insert_with(HashMap::new)
                            .insert(model.clone(), std::time::Instant::now() + dur);
                    }
                }
                if sent > 0 {
                    let _ = app.emit(
                        "chat:error",
                        serde_json::json!({ "id": id, "error": format!("terputus: {last_err}") }),
                    );
                    return;
                }
            }
        }
    }
    let _ = app.emit(
        "chat:error",
        serde_json::json!({ "id": id, "error": format!("semua model gagal. terakhir: {last_err}") }),
    );
}

async fn stream_once(
    app: &AppHandle,
    id: &str,
    model: &str,
    messages: &[serde_json::Value],
    speaker: &mut Option<Speaker>,
) -> Result<usize, (usize, String)> {
    let c = conf::get();
    let base = c.router.base_url.trim_end_matches('/').to_string();
    let mut msgs = vec![serde_json::json!({ "role": "system", "content": c.system_prompt })];
    msgs.extend_from_slice(messages);
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base}/chat/completions"))
        .header("Authorization", format!("Bearer {}", conf::router_key()))
        // 9Router injects its caveman/ponytail coding-agent prompt into every request it
        // proxies (≈800 extra prompt tokens) and the model then answers like a terse dev.
        // This header turns that off for Blip's chat only, leaving the user's setup alone.
        .header("x-9router-token-saver", "off")
        .json(&serde_json::json!({ "model": model, "stream": true, "messages": msgs }))
        .send()
        .await
        .map_err(|e| (0, e.to_string()))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err((0, format!("HTTP {status} {}", body.chars().take(160).collect::<String>())));
    }
    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    let mut n = 0usize;
    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(c) => c,
            Err(e) => return Err((n, e.to_string())),
        };
        buf.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(pos) = buf.find('\n') {
            let line: String = buf.drain(..=pos).collect();
            let line = line.trim();
            let Some(data) = line.strip_prefix("data:") else { continue };
            let data = data.trim();
            if data.is_empty() || data == "[DONE]" {
                continue;
            }
            let Ok(j) = serde_json::from_str::<serde_json::Value>(data) else { continue };
            if let Some(delta) = j["choices"][0]["delta"]["content"].as_str() {
                if !delta.is_empty() {
                    n += delta.len();
                    let _ = app.emit("chat:chunk", serde_json::json!({ "id": id, "delta": delta }));
                    if let Some(s) = speaker.as_mut() {
                        s.push(delta);
                    }
                }
            }
        }
    }
    Ok(n)
}

pub async fn stt(wav: Vec<u8>) -> Result<String, String> {
    let c = conf::get();
    if c.stt.provider == "groq" && !c.stt.api_key.is_empty() {
        let client = reqwest::Client::new();
        let part = reqwest::multipart::Part::bytes(wav)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| e.to_string())?;
        let mut form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("model", c.stt.model.clone());
        if !c.stt.language.is_empty() {
            form = form.text("language", c.stt.language.clone());
        }
        let r = client
            .post("https://api.groq.com/openai/v1/audio/transcriptions")
            .header("Authorization", format!("Bearer {}", c.stt.api_key))
            .multipart(form)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !r.status().is_success() {
            let status = r.status();
            let body = r.text().await.unwrap_or_default();
            return Err(format!("Groq {status} {}", body.chars().take(200).collect::<String>()));
        }
        let j: serde_json::Value = r.json().await.map_err(|e| e.to_string())?;
        return Ok(j["text"].as_str().unwrap_or("").trim().to_string());
    }
    local_stt(wav)
}

fn local_stt(wav: Vec<u8>) -> Result<String, String> {
    use std::process::Command;
    let tmp = std::env::temp_dir().join(format!(
        "blip-stt-{}.wav",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    std::fs::write(&tmp, &wav).map_err(|e| e.to_string())?;
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../scripts/stt_local.py");
    let out = Command::new("uv")
        .args(["run", "--with", "faster-whisper", "python"])
        .arg(&script)
        .arg(&tmp)
        .arg(&conf::get().stt.language)
        .creation_flags(0x08000000)
        .output();
    let _ = std::fs::remove_file(&tmp);
    let out = out.map_err(|e| format!("uv tidak ditemukan; isi stt.apiKey Groq atau install uv ({e})"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(format!("whisper lokal gagal: {}", err.chars().rev().take(300).collect::<String>().chars().rev().collect::<String>()))
    }
}

#[cfg(windows)]
trait NoWindow {
    fn creation_flags(&mut self, flags: u32) -> &mut Self;
}
#[cfg(windows)]
impl NoWindow for std::process::Command {
    fn creation_flags(&mut self, flags: u32) -> &mut Self {
        use std::os::windows::process::CommandExt;
        CommandExt::creation_flags(self, flags)
    }
}

// ---------- speech: speak each sentence while the answer is still streaming ----------

type Job = tauri::async_runtime::JoinHandle<Result<Vec<u8>, String>>;

/// Hard cap of spoken characters per answer (protects the ElevenLabs free quota).
const SPEAK_MAX: usize = 900;
static HTTP_TTS: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);
static TTS_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn speak_text(text: &str) -> String {
    let cleaned = regex::Regex::new(r"```[\s\S]*?```")
        .unwrap()
        .replace_all(text, " kode ");
    regex::Regex::new(r"[*_`#>]").unwrap().replace_all(&cleaned, "").into_owned()
}

/// Byte index just after the first sentence boundary that leaves a chunk of at least
/// `min` bytes. `first` additionally allows a clause break (", ") so speech starts sooner.
fn sentence_end(buf: &str, min: usize, first: bool) -> Option<usize> {
    let mut it = buf.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        let end = i + c.len_utf8();
        let boundary = match c {
            '\n' => true,
            '.' | '!' | '?' | '…' => it.peek().is_some_and(|(_, n)| n.is_whitespace()),
            _ => false,
        };
        if boundary && end >= min {
            return Some(end);
        }
    }
    if first && buf.len() >= 50 {
        if let Some(i) = buf.rfind(", ").filter(|i| *i >= 25) {
            return Some(i + 1);
        }
    }
    None
}

async fn eleven_bytes(t: &conf::Tts, text: &str, prev: &str) -> Result<Vec<u8>, String> {
    let mut body = serde_json::json!({ "text": text, "model_id": t.eleven_model });
    if !prev.is_empty() {
        body["previous_text"] = prev.into();
    }
    let resp = HTTP_TTS
        .post(format!(
            "https://api.elevenlabs.io/v1/text-to-speech/{}?output_format=mp3_44100_64",
            t.eleven_voice_id
        ))
        .header("xi-api-key", &t.eleven_api_key)
        .timeout(std::time::Duration::from_secs(20))
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let code = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("{code}: {}", body.chars().take(200).collect::<String>()));
    }
    resp.bytes().await.map(|b| b.to_vec()).map_err(|e| e.to_string())
}

async fn edge_bytes(voice: String, text: String) -> Result<Vec<u8>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        use std::process::Command;
        let path = std::env::temp_dir().join(format!(
            "blip-tts-{}-{}.mp3",
            std::process::id(),
            TTS_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let direct = std::env::var("USERPROFILE")
            .map(|h| PathBuf::from(h).join(".local").join("bin").join("edge-tts.exe"))
            .unwrap_or_default();
        let mut cmd = if direct.exists() {
            Command::new(direct)
        } else {
            let mut c = Command::new("uv");
            c.args(["tool", "run", "edge-tts"]);
            c
        };
        let res = cmd
            .args(["--voice", &voice, "--text", &text, "--write-media"])
            .arg(&path)
            .creation_flags(0x08000000)
            .output();
        let out = match res {
            Ok(o) if o.status.success() => std::fs::read(&path).map_err(|e| e.to_string()),
            Ok(o) => Err(format!(
                "edge-tts gagal: {}",
                String::from_utf8_lossy(&o.stderr).chars().rev().take(300).collect::<String>()
            )),
            Err(e) => Err(format!("edge-tts tidak bisa dijalankan: {e}")),
        };
        let _ = std::fs::remove_file(&path);
        out
    })
    .await
    .map_err(|e| e.to_string())?
}

async fn tts_bytes(text: String, prev: String) -> Result<Vec<u8>, String> {
    let t = &conf::get().tts;
    if t.provider == "elevenlabs" && !t.eleven_api_key.is_empty() && !t.eleven_voice_id.is_empty() {
        match eleven_bytes(t, &text, &prev).await {
            Ok(b) => return Ok(b),
            Err(e) => conf::log(&format!("elevenlabs gagal, pakai edge-tts: {e}")),
        }
    }
    edge_bytes(t.voice.clone(), text).await
}

/// Feeds streamed answer text in; speaks it sentence by sentence. Audio for the next
/// sentences is fetched in parallel while the current one plays.
struct Speaker {
    tx: Option<mpsc::Sender<Job>>,
    buf: String,
    prev: String,
    spoken: usize,
    first: bool,
    stop: Arc<AtomicBool>,
}

impl Speaker {
    fn new(app: AppHandle) -> Speaker {
        stop_current();
        let stop = Arc::new(AtomicBool::new(false));
        *SPEAK_STOP.lock().unwrap() = Some(stop.clone());
        let (tx, rx) = mpsc::channel::<Job>();
        let flag = stop.clone();
        std::thread::spawn(move || play_jobs(app, rx, flag));
        if conf::get().tts.provider == "elevenlabs" {
            // Open the TLS connection now so the first sentence does not pay for the handshake.
            tauri::async_runtime::spawn(async {
                let _ = HTTP_TTS.head("https://api.elevenlabs.io/").send().await;
            });
        }
        Speaker { tx: Some(tx), buf: String::new(), prev: String::new(), spoken: 0, first: true, stop }
    }

    fn push(&mut self, delta: &str) {
        self.buf.push_str(delta);
        self.drain(false);
    }

    fn finish(&mut self) {
        self.drain(true);
        self.tx = None;
    }

    fn drain(&mut self, fin: bool) {
        loop {
            // An unclosed code fence is held back; it is spoken as one " kode ".
            let open_fence = self.buf.matches("```").count() % 2 == 1;
            if open_fence {
                if !fin {
                    return;
                }
                let at = self.buf.rfind("```").unwrap_or(0);
                self.buf.truncate(at);
                self.buf.push_str(" kode ");
            }
            let min = if self.first { 15 } else { 40 };
            let cut = match sentence_end(&self.buf, min, self.first) {
                Some(c) => c,
                None if fin && !self.buf.trim().is_empty() => self.buf.len(),
                None => return,
            };
            let chunk: String = self.buf.drain(..cut).collect();
            self.say(&chunk);
        }
    }

    fn say(&mut self, chunk: &str) {
        let left = SPEAK_MAX.saturating_sub(self.spoken);
        let text: String = speak_text(chunk).trim().chars().take(left).collect();
        if left == 0 || !text.chars().any(|c| c.is_alphanumeric()) || self.stop.load(Ordering::Relaxed) {
            return;
        }
        self.spoken += text.chars().count();
        let prev = self.prev.clone();
        self.prev.push(' ');
        self.prev.push_str(&text);
        if self.prev.chars().count() > 250 {
            self.prev = self.prev.chars().rev().take(250).collect::<Vec<_>>().into_iter().rev().collect();
        }
        self.first = false;
        if let Some(tx) = &self.tx {
            let _ = tx.send(tauri::async_runtime::spawn(tts_bytes(text, prev)));
        }
    }
}

fn play_jobs(app: AppHandle, rx: mpsc::Receiver<Job>, stop: Arc<AtomicBool>) {
    let mut out: Option<(rodio::OutputStream, rodio::Sink)> = None;
    for job in rx {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let bytes = match tauri::async_runtime::block_on(job) {
            Ok(Ok(b)) => b,
            Ok(Err(e)) => {
                conf::log(&e);
                continue;
            }
            Err(e) => {
                conf::log(&format!("tts job: {e}"));
                continue;
            }
        };
        if stop.load(Ordering::Relaxed) {
            break;
        }
        if out.is_none() {
            let made = rodio::OutputStream::try_default()
                .map_err(|e| e.to_string())
                .and_then(|(s, h)| rodio::Sink::try_new(&h).map(|k| (s, k)).map_err(|e| e.to_string()));
            match made {
                Ok(p) => {
                    out = Some(p);
                    let _ = app.emit("tts:start", ());
                }
                Err(e) => {
                    conf::log(&format!("audio output gagal: {e}"));
                    break;
                }
            }
        }
        match rodio::Decoder::new(std::io::Cursor::new(bytes)) {
            Ok(d) => {
                if let Some((_, sink)) = &out {
                    sink.append(d);
                }
            }
            Err(e) => conf::log(&format!("decode tts gagal: {e}")),
        }
    }
    if let Some((_s, sink)) = &out {
        while !sink.empty() {
            if stop.load(Ordering::Relaxed) {
                sink.stop();
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
    let _ = app.emit("tts:end", ());
}

fn stop_current() {
    if let Some(f) = SPEAK_STOP.lock().unwrap().as_ref() {
        f.store(true, Ordering::Relaxed);
    }
}

#[tauri::command]
pub fn stop_speak(app: AppHandle) {
    stop_current();
    let _ = app.emit("tts:end", ());
}

#[tauri::command]
pub fn beep() {
    use rodio::Source;
    std::thread::spawn(|| {
        if let Ok((_stream, handle)) = rodio::OutputStream::try_default() {
            for _ in 0..2 {
                if let Ok(sink) = rodio::Sink::try_new(&handle) {
                    sink.append(rodio::source::SineWave::new(880.0).take_duration(std::time::Duration::from_millis(150)));
                    sink.sleep_until_end();
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::sentence_end;

    #[test]
    fn splits_at_sentence_end_not_decimals_or_short_chunks() {
        let s = "Halo dunia yang indah ini. Kedua";
        assert_eq!(sentence_end(s, 15, false), Some("Halo dunia yang indah ini.".len()));
        // "3.5" has no whitespace after the dot: not a boundary.
        assert_eq!(sentence_end("Suhu naik 3.5 derajat hari ini", 15, false), None);
        // Chunk shorter than `min` waits for more text.
        assert_eq!(sentence_end("Ya. Tentu", 15, false), None);
        // The very first chunk may break at a clause comma so speech starts earlier.
        let first = "Fotosintesis adalah proses tumbuhan, yang memakai cahaya";
        assert_eq!(sentence_end(first, 15, true), Some("Fotosintesis adalah proses tumbuhan,".len()));
        assert_eq!(sentence_end(first, 15, false), None);
    }
}
