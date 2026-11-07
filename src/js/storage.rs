//! `localStorage`, persisted to `%APPDATA%\Blip\localstorage.json` (atomic tmp + rename).
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct Storage {
    map: BTreeMap<String, String>,
    path: Option<PathBuf>,
    /// Set on change; written 500 ms after the last change.
    dirty_since: Option<Instant>,
}

impl Storage {
    /// In-memory only (snapshot mode and tests).
    pub fn memory() -> Storage {
        Storage::default()
    }

    pub fn load(path: PathBuf) -> Storage {
        let map = match std::fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_else(|_| {
                crate::conf::log("localstorage rusak, mulai kosong");
                BTreeMap::new()
            }),
            Err(_) => BTreeMap::new(),
        };
        Storage { map, path: Some(path), dirty_since: None }
    }

    pub fn get(&self, k: &str) -> Option<String> {
        self.map.get(k).cloned()
    }
    pub fn set(&mut self, k: &str, v: &str) {
        if self.map.get(k).map(String::as_str) != Some(v) {
            self.map.insert(k.to_string(), v.to_string());
            self.dirty_since = Some(Instant::now());
        }
    }
    pub fn remove(&mut self, k: &str) {
        if self.map.remove(k).is_some() {
            self.dirty_since = Some(Instant::now());
        }
    }
    pub fn clear(&mut self) {
        if !self.map.is_empty() {
            self.map.clear();
            self.dirty_since = Some(Instant::now());
        }
    }
    pub fn keys(&self) -> Vec<String> {
        self.map.keys().cloned().collect()
    }

    /// Milliseconds until a pending write is due.
    pub fn due_in(&self) -> Option<u64> {
        self.dirty_since.map(|t| 500u64.saturating_sub(t.elapsed().as_millis() as u64))
    }

    /// Writes when the debounce elapsed (or `force`). Errors are logged; data stays in memory.
    pub fn flush(&mut self, force: bool) {
        let Some(t) = self.dirty_since else { return };
        if !force && t.elapsed() < Duration::from_millis(500) {
            return;
        }
        self.dirty_since = None;
        let Some(path) = &self.path else { return };
        let tmp = path.with_extension("json.tmp");
        let body = match serde_json::to_string(&self.map) {
            Ok(b) => b,
            Err(e) => return crate::conf::log(&format!("localstorage: {e}")),
        };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&tmp, body).and_then(|_| std::fs::rename(&tmp, path)) {
            self.dirty_since = Some(Instant::now());
            crate::conf::log(&format!("localstorage: gagal simpan: {e}"));
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn roundtrip_and_corrupt() {
        let dir = std::env::temp_dir().join(format!("blip_ls_{}", std::process::id()));
        let p = dir.join("localstorage.json");
        let mut s = super::Storage::load(p.clone());
        s.set("blip.chat", "[1]");
        s.flush(true);
        assert_eq!(super::Storage::load(p.clone()).get("blip.chat").as_deref(), Some("[1]"));
        std::fs::write(&p, "{bad").unwrap();
        assert!(super::Storage::load(p).keys().is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn failed_write_retains_data_for_retry() {
        let dir = std::env::temp_dir().join(format!("blip_ls_retry_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let blocked = dir.join("blocked");
        std::fs::write(&blocked, "not a directory").unwrap();
        let mut s = super::Storage::load(blocked.join("localstorage.json"));
        s.set("chat", "reply");
        s.flush(true);
        assert!(s.due_in().is_some(), "failed save must remain pending");
        std::fs::remove_file(&blocked).unwrap();
        s.flush(true);
        assert_eq!(super::Storage::load(blocked.join("localstorage.json")).get("chat").as_deref(), Some("reply"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
