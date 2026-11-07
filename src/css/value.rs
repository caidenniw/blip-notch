//! Typed CSS values and parsers for the grammar used by `ui/style.css` and `ui/app.js`.
use crate::gfx::anim::Easing;

/// Straight-alpha RGBA, components 0..=1.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const TRANSPARENT: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };
    pub const BLACK: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const WHITE: Color = Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    pub fn rgba8(r: u8, g: u8, b: u8, a: f32) -> Color {
        Color { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a }
    }
    /// Interpolates in premultiplied space (CSS colour interpolation).
    pub fn lerp(a: Color, b: Color, t: f32) -> Color {
        let al = a.a + (b.a - a.a) * t;
        if al <= 0.0 {
            return Color::TRANSPARENT;
        }
        let ch = |x: f32, y: f32| ((x * a.a) + ((y * b.a) - (x * a.a)) * t) / al;
        Color { r: ch(a.r, b.r), g: ch(a.g, b.g), b: ch(a.b, b.b), a: al.clamp(0.0, 1.0) }
    }
    pub fn with_alpha(self, k: f32) -> Color {
        Color { a: self.a * k, ..self }
    }
}

/// `px + pct%` (a resolved `calc()` of lengths and percentages).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Lp {
    pub px: f32,
    pub pct: f32,
}

impl Lp {
    pub const ZERO: Lp = Lp { px: 0.0, pct: 0.0 };
    pub fn px(v: f32) -> Lp {
        Lp { px: v, pct: 0.0 }
    }
    pub fn pct(v: f32) -> Lp {
        Lp { px: 0.0, pct: v }
    }
    pub fn resolve(self, basis: f32) -> f32 {
        self.px + self.pct * basis / 100.0
    }
    pub fn lerp(a: Lp, b: Lp, t: f32) -> Lp {
        Lp { px: a.px + (b.px - a.px) * t, pct: a.pct + (b.pct - a.pct) * t }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Lpa {
    #[default]
    Auto,
    L(Lp),
}

impl Lpa {
    pub fn lp(self) -> Option<Lp> {
        match self {
            Lpa::Auto => None,
            Lpa::L(l) => Some(l),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stop {
    pub color: Color,
    pub pos: Option<Lp>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Gradient {
    /// Angle in degrees (CSS: 0 = to top, 90 = to right).
    Linear { angle: f32, stops: Vec<Stop> },
    /// `radial-gradient(circle at X Y, ...)` with farthest-corner size.
    Radial { at: (Lp, Lp), stops: Vec<Stop> },
    Conic { stops: Vec<Stop> },
}

#[derive(Clone, Debug, PartialEq, Default)]
pub enum Bg {
    #[default]
    None,
    Color(Color),
    Image(Gradient),
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Shadow {
    pub x: f32,
    pub y: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: Color,
    pub inset: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tf {
    Translate(Lp, Lp),
    Scale(f32, f32),
    /// Degrees.
    Rotate(f32),
    /// Resolved 2D matrix `[a b c d e f]` (interpolation fallback).
    Matrix([f32; 6]),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Filter {
    Blur(f32),
    DropShadow(Shadow),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Track {
    Px(f32),
    Fr(f32),
}

// ---------------------------------------------------------------- tokenizing helpers

/// Splits on `sep` outside parentheses and quotes.
pub fn split_top(s: &str, sep: char) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut quote, mut start) = (0i32, None::<char>, 0usize);
    for (i, c) in s.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '(') => depth += 1,
            (None, ')') => depth -= 1,
            (None, c) if c == sep && depth == 0 => {
                out.push(s[start..i].trim());
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    out.push(s[start..].trim());
    out
}

/// Whitespace-separated tokens outside parentheses.
pub fn words(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0i32, None::<usize>);
    for (i, c) in s.char_indices() {
        match c {
            '(' => {
                depth += 1;
                start.get_or_insert(i);
            }
            ')' => depth -= 1,
            c if c.is_whitespace() && depth == 0 => {
                if let Some(s0) = start.take() {
                    out.push(&s[s0..i]);
                }
            }
            _ => {
                start.get_or_insert(i);
            }
        }
    }
    if let Some(s0) = start {
        out.push(&s[s0..]);
    }
    out
}

/// `name(args)` → `(name, args)`.
pub fn func(s: &str) -> Option<(&str, &str)> {
    let s = s.trim();
    let o = s.find('(')?;
    if !s.ends_with(')') {
        return None;
    }
    Some((s[..o].trim(), &s[o + 1..s.len() - 1]))
}

// ---------------------------------------------------------------- numbers and calc

/// A calc() term: `px + pct% + num` (+ deg / ms for angle and time contexts).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Q {
    pub px: f32,
    pub pct: f32,
    pub num: f32,
    pub deg: f32,
    pub ms: f32,
    /// Bitmask of which units appeared (1 px, 2 pct, 4 num, 8 deg, 16 ms).
    pub units: u8,
}

impl Q {
    fn scale(self, k: f32) -> Q {
        Q { px: self.px * k, pct: self.pct * k, num: self.num * k, deg: self.deg * k, ms: self.ms * k, units: self.units }
    }
    fn add(self, o: Q, sign: f32) -> Q {
        Q {
            px: self.px + o.px * sign,
            pct: self.pct + o.pct * sign,
            num: self.num + o.num * sign,
            deg: self.deg + o.deg * sign,
            ms: self.ms + o.ms * sign,
            units: self.units | o.units,
        }
    }
    fn is_num(&self) -> bool {
        self.units & !4 == 0
    }
}

/// Parses `12px`, `-.5`, `50%`, `14deg`, `.3s`, `90ms`, `calc(...)`.
pub fn quantity(s: &str) -> Option<Q> {
    let s = s.trim();
    if let Some((f, a)) = func(s) {
        return match f {
            "calc" => calc(a),
            _ => None,
        };
    }
    let end = s
        .char_indices()
        .find(|&(i, c)| !(c.is_ascii_digit() || c == '.' || ((c == '-' || c == '+') && i == 0) || (c == 'e' && false)))
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    let (n, unit) = s.split_at(end);
    let v: f32 = n.parse().ok()?;
    let q = Q::default();
    Some(match unit {
        "" => Q { num: v, units: 4, ..q },
        "px" => Q { px: v, units: 1, ..q },
        "%" => Q { pct: v, units: 2, ..q },
        "deg" => Q { deg: v, units: 8, ..q },
        "turn" => Q { deg: v * 360.0, units: 8, ..q },
        "rad" => Q { deg: v.to_degrees(), units: 8, ..q },
        "s" => Q { ms: v * 1000.0, units: 16, ..q },
        "ms" => Q { ms: v, units: 16, ..q },
        _ => return None,
    })
}

fn calc(src: &str) -> Option<Q> {
    // Tokens: quantities, operators, parentheses.
    let mut toks: Vec<String> = Vec::new();
    let b = src.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as char;
        if c.is_whitespace() {
            i += 1;
        } else if "*/()".contains(c) {
            toks.push(c.to_string());
            i += 1;
        } else if (c == '+' || c == '-')
            && (i + 1 >= b.len() || (b[i + 1] as char).is_whitespace())
        {
            toks.push(c.to_string());
            i += 1;
        } else {
            let st = i;
            i += 1;
            while i < b.len() && !(b[i] as char).is_whitespace() && !"*/()".contains(b[i] as char) {
                i += 1;
            }
            toks.push(src[st..i].to_string());
        }
    }
    let mut p = 0;
    let r = calc_sum(&toks, &mut p)?;
    (p == toks.len()).then_some(r)
}

fn calc_sum(t: &[String], p: &mut usize) -> Option<Q> {
    let mut acc = calc_prod(t, p)?;
    while *p < t.len() && (t[*p] == "+" || t[*p] == "-") {
        let sign = if t[*p] == "+" { 1.0 } else { -1.0 };
        *p += 1;
        acc = acc.add(calc_prod(t, p)?, sign);
    }
    Some(acc)
}

fn calc_prod(t: &[String], p: &mut usize) -> Option<Q> {
    let mut acc = calc_atom(t, p)?;
    while *p < t.len() && (t[*p] == "*" || t[*p] == "/") {
        let mul = t[*p] == "*";
        *p += 1;
        let r = calc_atom(t, p)?;
        acc = if mul {
            if r.is_num() {
                acc.scale(r.num)
            } else if acc.is_num() {
                r.scale(acc.num)
            } else {
                return None;
            }
        } else if r.is_num() && r.num != 0.0 {
            acc.scale(1.0 / r.num)
        } else {
            return None;
        };
    }
    Some(acc)
}

fn calc_atom(t: &[String], p: &mut usize) -> Option<Q> {
    let tok = t.get(*p)?;
    *p += 1;
    if tok == "(" {
        let v = calc_sum(t, p)?;
        if t.get(*p).map(String::as_str) != Some(")") {
            return None;
        }
        *p += 1;
        return Some(v);
    }
    if tok == "calc" || tok.ends_with("calc") {
        return calc_atom(t, p);
    }
    quantity(tok)
}

pub fn number(s: &str) -> Option<f32> {
    let q = quantity(s)?;
    q.is_num().then_some(q.num)
}

/// Length-percentage. Unitless 0 is allowed.
pub fn lp(s: &str) -> Option<Lp> {
    let q = quantity(s)?;
    if q.units & !(1 | 2 | 4) != 0 || (q.units & 4 != 0 && q.num != 0.0) {
        return None;
    }
    Some(Lp { px: q.px, pct: q.pct })
}

pub fn lpa(s: &str) -> Option<Lpa> {
    if s.trim() == "auto" {
        return Some(Lpa::Auto);
    }
    lp(s).map(Lpa::L)
}

pub fn px(s: &str) -> Option<f32> {
    let l = lp(s)?;
    (l.pct == 0.0).then_some(l.px)
}

pub fn time_ms(s: &str) -> Option<f32> {
    let q = quantity(s)?;
    if q.units == 16 || (q.units == 4 && q.num == 0.0) {
        Some(q.ms)
    } else {
        None
    }
}

pub fn angle(s: &str) -> Option<f32> {
    let q = quantity(s)?;
    if q.units == 8 || (q.units == 4 && q.num == 0.0) {
        Some(q.deg)
    } else {
        None
    }
}

// ---------------------------------------------------------------- colours

pub fn color(s: &str, current: Color) -> Option<Color> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix('#') {
        let d = |i: usize, n: usize| u8::from_str_radix(&h[i..i + n], 16).ok();
        let x = |v: u8| v * 17;
        return match h.len() {
            3 => Some(Color::rgba8(x(d(0, 1)?), x(d(1, 1)?), x(d(2, 1)?), 1.0)),
            4 => Some(Color::rgba8(x(d(0, 1)?), x(d(1, 1)?), x(d(2, 1)?), x(d(3, 1)?) as f32 / 255.0)),
            6 => Some(Color::rgba8(d(0, 2)?, d(2, 2)?, d(4, 2)?, 1.0)),
            8 => Some(Color::rgba8(d(0, 2)?, d(2, 2)?, d(4, 2)?, d(6, 2)? as f32 / 255.0)),
            _ => None,
        };
    }
    match s.to_ascii_lowercase().as_str() {
        "transparent" => return Some(Color::TRANSPARENT),
        "currentcolor" => return Some(current),
        "black" => return Some(Color::BLACK),
        "white" => return Some(Color::WHITE),
        "red" => return Some(Color::rgba8(255, 0, 0, 1.0)),
        // Chromium dark color-scheme system colours used by the UA button style.
        "buttonface" | "buttonborder" => return Some(Color::rgba8(0x6b, 0x6b, 0x6b, 1.0)),
        "buttontext" | "fieldtext" => return Some(Color::WHITE),
        "field" => return Some(Color::rgba8(0x3b, 0x3b, 0x3b, 1.0)),
        _ => {}
    }
    let (f, args) = func(s)?;
    match f {
        "rgb" | "rgba" => {
            let parts: Vec<&str> = if args.contains(',') {
                split_top(args, ',')
            } else {
                let w = words(args);
                // `r g b / a`
                w.into_iter().filter(|x| *x != "/").collect()
            };
            if parts.len() < 3 {
                return None;
            }
            let ch = |p: &str| -> Option<f32> {
                let q = quantity(p)?;
                Some(if q.units == 2 { q.pct / 100.0 } else { q.num / 255.0 })
            };
            let a = match parts.get(3) {
                Some(p) => {
                    let q = quantity(p)?;
                    if q.units == 2 {
                        q.pct / 100.0
                    } else {
                        q.num
                    }
                }
                None => 1.0,
            };
            Some(Color {
                r: ch(parts[0])?.clamp(0.0, 1.0),
                g: ch(parts[1])?.clamp(0.0, 1.0),
                b: ch(parts[2])?.clamp(0.0, 1.0),
                a: a.clamp(0.0, 1.0),
            })
        }
        "color-mix" => {
            let parts = split_top(args, ',');
            if parts.len() != 3 || !parts[0].starts_with("in srgb") {
                return None;
            }
            let side = |p: &str| -> Option<(Color, Option<f32>)> {
                let w = words(p);
                let (mut c, mut pct) = (None, None);
                for t in w {
                    match quantity(t) {
                        Some(q) if q.units == 2 => pct = Some(q.pct / 100.0),
                        _ => c = Some(color(t, current)?),
                    }
                }
                Some((c?, pct))
            };
            let (c1, p1) = side(parts[1])?;
            let (c2, p2) = side(parts[2])?;
            let (w1, w2) = match (p1, p2) {
                (Some(a), Some(b)) => (a, b),
                (Some(a), None) => (a, 1.0 - a),
                (None, Some(b)) => (1.0 - b, b),
                (None, None) => (0.5, 0.5),
            };
            let sum = w1 + w2;
            if sum <= 0.0 {
                return None;
            }
            // Premultiplied mix: result alpha = weighted alpha; channels un-premultiplied after.
            let a = c1.a * w1 / sum + c2.a * w2 / sum;
            let ch = |x: f32, y: f32| if a > 0.0 { (x * c1.a * w1 / sum + y * c2.a * w2 / sum) / a } else { 0.0 };
            let mut out = Color { r: ch(c1.r, c2.r), g: ch(c1.g, c2.g), b: ch(c1.b, c2.b), a };
            if sum < 1.0 {
                out.a *= sum;
            }
            Some(out)
        }
        _ => None,
    }
}

// ---------------------------------------------------------------- images

fn stops(parts: &[&str], current: Color) -> Option<Vec<Stop>> {
    let mut out = Vec::new();
    for p in parts {
        let w = words(p);
        let c = color(w.first()?, current)?;
        let pos = match w.get(1) {
            Some(x) => Some(lp(x)?),
            None => None,
        };
        out.push(Stop { color: c, pos });
        if let Some(x) = w.get(2) {
            out.push(Stop { color: c, pos: Some(lp(x)?) });
        }
    }
    (out.len() >= 2).then_some(out)
}

fn side_angle(s: &str) -> Option<f32> {
    Some(match s.trim() {
        "to top" => 0.0,
        "to right" => 90.0,
        "to bottom" => 180.0,
        "to left" => 270.0,
        _ => return None,
    })
}

fn position(s: &str) -> Option<(Lp, Lp)> {
    let w = words(s);
    let kw = |t: &str| match t {
        "left" | "top" => Some(Lp::pct(0.0)),
        "center" => Some(Lp::pct(50.0)),
        "right" | "bottom" => Some(Lp::pct(100.0)),
        _ => lp(t),
    };
    match w.as_slice() {
        [x] => Some((kw(x)?, Lp::pct(50.0))),
        [x, y] => Some((kw(x)?, kw(y)?)),
        _ => None,
    }
}

pub fn gradient(s: &str, current: Color) -> Option<Gradient> {
    let (f, args) = func(s)?;
    let parts = split_top(args, ',');
    match f {
        "linear-gradient" => {
            let (angle, rest) = match side_angle(parts[0]).or_else(|| angle(parts[0])) {
                Some(a) => (a, &parts[1..]),
                None => (180.0, &parts[..]),
            };
            Some(Gradient::Linear { angle, stops: stops(rest, current)? })
        }
        "radial-gradient" => {
            let head = parts[0];
            let at = head.find(" at ").map(|i| &head[i + 4..]).or_else(|| head.strip_prefix("at "));
            let at = match at {
                Some(a) => position(a)?,
                None => (Lp::pct(50.0), Lp::pct(50.0)),
            };
            let rest = if head.starts_with("circle") || head.starts_with("at ") { &parts[1..] } else { &parts[..] };
            Some(Gradient::Radial { at, stops: stops(rest, current)? })
        }
        "conic-gradient" => Some(Gradient::Conic { stops: stops(&parts, current)? }),
        _ => None,
    }
}

pub fn background(s: &str, current: Color) -> Option<Bg> {
    let s = s.trim();
    if s == "none" {
        return Some(Bg::None);
    }
    if s.contains("gradient(") {
        return gradient(s, current).map(Bg::Image);
    }
    color(s, current).map(Bg::Color)
}

// ---------------------------------------------------------------- shadows, transforms, filters

pub fn shadows(s: &str, current: Color) -> Option<Vec<Shadow>> {
    if s.trim() == "none" {
        return Some(Vec::new());
    }
    let mut out = Vec::new();
    for part in split_top(s, ',') {
        let mut lens = Vec::new();
        let mut c = None;
        let mut inset = false;
        for w in words(part) {
            if w == "inset" {
                inset = true;
            } else if let Some(v) = px(w) {
                lens.push(v);
            } else {
                c = Some(color(w, current)?);
            }
        }
        if lens.len() < 2 {
            return None;
        }
        out.push(Shadow {
            x: lens[0],
            y: lens[1],
            blur: lens.get(2).copied().unwrap_or(0.0),
            spread: lens.get(3).copied().unwrap_or(0.0),
            color: c.unwrap_or(current),
            inset,
        });
    }
    Some(out)
}

pub fn transform(s: &str) -> Option<Vec<Tf>> {
    let s = s.trim();
    if s == "none" || s.is_empty() {
        return Some(Vec::new());
    }
    let mut out = Vec::new();
    for w in words(s) {
        let (f, a) = func(w)?;
        let args: Vec<&str> = if a.contains(',') { split_top(a, ',') } else { words(a) };
        let n = |i: usize| args.get(i).and_then(|x| number(x));
        let l = |i: usize| args.get(i).and_then(|x| lp(x));
        out.push(match f {
            "translate" => Tf::Translate(l(0)?, if args.len() > 1 { l(1)? } else { Lp::ZERO }),
            "translateX" => Tf::Translate(l(0)?, Lp::ZERO),
            "translateY" => Tf::Translate(Lp::ZERO, l(0)?),
            "scale" => {
                let x = n(0)?;
                Tf::Scale(x, if args.len() > 1 { n(1)? } else { x })
            }
            "scaleX" => Tf::Scale(n(0)?, 1.0),
            "scaleY" => Tf::Scale(1.0, n(0)?),
            "rotate" => Tf::Rotate(angle(args.first()?)?),
            "matrix" => {
                let m: Option<Vec<f32>> = (0..6).map(n).collect();
                let m = m?;
                Tf::Matrix([m[0], m[1], m[2], m[3], m[4], m[5]])
            }
            _ => return None,
        });
    }
    Some(out)
}

/// The `translate` property: `none` | `x [y]`.
pub fn translate(s: &str) -> Option<(Lp, Lp)> {
    let s = s.trim();
    if s == "none" {
        return Some((Lp::ZERO, Lp::ZERO));
    }
    let w = words(s);
    Some((lp(w.first()?)?, match w.get(1) {
        Some(y) => lp(y)?,
        None => Lp::ZERO,
    }))
}

pub fn filters(s: &str, current: Color) -> Option<Vec<Filter>> {
    if s.trim() == "none" {
        return Some(Vec::new());
    }
    let mut out = Vec::new();
    for w in words(s) {
        let (f, a) = func(w)?;
        out.push(match f {
            "blur" => Filter::Blur(px(a)?),
            "drop-shadow" => Filter::DropShadow(*shadows(a, current)?.first()?),
            _ => return None,
        });
    }
    Some(out)
}

pub fn tracks(s: &str) -> Option<Vec<Track>> {
    let s = s.trim();
    if s == "none" {
        return Some(Vec::new());
    }
    words(s)
        .into_iter()
        .map(|w| {
            if let Some(n) = w.strip_suffix("fr") {
                n.parse().ok().map(Track::Fr)
            } else {
                px(w).map(Track::Px)
            }
        })
        .collect()
}

// ---------------------------------------------------------------- easing

pub fn easing(s: &str) -> Option<Easing> {
    let s = s.trim();
    Some(match s {
        "linear" => Easing::Linear,
        "ease" => Easing::Bezier(0.25, 0.1, 0.25, 1.0),
        "ease-in" => Easing::Bezier(0.42, 0.0, 1.0, 1.0),
        "ease-out" => Easing::Bezier(0.0, 0.0, 0.58, 1.0),
        "ease-in-out" => Easing::Bezier(0.42, 0.0, 0.58, 1.0),
        "step-start" => Easing::Steps(1, true),
        "step-end" => Easing::Steps(1, false),
        _ => {
            let (f, a) = func(s)?;
            let args = split_top(a, ',');
            match f {
                "cubic-bezier" => {
                    let n: Option<Vec<f32>> = args.iter().map(|x| number(x)).collect();
                    let n = n?;
                    if n.len() != 4 {
                        return None;
                    }
                    Easing::Bezier(n[0], n[1], n[2], n[3])
                }
                "steps" => {
                    let n = number(args.first()?)? as u32;
                    let start = matches!(args.get(1).copied(), Some("start" | "jump-start"));
                    Easing::Steps(n, start)
                }
                "linear" => {
                    // Points with optional input percentages; missing inputs are spread evenly.
                    let mut pts: Vec<(Option<f32>, f32)> = Vec::new();
                    for p in args {
                        let w = words(p);
                        let y = number(w.first()?)?;
                        let x = match w.get(1) {
                            Some(x) => Some(quantity(x)?.pct / 100.0),
                            None => None,
                        };
                        pts.push((x, y));
                    }
                    let n = pts.len();
                    if n < 2 {
                        return None;
                    }
                    if pts[0].0.is_none() {
                        pts[0].0 = Some(0.0);
                    }
                    if pts[n - 1].0.is_none() {
                        pts[n - 1].0 = Some(1.0);
                    }
                    let mut i = 1;
                    while i < n {
                        if pts[i].0.is_none() {
                            let j = (i..n).find(|&j| pts[j].0.is_some()).unwrap();
                            let (a, b) = (pts[i - 1].0.unwrap(), pts[j].0.unwrap());
                            for k in i..j {
                                pts[k].0 = Some(a + (b - a) * (k - i + 1) as f32 / (j - i + 1) as f32);
                            }
                            i = j;
                        }
                        i += 1;
                    }
                    Easing::Points(pts.into_iter().map(|(x, y)| (x.unwrap(), y)).collect())
                }
                _ => return None,
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calc_and_lengths() {
        assert_eq!(lp("calc(-50% - 3px)"), Some(Lp { px: -3.0, pct: -50.0 }));
        assert_eq!(lp("calc(11px - .5px)"), Some(Lp::px(10.5)));
        assert_eq!(lp("calc(42.5 * 1%)"), Some(Lp::pct(42.5)));
        assert_eq!(lp("0"), Some(Lp::ZERO));
        assert_eq!(time_ms(".46s"), Some(460.0));
        assert_eq!(time_ms("-.3s"), Some(-300.0));
        assert_eq!(angle("-14deg"), Some(-14.0));
    }

    #[test]
    fn colours() {
        let c = color("color-mix(in srgb, #19b8ff 20%, transparent)", Color::WHITE).unwrap();
        assert!((c.a - 0.2).abs() < 1e-4 && (c.r - 25.0 / 255.0).abs() < 1e-4, "{c:?}");
        assert_eq!(color("rgba(255,255,255,.14)", Color::BLACK).unwrap().a, 0.14);
        assert_eq!(color("#fff", Color::BLACK), Some(Color::WHITE));
    }

    #[test]
    fn spring_linear() {
        let e = easing("linear(0, 0.024 3.3%, 0.95 50%, 1.011 80%, 1)").unwrap();
        assert!((e.apply(0.8) - 1.011).abs() < 1e-4);
        assert!((e.apply(0.5) - 0.95).abs() < 1e-4);
    }

    #[test]
    fn transforms_and_gradients() {
        assert_eq!(
            transform("translate(1px, 2px) scaleY(.45) translateY(-2px)").unwrap(),
            vec![Tf::Translate(Lp::px(1.0), Lp::px(2.0)), Tf::Scale(1.0, 0.45), Tf::Translate(Lp::ZERO, Lp::px(-2.0))]
        );
        match gradient("radial-gradient(circle at 0 100%, transparent calc(11px - .5px), #000 11px)", Color::BLACK) {
            Some(Gradient::Radial { at, stops }) => {
                assert_eq!(at, (Lp::ZERO, Lp::pct(100.0)));
                assert_eq!(stops[0].pos, Some(Lp::px(10.5)));
            }
            g => panic!("{g:?}"),
        }
        assert!(matches!(gradient("linear-gradient(to top, #000, rgba(0,0,0,.4) 30%, transparent)", Color::BLACK), Some(Gradient::Linear { angle: 0.0, .. })));
    }
}
