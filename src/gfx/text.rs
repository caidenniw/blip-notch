//! DirectWrite text: layout cache, ellipsis, tabular figures, hit-testing helpers.
//! All text positions handed to / returned from DirectWrite are UTF-16 offsets; use
//! `byte_to_utf16` / `utf16_to_byte` to convert from/to Rust `&str` byte offsets.
use std::collections::HashMap;
use std::time::{Duration, Instant};

use windows::core::{Interface, BOOL, HSTRING};
use windows::Win32::Graphics::Direct2D::{
    ID2D1Brush, ID2D1DeviceContext, D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
};
use windows::Win32::Graphics::DirectWrite::*;
use windows_numerics::Vector2;

const GC_AGE: Duration = Duration::from_secs(5);
const BIG: f32 = 100_000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    pub size: f32,
    /// 400 / 600 / 700
    pub weight: u16,
    pub wrap: bool,
    /// Single line, trailing "…" when the text does not fit `max_w` (forces no-wrap).
    pub ellipsis: bool,
    /// Uniform line height in px (baseline at 80%).
    pub line_height: Option<f32>,
    pub align: Align,
    /// `tnum` OpenType feature.
    pub tabular: bool,
    pub italic: bool,
}

impl Default for TextStyle {
    fn default() -> Self {
        TextStyle {
            size: 12.0,
            weight: 400,
            wrap: false,
            ellipsis: false,
            line_height: None,
            align: Align::Left,
            tabular: false,
            italic: false,
        }
    }
}

#[derive(PartialEq, Eq, Hash)]
struct Key {
    text: String,
    size: u32,
    weight: u16,
    wrap: bool,
    ellipsis: bool,
    line_height: Option<u32>,
    align: Align,
    tabular: bool,
    italic: bool,
    max_w: i32, // half-pixels
    max_h: u32,
}

struct Entry {
    layout: IDWriteTextLayout,
    used: Instant,
}

pub struct Text {
    factory: IDWriteFactory,
    family: HSTRING,
    family_name: String,
    cache: HashMap<Key, Entry>,
}

impl Text {
    /// "Segoe UI Variable Text" when installed (Windows 11), else "Segoe UI".
    pub fn new() -> windows::core::Result<Text> {
        unsafe {
            let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let mut col = None;
            factory.GetSystemFontCollection(&mut col, false)?;
            let mut has = false;
            if let Some(col) = col {
                let (mut idx, mut exists) = (0u32, BOOL(0));
                let name = HSTRING::from("Segoe UI Variable Text");
                has = col.FindFamilyName(&name, &mut idx, &mut exists).is_ok() && exists.as_bool();
            }
            let family_name = if has { "Segoe UI Variable Text" } else { "Segoe UI" };
            Ok(Text {
                factory,
                family: HSTRING::from(family_name),
                family_name: family_name.to_string(),
                cache: HashMap::new(),
            })
        }
    }

    pub fn family(&self) -> &str {
        &self.family_name
    }

    /// Cached layout. `max_w` is rounded to 0.5 px. Panics only if DirectWrite itself fails
    /// (out of memory / invalid factory).
    pub fn layout(&mut self, text: &str, style: &TextStyle, max_w: f32, max_h: f32) -> IDWriteTextLayout {
        let max_w = if max_w.is_finite() { ((max_w.max(0.0) * 2.0).round() / 2.0).min(BIG) } else { BIG };
        let max_h = if max_h.is_finite() { max_h.clamp(0.0, BIG) } else { BIG };
        let key = Key {
            text: text.to_owned(),
            size: style.size.to_bits(),
            weight: style.weight,
            wrap: style.wrap,
            ellipsis: style.ellipsis,
            line_height: style.line_height.map(f32::to_bits),
            align: style.align,
            tabular: style.tabular,
            italic: style.italic,
            max_w: (max_w * 2.0) as i32,
            max_h: max_h.to_bits(),
        };
        let now = Instant::now();
        if let Some(e) = self.cache.get_mut(&key) {
            e.used = now;
            return e.layout.clone();
        }
        let layout = self.build(text, style, max_w, max_h).expect("DirectWrite CreateTextLayout");
        self.cache.insert(key, Entry { layout: layout.clone(), used: now });
        layout
    }

    fn build(&self, text: &str, s: &TextStyle, max_w: f32, max_h: f32) -> windows::core::Result<IDWriteTextLayout> {
        unsafe {
            let size = if s.size.is_finite() && s.size > 0.0 { s.size } else { 12.0 };
            let format = self.factory.CreateTextFormat(
                &self.family,
                None::<&IDWriteFontCollection>,
                DWRITE_FONT_WEIGHT(s.weight as i32),
                if s.italic { DWRITE_FONT_STYLE_ITALIC } else { DWRITE_FONT_STYLE_NORMAL },
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                &HSTRING::from("en-us"),
            )?;
            let wide: Vec<u16> = text.encode_utf16().collect();
            let layout = self.factory.CreateTextLayout(&wide, &format, max_w, max_h)?;
            layout.SetTextAlignment(match s.align {
                Align::Left => DWRITE_TEXT_ALIGNMENT_LEADING,
                Align::Center => DWRITE_TEXT_ALIGNMENT_CENTER,
                Align::Right => DWRITE_TEXT_ALIGNMENT_TRAILING,
            })?;
            let no_wrap = s.ellipsis || !s.wrap;
            layout.SetWordWrapping(if no_wrap { DWRITE_WORD_WRAPPING_NO_WRAP } else { DWRITE_WORD_WRAPPING_WRAP })?;
            if s.ellipsis {
                let sign = self.factory.CreateEllipsisTrimmingSign(&format)?;
                let trimming = DWRITE_TRIMMING {
                    granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
                    delimiter: 0,
                    delimiterCount: 0,
                };
                layout.SetTrimming(&trimming, &sign)?;
            }
            if let Some(h) = s.line_height {
                layout.SetLineSpacing(DWRITE_LINE_SPACING_METHOD_UNIFORM, h, h * 0.8)?;
            }
            if s.tabular && !wide.is_empty() {
                let typo = self.factory.CreateTypography()?;
                typo.AddFontFeature(DWRITE_FONT_FEATURE {
                    nameTag: DWRITE_FONT_FEATURE_TAG_TABULAR_FIGURES,
                    parameter: 1,
                })?;
                layout.SetTypography(
                    &typo,
                    DWRITE_TEXT_RANGE { startPosition: 0, length: wide.len() as u32 },
                )?;
            }
            Ok(layout)
        }
    }

    /// Drop layouts unused for 5 s. Call occasionally (e.g. on the 250 ms tick).
    pub fn gc(&mut self) {
        let now = Instant::now();
        self.cache.retain(|_, e| now.duration_since(e.used) < GC_AGE);
    }

    /// (width, height) of the laid-out text. With `ellipsis`/no-wrap, width is capped by `max_w`.
    pub fn measure(&mut self, text: &str, style: &TextStyle, max_w: f32) -> (f32, f32) {
        let layout = self.layout(text, style, max_w, BIG);
        let mut m = DWRITE_TEXT_METRICS::default();
        if unsafe { layout.GetMetrics(&mut m) }.is_err() {
            return (0.0, 0.0);
        }
        (m.width, m.height)
    }

    pub fn draw(dc: &ID2D1DeviceContext, layout: &IDWriteTextLayout, x: f32, y: f32, brush: &ID2D1Brush) {
        unsafe {
            dc.DrawTextLayout(Vector2 { X: x, Y: y }, layout, brush, D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT);
        }
    }
}

/// Hit test in layout space. Returns (UTF-16 text position, trailing hit). The caret index for a
/// click is `pos + (trailing as u32)` for a single-unit cluster; see `hit_test_caret`.
pub fn hit_test_point(layout: &IDWriteTextLayout, x: f32, y: f32) -> (u32, bool) {
    let (mut trailing, mut inside) = (BOOL(0), BOOL(0));
    let mut m = DWRITE_HIT_TEST_METRICS::default();
    if unsafe { layout.HitTestPoint(x, y, &mut trailing, &mut inside, &mut m) }.is_err() {
        return (0, false);
    }
    (m.textPosition, trailing.as_bool())
}

/// Caret insertion index (UTF-16) nearest to the point: cluster start, or cluster end on a trailing hit.
pub fn hit_test_caret(layout: &IDWriteTextLayout, x: f32, y: f32) -> u32 {
    let (mut trailing, mut inside) = (BOOL(0), BOOL(0));
    let mut m = DWRITE_HIT_TEST_METRICS::default();
    if unsafe { layout.HitTestPoint(x, y, &mut trailing, &mut inside, &mut m) }.is_err() {
        return 0;
    }
    m.textPosition + if trailing.as_bool() { m.length } else { 0 }
}

/// Selection rectangles `(x, y, w, h)` for UTF-16 range `[start, start+len)`, layout space.
pub fn range_rects(layout: &IDWriteTextLayout, start: u32, len: u32) -> Vec<(f32, f32, f32, f32)> {
    if len == 0 {
        return Vec::new();
    }
    unsafe {
        let mut count = 0u32;
        // First call only reports the needed count (fails with E_NOT_SUFFICIENT_BUFFER).
        let _ = layout.HitTestTextRange(start, len, 0.0, 0.0, None, &mut count);
        if count == 0 {
            return Vec::new();
        }
        let mut v = vec![DWRITE_HIT_TEST_METRICS::default(); count as usize];
        let mut got = 0u32;
        if layout.HitTestTextRange(start, len, 0.0, 0.0, Some(&mut v[..]), &mut got).is_err() {
            return Vec::new();
        }
        v.truncate(got as usize);
        v.iter().map(|m| (m.left, m.top, m.width, m.height)).collect()
    }
}

/// Caret at UTF-16 index `pos`: `(x, y, height)` in layout space.
pub fn caret_rect(layout: &IDWriteTextLayout, pos: u32) -> (f32, f32, f32) {
    let (mut x, mut y) = (0f32, 0f32);
    let mut m = DWRITE_HIT_TEST_METRICS::default();
    if unsafe { layout.HitTestTextPosition(pos, false, &mut x, &mut y, &mut m) }.is_err() {
        return (0.0, 0.0, 0.0);
    }
    let mut h = m.height;
    if h <= 0.0 {
        let mut tm = DWRITE_TEXT_METRICS::default();
        if unsafe { layout.GetMetrics(&mut tm) }.is_ok() {
            h = tm.height;
        }
    }
    (x, y, h)
}

pub fn utf16_len(s: &str) -> u32 {
    s.encode_utf16().count() as u32
}

/// UTF-16 offset of byte offset `byte` (clamped to `s.len()`; an offset inside a char counts that char as not yet passed).
pub fn byte_to_utf16(s: &str, byte: usize) -> u32 {
    s.char_indices()
        .take_while(|(i, _)| *i < byte)
        .map(|(_, c)| c.len_utf16() as u32)
        .sum()
}

/// Byte offset of UTF-16 offset `pos` (clamped to `s.len()`; an offset inside a surrogate pair rounds up to the next char).
pub fn utf16_to_byte(s: &str, pos: u32) -> usize {
    let mut acc = 0u32;
    for (i, c) in s.char_indices() {
        if acc >= pos {
            return i;
        }
        acc += c.len_utf16() as u32;
    }
    s.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_helpers() {
        let s = "aé😀b"; // bytes: a=1, é=2, 😀=4, b=1 ; utf16: 1,1,2,1
        assert_eq!(utf16_len(s), 5);
        assert_eq!(utf16_len(""), 0);
        for (byte, u) in [(0, 0), (1, 1), (3, 2), (7, 4), (8, 5)] {
            assert_eq!(byte_to_utf16(s, byte), u, "byte {byte}");
            assert_eq!(utf16_to_byte(s, u), byte, "utf16 {u}");
        }
        assert_eq!(utf16_to_byte(s, 3), 7); // inside surrogate pair -> next char
        assert_eq!(byte_to_utf16(s, 99), 5);
        assert_eq!(utf16_to_byte(s, 99), s.len());
    }

    #[test]
    fn directwrite_measure_and_hit_test() {
        let mut t = Text::new().expect("dwrite");
        assert!(t.family() == "Segoe UI" || t.family() == "Segoe UI Variable Text");
        let st = TextStyle::default();
        let (w, h) = t.measure("Hello world", &st, 1000.0);
        assert!(w > 20.0 && h > 8.0);
        // ellipsis caps the width
        let el = TextStyle { ellipsis: true, ..st };
        let (w2, _) = t.measure("Hello world, this is a long line", &el, 40.0);
        assert!(w2 <= 40.5);
        // wrap grows height
        let wr = TextStyle { wrap: true, ..st };
        let (_, h2) = t.measure("Hello world, this is a long line", &wr, 40.0);
        assert!(h2 > h * 1.5);
        // cache hit returns same layout; hit test round-trip
        let l = t.layout("Hello", &st, 100.0, 100.0);
        assert!(!range_rects(&l, 0, 5).is_empty());
        let (cx, _, ch) = caret_rect(&l, 5);
        assert!(cx > 10.0 && ch > 0.0);
        assert_eq!(hit_test_caret(&l, 0.0, 2.0), 0);
        assert_eq!(hit_test_caret(&l, 500.0, 2.0), 5);
        t.gc();
    }
}
