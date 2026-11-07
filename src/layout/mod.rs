//! Box layout on taffy. The tree is rebuilt on every pass (a few hundred nodes); DirectWrite text
//! layouts are cached by `gfx::text::Text`, so a pass costs well under a millisecond.
use crate::css::cascade::{Align, Display, ElemStyle, LineHeight, Overflow, Position, Style, TextAlign, WhiteSpace};
use crate::css::value::{Lp, Lpa, Track};
use crate::dom::{Dom, NodeId};
use crate::gfx::text::{self, Text, TextStyle};
use std::collections::HashMap;
use std::rc::Rc;
use taffy::prelude::{
    AvailableSpace, Dimension, LengthPercentage, LengthPercentageAuto, NodeId as TNode, Rect, Size, TaffyTree,
};
use taffy::style_helpers::{FromFr, FromLength};
use windows::Win32::Graphics::DirectWrite::{IDWriteTextLayout, DWRITE_LINE_METRICS, DWRITE_TEXT_METRICS};

pub const VIEW_W: f32 = crate::win::window::WIN_W as f32;
pub const VIEW_H: f32 = crate::win::window::WIN_H as f32;

/// Which box of an element.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Part {
    Main,
    Before,
    After,
    /// Anonymous text run: `(element, index of the text child)`.
    Anon(u32),
}

/// Border box in document coordinates (before transforms and scrolling).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BoxGeom {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// top, right, bottom, left
    pub border: [f32; 4],
    pub padding: [f32; 4],
    /// Scrollable content extent from the padding-box origin (scroll containers and leaves).
    pub content_w: f32,
    pub content_h: f32,
}

impl BoxGeom {
    pub fn padding_box(&self) -> (f32, f32, f32, f32) {
        let b = self.border;
        (self.x + b[3], self.y + b[0], self.w - b[1] - b[3], self.h - b[0] - b[2])
    }
    pub fn content_box(&self) -> (f32, f32, f32, f32) {
        let (x, y, w, h) = self.padding_box();
        let p = self.padding;
        (x + p[3], y + p[0], (w - p[1] - p[3]).max(0.0), (h - p[0] - p[2]).max(0.0))
    }
    pub fn client_h(&self) -> f32 {
        self.h - self.border[0] - self.border[2]
    }
    pub fn client_w(&self) -> f32 {
        self.w - self.border[1] - self.border[3]
    }
    pub fn scroll_h(&self) -> f32 {
        self.client_h().max(self.content_h)
    }
    pub fn scroll_w(&self) -> f32 {
        self.client_w().max(self.content_w)
    }
}

/// Text of an inline leaf, ready to draw at its content box.
#[derive(Clone)]
pub struct Leaf {
    pub text: String,
    pub style: TextStyle,
    pub layout: IDWriteTextLayout,
    /// `.caret` inline-block positioned after the text.
    pub caret: Option<NodeId>,
    /// Text drawn for an empty form control (`::placeholder`).
    pub placeholder: bool,
    /// Offset of the text inside the content box (buttons and centring grids/flexes).
    pub dx: f32,
    pub dy: f32,
}

#[derive(Default)]
pub struct Layout {
    pub boxes: HashMap<(NodeId, Part), BoxGeom>,
    pub leaves: HashMap<(NodeId, Part), Leaf>,
    /// Order of boxes per element for painting (children parts in tree order).
    pub kids: HashMap<NodeId, Vec<Kid>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kid {
    Elem(NodeId),
    Part(Part),
}

#[derive(Clone)]
enum Ctx {
    Text { key: (NodeId, Part), text: String, style: TextStyle, inset: [f32; 4] },
    /// Form control: measured from its text but with Chromium's intrinsic width.
    Control { key: (NodeId, Part), text: String, style: TextStyle, inset: [f32; 4], multiline: bool },
}

fn lp(l: Lp) -> LengthPercentage {
    if l.pct != 0.0 && l.px == 0.0 {
        LengthPercentage::percent(l.pct / 100.0)
    } else if l.pct == 0.0 {
        LengthPercentage::length(l.px)
    } else {
        // taffy calc needs a resolver; px+% only appears in transforms here. Fall back to the
        // percentage share resolved against the viewport width (never hit by style.css layouts).
        LengthPercentage::length(l.px + l.pct * VIEW_W / 100.0)
    }
}
fn lpa(l: Lpa) -> LengthPercentageAuto {
    match l {
        Lpa::Auto => LengthPercentageAuto::auto(),
        Lpa::L(x) if x.pct != 0.0 && x.px == 0.0 => LengthPercentageAuto::percent(x.pct / 100.0),
        Lpa::L(x) => LengthPercentageAuto::length(x.px + x.pct * VIEW_W / 100.0),
    }
}
fn dim(l: Lpa) -> Dimension {
    match l {
        Lpa::Auto => Dimension::auto(),
        Lpa::L(x) if x.pct != 0.0 && x.px == 0.0 => Dimension::percent(x.pct / 100.0),
        Lpa::L(x) => Dimension::length(x.px + x.pct * VIEW_W / 100.0),
    }
}
fn ai(a: Option<Align>) -> Option<taffy::AlignItems> {
    use taffy::AlignItems as A;
    a.map(|a| match a {
        Align::Start => A::FLEX_START,
        Align::End => A::FLEX_END,
        Align::Center => A::CENTER,
        Align::Stretch => A::STRETCH,
        Align::Baseline => A::BASELINE,
        _ => A::STRETCH,
    })
}
fn ac(a: Option<Align>) -> Option<taffy::AlignContent> {
    use taffy::AlignContent as A;
    a.map(|a| match a {
        Align::Start => A::FLEX_START,
        Align::End => A::FLEX_END,
        Align::Center => A::CENTER,
        Align::Stretch => A::STRETCH,
        Align::SpaceBetween => A::SPACE_BETWEEN,
        Align::SpaceAround => A::SPACE_AROUND,
        Align::Baseline => A::FLEX_START,
    })
}

/// CSS whitespace processing + text-transform.
pub fn process_text(s: &str, st: &Style) -> String {
    let mut out = if st.white_space == WhiteSpace::PreWrap {
        s.to_string()
    } else {
        let mut o = String::with_capacity(s.len());
        let mut sp = false;
        for c in s.chars() {
            if c == ' ' || c == '\n' || c == '\t' || c == '\r' {
                sp = true;
            } else {
                if sp && !o.is_empty() {
                    o.push(' ');
                }
                sp = false;
                o.push(c);
            }
        }
        if sp && !o.is_empty() {
            o.push(' ');
        }
        o
    };
    if st.uppercase {
        out = out.to_uppercase();
    }
    out
}

pub fn text_style(st: &Style) -> TextStyle {
    TextStyle {
        size: st.font_size,
        weight: st.font_weight,
        wrap: st.white_space != WhiteSpace::NoWrap,
        ellipsis: st.ellipsis && st.white_space == WhiteSpace::NoWrap && st.clips(),
        line_height: match st.line_height {
            LineHeight::Normal => None,
            _ => Some(st.line_px(0.0)),
        },
        align: match st.text_align {
            TextAlign::Left => text::Align::Left,
            TextAlign::Center => text::Align::Center,
            TextAlign::Right => text::Align::Right,
        },
        tabular: st.tabular,
        italic: false,
        letter_spacing: st.letter_spacing,
        strike: st.line_through,
        break_word: st.break_word,
    }
}

/// Every box here is either blockified (flex/grid item) or a block; general inline layout is not
/// needed by the UI. ponytail: inline boxes inside text become separate rows; add an inline
/// formatting context if the UI ever nests styled spans inside running text.
fn effective_display(d: Display, _parent_is_box: bool) -> Display {
    match d {
        Display::Inline | Display::InlineBlock => Display::Block,
        x => x,
    }
}

fn is_control(tag: &str) -> bool {
    matches!(tag, "input" | "textarea")
}

struct Builder<'a> {
    dom: &'a Dom,
    styles: &'a [Option<Rc<ElemStyle>>],
    tree: TaffyTree<Ctx>,
    map: Vec<((NodeId, Part), TNode)>,
    kids: HashMap<NodeId, Vec<Kid>>,
    carets: HashMap<(NodeId, Part), NodeId>,
}

impl<'a> Builder<'a> {
    fn style(&self, n: NodeId) -> Option<&'a ElemStyle> {
        self.styles.get(n as usize).and_then(|s| s.as_deref())
    }

    fn taffy_style(&self, st: &Style, display: Display) -> taffy::Style {
        let mut t = taffy::Style {
            display: match display {
                Display::None => taffy::Display::None,
                Display::Flex => taffy::Display::Flex,
                Display::Grid => taffy::Display::Grid,
                _ => taffy::Display::Block,
            },
            box_sizing: if st.border_box { taffy::BoxSizing::BorderBox } else { taffy::BoxSizing::ContentBox },
            position: if st.position == Position::Absolute { taffy::Position::Absolute } else { taffy::Position::Relative },
            inset: Rect { top: lpa(st.inset[0]), right: lpa(st.inset[1]), bottom: lpa(st.inset[2]), left: lpa(st.inset[3]) },
            size: Size { width: dim(st.width), height: dim(st.height) },
            min_size: Size { width: lpa(st.min_width), height: lpa(st.min_height) },
            max_size: Size {
                width: st.max_width.map(|l| lpa(Lpa::L(l))).unwrap_or(LengthPercentageAuto::auto()),
                height: st.max_height.map(|l| lpa(Lpa::L(l))).unwrap_or(LengthPercentageAuto::auto()),
            },
            margin: Rect { top: lpa(st.margin[0]), right: lpa(st.margin[1]), bottom: lpa(st.margin[2]), left: lpa(st.margin[3]) },
            padding: Rect { top: lp(st.padding[0]), right: lp(st.padding[1]), bottom: lp(st.padding[2]), left: lp(st.padding[3]) },
            border: Rect {
                top: LengthPercentage::length(bw(st, 0)),
                right: LengthPercentage::length(bw(st, 1)),
                bottom: LengthPercentage::length(bw(st, 2)),
                left: LengthPercentage::length(bw(st, 3)),
            },
            flex_direction: if st.flex_row { taffy::FlexDirection::Row } else { taffy::FlexDirection::Column },
            flex_grow: st.flex_grow,
            flex_shrink: st.flex_shrink,
            flex_basis: dim(st.flex_basis),
            align_items: ai(st.align_items).unwrap_or(taffy::AlignItems::NORMAL),
            align_self: ai(st.align_self),
            justify_items: ai(st.justify_items).unwrap_or(taffy::AlignItems::NORMAL),
            justify_content: ac(st.justify_content).unwrap_or(taffy::AlignContent::NORMAL),
            align_content: ac(st.align_content).unwrap_or(taffy::AlignContent::NORMAL),
            gap: Size { width: lp(st.column_gap), height: lp(st.row_gap) },
            overflow: taffy::Point {
                x: if st.overflow_x == Overflow::Visible { taffy::Overflow::Visible } else { taffy::Overflow::Hidden },
                y: if st.overflow_y == Overflow::Visible { taffy::Overflow::Visible } else { taffy::Overflow::Hidden },
            },
            ..Default::default()
        };
        if !st.grid_cols.is_empty() {
            t.grid_template_columns = st
                .grid_cols
                .iter()
                .map(|tr| match *tr {
                    Track::Px(p) => taffy::GridTemplateComponent::from_length(p),
                    Track::Fr(f) => taffy::GridTemplateComponent::from_fr(f),
                })
                .collect();
        }
        t
    }

    /// Text and inline children only (an inline formatting context we can hand to DirectWrite).
    fn is_inline_leaf(&self, n: NodeId) -> bool {
        self.dom.children(n).iter().all(|&c| {
            if self.dom.text(c).is_some() {
                return true;
            }
            match self.style(c) {
                None => true,
                Some(s) if s.base.display == Display::None => true,
                Some(s) if s.base.display == Display::Inline => self.is_inline_leaf(c) && s.before.is_none() && s.after.is_none(),
                Some(_) => self.dom.has_class(c, "caret"),
            }
        })
    }

    fn build(&mut self, n: NodeId, parent_box: bool) -> Option<TNode> {
        let es = self.style(n)?;
        let st = &es.base;
        if st.display == Display::None {
            return None;
        }
        let tag = self.dom.tag(n);
        let display = effective_display(st.display, parent_box);
        let mut ts = self.taffy_style(st, display);
        let key = (n, Part::Main);
        let inset = [
            bw(st, 0) + st.padding[0].px,
            bw(st, 1) + st.padding[1].px,
            bw(st, 2) + st.padding[2].px,
            bw(st, 3) + st.padding[3].px,
        ];
        let node;
        if is_control(tag) {
            let mut v = self.dom.value(n).to_string();
            // One BMP char per char keeps caret offsets valid. ponytail: astral chars in a
            // password shift the caret by one; fine for API keys.
            if self.dom.attr(n, "type") == Some("password") {
                v = "•".repeat(v.chars().count());
            }
            let multiline = tag == "textarea";
            ts.item_is_replaced = false;
            node = self
                .tree
                .new_leaf_with_context(ts, Ctx::Control { key, text: v, style: text_style(st), inset, multiline })
                .ok()?;
        } else if tag == "img" || tag == "svg" {
            node = self.tree.new_leaf(ts).ok()?;
        } else if self.is_inline_leaf(n)
            && es.before.is_none()
            && es.after.is_none()
            && (!matches!(display, Display::Flex | Display::Grid)
                || self.dom.children(n).iter().all(|&c| self.dom.text(c).is_some() || self.dom.has_class(c, "caret")))
        {
            let raw = self.inline_text(n);
            let text = process_text(&raw, st);
            if let Some(c) = self.dom.children(n).iter().copied().find(|&c| self.dom.has_class(c, "caret") && self.style(c).is_some_and(|s| s.base.display != Display::None)) {
                self.carets.insert(key, c);
            }
            node = self.tree.new_leaf_with_context(ts, Ctx::Text { key, text, style: text_style(st), inset }).ok()?;
        } else {
            // Mixed text + boxes in a block container: lay the runs out as a baseline-aligned row.
            let has_text = self.dom.children(n).iter().any(|&c| self.dom.text(c).is_some_and(|t| !t.trim().is_empty()));
            if has_text && display == Display::Block {
                ts.display = taffy::Display::Flex;
                ts.flex_direction = taffy::FlexDirection::Row;
                ts.align_items = Some(taffy::AlignItems::BASELINE).unwrap();
            }
            let is_box = matches!(display, Display::Flex | Display::Grid) || ts.display == taffy::Display::Flex;
            let mut children = Vec::new();
            let mut kids = Vec::new();
            if let Some(b) = &es.before {
                if let Some(t) = self.pseudo(n, Part::Before, b, st) {
                    children.push(t);
                    kids.push(Kid::Part(Part::Before));
                }
            }
            for (i, &c) in self.dom.children(n).to_vec().iter().enumerate() {
                if let Some(t) = self.dom.text(c) {
                    let text = process_text(t, st);
                    if text.trim().is_empty() {
                        continue;
                    }
                    let part = Part::Anon(i as u32);
                    let k = (n, part);
                    let tsn = taffy::Style { display: taffy::Display::Block, flex_shrink: 1.0, ..Default::default() };
                    let leaf = self.tree.new_leaf_with_context(tsn, Ctx::Text { key: k, text, style: text_style(st), inset: [0.0; 4] }).ok()?;
                    self.map.push((k, leaf));
                    children.push(leaf);
                    kids.push(Kid::Part(part));
                } else if let Some(t) = self.build(c, is_box) {
                    children.push(t);
                    kids.push(Kid::Elem(c));
                }
            }
            if let Some(a) = &es.after {
                if let Some(t) = self.pseudo(n, Part::After, a, st) {
                    children.push(t);
                    kids.push(Kid::Part(Part::After));
                }
            }
            node = self.tree.new_with_children(ts, &children).ok()?;
            self.kids.insert(n, kids);
        }
        self.map.push((key, node));
        Some(node)
    }

    fn pseudo(&mut self, n: NodeId, part: Part, ps: &Style, _parent: &Style) -> Option<TNode> {
        if ps.display == Display::None {
            return None;
        }
        let ts = self.taffy_style(ps, effective_display(ps.display, true));
        let key = (n, part);
        let text = ps.content.clone().unwrap_or_default();
        let node = if text.is_empty() {
            self.tree.new_leaf(ts).ok()?
        } else {
            self.tree.new_leaf_with_context(ts, Ctx::Text { key, text, style: text_style(ps), inset: [0.0; 4] }).ok()?
        };
        self.map.push((key, node));
        Some(node)
    }

    fn inline_text(&self, n: NodeId) -> String {
        let mut s = String::new();
        for &c in self.dom.children(n) {
            if let Some(t) = self.dom.text(c) {
                s.push_str(t);
            } else if self.style(c).is_some_and(|x| x.base.display == Display::Inline) {
                s.push_str(&self.inline_text(c));
            }
        }
        s
    }
}

fn bw(st: &Style, i: usize) -> f32 {
    if st.border_style[i] == crate::css::cascade::BorderStyle::None {
        0.0
    } else {
        st.border_w[i]
    }
}

fn first_baseline(l: &IDWriteTextLayout) -> f32 {
    let mut m = [DWRITE_LINE_METRICS::default(); 1];
    let mut n = 0;
    let _ = unsafe { l.GetLineMetrics(Some(&mut m), &mut n) };
    m[0].baseline
}

fn metrics(l: &IDWriteTextLayout) -> DWRITE_TEXT_METRICS {
    let mut m = DWRITE_TEXT_METRICS::default();
    let _ = unsafe { l.GetMetrics(&mut m) };
    m
}

/// Chromium's intrinsic text-field width: `size=20` average chars.
fn control_width(st: &TextStyle) -> f32 {
    (st.size * 0.55 * 20.0).round()
}

pub fn compute(dom: &Dom, styles: &[Option<Rc<ElemStyle>>], text: &mut Text, view: (f32, f32)) -> Layout {
    let mut b = Builder { dom, styles, tree: TaffyTree::new(), map: Vec::new(), kids: HashMap::new(), carets: HashMap::new() };
    b.tree.disable_rounding();
    let Some(root) = b.build(dom.html, false) else { return Layout::default() };
    let _ = b.tree.compute_layout_with_measure(
        root,
        Size { width: AvailableSpace::Definite(view.0), height: AvailableSpace::Definite(view.1) },
        |inputs, _id, ctx, style| {
            let Some(ctx) = ctx else {
                return taffy::compute_leaf_layout(inputs, style, |_, _| 0.0, |_, _| Size::ZERO);
            };
            let (key, txt, ts, inset, control) = match ctx {
                Ctx::Text { key, text, style, inset } => (*key, text.clone(), *style, *inset, None),
                Ctx::Control { key, text, style, inset, multiline } => (*key, text.clone(), *style, *inset, Some(*multiline)),
            };
            let mut base = 0.0;
            let mut out = taffy::compute_leaf_layout(inputs, style, |_, _| 0.0, |known, avail| {
                let max_w = known.width.unwrap_or(match avail.width {
                    AvailableSpace::Definite(w) => w,
                    AvailableSpace::MinContent => {
                        if ts.wrap && control.is_none() {
                            text.min_width(&txt, &ts)
                        } else {
                            f32::INFINITY
                        }
                    }
                    AvailableSpace::MaxContent => f32::INFINITY,
                });
                let mut ts2 = ts;
                if control == Some(false) {
                    ts2.wrap = false;
                }
                let measure_txt = if txt.is_empty() { "\u{200b}".to_string() } else { txt.clone() };
                let l = text.layout(&measure_txt, &ts2, if max_w.is_finite() { max_w } else { f32::INFINITY }, f32::INFINITY);
                let m = metrics(&l);
                base = first_baseline(&l);
                let line = match ts.line_height {
                    Some(h) => h,
                    None => text.font_line(&ts, ts.size).0,
                };
                let w = match control {
                    Some(_) => known.width.unwrap_or(control_width(&ts)),
                    None if ts.ellipsis || !ts.wrap => m.widthIncludingTrailingWhitespace.min(if max_w.is_finite() { max_w } else { f32::INFINITY }),
                    None => m.widthIncludingTrailingWhitespace.min(max_w),
                };
                let h = match control {
                    Some(false) => line,
                    _ => m.height.max(line),
                };
                Size { width: w.ceil().min(if max_w.is_finite() && control.is_none() { max_w.max(w) } else { f32::INFINITY }), height: known.height.unwrap_or(h) }
            });
            out.baselines.first = Some(inset[0] + base);
            out
        },
    );
    
    // Absolute positions: taffy locations are relative to the parent for in-flow boxes and to
    // the containing block for absolutely positioned ones.
    let tmap: HashMap<(NodeId, Part), TNode> = b.map.iter().copied().collect();
    let mut out = Layout { kids: std::mem::take(&mut b.kids), ..Default::default() };
    // Walk the DOM in tree order, tracking the nearest positioned ancestor's origin.
    fn walk(
        n: NodeId,
        parent_origin: (f32, f32),
        cb_origin: (f32, f32),
        b: &Builder,
        tmap: &HashMap<(NodeId, Part), TNode>,
        out: &mut Layout,
    ) {
        let Some(&t) = tmap.get(&(n, Part::Main)) else { return };
        let Ok(l) = b.tree.layout(t) else { return };
        let st = &b.style(n).unwrap().base;
        let origin_base = if st.position == Position::Absolute { cb_origin } else { parent_origin };
        let x = origin_base.0 + l.location.x;
        let y = origin_base.1 + l.location.y;
        let g = BoxGeom {
            x,
            y,
            w: l.size.width,
            h: l.size.height,
            border: [l.border.top, l.border.right, l.border.bottom, l.border.left],
            padding: [l.padding.top, l.padding.right, l.padding.bottom, l.padding.left],
            content_w: l.scrollable_overflow_rect.right,
            content_h: l.scrollable_overflow_rect.bottom,
        };
        out.boxes.insert((n, Part::Main), g);
        let my_cb = if st.position != Position::Static || n == b.dom.html { (x, y) } else { cb_origin };
        for k in out.kids.get(&n).cloned().unwrap_or_default() {
            match k {
                Kid::Elem(c) => walk(c, (x, y), my_cb, b, tmap, out),
                Kid::Part(p) => {
                    if let Some(&pt) = tmap.get(&(n, p)) {
                        if let Ok(pl) = b.tree.layout(pt) {
                            let abs = matches!(p, Part::Before | Part::After)
                                && b.style(n).and_then(|s| if p == Part::Before { s.before.as_ref() } else { s.after.as_ref() }).is_some_and(|s| s.position == Position::Absolute);
                            let o = if abs { my_cb } else { (x, y) };
                            out.boxes.insert(
                                (n, p),
                                BoxGeom {
                                    x: o.0 + pl.location.x,
                                    y: o.1 + pl.location.y,
                                    w: pl.size.width,
                                    h: pl.size.height,
                                    ..Default::default()
                                },
                            );
                        }
                    }
                }
            }
        }
    }
    walk(dom.html, (0.0, 0.0), (0.0, 0.0), &b, &tmap, &mut out);

    // Text leaves: final DirectWrite layouts at the content-box width.
    for (key, t) in &b.map {
        let Some(Ctx::Text { text: s, style: ts, .. } | Ctx::Control { text: s, style: ts, .. }) = b.tree.get_node_context(*t).cloned() else { continue };
        let Some(g) = out.boxes.get(key).copied() else { continue };
        let (_, _, cw, ch) = g.content_box();
        let n = key.0;
        let is_ctl = is_control(dom.tag(n)) && key.1 == Part::Main;
        let mut ts = ts;
        let (draw, placeholder) = if is_ctl && s.is_empty() {
            (dom.attr(n, "placeholder").unwrap_or("").to_string(), true)
        } else {
            (s.clone(), false)
        };
        if is_ctl && dom.tag(n) == "input" {
            ts.wrap = false;
        }
        let max_w = if ts.wrap || ts.ellipsis || ts.align != text::Align::Left { cw } else { f32::INFINITY };
        let layout = text.layout(&draw, &ts, max_w.max(0.0), f32::INFINITY);
        let m = metrics(&layout);
        let (mut dx, mut dy) = (0.0, 0.0);
        let est = styles[n as usize].as_ref().map(|s| &s.base);
        if key.1 == Part::Main {
            if let Some(es) = est {
                let centre_x = match es.display {
                    Display::Grid => es.justify_items == Some(Align::Center) || es.justify_content == Some(Align::Center),
                    Display::Flex if es.flex_row => es.justify_content == Some(Align::Center),
                    Display::Flex => es.align_items == Some(Align::Center),
                    _ => false,
                };
                let centre_y = match es.display {
                    Display::Grid => es.align_items == Some(Align::Center) || es.align_content == Some(Align::Center),
                    Display::Flex if es.flex_row => es.align_items == Some(Align::Center),
                    Display::Flex => es.justify_content == Some(Align::Center),
                    _ => false,
                };
                if centre_x && ts.align == text::Align::Left && !ts.wrap {
                    dx = ((cw - m.widthIncludingTrailingWhitespace) / 2.0).max(0.0);
                }
                if centre_y || dom.tag(n) == "button" || (is_ctl && dom.tag(n) == "input") {
                    dy = ((ch - m.height) / 2.0).max(0.0);
                }
            }
        }
        if let Some(g) = out.boxes.get_mut(key) {
            let pad = g.padding[0] + g.padding[2];
            g.content_h = g.content_h.max(m.height + pad);
            g.content_w = g.content_w.max(m.widthIncludingTrailingWhitespace + g.padding[1] + g.padding[3]);
        }
        let caret = b.carets.get(key).copied();
        if let Some(c) = caret {
            let (cx, cy, _) = text::caret_rect(&layout, text::utf16_len(&draw));
            let base = if draw.is_empty() { first_baseline(&layout) } else {
                // Baseline of the last line: line top + baseline.
                let mut lines = vec![DWRITE_LINE_METRICS::default(); 64];
                let mut cnt = 0;
                let _ = unsafe { layout.GetLineMetrics(Some(&mut lines), &mut cnt) };
                let last = lines[..(cnt as usize).min(64)].last().copied().unwrap_or_default();
                cy + last.baseline
            };
            let cs = &b.style(c).unwrap().base;
            let w = cs.width.lp().map(|l| l.px).unwrap_or(7.0);
            let h = cs.height.lp().map(|l| l.px).unwrap_or(14.0);
            let ml = cs.margin[3].lp().map(|l| l.px).unwrap_or(0.0);
            let (bx, by, _, _) = g.content_box();
            let x = if draw.is_empty() { bx + ml } else { bx + cx + ml };
            let y = by + base - cs.vertical_align - h;
            out.boxes.insert((c, Part::Main), BoxGeom { x, y, w, h, ..Default::default() });
        }
        out.leaves.insert(*key, Leaf { text: draw, style: ts, layout, caret, placeholder, dx, dy });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css::cascade::Cascade;

    fn run(d: &Dom) -> Layout {
        let c = Cascade::new(include_str!("../../ui/style.css"));
        let st = c.compute(d);
        let mut t = Text::new().unwrap();
        compute(d, &st, &mut t, (VIEW_W, VIEW_H))
    }

    #[test]
    fn idle_island_box() {
        let d = Dom::from_index();
        let l = run(&d);
        let island = d.by_id("island").unwrap();
        let g = l.boxes[&(island, Part::Main)];
        // translateX(-50%) is a paint-time transform: layout box starts at 50%.
        assert_eq!((g.x, g.y, g.w, g.h), (320.0, 0.0, 168.0, 32.0));
        let before = l.boxes[&(island, Part::Before)];
        assert_eq!((before.x, before.w, before.h), (309.0, 11.0, 11.0));
    }

    #[test]
    fn open_today_sizes() {
        let mut d = Dom::from_index();
        d.set_attr(d.body, "data-state", "open");
        d.style_set(d.body, "--w", "460px");
        d.style_set(d.body, "--h", "140px");
        d.style_set(d.body, "--ow", "460px");
        d.style_set(d.body, "--oh", "140px");
        let nav = d.by_id("pageNav").unwrap();
        let mut tabs = Vec::new();
        for (i, name) in ["Hari ini", "Fokus"].iter().enumerate() {
            let t = d.create_element("button");
            d.set_attr(t, "class", if i == 0 { "page-tab on" } else { "page-tab" });
            d.append_child(nav, t);
            d.set_inner_html(t, &format!("<svg viewBox=\"0 0 24 24\"></svg><span>{name}</span>"));
            tabs.push(t);
        }
        let l = run(&d);
        let q = |s: &str| d.query_str(d.html, s, false)[0];
        assert_eq!(l.boxes[&(q(".hdr"), Part::Main)].h, 24.0);
        assert_eq!(l.boxes[&(nav, Part::Main)].h, 34.0);
        assert_eq!(l.boxes[&(q(".pages"), Part::Main)].h, 82.0);
        let on = l.boxes[&(tabs[0], Part::Main)];
        let off = l.boxes[&(tabs[1], Part::Main)];
        assert!((on.w - off.w).abs() < 0.5, "{} {}", on.w, off.w);
    }

    #[test]
    fn caret_after_text() {
        let mut d = Dom::from_index();
        d.set_attr(d.body, "data-state", "answer");
        let a = d.by_id("answer").unwrap();
        d.set_text_content(a, "Halo");
        let c = d.create_element("span");
        d.set_attr(c, "class", "caret");
        d.append_child(a, c);
        let l = run(&d);
        let ab = l.boxes[&(a, Part::Main)];
        let cb = l.boxes[&(c, Part::Main)];
        assert!(cb.x > ab.x + 24.0 + 10.0, "{cb:?} {ab:?}");
        assert_eq!((cb.w, cb.h), (7.0, 14.0));
        assert!(l.leaves.contains_key(&(a, Part::Main)));
    }
}
