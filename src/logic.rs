//! Port of `ui/lib.js` with identical behavior and unit tests.
use std::collections::HashMap;

pub const CHAT_MIN: f32 = 170.0;
pub const CHAT_MAX: f32 = 280.0;

pub fn parse_timer(text: &str) -> Option<u32> {
    let text = text.trim();
    let re = regex::Regex::new(
        r"(?i)\b(?:timer|pasang timer|ingatkan(?: saya)?(?: dalam)?)\s*(?:selama\s*)?(\d+)\s*(detik|menit|jam)\b",
    ).ok()?;
    let caps = re.captures(text)?;
    let n: u32 = caps.get(1)?.as_str().parse().ok()?;
    let unit = caps.get(2)?.as_str().to_lowercase();
    let mult = match unit.as_str() {
        "detik" => 1,
        "menit" => 60,
        "jam" => 3600,
        _ => return None,
    };
    let secs = n * mult;
    if secs > 0 {
        Some(secs)
    } else {
        None
    }
}

pub fn fmt(secs: u32) -> String {
    let m = secs / 60;
    let s = secs % 60;
    format!("{:02}:{:02}", m, s)
}

pub fn fmt_track(secs: f64) -> String {
    let t = secs.max(0.0).floor() as u64;
    format!("{}:{:02}", t / 60, t % 60)
}

pub fn track_pos(position: f64, duration: f64, playing: bool, at_ms: i64, now_ms: i64) -> f64 {
    if duration <= 0.0 {
        return 0.0;
    }
    let p = position
        + if playing {
            (now_ms - at_ms) as f64 / 1000.0
        } else {
            0.0
        };
    p.clamp(0.0, duration)
}

pub const APPS: &[(&str, &str)] = &[
    ("spotify", "spotify:"),
    ("youtube", "https://www.youtube.com"),
    ("youtube music", "https://music.youtube.com"),
    ("whatsapp", "whatsapp:"),
    ("gmail", "https://mail.google.com"),
    ("github", "https://github.com"),
    ("chatgpt", "https://chatgpt.com"),
];

#[derive(Clone, Debug, PartialEq)]
pub enum Intent {
    Media { cmd: String },
    Volume { dir: String, steps: Option<u32>, level: Option<u32> },
    Open { label: String, target: String },
    Todo { text: String },
    Water,
    Focus { cmd: String },
}

pub fn parse_intent(text: &str) -> Option<Intent> {
    let t = text.trim().to_lowercase();
    let t = t.trim_end_matches(|c| c == '.' || c == ',' || c == '!' || c == '?').trim();
    if t.is_empty() {
        return None;
    }

    if let Ok(re) = regex::Regex::new(r"^(?:tambah|catat)\s+(?:tugas|todo)\s+(.+)$") {
        if let Some(caps) = re.captures(t) {
            let body = caps.get(1).map_or("", |m| m.as_str()).trim();
            if !body.is_empty() && body.split_whitespace().count() <= 20 {
                return Some(Intent::Todo { text: body.to_string() });
            }
            return None;
        }
    }

    let words = t.split_whitespace().count();
    if words > 7 {
        return None;
    }

    if let Ok(re) = regex::Regex::new(r"^(?:sudah\s+)?minum(?:\s+air)?$") {
        if re.is_match(t) {
            return Some(Intent::Water);
        }
    }
    if let Ok(re) = regex::Regex::new(r"^(?:mulai\s+)?(?:fokus|pomodoro)$") {
        if re.is_match(t) {
            return Some(Intent::Focus { cmd: "start".to_string() });
        }
    }
    if let Ok(re) = regex::Regex::new(r"^(?:stop|berhenti|hentikan|selesai)\s+(?:fokus|pomodoro)$") {
        if re.is_match(t) {
            return Some(Intent::Focus { cmd: "stop".to_string() });
        }
    }

    if let Ok(re) = regex::Regex::new(r"^(?:putar|puter|mainkan|play)\s+(.+?)\s+di\s+(youtube|spotify)$") {
        if let Some(caps) = re.captures(t) {
            let q_raw = caps.get(1)?.as_str();
            let dest = caps.get(2)?.as_str();
            let mut q_enc = String::new();
            for b in q_raw.bytes() {
                if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
                    q_enc.push(b as char);
                } else if b == b' ' {
                    q_enc.push_str("%20");
                } else {
                    use std::fmt::Write;
                    let _ = write!(q_enc, "%{:02X}", b);
                }
            }
            return if dest == "spotify" {
                Some(Intent::Open {
                    label: "spotify".to_string(),
                    target: format!("spotify:search:{}", q_enc.to_lowercase()),
                })
            } else {
                Some(Intent::Open {
                    label: "youtube".to_string(),
                    target: format!("https://www.youtube.com/results?search_query={}", q_enc),
                })
            };
        }
    }

    if let Ok(re) = regex::Regex::new(r"^(?:buka|open|jalankan)\s+(.+)$") {
        if let Some(caps) = re.captures(t) {
            let name = caps.get(1)?.as_str();
            for &(app_name, target) in APPS {
                if app_name == name {
                    return Some(Intent::Open {
                        label: name.to_string(),
                        target: target.to_string(),
                    });
                }
            }
        }
    }

    if t.contains("volume") || t.contains("suara") {
        if let Ok(re) = regex::Regex::new(r"\b(?:volume|suara)\s+(\d{1,3})\b") {
            if let Some(caps) = re.captures(t) {
                if let Ok(lvl) = caps.get(1).unwrap().as_str().parse::<u32>() {
                    return Some(Intent::Volume {
                        dir: "set".to_string(),
                        steps: None,
                        level: Some(lvl.min(100)),
                    });
                }
            }
        }
        let steps = if t.contains("banget") || t.contains("sekali") || t.contains("full") {
            10
        } else {
            5
        };
        if t.contains("mute") || t.contains("bisu") || t.contains("matikan") {
            return Some(Intent::Volume {
                dir: "mute".to_string(),
                steps: Some(1),
                level: None,
            });
        }
        if t.contains("naik") || t.contains("besar") || t.contains("besarkan")
            || t.contains("keras") || t.contains("kencang") || t.contains("tambah") || t.contains("up")
        {
            return Some(Intent::Volume {
                dir: "up".to_string(),
                steps: Some(steps),
                level: None,
            });
        }
        if t.contains("turun") || t.contains("kecil") || t.contains("kecilkan")
            || t.contains("pelan") || t.contains("kurangi") || t.contains("down")
        {
            return Some(Intent::Volume {
                dir: "down".to_string(),
                steps: Some(steps),
                level: None,
            });
        }
    }

    let is_media_ctx = t.contains("lagu") || t.contains("lagunya") || t.contains("musik")
        || t.contains("musiknya") || t.contains("music") || t.contains("track") || words <= 2;

    if is_media_ctx {
        if t.contains("next") || t.contains("skip") || t.contains("selanjutnya")
            || t.contains("berikutnya") || t.contains("ganti lagu")
        {
            return Some(Intent::Media { cmd: "next".to_string() });
        }
        if t.contains("sebelumnya") || t.contains("previous") || t.contains("prev") || t.contains("lagu tadi") {
            return Some(Intent::Media { cmd: "prev".to_string() });
        }
        if t.contains("pause") || t.contains("jeda") || t.contains("stop")
            || t.contains("berhenti") || t.contains("hentikan")
        {
            return Some(Intent::Media { cmd: "pause".to_string() });
        }
        if t.contains("play") || t.contains("putar") || t.contains("puter")
            || t.contains("lanjut") || t.contains("lanjutkan") || t.contains("resume")
        {
            return Some(Intent::Media { cmd: "play".to_string() });
        }
    }

    None
}

pub fn norm_key(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.to_lowercase().chars() {
        let mapped = match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            'ý' | 'ÿ' => 'y',
            other => other,
        };
        if mapped.is_ascii_alphanumeric() {
            out.push(mapped);
        }
    }
    out
}

pub fn parse_track(title: &str, artist: &str) -> (String, String) {
    let mut a = artist.trim().to_string();
    if let Ok(re) = regex::Regex::new(r"(?i)\s*-\s*topic$") {
        a = re.replace(&a, "").to_string();
    }
    if let Ok(re) = regex::Regex::new(r"(?i)vevo$") {
        a = re.replace(&a, "").to_string();
    }
    if let Ok(re) = regex::Regex::new(r"(?i)\s*official(\s+channel)?$") {
        a = re.replace(&a, "").to_string();
    }
    let a = a.trim().to_string();

    let mut t = title.split(" | ").next().unwrap_or("").to_string();
    if let Ok(re) = regex::Regex::new(r"\([^)]*\)") {
        t = re.replace_all(&t, " ").to_string();
    }
    if let Ok(re) = regex::Regex::new(r"\[[^\]]*\]") {
        t = re.replace_all(&t, " ").to_string();
    }
    if let Ok(re) = regex::Regex::new(r"(?i)\s+-\s+(remaster(ed)?(\s+\d{4})?|live|radio edit|single version)\b.*$") {
        t = re.replace(&t, "").to_string();
    }
    if let Ok(re) = regex::Regex::new(r"(?i)\b(official\s+(music\s+)?video|official\s+audio|lyrics?\s+video|visuali[sz]er|mv|hd|4k)\b") {
        t = re.replace_all(&t, "").to_string();
    }
    if let Ok(re) = regex::Regex::new(r"\s+") {
        t = re.replace_all(&t, " ").to_string();
    }
    let t = t.trim_end_matches('-').trim().to_string();

    if let Some(idx) = t.find(" - ") {
        let left = t[..idx].trim().to_string();
        let right = t[idx + 3..].trim().to_string();
        let nl = norm_key(&left);
        let na = norm_key(&a);
        if a.is_empty() || (!nl.is_empty() && !na.is_empty() && (nl.contains(&na) || na.contains(&nl))) {
            return (right, left);
        }
    }
    (t, a)
}

pub fn app_name(id: &str) -> String {
    let s = id.trim();
    if s.is_empty() {
        return String::new();
    }
    let low = s.to_lowercase();
    if low.contains("spotify") {
        return "Spotify".to_string();
    }
    if low.contains("brave") {
        return "Brave".to_string();
    }
    if low.contains("chrome") {
        return "Chrome".to_string();
    }
    if low.contains("msedge") || low.contains("edge") {
        return "Edge".to_string();
    }
    if low.contains("firefox") {
        return "Firefox".to_string();
    }
    if low.contains("zune") {
        return "Media Player".to_string();
    }
    if low.contains("applemusic") || low.contains("itunes") {
        return "Apple Music".to_string();
    }
    let tail = s.rsplit(|c| c == '!' || c == '\\').next().unwrap_or(s);
    let mut cleaned = tail.to_string();
    if let Ok(re) = regex::Regex::new(r"(?i)\.exe$") {
        cleaned = re.replace(&cleaned, "").to_string();
    }
    let cleaned = cleaned.trim();
    cleaned.chars().take(16).collect()
}

#[derive(Clone, Debug, PartialEq)]
pub struct LrcLine {
    pub t: f64,
    pub line: String,
}

pub fn parse_lrc(text: &str) -> Vec<LrcLine> {
    let mut out = Vec::new();
    let shift = if let Ok(re) = regex::Regex::new(r"(?im)^\[offset:\s*([+-]?\d+)\s*\]") {
        if let Some(caps) = re.captures(text) {
            caps.get(1)
                .and_then(|m| m.as_str().parse::<f64>().ok())
                .unwrap_or(0.0)
                / 1000.0
        } else {
            0.0
        }
    } else {
        0.0
    };

    let tag_re = regex::Regex::new(r"\[(\d{1,3}):(\d{2})(?:[.:](\d{1,3}))?\]").unwrap();
    let strip_re = regex::Regex::new(r"\[[^\]]*\]").unwrap();

    for raw in text.lines() {
        let tag_matches: Vec<_> = tag_re.captures_iter(raw).collect();
        if tag_matches.is_empty() {
            continue;
        }
        let line = strip_re.replace_all(raw, "").trim().to_string();
        if line.is_empty() {
            continue;
        }
        for caps in tag_matches {
            let m: f64 = caps.get(1).unwrap().as_str().parse().unwrap_or(0.0);
            let s: f64 = caps.get(2).unwrap().as_str().parse().unwrap_or(0.0);
            let frac = caps.get(3).map_or(0.0, |c| {
                let s = c.as_str();
                let padded = format!("{:0<3}", s);
                padded[..3].parse::<f64>().unwrap_or(0.0) / 1000.0
            });
            let t = m * 60.0 + s + frac;
            out.push(LrcLine {
                t: (t - shift).max(0.0),
                line: line.clone(),
            });
        }
    }
    out.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
    out
}

pub fn lrc_index(lines: &[LrcLine], pos: f64) -> i32 {
    let mut idx = -1;
    for (i, l) in lines.iter().enumerate() {
        if l.t <= pos + 0.25 {
            idx = i as i32;
        } else {
            break;
        }
    }
    idx
}

pub fn parse_todo(text: &str, now_ms: i64) -> (String, Option<i64>) {
    let t = text.trim();
    let re = match regex::Regex::new(
        r"(?i)\s+(?:@\s*(\d{1,2})(?::(\d{2}))?|jam\s+(\d{1,2})(?::(\d{2}))?)\s*$",
    ) {
        Ok(r) => r,
        Err(_) => return (t.to_string(), None),
    };
    let Some(caps) = re.captures(t) else {
        return (t.to_string(), None);
    };
    let hh_str = caps.get(1).or_else(|| caps.get(3)).map(|m| m.as_str());
    let mm_str = caps.get(2).or_else(|| caps.get(4)).map(|m| m.as_str());
    let hh: u32 = hh_str.and_then(|s| s.parse().ok()).unwrap_or(0);
    let mm: u32 = mm_str.and_then(|s| s.parse().ok()).unwrap_or(0);
    let body = t[..caps.get(0).unwrap().start()].trim().to_string();

    if hh > 23 || mm > 59 {
        return (body, None);
    }

    let loc = crate::time::local_from_ms(now_ms);
    let mut target_ms = crate::time::ms_from_local(loc.year, loc.month, loc.day, hh, mm, 0);
    if target_ms <= now_ms {
        // next day: +24h approx in ms
        target_ms += 86_400_000;
        let next_loc = crate::time::local_from_ms(target_ms);
        target_ms = crate::time::ms_from_local(next_loc.year, next_loc.month, next_loc.day, hh, mm, 0);
    }
    (body, Some(target_ms))
}

pub fn day_key(ms: i64) -> String {
    crate::time::day_key(ms)
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WaterEntry {
    pub t: i64,
    pub ml: u32,
}

pub fn water_buckets(entries: &[WaterEntry], start_hour: u32, end_hour: u32) -> Vec<u32> {
    let n = ((end_hour.saturating_sub(start_hour)) / 2) as usize;
    let mut out = vec![0u32; n];
    for e in entries {
        let loc = crate::time::local_from_ms(e.t);
        let h_frac = loc.hour as f64 + loc.minute as f64 / 60.0;
        let idx = ((h_frac - start_hour as f64) / 2.0).floor() as isize;
        if idx >= 0 && (idx as usize) < n {
            out[idx as usize] += e.ml;
        }
    }
    out
}

pub fn fmt_size(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{} B", bytes);
    }
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    let mut v = bytes as f64 / 1024.0;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if v >= 100.0 {
        format!("{:.0} {}", v, UNITS[i])
    } else {
        format!("{:.1} {}", v, UNITS[i])
    }
}

pub fn file_kind(ext: &str, is_dir: bool) -> &'static str {
    if is_dir {
        return "folder";
    }
    let val = ext.trim_start_matches('.').to_lowercase();
    match val.as_str() {
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "svg" | "heic" => "image",
        "pdf" => "pdf",
        "doc" | "docx" | "odt" | "rtf" => "word",
        "xls" | "xlsx" | "csv" | "ods" => "sheet",
        "ppt" | "pptx" | "odp" => "slides",
        "zip" | "rar" | "7z" | "tar" | "gz" => "archive",
        "mp3" | "wav" | "flac" | "m4a" | "ogg" => "audio",
        "mp4" | "mkv" | "avi" | "mov" | "webm" => "video",
        "js" | "ts" | "tsx" | "jsx" | "py" | "rs" | "json" | "html" | "css" | "md" | "txt" => "code",
        _ => "file",
    }
}

pub fn chat_height_for(chrome: f32, content: f32) -> f32 {
    let total = chrome + content;
    let step = (total / 8.0).ceil() * 8.0;
    step.clamp(CHAT_MIN, CHAT_MAX)
}

pub fn level01(rms: f32, gain: f32) -> f32 {
    let v = rms * gain;
    if v.is_nan() || v <= 0.0 {
        0.0
    } else {
        v.min(1.0)
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Msg {
    pub role: String,
    pub content: String,
}

pub fn trim_history(mut h: Vec<Msg>, n: usize) -> Vec<Msg> {
    if h.len() > n {
        let drain_count = h.len() - n;
        h.drain(0..drain_count);
    }
    while !h.is_empty() && h[0].role != "user" {
        h.remove(0);
    }
    h
}

#[derive(Clone, Debug, PartialEq)]
pub enum MemoryCmd {
    Remember(String),
    Forget,
    Recall,
}

pub fn parse_memory(text: &str) -> Option<MemoryCmd> {
    let text = text.trim();
    if let Ok(re) = regex::Regex::new(r"(?i)^\s*(?:tolong\s+)?ingat(?:lah)?\s+(?:bahwa\s+)?(.{3,200}?)\s*$") {
        if let Some(caps) = re.captures(text) {
            let body = caps.get(1)?.as_str().trim();
            if !body.ends_with('?') {
                return Some(MemoryCmd::Remember(body.to_string()));
            }
            return None;
        }
    }
    if let Ok(re) = regex::Regex::new(r"(?i)^\s*lupakan\s+(?:semua|semuanya)\s*$") {
        if re.is_match(text) {
            return Some(MemoryCmd::Forget);
        }
    }
    if let Ok(re) = regex::Regex::new(r"(?i)^\s*(?:apa\s+(?:yang\s+)?kamu\s+ingat|kamu\s+ingat\s+apa)\s*\??\s*$") {
        if re.is_match(text) {
            return Some(MemoryCmd::Recall);
        }
    }
    None
}

pub fn greeting(hour: u32) -> &'static str {
    if hour < 11 {
        "Selamat pagi"
    } else if hour < 15 {
        "Selamat siang"
    } else if hour < 18 {
        "Selamat sore"
    } else {
        "Selamat malam"
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Todo {
    pub id: String,
    pub text: String,
    pub done: bool,
    #[serde(rename = "dueAt", default)]
    pub due_at: Option<i64>,
}

pub fn next_task<'a>(todos: &'a [Todo]) -> Option<&'a Todo> {
    let open: Vec<&'a Todo> = todos.iter().filter(|t| !t.done).collect();
    let due: Vec<&'a Todo> = open.iter().copied().filter(|t| t.due_at.is_some()).collect();
    if !due.is_empty() {
        return due.into_iter().min_by_key(|t| t.due_at.unwrap());
    }
    open.into_iter().next()
}

pub struct NudgeContext<'a> {
    pub now: i64,
    pub hour: u32,
    pub tasks_left: u32,
    pub battery: Option<u32>,
    pub plugged: bool,
    pub shown: &'a HashMap<String, i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Nudge {
    pub id: &'static str,
    pub text: String,
    pub tone: &'static str,
    pub mood: &'static str,
}

pub fn pick_nudge(c: &NudgeContext) -> Option<Nudge> {
    let ok = |id: &str, ms: i64| {
        c.shown.get(id).map_or(true, |&last| c.now - last >= ms)
    };

    if let Some(b) = c.battery {
        if b <= 20 && !c.plugged && ok("battery", 30 * 60_000) {
            return Some(Nudge {
                id: "battery",
                text: format!("baterai {}%, colok charger", b),
                tone: "",
                mood: "scared",
            });
        }
    }

    if (c.hour >= 23 || c.hour < 4) && ok("night", 8 * 3600_000) {
        return Some(Nudge {
            id: "night",
            text: "sudah larut, jangan lupa istirahat".to_string(),
            tone: "ok",
            mood: "tired",
        });
    }

    if c.hour >= 17 && c.hour < 19 && c.tasks_left > 0 && ok("evening", 12 * 3600_000) {
        return Some(Nudge {
            id: "evening",
            text: format!("{} tugas belum selesai hari ini", c.tasks_left),
            tone: "ok",
            mood: "happy",
        });
    }

    if c.hour >= 5 && c.hour < 22 && ok("greet", 12 * 3600_000) {
        let sub = if c.tasks_left > 0 {
            format!("{} tugas menunggu", c.tasks_left)
        } else {
            "belum ada tugas".to_string()
        };
        return Some(Nudge {
            id: "greet",
            text: format!("{}. {}", greeting(c.hour), sub),
            tone: "ok",
            mood: "cheer",
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer_fmt_trackpos() {
        assert_eq!(parse_timer("timer 5 menit"), Some(300));
        assert_eq!(fmt(65), "01:05");
        assert_eq!(fmt_track(65.9), "1:05");
        assert_eq!(fmt_track(-3.0), "0:00");
        assert_eq!(track_pos(10.0, 100.0, true, 0, 5000), 15.0);
        assert_eq!(track_pos(98.0, 100.0, true, 0, 10000), 100.0);
        assert_eq!(track_pos(10.0, 100.0, false, 0, 5000), 10.0);
        assert_eq!(track_pos(10.0, 0.0, true, 0, 5000), 0.0);
    }

    #[test]
    fn test_intents() {
        assert_eq!(parse_intent("lagu berikutnya"), Some(Intent::Media { cmd: "next".into() }));
        assert_eq!(parse_intent("skip"), Some(Intent::Media { cmd: "next".into() }));
        assert_eq!(parse_intent("pause"), Some(Intent::Media { cmd: "pause".into() }));
        assert_eq!(parse_intent("lanjutkan musik"), Some(Intent::Media { cmd: "play".into() }));
        assert_eq!(parse_intent("lagu sebelumnya"), Some(Intent::Media { cmd: "prev".into() }));
        assert_eq!(parse_intent("apa langkah berikutnya"), None);
        assert_eq!(parse_intent("volume naik"), Some(Intent::Volume { dir: "up".into(), steps: Some(5), level: None }));
        assert_eq!(parse_intent("kecilkan suara banget"), Some(Intent::Volume { dir: "down".into(), steps: Some(10), level: None }));
        assert_eq!(parse_intent("mute suara"), Some(Intent::Volume { dir: "mute".into(), steps: Some(1), level: None }));
        assert_eq!(parse_intent("buka spotify"), Some(Intent::Open { label: "spotify".into(), target: "spotify:".into() }));
        assert_eq!(parse_intent("buka notepad"), None);
        if let Some(Intent::Open { target, .. }) = parse_intent("putar lofi hip hop di youtube") {
            assert_eq!(target, "https://www.youtube.com/results?search_query=lofi%20hip%20hop");
        } else {
            panic!();
        }
        if let Some(Intent::Open { target, .. }) = parse_intent("Putar Hindia di Spotify!") {
            assert_eq!(target, "spotify:search:hindia");
        } else {
            panic!();
        }
        assert_eq!(parse_intent("volume 50"), Some(Intent::Volume { dir: "set".into(), steps: None, level: Some(50) }));
        assert_eq!(parse_intent("volume 250"), Some(Intent::Volume { dir: "set".into(), steps: None, level: Some(100) }));
        assert_eq!(parse_intent("tambah tugas kirim laporan jam 14"), Some(Intent::Todo { text: "kirim laporan jam 14".into() }));
        assert_eq!(parse_intent("minum"), Some(Intent::Water));
        assert_eq!(parse_intent("sudah minum air"), Some(Intent::Water));
        assert_eq!(parse_intent("mulai fokus"), Some(Intent::Focus { cmd: "start".into() }));
        assert_eq!(parse_intent("pomodoro"), Some(Intent::Focus { cmd: "start".into() }));
        assert_eq!(parse_intent("stop fokus"), Some(Intent::Focus { cmd: "stop".into() }));
    }

    #[test]
    fn test_track_and_lrc() {
        assert_eq!(
            parse_track("Justin Timberlake - Mirrors (Official Video)", "justintimberlakeVEVO"),
            ("Mirrors".into(), "Justin Timberlake".into())
        );
        assert_eq!(
            parse_track("Tems - Me & U (Official Video)", "TemsVEVO"),
            ("Me & U".into(), "Tems".into())
        );
        assert_eq!(
            parse_track("Doubt", "Twenty One Pilots - Topic"),
            ("Doubt".into(), "Twenty One Pilots".into())
        );
        assert_eq!(
            parse_track("Mirrors", "Justin Timberlake"),
            ("Mirrors".into(), "Justin Timberlake".into())
        );
        assert_eq!(
            parse_track("Song [Official Music Video] | HD", "Band"),
            ("Song".into(), "Band".into())
        );
        assert_eq!(
            parse_track("Hey Jude - Remastered 2015", "The Beatles"),
            ("Hey Jude".into(), "The Beatles".into())
        );
        assert_eq!(app_name("Spotify.exe"), "Spotify");
        assert_eq!(app_name(""), "");

        let lrc = parse_lrc("[00:23.76]Scared\n[00:25.76]of\n");
        assert_eq!(
            lrc,
            vec![
                LrcLine { t: 23.76, line: "Scared".into() },
                LrcLine { t: 25.76, line: "of".into() },
            ]
        );
        assert_eq!(lrc_index(&lrc, 24.0), 0);
        assert_eq!(lrc_index(&lrc, 10.0), -1);
        assert_eq!(lrc_index(&lrc, 99.0), 1);
        let lrc2 = parse_lrc("[00:10.00][00:20.00]chorus");
        assert_eq!(lrc2.len(), 2);
        assert_eq!(lrc2[0].t, 10.0);
        assert_eq!(lrc2[1].t, 20.0);
        let lrc3 = parse_lrc("[offset:500]\n[00:10.00]a");
        assert_eq!(lrc3[0].t, 9.5);
    }

    #[test]
    fn test_sizes_and_helpers() {
        assert_eq!(fmt_size(512), "512 B");
        assert_eq!(fmt_size(2516582), "2.4 MB");
        assert_eq!(fmt_size(0), "0 B");

        assert_eq!(file_kind("", true), "folder");
        assert_eq!(file_kind("PDF", false), "pdf");
        assert_eq!(file_kind("docx", false), "word");
        assert_eq!(file_kind("xlsx", false), "sheet");
        assert_eq!(file_kind("pptx", false), "slides");
        assert_eq!(file_kind("zip", false), "archive");
        assert_eq!(file_kind("mp3", false), "audio");
        assert_eq!(file_kind("mp4", false), "video");
        assert_eq!(file_kind("ts", false), "code");
        assert_eq!(file_kind("jpg", false), "image");
        assert_eq!(file_kind("bin", false), "file");

        assert_eq!(chat_height_for(142.0, 0.0), 170.0);
        assert_eq!(chat_height_for(142.0, 60.0), 208.0);
        assert_eq!(chat_height_for(142.0, 500.0), 280.0);
        assert_eq!(level01(0.1, 5.0), 0.5);
        assert_eq!(level01(0.5, 5.0), 1.0);
        assert_eq!(level01(-1.0, 5.0), 0.0);
        assert_eq!(level01(f32::NAN, 5.0), 0.0);

        let h = vec![
            Msg { role: "assistant".into(), content: "a".into() },
            Msg { role: "user".into(), content: "b".into() },
            Msg { role: "assistant".into(), content: "c".into() },
        ];
        assert_eq!(trim_history(h.clone(), 2), vec![h[1].clone(), h[2].clone()]);
        assert_eq!(trim_history(h, 1), Vec::<Msg>::new());

        assert_eq!(parse_memory("ingat nama saya Caiden"), Some(MemoryCmd::Remember("nama saya Caiden".into())));
        assert_eq!(parse_memory("ingat bahwa saya suka kopi"), Some(MemoryCmd::Remember("saya suka kopi".into())));
        assert_eq!(parse_memory("kamu ingat tidak kemarin?"), None);
        assert_eq!(parse_memory("ingat apa kemarin?"), None);
        assert_eq!(parse_memory("lupakan semua"), Some(MemoryCmd::Forget));
        assert_eq!(parse_memory("apa yang kamu ingat?"), Some(MemoryCmd::Recall));

        assert_eq!(
            [5, 12, 16, 20].iter().map(|&h| greeting(h)).collect::<Vec<_>>(),
            vec!["Selamat pagi", "Selamat siang", "Selamat sore", "Selamat malam"]
        );

        let t = vec![
            Todo { id: "1".into(), text: "x".into(), done: false, due_at: None },
            Todo { id: "2".into(), text: "late".into(), done: false, due_at: Some(200) },
            Todo { id: "3".into(), text: "soon".into(), done: false, due_at: Some(100) },
        ];
        assert_eq!(next_task(&t).unwrap().text, "soon");
        assert_eq!(next_task(&[t[0].clone(), Todo { id: "4".into(), text: "y".into(), done: false, due_at: None }]).unwrap().text, "x");
        assert!(next_task(&[Todo { id: "5".into(), text: "d".into(), done: true, due_at: None }]).is_none());
    }

    #[test]
    fn test_nudges() {
        let n = 1_000_000_000_000i64;
        let mut shown = HashMap::new();
        fn make_base<'a>(s: &'a HashMap<String, i64>, n: i64) -> NudgeContext<'a> {
            NudgeContext {
                now: n,
                hour: 12,
                tasks_left: 2,
                battery: Some(15),
                plugged: false,
                shown: s,
            }
        }
        assert_eq!(pick_nudge(&make_base(&shown, n)).unwrap().id, "battery");

        shown.insert("battery".into(), n - 60_000);
        let ctx2 = NudgeContext { shown: &shown, ..make_base(&shown, n) };
        assert_eq!(pick_nudge(&ctx2).unwrap().id, "greet");

        let ctx3 = NudgeContext { plugged: true, shown: &shown, ..make_base(&shown, n) };
        assert_eq!(pick_nudge(&ctx3).unwrap().id, "greet");

        let ctx4 = NudgeContext { battery: None, hour: 23, shown: &shown, ..make_base(&shown, n) };
        assert_eq!(pick_nudge(&ctx4).unwrap().id, "night");

        shown.insert("greet".into(), n - 1000);
        let ctx5 = NudgeContext { battery: None, hour: 17, shown: &shown, ..make_base(&shown, n) };
        assert_eq!(pick_nudge(&ctx5).unwrap().id, "evening");

        shown.insert("night".into(), n - 1000);
        let ctx6 = NudgeContext { battery: None, hour: 3, shown: &shown, ..make_base(&shown, n) };
        assert!(pick_nudge(&ctx6).is_none());
    }
}
