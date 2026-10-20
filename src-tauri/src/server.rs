use crate::conf;
use tauri::{AppHandle, Emitter};

pub fn start(app: AppHandle) {
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
            let (code, json) = route(&app, &path, &method, &body);
            let _ = req.respond(
                tiny_http::Response::from_string(json).with_status_code(code).with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
                ),
            );
        }
    });
}

fn route(app: &AppHandle, path: &str, method: &tiny_http::Method, body: &str) -> (u16, String) {
    let post = *method == tiny_http::Method::Post;
    let ok = r#"{"ok":true}"#.to_string();
    match path {
        "/health" => (200, ok),
        "/toggle" if post => {
            let _ = app.emit("hotkey", "tap");
            (200, ok)
        }
        "/show" if post => {
            let _ = app.emit("ui:show", ());
            (200, ok)
        }
        "/hide" if post => {
            let _ = app.emit("ui:hide", ());
            (200, ok)
        }
        "/ask" if post => {
            let text = serde_json::from_str::<serde_json::Value>(body)
                .ok()
                .and_then(|v| v["text"].as_str().map(|s| s.trim().to_string()))
                .unwrap_or_default();
            if text.is_empty() {
                return (400, r#"{"ok":false,"error":"text kosong"}"#.into());
            }
            let _ = app.emit("ui:ask", serde_json::json!({ "text": text }));
            (200, ok)
        }
        "/agent" if post => {
            let v: serde_json::Value = match serde_json::from_str(body) {
                Ok(v) => v,
                Err(_) => return (400, r#"{"ok":false,"error":"json rusak"}"#.into()),
            };
            let state = v["state"].as_str().unwrap_or("");
            if !matches!(state, "working" | "waiting" | "done") {
                return (400, r#"{"ok":false,"error":"state harus working|waiting|done"}"#.into());
            }
            let _ = app.emit(
                "agent",
                serde_json::json!({
                    "agent": v["agent"].as_str().unwrap_or("agent"),
                    "state": state,
                    "text": v["text"].as_str().unwrap_or(""),
                }),
            );
            (200, ok)
        }
        _ => (404, r#"{"ok":false,"error":"not found"}"#.into()),
    }
}
