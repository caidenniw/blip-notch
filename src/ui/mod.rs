//! Immediate-mode UI widgets, hit testing, and input routing.
use std::collections::HashMap;
use std::time::{Duration, Instant};
use windows::core::PCWSTR;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::UI::WindowsAndMessaging::IDC_ARROW;

pub type WidgetId = u64;

pub fn id_from_str(s: &str, index: usize) -> WidgetId {
    let mut h = 0xcbf29ce484222325u64;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h ^= index as u64;
    h.wrapping_mul(0x100000001b3)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Rect { x, y, w, h }
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }

    pub fn d2d_rect(&self) -> windows::Win32::Foundation::RECT {
        windows::Win32::Foundation::RECT {
            left: self.x.round() as i32,
            top: self.y.round() as i32,
            right: (self.x + self.w).round() as i32,
            bottom: (self.y + self.h).round() as i32,
        }
    }

    pub fn d2d_rect_f(&self) -> D2D_RECT_F {
        D2D_RECT_F {
            left: self.x,
            top: self.y,
            right: self.x + self.w,
            bottom: self.y + self.h,
        }
    }
}

#[derive(Clone, Debug)]
pub struct HitTarget {
    pub id: WidgetId,
    pub rect: Rect,
    pub title: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct TextEditState {
    pub text: String,
    pub cursor: usize,
    pub selection_anchor: Option<usize>,
}

impl TextEditState {
    pub fn new(initial: &str) -> Self {
        let len = initial.chars().count();
        TextEditState {
            text: initial.to_string(),
            cursor: len,
            selection_anchor: None,
        }
    }

    pub fn select_all(&mut self) {
        self.selection_anchor = Some(0);
        self.cursor = self.text.chars().count();
    }

    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.selection_anchor?;
        if anchor == self.cursor {
            None
        } else if anchor < self.cursor {
            Some((anchor, self.cursor))
        } else {
            Some((self.cursor, anchor))
        }
    }

    pub fn delete_selection(&mut self) -> bool {
        let Some((start, end)) = self.selection_range() else {
            return false;
        };
        let chars: Vec<char> = self.text.chars().collect();
        let mut new_chars = Vec::with_capacity(chars.len());
        new_chars.extend_from_slice(&chars[..start]);
        new_chars.extend_from_slice(&chars[end..]);
        self.text = new_chars.into_iter().collect();
        self.cursor = start;
        self.selection_anchor = None;
        true
    }

    pub fn insert_char(&mut self, c: char) {
        self.delete_selection();
        let mut chars: Vec<char> = self.text.chars().collect();
        let idx = self.cursor.min(chars.len());
        chars.insert(idx, c);
        self.text = chars.into_iter().collect();
        self.cursor = idx + 1;
        self.selection_anchor = None;
    }

    pub fn insert_str(&mut self, s: &str) {
        self.delete_selection();
        let mut chars: Vec<char> = self.text.chars().collect();
        let idx = self.cursor.min(chars.len());
        for (i, c) in s.chars().enumerate() {
            chars.insert(idx + i, c);
        }
        let count = s.chars().count();
        self.text = chars.into_iter().collect();
        self.cursor = idx + count;
        self.selection_anchor = None;
    }

    pub fn backspace(&mut self) {
        if self.delete_selection() {
            return;
        }
        if self.cursor > 0 {
            let mut chars: Vec<char> = self.text.chars().collect();
            chars.remove(self.cursor - 1);
            self.text = chars.into_iter().collect();
            self.cursor -= 1;
        }
    }

    pub fn delete_forward(&mut self) {
        if self.delete_selection() {
            return;
        }
        let mut chars: Vec<char> = self.text.chars().collect();
        if self.cursor < chars.len() {
            chars.remove(self.cursor);
            self.text = chars.into_iter().collect();
        }
    }
}

pub struct UiState {
    pub hot: Option<WidgetId>,
    pub active: Option<WidgetId>,
    pub focus: Option<WidgetId>,
    pub scroll_offsets: HashMap<WidgetId, f32>,
    pub text_edits: HashMap<WidgetId, TextEditState>,
    pub pointer_pos: (f32, f32),
    pub hover_since: Option<(WidgetId, Instant)>,
    pub clipboard: Option<arboard::Clipboard>,
}

impl Default for UiState {
    fn default() -> Self {
        UiState {
            hot: None,
            active: None,
            focus: None,
            scroll_offsets: HashMap::new(),
            text_edits: HashMap::new(),
            pointer_pos: (0.0, 0.0),
            hover_since: None,
            clipboard: arboard::Clipboard::new().ok(),
        }
    }
}

impl UiState {
    pub fn clipboard_paste(&mut self) -> Option<String> {
        self.clipboard.as_mut().and_then(|c| c.get_text().ok())
    }

    pub fn clipboard_copy(&mut self, text: &str) {
        if let Some(c) = self.clipboard.as_mut() {
            let _ = c.set_text(text);
        }
    }
}

pub struct Frame {
    pub hits: Vec<HitTarget>,
    pub cursor_shape: PCWSTR,
}

impl Default for Frame {
    fn default() -> Self {
        Frame {
            hits: Vec::new(),
            cursor_shape: IDC_ARROW,
        }
    }
}
impl Frame {
    pub fn register(&mut self, id: WidgetId, rect: Rect, title: Option<String>) {
        self.hits.push(HitTarget { id, rect, title });
    }

    pub fn hit_test(&self, x: f32, y: f32) -> Option<&HitTarget> {
        self.hits.iter().rev().find(|h| h.rect.contains(x, y))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ButtonStyle {
    Pill,
    Ghost,
    Link,
    Nav,
    Icon,
    Tab { active: bool },
    Chip,
    Danger,
}

pub struct ButtonResponse {
    pub clicked: bool,
    pub hovered: bool,
    pub pressed: bool,
}

pub fn button_logic(
    ui: &UiState,
    id: WidgetId,
    rect: Rect,
    frame: &mut Frame,
    title: Option<&str>,
) -> ButtonResponse {
    frame.register(id, rect, title.map(|s| s.to_string()));
    let hovered = ui.hot == Some(id);
    let pressed = ui.active == Some(id);
    let clicked = hovered && pressed;
    ButtonResponse {
        clicked,
        hovered,
        pressed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_edit() {
        let mut t = TextEditState::new("halo");
        assert_eq!(t.cursor, 4);
        t.insert_char('!');
        assert_eq!(t.text, "halo!");
        assert_eq!(t.cursor, 5);

        t.backspace();
        assert_eq!(t.text, "halo");

        t.select_all();
        assert_eq!(t.selection_range(), Some((0, 4)));
        t.insert_str("dunia");
        assert_eq!(t.text, "dunia");
        assert_eq!(t.cursor, 5);
    }

    #[test]
    fn test_rect_contains() {
        let r = Rect::new(10.0, 10.0, 50.0, 20.0);
        assert!(r.contains(15.0, 15.0));
        assert!(!r.contains(5.0, 15.0));
        assert!(!r.contains(70.0, 15.0));
    }
}
