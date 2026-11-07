//! `blip.exe --snapshot <scenario.js> <out.png>`: renders a parity scenario headless on a
//! virtual clock (no window, services, tray or single-instance mutex).
use crate::engine::{Doc, Engine};
use crate::js::storage::Storage;

pub fn run(scenario: &str, out: &str) -> i32 {
    let src = match std::fs::read_to_string(scenario) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("snapshot: {scenario}: {e}");
            return 2;
        }
    };
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED);
    }
    // Same order as tools/ref/make.mjs: fixed, mock, scenario, lib, app.
    let extra = vec![
        ("fixed.js", include_str!("../tools/ref/fixed.js").to_string()),
        ("mock.js", include_str!("../tools/ref/mock.js").to_string()),
        ("scenario", src.clone()),
    ];
    let shot: f64 = regex::Regex::new(r"__shotAt\s*=\s*(\d+)")
        .unwrap()
        .captures(&src)
        .and_then(|c| c[1].parse().ok())
        .unwrap_or(3000.0);
    let settings = std::path::Path::new(scenario).file_stem().is_some_and(|s| s.to_string_lossy().starts_with("settings"));
    let doc = if settings { &Doc::SETTINGS } else { &Doc::ISLAND };
    let e = Engine::boot(doc, Storage::memory(), 0.0, true, &extra);
    let mut t = 0.0;
    while t < shot {
        t = (t + 16.0).min(shot);
        e.pump(t, true);
    }
    let mut gfx = match crate::gfx::Gfx::new(doc.view.0 as u32, doc.view.1 as u32) {
        Ok(g) => g,
        Err(err) => {
            eprintln!("snapshot: gfx: {err}");
            return 3;
        }
    };
    match e.snapshot(&mut gfx, std::path::Path::new(out)) {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("snapshot: {err}");
            4
        }
    }
}
