use crate::bus::{post, Event};
use crate::conf;

pub fn start() {
    let port = conf::get().ui.port;
    std::thread::spawn(move || {
        let server = match tiny_http::Server::http(("127.0.0.1", port)) {
            Ok(s) => s,
            Err(e) => {
                conf::log(&format!("control server gagal bind {port}: {e}"));
                return;
            }
        };
        conf::log(&format!("control server up 127.0.0.1:{port}"));
        for mut req in server.incoming_requests() {
            let mut body = String::new();
            let _ = req.as_reader().read_to_string(&mut body);
            let path = req.url().split('?').next().unwrap_or("").to_string();
            let method = req.method().clone();
            let (code, json) = route(&path, &method, &body);
            let _ = req.respond(
                tiny_http::Response::from_string(json).with_status_code(code).with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
                ),
            );
        }
    });
}

fn route(path: &str, method: &tiny_http::Method, body: &str) -> (u16, String) {
    let is_post = *method == tiny_http::Method::Post;
    let ok = r#"{"ok":true}"#.to_string();
    match path {
        "/health" => (200, ok),
        "/toggle" if is_post => {
            post(Event::Hotkey);
            (200, ok)
        }
        "/show" if is_post => {
            post(Event::UiShow);
            (200, ok)
        }
        "/hide" if is_post => {
            post(Event::UiHide);
            (200, ok)
        }
        "/ask" if is_post => {
            let text = serde_json::from_str::<serde_json::Value>(body)
                .ok()
                .and_then(|v| v["text"].as_str().map(|s| s.trim().to_string()))
                .unwrap_or_default();
            if text.is_empty() {
                return (400, r#"{"ok":false,"error":"text kosong"}"#.into());
            }
            post(Event::UiAsk(text));
            (200, ok)
        }
        "/agent" if is_post => {
            let v: serde_json::Value = match serde_json::from_str(body) {
                Ok(v) => v,
                Err(_) => return (400, r#"{"ok":false,"error":"json rusak"}"#.into()),
            };
            let state = v["state"].as_str().unwrap_or("");
            if !matches!(state, "working" | "waiting" | "done") {
                return (400, r#"{"ok":false,"error":"state harus working|waiting|done"}"#.into());
            }
            let agent = v["agent"].as_str().unwrap_or("agent").to_string();
            let text = Some(v["text"].as_str().unwrap_or("").to_string());
            post(Event::Agent { agent, state: state.to_string(), text });
            (200, ok)
        }
        _ => (404, r#"{"ok":false,"error":"not found"}"#.into()),
    }
}
