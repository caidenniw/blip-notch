//! Cascade, inheritance, `var()` substitution and the typed computed style.
use super::parse::{parse_sheet, Decl, Keyframes, Rule, Sheet};
use super::select::{matches_selector, PseudoEl, Selector};
use super::value::{self as v, Bg, Color, Filter, Gradient, Lp, Lpa, Shadow, Tf, Track};
use crate::dom::{Dom, Kind, NodeId};
use crate::gfx::anim::Easing;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Display {
    None,
    #[default]
    Inline,
    InlineBlock,
    Block,
    Flex,
    Grid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Position {
    #[default]
    Static,
    Relative,
    Absolute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Overflow {
    #[default]
    Visible,
    Hidden,
    Auto,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BorderStyle {
    #[default]
    None,
    Solid,
    Dashed,
    Outset,
    Inset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Start,
    End,
    Center,
    Stretch,
    Baseline,
    SpaceBetween,
    SpaceAround,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WhiteSpace {
    #[default]
    Normal,
    NoWrap,
    PreWrap,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum LineHeight {
    #[default]
    Normal,
    Mul(f32),
    Px(f32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// SVG paint: `None` = `none`, `Some(None)` = currentColor.
pub type Paint = Option<Option<Color>>;

#[derive(Clone, Debug, PartialEq)]
pub struct Transition {
    pub prop: String,
    pub dur: f32,
    pub delay: f32,
    pub easing: Easing,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Animation {
    pub name: String,
    pub dur: f32,
    pub delay: f32,
    pub easing: Easing,
    /// `f32::INFINITY` for `infinite`.
    pub count: f32,
    pub alternate: bool,
    pub paused: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Style {
    pub display: Display,
    pub position: Position,
    /// top, right, bottom, left
    pub inset: [Lpa; 4],
    pub width: Lpa,
    pub height: Lpa,
    pub min_width: Lpa,
    pub min_height: Lpa,
    pub max_width: Option<Lp>,
    pub max_height: Option<Lp>,
    pub border_box: bool,
    pub margin: [Lpa; 4],
    pub padding: [Lp; 4],
    pub border_w: [f32; 4],
    pub border_style: [BorderStyle; 4],
    /// `None` = currentColor.
    pub border_color: [Option<Color>; 4],
    /// top-left, top-right, bottom-right, bottom-left; (horizontal, vertical).
    pub radius: [(Lp, Lp); 4],
    pub flex_row: bool,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: Lpa,
    pub align_items: Option<Align>,
    pub align_self: Option<Align>,
    pub justify_content: Option<Align>,
    pub justify_items: Option<Align>,
    pub align_content: Option<Align>,
    pub row_gap: Lp,
    pub column_gap: Lp,
    pub grid_cols: Vec<Track>,
    pub overflow_x: Overflow,
    pub overflow_y: Overflow,
    pub z_index: Option<i32>,
    pub opacity: f32,
    pub visible: bool,
    pub pointer_events: bool,
    pub background: Bg,
    pub shadows: Vec<Shadow>,
    pub transform: Vec<Tf>,
    pub origin: (Lp, Lp),
    pub translate: (Lp, Lp),
    pub filter: Vec<Filter>,
    pub mask: Option<Gradient>,
    pub outline: Option<(f32, Color)>,
    pub outline_offset: f32,
    // ---- text (inherited)
    pub color: Color,
    pub font_family: Rc<str>,
    pub font_size: f32,
    pub font_weight: u16,
    pub line_height: LineHeight,
    pub letter_spacing: f32,
    pub text_align: TextAlign,
    pub uppercase: bool,
    pub white_space: WhiteSpace,
    pub break_word: bool,
    pub tabular: bool,
    pub text_shadow: Vec<Shadow>,
    pub fill: Paint,
    pub stroke: Paint,
    pub stroke_width: f32,
    pub round_cap: bool,
    pub round_join: bool,
    pub cursor_pointer: bool,
    pub select_text: bool,
    // ---- non-inherited text bits
    pub ellipsis: bool,
    pub line_through: bool,
    pub content: Option<String>,
    pub cover: bool,
    pub vertical_align: f32,
    pub transitions: Rc<Vec<Transition>>,
    pub animations: Rc<Vec<Animation>>,
    pub vars: Rc<HashMap<String, String>>,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            display: Display::Inline,
            position: Position::Static,
            inset: [Lpa::Auto; 4],
            width: Lpa::Auto,
            height: Lpa::Auto,
            min_width: Lpa::Auto,
            min_height: Lpa::Auto,
            max_width: None,
            max_height: None,
            border_box: false,
            margin: [Lpa::L(Lp::ZERO); 4],
            padding: [Lp::ZERO; 4],
            border_w: [0.0; 4],
            border_style: [BorderStyle::None; 4],
            border_color: [None; 4],
            radius: [(Lp::ZERO, Lp::ZERO); 4],
            flex_row: true,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: Lpa::Auto,
            align_items: None,
            align_self: None,
            justify_content: None,
            justify_items: None,
            align_content: None,
            row_gap: Lp::ZERO,
            column_gap: Lp::ZERO,
            grid_cols: Vec::new(),
            overflow_x: Overflow::Visible,
            overflow_y: Overflow::Visible,
            z_index: None,
            opacity: 1.0,
            visible: true,
            pointer_events: true,
            background: Bg::None,
            shadows: Vec::new(),
            transform: Vec::new(),
            origin: (Lp::pct(50.0), Lp::pct(50.0)),
            translate: (Lp::ZERO, Lp::ZERO),
            filter: Vec::new(),
            mask: None,
            outline: None,
            outline_offset: 0.0,
            color: Color::BLACK,
            font_family: Rc::from("system-ui"),
            font_size: 16.0,
            font_weight: 400,
            line_height: LineHeight::Normal,
            letter_spacing: 0.0,
            text_align: TextAlign::Left,
            uppercase: false,
            white_space: WhiteSpace::Normal,
            break_word: false,
            tabular: false,
            text_shadow: Vec::new(),
            fill: Some(Some(Color::BLACK)),
            stroke: None,
            stroke_width: 1.0,
            round_cap: false,
            round_join: false,
            cursor_pointer: false,
            select_text: true,
            ellipsis: false,
            line_through: false,
            content: None,
            cover: false,
            vertical_align: 0.0,
            transitions: Rc::new(Vec::new()),
            animations: Rc::new(Vec::new()),
            vars: Rc::new(HashMap::new()),
        }
    }
}

impl Style {
    /// Initial values with the inherited properties copied from `p`.
    pub fn inherit(p: &Style) -> Style {
        Style {
            color: p.color,
            font_family: p.font_family.clone(),
            font_size: p.font_size,
            font_weight: p.font_weight,
            line_height: p.line_height,
            letter_spacing: p.letter_spacing,
            text_align: p.text_align,
            uppercase: p.uppercase,
            white_space: p.white_space,
            break_word: p.break_word,
            tabular: p.tabular,
            text_shadow: p.text_shadow.clone(),
            fill: p.fill,
            stroke: p.stroke,
            stroke_width: p.stroke_width,
            round_cap: p.round_cap,
            round_join: p.round_join,
            cursor_pointer: p.cursor_pointer,
            select_text: p.select_text,
            visible: p.visible,
            pointer_events: p.pointer_events,
            vars: p.vars.clone(),
            ..Style::default()
        }
    }

    /// Line box height in px for `line-height: normal` fallback `normal_lh` (font ascent+descent).
    pub fn line_px(&self, normal_lh: f32) -> f32 {
        match self.line_height {
            LineHeight::Normal => normal_lh,
            LineHeight::Mul(m) => m * self.font_size,
            LineHeight::Px(p) => p,
        }
    }

    pub fn is_scroll_y(&self) -> bool {
        self.overflow_y == Overflow::Auto
    }
    pub fn clips(&self) -> bool {
        self.overflow_x != Overflow::Visible || self.overflow_y != Overflow::Visible
    }
}

/// Styles of one element and its pseudo-elements.
#[derive(Clone, Debug, PartialEq)]
pub struct ElemStyle {
    pub base: Style,
    pub before: Option<Style>,
    pub after: Option<Style>,
    pub placeholder: Option<Color>,
}

// ------------------------------------------------------------------------------------------------

const UA: &str = r#"
html, body, div, section, nav, ol, ul, li, p, header, footer, main { display: block; }
head, script, style, title, meta, link, [hidden] { display: none; }
b, strong { font-weight: 700; }
ol, ul { padding-left: 40px; }
svg { overflow: hidden; }
img, svg { display: inline-block; }
button {
  display: inline-block; padding: 1px 6px; border: 2px outset ButtonBorder;
  background-color: ButtonFace; color: ButtonText; text-align: center;
  font-size: 13.333px; font-weight: 400; cursor: default;
  align-items: center; justify-content: center;
}
input, textarea {
  display: inline-block; padding: 1px 2px; border: 2px inset ButtonBorder;
  background-color: Field; color: FieldText; font-size: 13.333px; font-weight: 400;
  user-select: text; letter-spacing: normal; text-transform: none; text-align: start;
}
textarea { white-space: pre-wrap; word-break: break-word; }
input { white-space: nowrap; }
textarea::placeholder, input::placeholder { color: rgba(255, 255, 255, .5); }
"#;

#[derive(Clone)]
struct IndexedRule {
    sel: Selector,
    decls: Rc<Vec<Decl>>,
    order: u32,
    ua: bool,
    reduced: bool,
}

pub struct Cascade {
    rules: Vec<IndexedRule>,
    /// Rule indices by key of the rightmost compound (`#id`, `.class`, `tag`, `*`).
    index: HashMap<String, Vec<usize>>,
    pub keyframes: HashMap<String, Keyframes>,
    pub reduced_motion: bool,
    warned: RefCell<HashSet<String>>,
}

fn key_of(sel: &Selector) -> String {
    let c = sel.parts.last().unwrap();
    if let Some(id) = &c.id {
        format!("#{id}")
    } else if let Some(cl) = c.classes.first() {
        format!(".{cl}")
    } else if let Some(t) = &c.tag {
        t.clone()
    } else {
        "*".into()
    }
}

impl Cascade {
    pub fn new(author: &str) -> Cascade {
        let ua = parse_sheet(UA);
        let au = parse_sheet(author);
        let mut c = Cascade {
            rules: Vec::new(),
            index: HashMap::new(),
            keyframes: HashMap::new(),
            reduced_motion: false,
            warned: RefCell::new(HashSet::new()),
        };
        let mut order = 0;
        for (sheet, is_ua) in [(&ua, true), (&au, false)] {
            let Sheet { rules, keyframes } = sheet;
            for Rule { selectors, decls, reduced_motion } in rules {
                let decls = Rc::new(decls.clone());
                for sel in selectors {
                    let i = c.rules.len();
                    c.index.entry(key_of(sel)).or_default().push(i);
                    c.rules.push(IndexedRule { sel: sel.clone(), decls: decls.clone(), order, ua: is_ua, reduced: *reduced_motion });
                    order += 1;
                }
            }
            for k in keyframes {
                c.keyframes.insert(k.name.clone(), k.clone());
            }
        }
        c
    }

    fn candidates(&self, d: &Dom, n: NodeId) -> Vec<usize> {
        let e = d.elem(n).unwrap();
        let mut out: Vec<usize> = Vec::new();
        let mut push = |k: &str| {
            if let Some(v) = self.index.get(k) {
                out.extend_from_slice(v);
            }
        };
        push("*");
        push(&e.tag);
        if let Some(id) = d.attr(n, "id") {
            push(&format!("#{id}"));
        }
        for c in &e.classes {
            push(&format!(".{c}"));
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Computes styles for every connected element. `out[id]` = `None` for text/detached nodes.
    pub fn compute(&self, d: &Dom) -> Vec<Option<Rc<ElemStyle>>> {
        let mut out: Vec<Option<Rc<ElemStyle>>> = vec![None; d.len()];
        let root_parent = Style { color: Color::BLACK, ..Style::default() };
        let mut stack = vec![(d.html, Rc::new(ElemStyle { base: root_parent, before: None, after: None, placeholder: None }))];
        while let Some((n, parent)) = stack.pop() {
            let es = Rc::new(self.compute_one(d, n, &parent.base));
            for &c in d.children(n).iter().rev() {
                if d.is_element(c) {
                    stack.push((c, es.clone()));
                }
            }
            out[n as usize] = Some(es);
        }
        out
    }

    fn compute_one(&self, d: &Dom, n: NodeId, parent: &Style) -> ElemStyle {
        let mut buckets: [Vec<(bool, (u16, u16, u16), u32, &Rc<Vec<Decl>>)>; 4] = Default::default();
        for i in self.candidates(d, n) {
            let r = &self.rules[i];
            if r.reduced && !self.reduced_motion {
                continue;
            }
            if !matches_selector(d, n, &r.sel) {
                continue;
            }
            let slot = match r.sel.pe {
                None => 0,
                Some(PseudoEl::Before) => 1,
                Some(PseudoEl::After) => 2,
                Some(PseudoEl::Placeholder) => 3,
                Some(_) => continue,
            };
            buckets[slot].push((!r.ua, r.sel.spec, r.order, &r.decls));
        }
        let inline: Vec<Decl> = match &d.node(n).unwrap().kind {
            Kind::Element(e) => e.inline.iter().map(|(k, v)| Decl { name: k.clone(), value: v.clone(), important: false }).collect(),
            _ => Vec::new(),
        };
        let base = self.cascade_decls(&mut buckets[0], Some(&inline), parent);
        let pseudo = |b: &mut Vec<_>| -> Option<Style> {
            if b.is_empty() {
                return None;
            }
            let s = self.cascade_decls(b, None, &base);
            s.content.is_some().then_some(s)
        };
        let before = pseudo(&mut buckets[1]);
        let after = pseudo(&mut buckets[2]);
        let placeholder = if buckets[3].is_empty() { None } else { Some(self.cascade_decls(&mut buckets[3], None, &base).color) };
        ElemStyle { base, before, after, placeholder }
    }

    fn cascade_decls(
        &self,
        matched: &mut [(bool, (u16, u16, u16), u32, &Rc<Vec<Decl>>)],
        inline: Option<&[Decl]>,
        parent: &Style,
    ) -> Style {
        matched.sort_by(|a, b| (a.0, a.1, a.2).cmp(&(b.0, b.1, b.2)));
        let mut normal: Vec<&Decl> = Vec::new();
        let mut important: Vec<&Decl> = Vec::new();
        for (_, _, _, decls) in matched.iter() {
            for dcl in decls.iter() {
                if dcl.important {
                    important.push(dcl);
                } else {
                    normal.push(dcl);
                }
            }
        }
        if let Some(i) = inline {
            normal.extend(i.iter());
        }
        normal.extend(important);

        // Custom properties first, then substitute against the element's final map.
        let mut s = Style::inherit(parent);
        let mut own: Vec<(&str, &str)> = Vec::new();
        for dcl in &normal {
            if dcl.name.starts_with("--") {
                own.push((&dcl.name, &dcl.value));
            }
        }
        if !own.is_empty() {
            let mut map = (*parent.vars).clone();
            for (k, val) in &own {
                map.insert(k.to_string(), val.to_string());
            }
            // Resolve references inside custom properties (one pass is enough after recursion).
            let snapshot = map.clone();
            for (k, val) in map.iter_mut() {
                if val.contains("var(") {
                    *val = subst(val, &snapshot, 0).unwrap_or_default();
                }
                let _ = k;
            }
            s.vars = Rc::new(map);
        }
        // `color` first so later `currentColor` values see the element's own colour.
        for dcl in normal.iter().filter(|d| d.name == "color").chain(normal.iter().filter(|d| d.name != "color" && !d.name.starts_with("--"))) {
            let value = if dcl.value.contains("var(") {
                match subst(&dcl.value, &s.vars, 0) {
                    Some(x) => x,
                    None => {
                        self.unset(&mut s, parent, &dcl.name);
                        continue;
                    }
                }
            } else {
                dcl.value.clone()
            };
            if !apply(&mut s, parent, &dcl.name, value.trim()) {
                if !KNOWN_IGNORED.contains(&dcl.name.as_str()) && self.warned.borrow_mut().insert(dcl.name.clone()) {
                    crate::conf::log(&format!("css: unknown property {}", dcl.name));
                }
            }
        }
        s
    }

    fn unset(&self, s: &mut Style, parent: &Style, name: &str) {
        let fresh = Style::inherit(parent);
        let mut tmp = fresh.clone();
        // Copy the initial/inherited value for `name` by applying `inherit`/`initial` semantics:
        // inherited props already equal `fresh`; non-inherited are reset to initial.
        if apply(&mut tmp, parent, name, "initial") {
            copy_prop(s, &tmp, name);
        }
    }
}

const KNOWN_IGNORED: &[&str] = &[
    "color-scheme",
    "-webkit-font-smoothing",
    "contain",
    "will-change",
    "scrollbar-width",
    "scrollbar-color",
    "list-style",
    "resize",
    "-webkit-user-select",
    "-webkit-mask",
];

/// Substitutes `var(--x, fallback)`; `None` when a reference without fallback is missing.
pub fn subst(s: &str, vars: &HashMap<String, String>, depth: u32) -> Option<String> {
    if depth > 16 {
        return None;
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("var(") {
        out.push_str(&rest[..i]);
        let body_start = i + 4;
        let mut depth_p = 1;
        let mut end = rest.len();
        for (j, c) in rest[body_start..].char_indices() {
            match c {
                '(' => depth_p += 1,
                ')' => {
                    depth_p -= 1;
                    if depth_p == 0 {
                        end = body_start + j;
                        break;
                    }
                }
                _ => {}
            }
        }
        let inner = &rest[body_start..end];
        let (name, fallback) = match inner.find(',') {
            Some(c) => (inner[..c].trim(), Some(inner[c + 1..].trim())),
            None => (inner.trim(), None),
        };
        let val = match vars.get(name) {
            Some(x) => x.clone(),
            None => fallback?.to_string(),
        };
        let val = if val.contains("var(") { subst(&val, vars, depth + 1)? } else { val };
        out.push_str(&val);
        rest = &rest[(end + 1).min(rest.len())..];
    }
    out.push_str(rest);
    Some(out)
}

fn align(s: &str) -> Option<Option<Align>> {
    Some(Some(match s {
        "normal" => return Some(None),
        "flex-start" | "start" | "left" => Align::Start,
        "flex-end" | "end" | "right" => Align::End,
        "center" => Align::Center,
        "stretch" => Align::Stretch,
        "baseline" => Align::Baseline,
        "space-between" => Align::SpaceBetween,
        "space-around" => Align::SpaceAround,
        _ => return None,
    }))
}

/// 1–4 value box shorthand → [top, right, bottom, left].
fn four<T: Copy>(s: &str, f: impl Fn(&str) -> Option<T>) -> Option<[T; 4]> {
    let w: Option<Vec<T>> = v::words(s).into_iter().map(f).collect();
    let w = w?;
    Some(match w.len() {
        1 => [w[0]; 4],
        2 => [w[0], w[1], w[0], w[1]],
        3 => [w[0], w[1], w[2], w[1]],
        4 => [w[0], w[1], w[2], w[3]],
        _ => return None,
    })
}

fn border_style(s: &str) -> Option<BorderStyle> {
    Some(match s {
        "none" | "hidden" => BorderStyle::None,
        "solid" => BorderStyle::Solid,
        "dashed" | "dotted" => BorderStyle::Dashed,
        "outset" => BorderStyle::Outset,
        "inset" => BorderStyle::Inset,
        _ => return None,
    })
}

fn border_width(s: &str) -> Option<f32> {
    match s {
        "thin" => Some(1.0),
        "medium" => Some(3.0),
        "thick" => Some(5.0),
        _ => v::px(s),
    }
}

/// `border: width style color` (any order, any subset).
fn border_parts(s: &str, cur: Color) -> Option<(f32, BorderStyle, Option<Color>)> {
    let (mut w, mut st, mut c) = (3.0, BorderStyle::None, None);
    if s == "0" || s == "none" {
        return Some((0.0, BorderStyle::None, None));
    }
    for t in v::words(s) {
        if let Some(x) = border_width(t) {
            w = x;
        } else if let Some(x) = border_style(t) {
            st = x;
        } else {
            c = Some(v::color(t, cur)?);
        }
    }
    Some((w, st, c))
}

fn radii(s: &str) -> Option<[(Lp, Lp); 4]> {
    let (h, vv) = match s.split_once('/') {
        Some((a, b)) => (a.trim(), b.trim()),
        None => (s, s),
    };
    let h = four(h, v::lp)?;
    let vv = four(vv, v::lp)?;
    // four() returns [top, right, bottom, left] ordering which maps to tl, tr, br, bl for radii.
    Some([(h[0], vv[0]), (h[1], vv[1]), (h[2], vv[2]), (h[3], vv[3])])
}

fn transitions(s: &str) -> Option<Vec<Transition>> {
    if s == "none" {
        return Some(Vec::new());
    }
    let mut out = Vec::new();
    for part in v::split_top(s, ',') {
        let (mut prop, mut times, mut easing) = (None, Vec::new(), None);
        for w in v::words(part) {
            if let Some(t) = v::time_ms(w).filter(|_| w != "0" || !times.is_empty() || prop.is_some()) {
                times.push(t);
            } else if let Some(e) = v::easing(w) {
                easing = Some(e);
            } else {
                prop = Some(w.to_string());
            }
        }
        out.push(Transition {
            prop: prop.unwrap_or_else(|| "all".into()),
            dur: times.first().copied().unwrap_or(0.0),
            delay: times.get(1).copied().unwrap_or(0.0),
            easing: easing.unwrap_or(crate::gfx::anim::EASE),
        });
    }
    Some(out)
}

fn animations(s: &str) -> Option<Vec<Animation>> {
    if s == "none" {
        return Some(Vec::new());
    }
    let mut out = Vec::new();
    for part in v::split_top(s, ',') {
        let mut a = Animation {
            name: String::new(),
            dur: 0.0,
            delay: 0.0,
            easing: crate::gfx::anim::EASE,
            count: 1.0,
            alternate: false,
            paused: false,
        };
        let mut times = 0;
        for w in v::words(part) {
            if let Some(t) = v::time_ms(w).filter(|_| w.ends_with('s')) {
                if times == 0 {
                    a.dur = t;
                } else {
                    a.delay = t;
                }
                times += 1;
            } else if w == "infinite" {
                a.count = f32::INFINITY;
            } else if w == "alternate" {
                a.alternate = true;
            } else if w == "normal" || w == "none" || w == "forwards" || w == "both" || w == "running" {
            } else if w == "paused" {
                a.paused = true;
            } else if let Some(n) = v::number(w) {
                a.count = n;
            } else if let Some(e) = v::easing(w) {
                a.easing = e;
            } else {
                a.name = w.to_string();
            }
        }
        out.push(a);
    }
    Some(out)
}

fn copy_prop(dst: &mut Style, src: &Style, name: &str) {
    macro_rules! cp {
        ($($f:ident),*) => {{ $(dst.$f = src.$f.clone();)* }};
    }
    match name {
        "border-radius" => cp!(radius),
        "font" => cp!(font_family, font_size, font_weight, line_height, tabular),
        "font-family" => cp!(font_family),
        "font-size" => cp!(font_size),
        "font-weight" => cp!(font_weight),
        "line-height" => cp!(line_height),
        "color" => cp!(color),
        "display" => cp!(display),
        "width" => cp!(width),
        "height" => cp!(height),
        "opacity" => cp!(opacity),
        "transform" => cp!(transform),
        "transition" => cp!(transitions),
        "animation" | "animation-name" => cp!(animations),
        "background" | "background-color" | "background-image" => cp!(background),
        "visibility" => cp!(visible),
        "box-shadow" => cp!(shadows),
        "filter" => cp!(filter),
        "translate" => cp!(translate),
        _ => {}
    }
}

/// Applies one declaration (value already `var()`-substituted). Returns false if unsupported.
pub fn apply(s: &mut Style, parent: &Style, name: &str, val: &str) -> bool {
    if val == "inherit" {
        copy_prop(s, parent, name);
        if name.starts_with("border-") && name.ends_with("-radius") {
            s.radius = parent.radius;
        }
        return true;
    }
    if val == "initial" || val == "unset" {
        let init = Style::inherit(parent);
        let init = if val == "initial" { Style { vars: s.vars.clone(), ..Style::default() } } else { init };
        copy_prop(s, &init, name);
        return true;
    }
    let cur = s.color;
    macro_rules! set {
        ($e:expr) => {
            match $e {
                Some(x) => x,
                None => return false,
            }
        };
    }
    match name {
        "display" => {
            s.display = set!(match val {
                "none" => Some(Display::None),
                "block" | "list-item" => Some(Display::Block),
                "flex" => Some(Display::Flex),
                "grid" => Some(Display::Grid),
                "inline" => Some(Display::Inline),
                "inline-block" | "inline-flex" | "inline-grid" => Some(Display::InlineBlock),
                _ => None,
            })
        }
        "position" => {
            s.position = set!(match val {
                "static" => Some(Position::Static),
                "relative" => Some(Position::Relative),
                "absolute" | "fixed" => Some(Position::Absolute),
                _ => None,
            })
        }
        "top" => s.inset[0] = set!(v::lpa(val)),
        "right" => s.inset[1] = set!(v::lpa(val)),
        "bottom" => s.inset[2] = set!(v::lpa(val)),
        "left" => s.inset[3] = set!(v::lpa(val)),
        "inset" => s.inset = set!(four(val, v::lpa)),
        "width" => s.width = set!(v::lpa(val)),
        "height" => s.height = set!(v::lpa(val)),
        "min-width" => s.min_width = set!(v::lpa(val)),
        "min-height" => s.min_height = set!(v::lpa(val)),
        "max-width" => s.max_width = if val == "none" { None } else { Some(set!(v::lp(val))) },
        "max-height" => s.max_height = if val == "none" { None } else { Some(set!(v::lp(val))) },
        "box-sizing" => s.border_box = val == "border-box",
        "margin" => s.margin = set!(four(val, v::lpa)),
        "margin-top" => s.margin[0] = set!(v::lpa(val)),
        "margin-right" => s.margin[1] = set!(v::lpa(val)),
        "margin-bottom" => s.margin[2] = set!(v::lpa(val)),
        "margin-left" => s.margin[3] = set!(v::lpa(val)),
        "padding" => s.padding = set!(four(val, v::lp)),
        "padding-top" => s.padding[0] = set!(v::lp(val)),
        "padding-right" => s.padding[1] = set!(v::lp(val)),
        "padding-bottom" => s.padding[2] = set!(v::lp(val)),
        "padding-left" => s.padding[3] = set!(v::lp(val)),
        "border" => {
            let (w, st, c) = set!(border_parts(val, cur));
            s.border_w = [w; 4];
            s.border_style = [st; 4];
            s.border_color = [c; 4];
        }
        "border-width" => s.border_w = set!(four(val, border_width)),
        "border-style" => s.border_style = set!(four(val, border_style)),
        "border-color" => s.border_color = set!(four(val, |t| v::color(t, cur).map(Some))),
        "border-radius" => s.radius = set!(radii(val)),
        "border-top-left-radius" | "border-top-right-radius" | "border-bottom-right-radius" | "border-bottom-left-radius" => {
            let w = v::words(val);
            let h = set!(w.first().and_then(|x| v::lp(x)));
            let vv = match w.get(1) {
                Some(x) => set!(v::lp(x)),
                None => h,
            };
            let i = match name {
                "border-top-left-radius" => 0,
                "border-top-right-radius" => 1,
                "border-bottom-right-radius" => 2,
                _ => 3,
            };
            s.radius[i] = (h, vv);
        }
        "flex" => {
            let w = v::words(val);
            match w.as_slice() {
                ["none"] => (s.flex_grow, s.flex_shrink, s.flex_basis) = (0.0, 0.0, Lpa::Auto),
                ["auto"] => (s.flex_grow, s.flex_shrink, s.flex_basis) = (1.0, 1.0, Lpa::Auto),
                [g] => {
                    if let Some(n) = v::number(g) {
                        (s.flex_grow, s.flex_shrink, s.flex_basis) = (n, 1.0, Lpa::L(Lp::pct(0.0)));
                    } else {
                        (s.flex_grow, s.flex_shrink, s.flex_basis) = (1.0, 1.0, set!(v::lpa(g)));
                    }
                }
                [g, sh] => {
                    s.flex_grow = set!(v::number(g));
                    match v::number(sh) {
                        Some(n) => (s.flex_shrink, s.flex_basis) = (n, Lpa::L(Lp::pct(0.0))),
                        None => (s.flex_shrink, s.flex_basis) = (1.0, set!(v::lpa(sh))),
                    }
                }
                [g, sh, b] => {
                    s.flex_grow = set!(v::number(g));
                    s.flex_shrink = set!(v::number(sh));
                    s.flex_basis = set!(v::lpa(b));
                }
                _ => return false,
            }
        }
        "flex-direction" => s.flex_row = !val.starts_with("column"),
        "flex-grow" => s.flex_grow = set!(v::number(val)),
        "flex-shrink" => s.flex_shrink = set!(v::number(val)),
        "flex-basis" => s.flex_basis = set!(v::lpa(val)),
        "flex-wrap" => {}
        "align-items" => s.align_items = set!(align(val)),
        "align-self" => s.align_self = if val == "auto" { None } else { set!(align(val)) },
        "justify-content" => s.justify_content = set!(align(val)),
        "justify-items" => s.justify_items = set!(align(val)),
        "align-content" => s.align_content = set!(align(val)),
        "place-items" => {
            let w = v::words(val);
            s.align_items = set!(align(w[0]));
            s.justify_items = set!(align(w.get(1).copied().unwrap_or(w[0])));
        }
        "place-content" => {
            let w = v::words(val);
            s.align_content = set!(align(w[0]));
            s.justify_content = set!(align(w.get(1).copied().unwrap_or(w[0])));
        }
        "gap" => {
            let w = v::words(val);
            s.row_gap = set!(v::lp(w[0]));
            s.column_gap = set!(v::lp(w.get(1).copied().unwrap_or(w[0])));
        }
        "row-gap" => s.row_gap = set!(v::lp(val)),
        "column-gap" => s.column_gap = set!(v::lp(val)),
        "grid-template-columns" => s.grid_cols = set!(v::tracks(val)),
        "overflow" | "overflow-x" | "overflow-y" => {
            let o = set!(match val {
                "visible" => Some(Overflow::Visible),
                "hidden" | "clip" => Some(Overflow::Hidden),
                "auto" | "scroll" => Some(Overflow::Auto),
                _ => None,
            });
            if name != "overflow-y" {
                s.overflow_x = o;
            }
            if name != "overflow-x" {
                s.overflow_y = o;
            }
        }
        "z-index" => s.z_index = if val == "auto" { None } else { Some(set!(val.parse().ok())) },
        "opacity" => s.opacity = set!(v::quantity(val).map(|q| if q.units == 2 { q.pct / 100.0 } else { q.num })).clamp(0.0, 1.0),
        "visibility" => s.visible = val == "visible",
        "pointer-events" => s.pointer_events = val != "none",
        "background" | "background-image" => s.background = set!(v::background(val, cur)),
        "background-color" => s.background = Bg::Color(set!(v::color(val, cur))),
        "box-shadow" => s.shadows = set!(v::shadows(val, cur)),
        "transform" => s.transform = set!(v::transform(val)),
        "transform-origin" => {
            let w = v::words(val);
            let kw = |t: &str| match t {
                "left" | "top" => Some(Lp::pct(0.0)),
                "center" => Some(Lp::pct(50.0)),
                "right" | "bottom" => Some(Lp::pct(100.0)),
                _ => v::lp(t),
            };
            let x = set!(w.first().and_then(|t| kw(t)));
            let y = match w.get(1) {
                Some(t) => set!(kw(t)),
                None => Lp::pct(50.0),
            };
            s.origin = (x, y);
        }
        "translate" => s.translate = set!(v::translate(val)),
        "filter" => s.filter = set!(v::filters(val, cur)),
        "mask" | "-webkit-mask" | "mask-image" | "-webkit-mask-image" => s.mask = if val == "none" { None } else { Some(set!(v::gradient(val, cur))) },
        "outline" => {
            if val == "none" || val == "0" {
                s.outline = None;
            } else {
                let (w, st, c) = set!(border_parts(val, cur));
                s.outline = (st != BorderStyle::None).then_some((w, c.unwrap_or(cur)));
            }
        }
        "outline-offset" => s.outline_offset = set!(v::px(val)),
        "color" => s.color = set!(v::color(val, parent.color)),
        "font" => return false,
        "font-family" => s.font_family = Rc::from(val),
        "font-size" => s.font_size = set!(v::px(val)),
        "font-weight" => {
            s.font_weight = set!(match val {
                "normal" => Some(400),
                "bold" => Some(700),
                _ => val.parse().ok(),
            })
        }
        "line-height" => {
            s.line_height = if val == "normal" {
                LineHeight::Normal
            } else if let Some(n) = v::number(val) {
                LineHeight::Mul(n)
            } else {
                LineHeight::Px(set!(v::px(val)))
            }
        }
        "letter-spacing" => s.letter_spacing = if val == "normal" { 0.0 } else { set!(v::px(val)) },
        "text-align" => {
            s.text_align = match val {
                "center" => TextAlign::Center,
                "right" | "end" => TextAlign::Right,
                _ => TextAlign::Left,
            }
        }
        "text-transform" => s.uppercase = val == "uppercase",
        "white-space" => {
            s.white_space = set!(match val {
                "normal" => Some(WhiteSpace::Normal),
                "nowrap" => Some(WhiteSpace::NoWrap),
                "pre-wrap" | "pre-line" | "pre" => Some(WhiteSpace::PreWrap),
                _ => None,
            })
        }
        "word-break" | "overflow-wrap" => s.break_word = val == "break-word" || val == "anywhere",
        "text-overflow" => s.ellipsis = val == "ellipsis",
        "text-decoration" => s.line_through = val.contains("line-through"),
        "font-variant-numeric" => s.tabular = val.contains("tabular-nums"),
        "text-shadow" => s.text_shadow = set!(v::shadows(val, cur)),
        "fill" | "stroke" => {
            let p: Paint = if val == "none" {
                None
            } else if val.eq_ignore_ascii_case("currentcolor") {
                Some(None)
            } else {
                Some(Some(set!(v::color(val, cur))))
            };
            if name == "fill" {
                s.fill = p
            } else {
                s.stroke = p
            }
        }
        "stroke-width" => s.stroke_width = set!(v::number(val).or_else(|| v::px(val))),
        "stroke-linecap" => s.round_cap = val == "round",
        "stroke-linejoin" => s.round_join = val == "round",
        "cursor" => s.cursor_pointer = val == "pointer",
        "user-select" => s.select_text = val != "none",
        "content" => {
            s.content = if val == "none" || val == "normal" {
                None
            } else {
                Some(crate::dom::html::decode_entities(val.trim_matches(['"', '\''])))
            }
        }
        "object-fit" => s.cover = val == "cover",
        "vertical-align" => s.vertical_align = v::px(val).unwrap_or(0.0),
        "transition" => s.transitions = Rc::new(set!(transitions(val))),
        "animation" => s.animations = Rc::new(set!(animations(val))),
        "animation-name" => {
            let mut a = (*s.animations).clone();
            if let Some(x) = a.first_mut() {
                x.name = val.to_string();
            }
            s.animations = Rc::new(a);
        }
        "animation-delay" => {
            let mut a = (*s.animations).clone();
            for x in a.iter_mut() {
                x.delay = set!(v::time_ms(val));
            }
            s.animations = Rc::new(a);
        }
        "animation-play-state" => {
            let mut a = (*s.animations).clone();
            for x in a.iter_mut() {
                x.paused = val == "paused";
            }
            s.animations = Rc::new(a);
        }
        n if KNOWN_IGNORED.contains(&n) => {}
        _ => return false,
    }
    true
}

/// `font: inherit` is the only `font` shorthand in the sheet.
fn _font_note() {}

/// Serializes a computed property for `getComputedStyle`.
pub fn serialize(s: &Style, prop: &str, box_w: f32, box_h: f32) -> String {
    let pxs = |x: f32| format!("{}px", (x * 1000.0).round() / 1000.0);
    match prop {
        "width" => pxs(box_w),
        "height" => pxs(box_h),
        "border-bottom-left-radius" => pxs(s.radius[3].0.resolve(box_w)),
        "border-bottom-right-radius" => pxs(s.radius[2].0.resolve(box_w)),
        "border-top-left-radius" => pxs(s.radius[0].0.resolve(box_w)),
        "border-top-right-radius" => pxs(s.radius[1].0.resolve(box_w)),
        "opacity" => format!("{}", s.opacity),
        "display" => format!("{:?}", s.display).to_ascii_lowercase(),
        "visibility" => (if s.visible { "visible" } else { "hidden" }).into(),
        "font-size" => pxs(s.font_size),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn styles(d: &Dom) -> Vec<Option<Rc<ElemStyle>>> {
        Cascade::new(include_str!("../../ui/style.css")).compute(d)
    }

    #[test]
    fn island_idle_and_open() {
        let mut d = Dom::from_index();
        let island = d.by_id("island").unwrap();
        let st = styles(&d);
        let s = &st[island as usize].as_ref().unwrap().base;
        assert_eq!(s.width, Lpa::L(Lp::px(168.0)));
        assert_eq!(s.radius[3], (Lp::px(16.0), Lp::px(16.0)));
        assert_eq!(s.transitions.len(), 3);
        let before = st[island as usize].as_ref().unwrap().before.as_ref().unwrap();
        assert_eq!(before.width, Lpa::L(Lp::px(11.0)));

        d.set_attr(d.body, "data-state", "open");
        let st = styles(&d);
        let s = &st[island as usize].as_ref().unwrap().base;
        assert_eq!(s.vars.get("--r").map(String::as_str), Some("26px"));
        assert_eq!(s.radius[3].0, Lp::px(26.0));
        let open = d.query_str(d.html, ".open", false)[0];
        let o = &st[open as usize].as_ref().unwrap().base;
        assert_eq!(o.opacity, 1.0);
        assert!(o.visible);
        assert_eq!(o.transitions.iter().map(|t| t.prop.as_str()).collect::<Vec<_>>(), ["opacity", "transform", "visibility"]);
        assert_eq!(o.transitions[0].delay, 200.0);
    }

    #[test]
    fn page_tab_and_eyes_specificity() {
        let mut d = Dom::from_index();
        let nav = d.by_id("pageNav").unwrap();
        let tab = d.create_element("button");
        d.set_attr(tab, "class", "page-tab on");
        d.append_child(nav, tab);
        d.set_attr(d.body, "data-state", "open");
        let st = styles(&d);
        let t = &st[tab as usize].as_ref().unwrap().base;
        match t.background {
            Bg::Color(c) => assert!((c.a - 0.16).abs() < 1e-4 && (c.g - 184.0 / 255.0).abs() < 1e-3, "{c:?}"),
            ref b => panic!("{b:?}"),
        }
        assert_eq!(t.border_w, [0.0; 4]);
        let mini = d.by_id("miniEyes").unwrap();
        assert_eq!(st[mini as usize].as_ref().unwrap().base.column_gap, Lp::px(10.0));
    }

    #[test]
    fn reduced_motion_kills_transitions() {
        let d = Dom::from_index();
        let mut c = Cascade::new(include_str!("../../ui/style.css"));
        c.reduced_motion = true;
        let st = c.compute(&d);
        let island = d.by_id("island").unwrap();
        assert!(st[island as usize].as_ref().unwrap().base.transitions.is_empty());
    }

    #[test]
    fn every_declaration_is_understood() {
        let sheet = parse_sheet(include_str!("../../ui/style.css"));
        let root = sheet.rules.iter().find(|r| r.selectors[0].parts[0].pseudos.contains(&crate::css::select::Pseudo::Root)).unwrap();
        let mut vars: HashMap<String, String> = root.decls.iter().filter(|d| d.name.starts_with("--")).map(|d| (d.name.clone(), d.value.clone())).collect();
        vars.extend([("--pupil-x", "0px"), ("--pupil-y", "0px"), ("--eye-expression", "scaleY(1)"), ("--dx", "18px")].map(|(a, b)| (a.to_string(), b.to_string())));
        let mut bad = Vec::new();
        let decls = sheet.rules.iter().flat_map(|r| r.decls.iter()).chain(sheet.keyframes.iter().flat_map(|k| k.frames.iter().flat_map(|f| f.1.iter())));
        for d in decls {
            if d.name.starts_with("--") || KNOWN_IGNORED.contains(&d.name.as_str()) {
                continue;
            }
            let val = subst(&d.value, &vars, 0).unwrap_or_default();
            let mut s = Style::default();
            if !apply(&mut s, &Style::default(), &d.name, val.trim()) {
                bad.push(format!("{}: {}", d.name, val));
            }
        }
        assert!(bad.is_empty(), "{bad:#?}");
    }
}
