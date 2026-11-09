//! `window.__TAURI__` for the original UI: `invoke` → Rust services, `bus::Event` → `listen`.
use crate::bus::{self, DictatePhase, Event, HudEvent};
use serde_json::{json, Value};

/// JS side: promise table + event subscribers. Evaluated after the prelude.
pub const BRIDGE_JS: &str = r#"
(function (g) {
  const pending = new Map();
  let seq = 0;
  const subs = {};
  g.__TAURI__ = {
    core: {
      invoke(cmd, args) {
        const id = ++seq;
        return new Promise((resolve, reject) => {
          pending.set(id, { resolve, reject });
          __dom.invoke(id, String(cmd), JSON.stringify(args === undefined ? {} : args));
        });
      },
    },
    event: {
      listen(name, cb) {
        (subs[name] || (subs[name] = [])).push(cb);
        return Promise.resolve(() => { subs[name] = (subs[name] || []).filter(f => f !== cb); });
      },
    },
  };
  g.__resolve = (id, ok, value) => {
    const p = pending.get(id);
    if (!p) return;
    pending.delete(id);
    if (ok) p.resolve(value); else p.reject(value);
  };
  g.__emit = (name, payload) => {
    for (const cb of (subs[name] || []).slice()) {
      try { cb({ event: name, payload }); } catch (e) { __reportError(e); }
    }
  };
})(globalThis);
"#;

/// Result of a dispatched call.
pub enum Outcome {
    Now(Result<Value, String>),
    /// Completion arrives later as `Event::Resolve`.
    Later,
    /// `shelf_drag_out`: run outside JS by the window procedure, then resolve.
    DragOut(Vec<String>),
}

fn s(a: &Value, k: &str) -> String {
    a.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}
fn strings(a: &Value, k: &str) -> Vec<String> {
    a.get(k).and_then(Value::as_array).map(|v| v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
}
fn unit(r: Result<(), String>) -> Outcome {
    Outcome::Now(r.map(|_| Value::Null))
}

/// Maps one `invoke(cmd, args)` to the native services (spec-ui-behavior §6.1).
pub fn dispatch(id: u32, cmd: &str, args: &str) -> Outcome {
    let a: Value = serde_json::from_str(args).unwrap_or(Value::Null);
    match cmd {
        "info" => Outcome::Now(Ok(info_json())),
        "config_get" => Outcome::Now(serde_json::to_value(&*crate::conf::get()).map_err(|e| e.to_string())),
        "config_set" => {
            let Ok(c) = serde_json::from_value::<crate::conf::Conf>(a.get("conf").cloned().unwrap_or(Value::Null)) else {
                return Outcome::Now(Err("Invalid config".into()));
            };
            let old = crate::conf::get();
            if c.ui.toggle_hotkey != old.ui.toggle_hotkey {
                if let Err(e) = crate::win::window::set_toggle_hotkey(bus::hwnd(), &c.ui.toggle_hotkey) {
                    return Outcome::Now(Err(e));
                }
            }
            let lang_changed = c.ui.language != old.ui.language;
            if let Err(e) = crate::conf::save(c) {
                // Keep the registered hotkey in sync with the file that stayed on disk.
                let _ = crate::win::window::set_toggle_hotkey(bus::hwnd(), &old.ui.toggle_hotkey);
                return Outcome::Now(Err(e));
            }
            let sh = bus::settings_hwnd();
            if lang_changed && !sh.0.is_null() {
                let title = windows::core::HSTRING::from(crate::win::window::settings_title());
                unsafe {
                    let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowTextW(sh, &title);
                }
            }
            bus::post(Event::Config);
            Outcome::Now(Ok(Value::Null))
        }
        "autostart_get" => Outcome::Now(Ok(json!(crate::win::autostart::is_enabled()))),
        "autostart_set" => unit(crate::win::autostart::set_enabled(a.get("on").and_then(Value::as_bool).unwrap_or(false))),
        "ai_connect" => {
            let models = strings(&a, "models");
            if models.is_empty() {
                return Outcome::Now(Err("no model".into()));
            }
            let (base, key) = (s(&a, "baseUrl"), s(&a, "apiKey"));
            crate::rt::spawn(async move {
                let r = crate::ai::probe_models(base, key, models).await;
                bus::post_settings(Event::Resolve { id, result: Ok(json!(r)) });
            });
            Outcome::Later
        }
        "open_settings" => {
            unsafe {
                use windows::Win32::Foundation::{LPARAM, WPARAM};
                let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(Some(bus::hwnd()), bus::WM_APP_SETTINGS, WPARAM(0), LPARAM(0));
            }
            Outcome::Now(Ok(Value::Null))
        }
        "open_config_file" => unit(std::process::Command::new("notepad.exe").arg(crate::conf::config_path()).spawn().map(|_| ()).map_err(|e| e.to_string())),
        "prefs_request" => {
            bus::post(Event::PrefsRequest);
            Outcome::Now(Ok(Value::Null))
        }
        "prefs_reply" => {
            bus::post_settings(Event::Prefs(a.get("prefs").cloned().unwrap_or(Value::Null)));
            Outcome::Now(Ok(Value::Null))
        }
        "pref_set" => {
            bus::post(Event::PrefSet { key: s(&a, "key"), value: a.get("value").cloned().unwrap_or(Value::Null) });
            Outcome::Now(Ok(Value::Null))
        }
        "sys_status" => Outcome::Now(Ok(crate::sys::sys_status())),
        "set_rect" => {
            crate::hover::set_rect(a.get("w").and_then(Value::as_f64).unwrap_or(168.0), a.get("h").and_then(Value::as_f64).unwrap_or(32.0));
            Outcome::Now(Ok(Value::Null))
        }
        "focus_input" => {
            crate::hover::focus_input();
            crate::win::window::take_focus(bus::hwnd());
            Outcome::Now(Ok(Value::Null))
        }
        "release_focus" => {
            crate::win::window::drop_focus(bus::hwnd());
            crate::hover::release_focus();
            Outcome::Now(Ok(Value::Null))
        }
        "chat_send" => {
            let messages = a.get("messages").and_then(Value::as_array).cloned().unwrap_or_default();
            crate::ai::chat_send(
                s(&a, "id"),
                messages,
                Some(a.get("speak").and_then(Value::as_bool).unwrap_or(false)),
                Some(s(&a, "memory")),
            );
            Outcome::Now(Ok(Value::Null))
        }
        "chat_abort" => {
            crate::ai::chat_abort(s(&a, "id"));
            Outcome::Now(Ok(Value::Null))
        }
        "stop_speak" => {
            crate::ai::stop_speak();
            Outcome::Now(Ok(Value::Null))
        }
        "beep" => {
            crate::ai::beep();
            Outcome::Now(Ok(Value::Null))
        }
        "rec_start" => unit(crate::rec::rec_start(s(&a, "mode"))),
        "rec_stop" => {
            crate::rec::rec_stop();
            Outcome::Now(Ok(Value::Null))
        }
        "media_cmd" => {
            let (c, pos) = (s(&a, "cmd"), a.get("pos").and_then(Value::as_f64));
            std::thread::spawn(move || crate::media::media_cmd(c, pos));
            Outcome::Now(Ok(Value::Null))
        }
        "lyrics" => {
            let (t, ar, d) = (s(&a, "title"), s(&a, "artist"), a.get("duration").and_then(Value::as_f64).unwrap_or(0.0));
            crate::rt::spawn(async move {
                let result = crate::media::lyrics(t, ar, d).await;
                bus::post(Event::Resolve { id, result });
            });
            Outcome::Later
        }
        "volume" => {
            let steps = a.get("steps").and_then(Value::as_u64).map(|x| x as u32);
            let level = a.get("level").and_then(Value::as_f64).map(|x| x.max(0.0) as u32);
            unit(crate::sys::volume(s(&a, "dir"), steps, level))
        }
        "open_target" => unit(crate::media::open_target(s(&a, "target"))),
        "clipboard_text" => Outcome::Now(crate::sys::clipboard_text().map(Value::String)),
        "shelf_info" => {
            let paths = strings(&a, "paths");
            crate::rt::spawn_blocking(move || bus::post(Event::Resolve { id, result: Ok(Value::Array(crate::sys::shelf_info(paths))) }));
            Outcome::Later
        }
        "shelf_from_clipboard" => Outcome::Now(crate::sys::shelf_from_clipboard().map(Value::Array)),
        "shelf_copy" => unit(crate::sys::shelf_copy(strings(&a, "paths"))),
        "open_path" => unit(crate::sys::open_path(s(&a, "path"))),
        "reveal_path" => unit(crate::sys::reveal_path(s(&a, "path"))),
        "shelf_drag_out" => Outcome::DragOut(strings(&a, "paths")),
        _ => Outcome::Now(Err(format!("unknown command: {cmd}"))),
    }
}

fn info_json() -> Value {
    let i = crate::conf::info();
    json!({
        "model": i.model, "stt": i.stt, "tts": i.tts, "ttsVoice": i.tts_voice,
        "routerReady": i.router_ready, "autoCollapseMs": i.auto_collapse_ms, "language": i.language,
    })
}

/// Converts a bus event to `(event name, payload)` for `listen` (spec-ui-behavior §6.2).
/// `put_image` stores cover-art bytes and returns their `src`.
pub fn event_json(e: Event, put_image: &mut dyn FnMut(Vec<u8>) -> String) -> Option<(&'static str, Value)> {
    Some(match e {
        Event::Hotkey => ("hotkey", json!("tap")),
        Event::Dictate(DictatePhase::Start) => ("dictate", json!("start")),
        Event::Dictate(DictatePhase::Done) => ("dictate", json!("done")),
        Event::Hover(b) => ("hover", json!(b)),
        Event::Cursor { x, y } => ("cursor", json!({ "x": x, "y": y })),
        Event::Compact(b) => ("compact", json!(b)),
        Event::DragNear(b) => ("drag-near", json!(b)),
        Event::ChatChunk { id, delta } => ("chat:chunk", json!({ "id": id, "delta": delta })),
        Event::ChatDone { id, model, aborted } => {
            let mut v = json!({ "id": id });
            if let Some(m) = model {
                v["model"] = json!(m);
            }
            if aborted {
                v["aborted"] = json!(true);
            }
            ("chat:done", v)
        }
        Event::ChatError { id, error } => ("chat:error", json!({ "id": id, "error": error })),
        Event::TtsStart => ("tts:start", Value::Null),
        Event::TtsEnd => ("tts:end", Value::Null),
        Event::TtsLevel(f) => ("tts:level", json!(f)),
        Event::RecLevel(f) => ("rec:level", json!(f)),
        Event::SttResult(t) => ("stt:result", json!({ "text": t })),
        Event::SttEmpty => ("stt:empty", Value::Null),
        Event::SttError(e) => ("stt:error", json!({ "error": e })),
        Event::Media(m) => {
            let mut v = json!({
                "title": m.title, "artist": m.artist, "playing": m.playing, "position": m.position,
                "duration": m.duration, "app": m.app, "ts": m.ts,
            });
            match m.art {
                None => {}
                Some(None) => v["art"] = Value::Null,
                Some(Some(bytes)) => v["art"] = json!(put_image(bytes)),
            }
            ("media", v)
        }
        Event::Hud(h) => (
            "hud",
            match h {
                HudEvent::Volume { value, muted } => json!({ "kind": "volume", "value": value, "muted": muted }),
                HudEvent::Caps { on } => json!({ "kind": "caps", "on": on }),
                HudEvent::Power { plugged, battery } => json!({ "kind": "power", "plugged": plugged, "battery": battery }),
                HudEvent::Lang { value } => json!({ "kind": "lang", "value": value }),
                HudEvent::Net { online } => json!({ "kind": "net", "online": online }),
                HudEvent::Audio { name } => json!({ "kind": "audio", "name": name }),
                HudEvent::Brightness { value } => json!({ "kind": "brightness", "value": value }),
                HudEvent::Bt { name, on } => json!({ "kind": "bt", "name": name, "on": on }),
            },
        ),
        Event::Sys(s) => ("sys", s.json()),
        Event::ShelfDrop(v) => ("shelf:drop", json!(v)),
        Event::ShelfDragEnd(b) => ("shelf:drag-end", json!(b)),
        Event::DragEnter => ("tauri://drag-enter", Value::Null),
        Event::DragLeave => ("tauri://drag-leave", Value::Null),
        Event::UiShow => ("ui:show", Value::Null),
        Event::UiHide => ("ui:hide", Value::Null),
        Event::UiAsk(t) => ("ui:ask", json!({ "text": t })),
        Event::Agent { agent, state, text } => ("agent", json!({ "agent": agent, "state": state, "text": text.unwrap_or_default() })),
        Event::Config => ("config", info_json()),
        Event::PrefsRequest => ("prefs-request", Value::Null),
        Event::Prefs(v) => ("prefs", v),
        Event::PrefSet { key, value } => ("pref-set", json!({ "key": key, "value": value })),
        Event::Resolve { .. } => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_shapes() {
        let mut put = |_b: Vec<u8>| "blip-img:1".to_string();
        let (n, v) = event_json(Event::ChatDone { id: "a1".into(), model: None, aborted: false }, &mut put).unwrap();
        assert_eq!((n, v), ("chat:done", json!({"id": "a1"})));
        let m = bus::MediaInfo { title: "t".into(), art: Some(Some(vec![1])), ..Default::default() };
        let (_, v) = event_json(Event::Media(m), &mut put).unwrap();
        assert_eq!(v["art"], json!("blip-img:1"));
        let m = bus::MediaInfo { title: "t".into(), art: None, ..Default::default() };
        let (_, v) = event_json(Event::Media(m), &mut put).unwrap();
        assert!(v.get("art").is_none());
        assert!(matches!(dispatch(1, "nope", "{}"), Outcome::Now(Err(e)) if e == "unknown command: nope"));
    }
}
