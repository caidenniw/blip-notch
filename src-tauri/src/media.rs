use std::time::Duration;
use tauri::{AppHandle, Emitter};

#[cfg(windows)]
mod imp {
    use super::*;
    use base64::Engine;
    use windows::core::Interface;
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSession as Session,
        GlobalSystemMediaTransportControlsSessionManager,
        GlobalSystemMediaTransportControlsSessionMediaProperties as Props,
    };
    use windows::Storage::Streams::{DataReader, IInputStream};

    #[derive(Default)]
    pub struct Info {
        pub title: String,
        pub artist: String,
        pub playing: bool,
        pub position: f64,
        pub duration: f64,
        pub app: String,
    }

    fn manager() -> Option<GlobalSystemMediaTransportControlsSessionManager> {
        let op = GlobalSystemMediaTransportControlsSessionManager::RequestAsync().ok()?;
        op.join().ok()
    }

    fn is_playing(s: &Session) -> bool {
        s.GetPlaybackInfo()
            .ok()
            .and_then(|i| i.PlaybackStatus().ok())
            .map(|st| st.0 == 4)
            .unwrap_or(false)
    }

    /// Prefer the active session Windows points at; fallback to the first currently-playing session.
    fn session() -> Option<Session> {
        let m = manager()?;
        let curr = m.GetCurrentSession().ok();
        if let Some(s) = &curr {
            if is_playing(s) {
                return Some(s.clone());
            }
        }
        curr
    }

    pub fn loop_media(app: AppHandle) {
        let mut last_track = String::new();
        let mut last_playing = false;
        let mut last_pos = 0.0f64;
        loop {
            std::thread::sleep(Duration::from_millis(1000));
            let (info, props) = current();
            let track = format!("{}|{}|{}", info.title, info.artist, info.app);
            let expected = last_pos + if last_playing { 1.0 } else { 0.0 };
            let new_track = track != last_track;
            let jumped = (info.position - expected).abs() > 2.0;
            if new_track || info.playing != last_playing || jumped {
                let mut payload = serde_json::json!({
                    "title": info.title,
                    "artist": info.artist,
                    "playing": info.playing,
                    "position": info.position,
                    "duration": info.duration,
                    "app": info.app,
                });
                if new_track {
                    // ponytail: cover dibaca hanya saat ganti lagu; bila player telat set thumbnail, cover baru muncul di lagu berikutnya.
                    payload["art"] = props
                        .as_ref()
                        .and_then(art)
                        .map(Into::into)
                        .unwrap_or(serde_json::Value::Null);
                }
                let _ = app.emit("media", payload);
            }
            last_track = track;
            last_playing = info.playing;
            last_pos = info.position;
        }
    }

    fn current() -> (Info, Option<Props>) {
        let Some(s) = session() else {
            return (Info::default(), None);
        };
        let playing = is_playing(&s);
        let tl = s.GetTimelineProperties().ok();
        let mut position = tl
            .as_ref()
            .and_then(|t| t.Position().ok())
            .map(|d| d.Duration as f64 / 1e7)
            .unwrap_or(0.0);
        let duration = tl
            .as_ref()
            .and_then(|t| t.EndTime().ok())
            .map(|d| d.Duration as f64 / 1e7)
            .unwrap_or(0.0);
        // Browsers update Position rarely; extrapolate from LastUpdatedTime (FILETIME ticks).
        let last = tl
            .as_ref()
            .and_then(|t| t.LastUpdatedTime().ok())
            .map(|d| d.UniversalTime)
            .unwrap_or(0);
        if playing && duration > 0.0 && last > 0 {
            let now_ft = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| (d.as_nanos() / 100) as i64 + 116_444_736_000_000_000)
                .unwrap_or(last);
            position = (position + (now_ft - last).max(0) as f64 / 1e7).min(duration);
        }
        let props = s
            .TryGetMediaPropertiesAsync()
            .ok()
            .and_then(|op| op.join().ok());
        let title = props
            .as_ref()
            .and_then(|p| p.Title().ok())
            .map(|h| h.to_string())
            .unwrap_or_default();
        let artist = props
            .as_ref()
            .and_then(|p| p.Artist().ok())
            .map(|h| h.to_string())
            .unwrap_or_default();
        let app = s
            .SourceAppUserModelId()
            .map(|h| h.to_string())
            .unwrap_or_default();
        (
            Info {
                title,
                artist,
                playing,
                position,
                duration,
                app,
            },
            props,
        )
    }

    fn art(props: &Props) -> Option<String> {
        let stream = props.Thumbnail().ok()?.OpenReadAsync().ok()?.join().ok()?;
        let size = stream.Size().ok()?;
        if size == 0 || size > 2_000_000 {
            return None;
        }
        let reader = DataReader::CreateDataReader(&stream.cast::<IInputStream>().ok()?).ok()?;
        reader.LoadAsync(size as u32).ok()?.join().ok()?;
        let mut buf = vec![0u8; size as usize];
        reader.ReadBytes(&mut buf).ok()?;
        let mime = stream
            .ContentType()
            .map(|h| h.to_string())
            .unwrap_or_default();
        let mime = if mime.is_empty() {
            "image/png".to_string()
        } else {
            mime
        };
        Some(format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(buf)
        ))
    }

    pub fn cmd(app: AppHandle, cmd: String, pos: Option<f64>) {
        let _ = app;
        let Some(s) = session() else { return };
        match cmd.as_str() {
            "toggle" => {
                let _ = s
                    .TryTogglePlayPauseAsync()
                    .and_then(|op| op.join().map(|_| ()));
            }
            "play" => {
                let _ = s.TryPlayAsync().and_then(|op| op.join().map(|_| ()));
            }
            "pause" => {
                let _ = s.TryPauseAsync().and_then(|op| op.join().map(|_| ()));
            }
            "next" => {
                let _ = s.TrySkipNextAsync().and_then(|op| op.join().map(|_| ()));
            }
            "prev" => {
                let _ = s.TrySkipPreviousAsync().and_then(|op| op.join().map(|_| ()));
            }
            "seek" => {
                if let Some(p) = pos {
                    let _ = s
                        .TryChangePlaybackPositionAsync((p.max(0.0) * 1e7) as i64)
                        .and_then(|op| op.join().map(|_| ()));
                }
            }
            _ => {}
        }
    }
}

#[cfg(windows)]
pub use imp::{cmd as media_cmd_impl, loop_media};

pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        unsafe {
            let _ = windows::Win32::System::WinRT::RoInitialize(
                windows::Win32::System::WinRT::RO_INIT_MULTITHREADED,
            );
        }
        #[cfg(windows)]
        loop_media(app);
    });
}

#[tauri::command]
pub fn media_cmd(app: AppHandle, cmd: String, pos: Option<f64>) {
    #[cfg(windows)]
    media_cmd_impl(app, cmd, pos);
    #[cfg(not(windows))]
    let _ = (app, cmd, pos);
}

pub fn norm(s: &str) -> String {
    s.chars()
        .filter_map(|c| {
            if c.is_ascii_alphanumeric() {
                Some(c.to_ascii_lowercase())
            } else {
                None
            }
        })
        .collect()
}

fn contains_each_other(a: &str, b: &str) -> bool {
    !a.is_empty() && !b.is_empty() && (a.contains(b) || b.contains(a))
}

pub fn pick_lyrics(
    items: &[serde_json::Value],
    title: &str,
    artist: &str,
    duration: f64,
) -> (Option<String>, Option<String>) {
    let t_norm = norm(title);
    let a_norm = norm(artist);

    let mut best_synced: Option<(f64, String)> = None;
    let mut best_plain: Option<(f64, String)> = None;

    for it in items {
        let item_title = it.get("trackName").and_then(|v| v.as_str()).unwrap_or("");
        let it_norm = norm(item_title);
        let title_ok = !t_norm.is_empty()
            && !it_norm.is_empty()
            && (it_norm == t_norm || contains_each_other(&it_norm, &t_norm));
        if !title_ok {
            continue;
        }

        if !a_norm.is_empty() {
            let item_artist = it.get("artistName").and_then(|v| v.as_str()).unwrap_or("");
            let ia_norm = norm(item_artist);
            let artist_ok = !ia_norm.is_empty()
                && (ia_norm == a_norm || contains_each_other(&ia_norm, &a_norm));
            if !artist_ok {
                continue;
            }
        }

        let item_dur = it.get("duration").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let diff = if item_dur > 0.0 && duration > 0.0 {
            (item_dur - duration).abs()
        } else {
            0.0
        };

        if let Some(s) = it.get("syncedLyrics").and_then(|v| v.as_str()) {
            let s = s.trim();
            if !s.is_empty() && diff <= 15.0 {
                if best_synced.as_ref().map_or(true, |(d, _)| diff < *d) {
                    best_synced = Some((diff, s.to_string()));
                }
            }
        }

        if let Some(p) = it.get("plainLyrics").and_then(|v| v.as_str()) {
            let p = p.trim();
            if !p.is_empty() {
                if best_plain.as_ref().map_or(true, |(d, _)| diff < *d) {
                    best_plain = Some((diff, p.to_string()));
                }
            }
        }
    }

    (best_synced.map(|(_, s)| s), best_plain.map(|(_, p)| p))
}

#[tauri::command]
pub async fn lyrics(
    title: String,
    artist: String,
    duration: f64,
) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::builder()
        .user_agent("Blip/0.1 (https://github.com/deni/blip)")
        .build()
        .map_err(|e| e.to_string())?;

    let secs = duration.round() as i64;
    let mut any_network = false;
    let mut best_synced: Option<String> = None;
    let mut best_plain: Option<String> = None;

    // Step 1: get-cached
    if !artist.trim().is_empty() {
        let mut url = format!(
            "https://lrclib.net/api/get-cached?track_name={}&artist_name={}",
            urlenc(&title),
            urlenc(&artist)
        );
        if secs > 0 {
            url.push_str(&format!("&duration={secs}"));
        }
        if let Ok(resp) = client
            .get(&url)
            .timeout(Duration::from_secs(6))
            .send()
            .await
        {
            any_network = true;
            if resp.status().is_success() {
                if let Ok(v) = resp.json::<serde_json::Value>().await {
                    let synced = v
                        .get("syncedLyrics")
                        .and_then(|x| x.as_str())
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string);
                    let plain = v
                        .get("plainLyrics")
                        .and_then(|x| x.as_str())
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string);
                    if synced.is_some() || plain.is_some() {
                        return Ok(serde_json::json!({ "synced": synced, "plain": plain }));
                    }
                }
            }
        }
    }

    // Step 2: search by track_name & artist_name
    let mut search_url = format!(
        "https://lrclib.net/api/search?track_name={}",
        urlenc(&title)
    );
    if !artist.trim().is_empty() {
        search_url.push_str(&format!("&artist_name={}", urlenc(&artist)));
    }
    if let Ok(resp) = client
        .get(&search_url)
        .timeout(Duration::from_secs(8))
        .send()
        .await
    {
        any_network = true;
        if resp.status().is_success() {
            if let Ok(items) = resp.json::<Vec<serde_json::Value>>().await {
                let (s, p) = pick_lyrics(&items, &title, &artist, duration);
                if s.is_some() {
                    best_synced = s;
                }
                if p.is_some() {
                    best_plain = p;
                }
                if best_synced.is_some() {
                    return Ok(serde_json::json!({
                        "synced": best_synced,
                        "plain": best_plain,
                    }));
                }
            }
        }
    }

    // Step 3: search by q=<title>
    let q_url = format!("https://lrclib.net/api/search?q={}", urlenc(&title));
    if let Ok(resp) = client.get(&q_url).timeout(Duration::from_secs(8)).send().await {
        any_network = true;
        if resp.status().is_success() {
            if let Ok(items) = resp.json::<Vec<serde_json::Value>>().await {
                let (s, p) = pick_lyrics(&items, &title, &artist, duration);
                if s.is_some() {
                    best_synced = s;
                }
                if best_plain.is_none() && p.is_some() {
                    best_plain = p;
                }
                if best_synced.is_some() {
                    return Ok(serde_json::json!({
                        "synced": best_synced,
                        "plain": best_plain,
                    }));
                }
            }
        }
    }

    // Step 4: uncached /api/get
    if best_synced.is_none() && !artist.trim().is_empty() {
        let mut url = format!(
            "https://lrclib.net/api/get?track_name={}&artist_name={}",
            urlenc(&title),
            urlenc(&artist)
        );
        if secs > 0 {
            url.push_str(&format!("&duration={secs}"));
        }
        if let Ok(resp) = client
            .get(&url)
            .timeout(Duration::from_secs(15))
            .send()
            .await
        {
            any_network = true;
            if resp.status().is_success() {
                if let Ok(v) = resp.json::<serde_json::Value>().await {
                    let synced = v
                        .get("syncedLyrics")
                        .and_then(|x| x.as_str())
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string);
                    let plain = v
                        .get("plainLyrics")
                        .and_then(|x| x.as_str())
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string);
                    if synced.is_some() {
                        best_synced = synced;
                    }
                    if best_plain.is_none() && plain.is_some() {
                        best_plain = plain;
                    }
                }
            }
        }
    }

    if !any_network {
        return Err("koneksi lrclib gagal".into());
    }

    Ok(serde_json::json!({
        "synced": best_synced,
        "plain": best_plain,
    }))
}

fn urlenc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Opens only web/app URIs: text comes from voice/LLM, so arbitrary paths/commands are refused.
#[tauri::command]
pub fn open_target(target: String) -> Result<(), String> {
    if !["https://", "spotify:", "whatsapp:"]
        .iter()
        .any(|p| target.starts_with(p))
    {
        return Err("target ditolak".into());
    }
    std::process::Command::new("explorer")
        .arg(&target)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_selection() {
        let items = serde_json::json!([
            {
                "trackName": "Mirrors",
                "artistName": "Justin Timberlake",
                "duration": 278.0,
                "syncedLyrics": "[00:10.00] short",
            },
            {
                "trackName": "Mirrors",
                "artistName": "Justin Timberlake",
                "duration": 486.0,
                "syncedLyrics": "[00:10.00] long",
            }
        ]);
        let items = items.as_array().unwrap();
        let (synced, _) = pick_lyrics(items, "Mirrors", "Justin Timberlake", 484.0);
        assert_eq!(synced.as_deref(), Some("[00:10.00] long"));
    }

    #[test]
    fn artist_mismatch() {
        let items = serde_json::json!([
            {
                "trackName": "Doubt",
                "artistName": "Some Other Band",
                "duration": 190.0,
                "syncedLyrics": "[00:10.00] text",
                "plainLyrics": "text",
            }
        ]);
        let items = items.as_array().unwrap();
        let (synced, plain) = pick_lyrics(items, "Doubt", "Twenty One Pilots", 190.0);
        assert_eq!(synced, None);
        assert_eq!(plain, None);
    }

    #[test]
    fn large_diff_plain_only() {
        let items = serde_json::json!([
            {
                "trackName": "Song",
                "artistName": "Band",
                "duration": 240.0,
                "syncedLyrics": "[00:10.00] synced",
                "plainLyrics": "plain text",
            }
        ]);
        let items = items.as_array().unwrap();
        // target duration 200, item 240 -> diff 40 (> 15). Synced dropped, plain kept.
        let (synced, plain) = pick_lyrics(items, "Song", "Band", 200.0);
        assert_eq!(synced, None);
        assert_eq!(plain.as_deref(), Some("plain text"));
    }
}
