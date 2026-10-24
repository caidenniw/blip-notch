//! Persistent state in `%APPDATA%\Blip\state.json`.
//! One JSON file mapped 1:1 to the state keys.
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::logic::{Msg, WaterEntry};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusState {
    #[serde(default = "default_focus_phase")]
    pub phase: String,
    #[serde(default)]
    pub running_until: Option<i64>,
    #[serde(default = "default_focus_remaining")]
    pub remaining: u32,
    #[serde(default)]
    pub count: u32,
}

fn default_focus_phase() -> String {
    "focus".to_string()
}
fn default_focus_remaining() -> u32 {
    25 * 60
}

impl Default for FocusState {
    fn default() -> Self {
        FocusState {
            phase: default_focus_phase(),
            running_until: None,
            remaining: default_focus_remaining(),
            count: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusDur {
    #[serde(default = "default_dur_focus")]
    pub focus: u32,
    #[serde(default = "default_dur_short")]
    pub short: u32,
    #[serde(default = "default_dur_long")]
    pub long: u32,
}

fn default_dur_focus() -> u32 {
    25
}
fn default_dur_short() -> u32 {
    5
}
fn default_dur_long() -> u32 {
    15
}

impl Default for FocusDur {
    fn default() -> Self {
        FocusDur {
            focus: default_dur_focus(),
            short: default_dur_short(),
            long: default_dur_long(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    pub id: String,
    pub text: String,
    pub done: bool,
    #[serde(default)]
    pub due_at: Option<i64>,
    #[serde(default)]
    pub notified: Option<bool>,
    #[serde(default)]
    pub done_at: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaterState {
    #[serde(default = "default_water_goal")]
    pub goal: u32,
    #[serde(default = "default_water_cup")]
    pub cup: u32,
    #[serde(default = "default_water_start")]
    pub start: u32,
    #[serde(default = "default_water_end")]
    pub end: u32,
    #[serde(default = "default_water_every")]
    pub every: u32,
    #[serde(default)]
    pub log: HashMap<String, Vec<WaterEntry>>,
    #[serde(default)]
    pub last_nudge: Option<i64>,
}

fn default_water_goal() -> u32 {
    2000
}
fn default_water_cup() -> u32 {
    250
}
fn default_water_start() -> u32 {
    8
}
fn default_water_end() -> u32 {
    22
}
fn default_water_every() -> u32 {
    60
}

impl Default for WaterState {
    fn default() -> Self {
        WaterState {
            goal: default_water_goal(),
            cup: default_water_cup(),
            start: default_water_start(),
            end: default_water_end(),
            every: default_water_every(),
            log: HashMap::new(),
            last_nudge: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShelfItem {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub dir: bool,
    pub ext: String,
}

fn default_true() -> bool {
    true
}

fn default_page() -> String {
    "today".to_string()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Store {
    pub chat: Vec<Msg>,
    pub memory: Vec<String>,
    pub page: String,
    pub page_at: i64,
    pub focus: FocusState,
    pub focus_dur: FocusDur,
    pub focus_log: BTreeMap<String, u32>,
    pub todos: Vec<TodoItem>,
    pub water: WaterState,
    pub shelf: Vec<ShelfItem>,
    pub lyr_off: HashMap<String, f64>,
    pub nudge_on: bool,
    pub nudges: HashMap<String, i64>,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            chat: Vec::new(),
            memory: Vec::new(),
            page: default_page(),
            page_at: 0,
            focus: FocusState::default(),
            focus_dur: FocusDur::default(),
            focus_log: BTreeMap::new(),
            todos: Vec::new(),
            water: WaterState::default(),
            shelf: Vec::new(),
            lyr_off: HashMap::new(),
            nudge_on: default_true(),
            nudges: HashMap::new(),
        }
    }
}

fn state_path() -> PathBuf {
    crate::conf::dir().join("state.json")
}

impl Store {
    pub fn load() -> Self {
        Self::load_from(&state_path())
    }

    pub fn load_from(path: &Path) -> Self {
        if !path.exists() {
            return Store::default();
        }
        match std::fs::read_to_string(path) {
            Ok(content) => match serde_json::from_str::<Store>(&content) {
                Ok(s) => s,
                Err(e) => {
                    crate::conf::log(&format!("state rusak, mulai kosong: {e}"));
                    Store::default()
                }
            },
            Err(e) => {
                crate::conf::log(&format!("gagal baca state: {e}"));
                Store::default()
            }
        }
    }

    pub fn save(&self) {
        let _ = Self::save_to(self, &state_path());
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let tmp = path.with_extension("tmp");
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

pub struct Saver {
    dirty_at: Option<Instant>,
    path: PathBuf,
}

impl Saver {
    pub fn new() -> Self {
        Self::with_path(state_path())
    }

    pub fn with_path(path: PathBuf) -> Self {
        Saver {
            dirty_at: None,
            path,
        }
    }

    pub fn mark_dirty(&mut self) {
        if self.dirty_at.is_none() {
            self.dirty_at = Some(Instant::now());
        }
    }

    pub fn due(&self, now: Instant) -> bool {
        match self.dirty_at {
            Some(t) => now.saturating_duration_since(t) >= Duration::from_millis(500),
            None => false,
        }
    }

    pub fn flush(&mut self, store: &Store) {
        if self.dirty_at.is_some() {
            self.dirty_at = None;
            let _ = store.save_to(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let dir = std::env::temp_dir().join(format!("blip_test_{}", crate::time::now_ms()));
        let p = dir.join("state.json");
        let mut s = Store::default();
        s.memory.push("user suka kopi".into());
        s.nudge_on = false;
        s.save_to(&p).unwrap();

        let loaded = Store::load_from(&p);
        assert_eq!(loaded.memory, vec!["user suka kopi"]);
        assert!(!loaded.nudge_on);
        assert_eq!(loaded.page, "today");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_json_gives_default() {
        let dir = std::env::temp_dir().join(format!("blip_test_corrupt_{}", crate::time::now_ms()));
        let p = dir.join("state.json");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(&p, "{ bad json }").unwrap();

        let loaded = Store::load_from(&p);
        assert_eq!(loaded, Store::default());
        assert!(loaded.nudge_on);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_fields_use_defaults() {
        let dir = std::env::temp_dir().join(format!("blip_test_missing_{}", crate::time::now_ms()));
        let p = dir.join("state.json");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(&p, r#"{"chat":[{"role":"user","content":"halo"}]}"#).unwrap();

        let loaded = Store::load_from(&p);
        assert_eq!(loaded.chat.len(), 1);
        assert_eq!(loaded.chat[0].content, "halo");
        assert!(loaded.nudge_on);
        assert_eq!(loaded.focus_dur.focus, 25);
        assert_eq!(loaded.water.goal, 2000);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
