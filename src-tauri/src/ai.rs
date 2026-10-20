use crate::conf;
use futures_util::future::{AbortHandle, Abortable};
use futures_util::StreamExt;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

static ABORTS: Mutex<Option<HashMap<String, AbortHandle>>> = Mutex::new(None);
static SPEAK_STOP: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);
static COOLDOWN: Mutex<Option<HashMap<String, std::time::Instant>>> = Mutex::new(None);

fn aborts() -> std::sync::MutexGuard<'static, Option<HashMap<String, AbortHandle>>> {
    ABORTS.lock().unwrap()
}

#[tauri::command]
pub async fn chat_send(app: AppHandle, id: String, messages: Vec<serde_json::Value>) {
    let (handle, reg) = AbortHandle::new_pair();
    aborts().get_or_insert_with(HashMap::new).insert(id.clone(), handle);
    let app2 = app.clone();
    let id2 = id.clone();
    let fut = Abortable::new(run_chat(app2, id2, messages), reg);
    let _ = fut.await;
    aborts().get_or_insert_with(HashMap::new).remove(&id);
}

#[tauri::command]
pub fn chat_abort(app: AppHandle, id: String) {
    if let Some(h) = aborts().get_or_insert_with(HashMap::new).remove(&id) {
        h.abort();
        let _ = app.emit("chat:done", serde_json::json!({ "id": id, "aborted": true }));
    }
}

async fn run_chat(app: AppHandle, id: String, messages: Vec<serde_json::Value>) {
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
    for model in chain {
        match stream_once(&app, &id, &model, &messages).await {
            Ok(n) if n > 0 => {
                conf::log(&format!("chat done {id} model {model} chars {n}"));
                let _ = app.emit("chat:done", serde_json::json!({ "id": id, "model": model }));
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
) -> Result<usize, (usize, String)> {
    let c = conf::get();
    let base = c.router.base_url.trim_end_matches('/').to_string();
    let mut msgs = vec![serde_json::json!({ "role": "system", "content": c.system_prompt })];
    msgs.extend_from_slice(messages);
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base}/chat/completions"))
        .header("Authorization", format!("Bearer {}", conf::router_key()))
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

fn speak_text(text: &str) -> String {
    let cleaned = regex::Regex::new(r"```[\s\S]*?```")
        .unwrap()
        .replace_all(text, " kode ");
    let cleaned = regex::Regex::new(r"[*_`#>]").unwrap().replace_all(&cleaned, "");
    cleaned.chars().take(900).collect()
}

#[tauri::command]
pub async fn speak(app: AppHandle, text: String) -> Result<(), String> {
    let c = conf::get();
    if !c.tts.enabled || text.trim().is_empty() {
        let _ = app.emit("tts:end", ());
        return Ok(());
    }
    let stop = Arc::new(AtomicBool::new(false));
    *SPEAK_STOP.lock().unwrap() = Some(stop.clone());
    let out_path = std::env::temp_dir().join(format!(
        "blip-tts-{}.mp3",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    let text = speak_text(&text);
    let voice = c.tts.voice.clone();
    let gen_path = out_path.clone();
    let generated = tauri::async_runtime::spawn_blocking(move || {
        use std::process::Command;
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
            .arg(&gen_path)
            .creation_flags(0x08000000)
            .output();
        match res {
            Ok(o) if o.status.success() => Ok(()),
            Ok(o) => Err(format!(
                "edge-tts gagal: {}",
                String::from_utf8_lossy(&o.stderr).chars().rev().take(300).collect::<String>()
            )),
            Err(e) => Err(format!("edge-tts tidak bisa dijalankan: {e}")),
        }
    })
    .await
    .map_err(|e| e.to_string())?;

    if let Err(e) = generated {
        conf::log(&e);
        let _ = std::fs::remove_file(&out_path);
        let _ = app.emit("tts:end", ());
        return Err(e);
    }

    let _ = app.emit("tts:start", ());
    let p = out_path.clone();
    let flag = stop.clone();
    let played = tauri::async_runtime::spawn_blocking(move || play_mp3(&p, &flag))
        .await
        .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&out_path);
    *SPEAK_STOP.lock().unwrap() = None;
    let _ = app.emit("tts:end", ());
    played
}

fn play_mp3(path: &std::path::Path, stop: &AtomicBool) -> Result<(), String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let decoder = rodio::Decoder::new(std::io::BufReader::new(file)).map_err(|e| e.to_string())?;
    let (_stream, handle) = rodio::OutputStream::try_default().map_err(|e| e.to_string())?;
    let sink = rodio::Sink::try_new(&handle).map_err(|e| e.to_string())?;
    sink.append(decoder);
    while !sink.empty() {
        if stop.load(Ordering::Relaxed) {
            sink.stop();
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    Ok(())
}

#[tauri::command]
pub fn stop_speak(app: AppHandle) {
    if let Some(f) = SPEAK_STOP.lock().unwrap().as_ref() {
        f.store(true, Ordering::Relaxed);
    }
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
