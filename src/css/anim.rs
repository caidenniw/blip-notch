//! CSS transitions and `@keyframes` animations applied on top of computed styles.
use super::cascade::{apply, subst, Cascade, ElemStyle, Style};
use super::value::{Bg, Color, Filter, Lp, Lpa, Shadow, Tf, Track};
use crate::dom::NodeId;
use crate::gfx::anim::Easing;
use crate::layout::Part;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Prop {
    Opacity,
    Width,
    Height,
    Radius,
    Color,
    Background,
    BorderColor,
    Transform,
    Translate,
    FlexGrow,
    ColumnGap,
    RowGap,
    GridCols,
    BoxShadow,
    TextShadow,
    Filter,
    Visibility,
}

const PROPS: [Prop; 17] = [
    Prop::Opacity,
    Prop::Width,
    Prop::Height,
    Prop::Radius,
    Prop::Color,
    Prop::Background,
    Prop::BorderColor,
    Prop::Transform,
    Prop::Translate,
    Prop::FlexGrow,
    Prop::ColumnGap,
    Prop::RowGap,
    Prop::GridCols,
    Prop::BoxShadow,
    Prop::TextShadow,
    Prop::Filter,
    Prop::Visibility,
];

impl Prop {
    fn names(self) -> &'static [&'static str] {
        match self {
            Prop::Opacity => &["opacity"],
            Prop::Width => &["width"],
            Prop::Height => &["height"],
            Prop::Radius => &["border-radius", "border-bottom-left-radius", "border-bottom-right-radius", "border-top-left-radius", "border-top-right-radius"],
            Prop::Color => &["color"],
            Prop::Background => &["background", "background-color", "background-image"],
            Prop::BorderColor => &["border-color", "border"],
            Prop::Transform => &["transform"],
            Prop::Translate => &["translate"],
            Prop::FlexGrow => &["flex-grow", "flex"],
            Prop::ColumnGap => &["column-gap", "gap"],
            Prop::RowGap => &["row-gap", "gap"],
            Prop::GridCols => &["grid-template-columns"],
            Prop::BoxShadow => &["box-shadow"],
            Prop::TextShadow => &["text-shadow"],
            Prop::Filter => &["filter"],
            Prop::Visibility => &["visibility"],
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    Num(f32),
    Lpa(Lpa),
    Lp(Lp),
    Radius([(Lp, Lp); 4]),
    Color(Color),
    Colors([Option<Color>; 4]),
    Bg(Bg),
    Tf(Vec<Tf>),
    Tr((Lp, Lp)),
    Tracks(Vec<Track>),
    Shadows(Vec<Shadow>),
    Filters(Vec<Filter>),
    Vis(bool),
}

fn get(s: &Style, p: Prop) -> Val {
    match p {
        Prop::Opacity => Val::Num(s.opacity),
        Prop::Width => Val::Lpa(s.width),
        Prop::Height => Val::Lpa(s.height),
        Prop::Radius => Val::Radius(s.radius),
        Prop::Color => Val::Color(s.color),
        Prop::Background => Val::Bg(s.background.clone()),
        Prop::BorderColor => Val::Colors(s.border_color),
        Prop::Transform => Val::Tf(s.transform.clone()),
        Prop::Translate => Val::Tr(s.translate),
        Prop::FlexGrow => Val::Num(s.flex_grow),
        Prop::ColumnGap => Val::Lp(s.column_gap),
        Prop::RowGap => Val::Lp(s.row_gap),
        Prop::GridCols => Val::Tracks(s.grid_cols.clone()),
        Prop::BoxShadow => Val::Shadows(s.shadows.clone()),
        Prop::TextShadow => Val::Shadows(s.text_shadow.clone()),
        Prop::Filter => Val::Filters(s.filter.clone()),
        Prop::Visibility => Val::Vis(s.visible),
    }
}

fn set(s: &mut Style, p: Prop, v: Val) {
    match (p, v) {
        (Prop::Opacity, Val::Num(x)) => s.opacity = x.clamp(0.0, 1.0),
        (Prop::Width, Val::Lpa(x)) => s.width = x,
        (Prop::Height, Val::Lpa(x)) => s.height = x,
        (Prop::Radius, Val::Radius(x)) => s.radius = x,
        (Prop::Color, Val::Color(x)) => s.color = x,
        (Prop::Background, Val::Bg(x)) => s.background = x,
        (Prop::BorderColor, Val::Colors(x)) => s.border_color = x,
        (Prop::Transform, Val::Tf(x)) => s.transform = x,
        (Prop::Translate, Val::Tr(x)) => s.translate = x,
        (Prop::FlexGrow, Val::Num(x)) => s.flex_grow = x.max(0.0),
        (Prop::ColumnGap, Val::Lp(x)) => s.column_gap = x,
        (Prop::RowGap, Val::Lp(x)) => s.row_gap = x,
        (Prop::GridCols, Val::Tracks(x)) => s.grid_cols = x,
        (Prop::BoxShadow, Val::Shadows(x)) => s.shadows = x,
        (Prop::TextShadow, Val::Shadows(x)) => s.text_shadow = x,
        (Prop::Filter, Val::Filters(x)) => s.filter = x,
        (Prop::Visibility, Val::Vis(x)) => s.visible = x,
        _ => {}
    }
}

fn l(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp_shadow(a: &Shadow, b: &Shadow, t: f32) -> Shadow {
    Shadow {
        x: l(a.x, b.x, t),
        y: l(a.y, b.y, t),
        blur: l(a.blur, b.blur, t).max(0.0),
        spread: l(a.spread, b.spread, t),
        color: Color::lerp(a.color, b.color, t),
        inset: b.inset,
    }
}

fn clear_shadow(s: &Shadow) -> Shadow {
    Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 0.0, color: Color::TRANSPARENT, inset: s.inset }
}

fn identity_of(t: &Tf) -> Tf {
    match t {
        Tf::Translate(..) => Tf::Translate(Lp::ZERO, Lp::ZERO),
        Tf::Scale(..) => Tf::Scale(1.0, 1.0),
        Tf::Rotate(_) => Tf::Rotate(0.0),
        Tf::Matrix(_) => Tf::Matrix([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]),
    }
}

fn lerp_tf(a: &[Tf], b: &[Tf], t: f32) -> Vec<Tf> {
    let mut out = Vec::with_capacity(a.len().max(b.len()));
    for i in 0..a.len().max(b.len()) {
        // Pad the shorter list with matching identities; keep percentages symbolic.
        let x = a.get(i).copied().unwrap_or_else(|| identity_of(&b[i]));
        let y = b.get(i).copied().unwrap_or_else(|| identity_of(&a[i]));
        let value = match (x, y) {
            (Tf::Translate(x1, y1), Tf::Translate(x2, y2)) => Tf::Translate(Lp::lerp(x1, x2, t), Lp::lerp(y1, y2, t)),
            (Tf::Scale(x1, y1), Tf::Scale(x2, y2)) => Tf::Scale(l(x1, x2, t), l(y1, y2, t)),
            (Tf::Rotate(a1), Tf::Rotate(a2)) => Tf::Rotate(l(a1, a2, t)),
            (Tf::Matrix(m1), Tf::Matrix(m2)) => Tf::Matrix(std::array::from_fn(|i| l(m1[i], m2[i], t))),
            _ => {
                // Only the incompatible suffix needs matrix interpolation.
                let m = |list: &[Tf]| {
                    let a = crate::paint::geom::tf_matrix(list, 0.0, 0.0);
                    [a.a, a.b, a.c, a.d, a.e, a.f]
                };
                let (ma, mb) = (m(&a[i..]), m(&b[i..]));
                out.push(Tf::Matrix(std::array::from_fn(|i| l(ma[i], mb[i], t))));
                break;
            }
        };
        out.push(value);
    }
    out
}

fn lerp(a: &Val, b: &Val, t: f32) -> Val {
    let snap = || if t < 0.5 { a.clone() } else { b.clone() };
    match (a, b) {
        (Val::Num(x), Val::Num(y)) => Val::Num(l(*x, *y, t)),
        (Val::Lp(x), Val::Lp(y)) => Val::Lp(Lp::lerp(*x, *y, t)),
        (Val::Lpa(Lpa::L(x)), Val::Lpa(Lpa::L(y))) => Val::Lpa(Lpa::L(Lp::lerp(*x, *y, t))),
        (Val::Radius(x), Val::Radius(y)) => {
            Val::Radius(std::array::from_fn(|i| (Lp::lerp(x[i].0, y[i].0, t), Lp::lerp(x[i].1, y[i].1, t))))
        }
        (Val::Color(x), Val::Color(y)) => Val::Color(Color::lerp(*x, *y, t)),
        (Val::Colors(x), Val::Colors(y)) if x.iter().zip(y).all(|(p, q)| p.is_some() && q.is_some()) => {
            Val::Colors(std::array::from_fn(|i| Some(Color::lerp(x[i].unwrap(), y[i].unwrap(), t))))
        }
        (Val::Bg(x), Val::Bg(y)) => {
            let c = |b: &Bg| match b {
                Bg::None => Some(Color::TRANSPARENT),
                Bg::Color(c) => Some(*c),
                Bg::Image(_) => None,
            };
            match (c(x), c(y)) {
                (Some(p), Some(q)) => Val::Bg(Bg::Color(Color::lerp(p, q, t))),
                _ => snap(),
            }
        }
        (Val::Tf(x), Val::Tf(y)) => Val::Tf(lerp_tf(x, y, t)),
        (Val::Tr(x), Val::Tr(y)) => Val::Tr((Lp::lerp(x.0, y.0, t), Lp::lerp(x.1, y.1, t))),
        (Val::Tracks(x), Val::Tracks(y))
            if x.len() == y.len() && x.iter().zip(y).all(|(p, q)| std::mem::discriminant(p) == std::mem::discriminant(q)) =>
        {
            Val::Tracks(
                x.iter()
                    .zip(y)
                    .map(|(p, q)| match (p, q) {
                        (Track::Px(a), Track::Px(b)) => Track::Px(l(*a, *b, t)),
                        (Track::Fr(a), Track::Fr(b)) => Track::Fr(l(*a, *b, t)),
                        _ => *q,
                    })
                    .collect(),
            )
        }
        (Val::Shadows(x), Val::Shadows(y)) => {
            let n = x.len().max(y.len());
            if x.len() != y.len() && !x.is_empty() && !y.is_empty() {
                return snap();
            }
            Val::Shadows(
                (0..n)
                    .map(|i| {
                        let p = x.get(i).copied().unwrap_or_else(|| clear_shadow(&y[i]));
                        let q = y.get(i).copied().unwrap_or_else(|| clear_shadow(&x[i]));
                        lerp_shadow(&p, &q, t)
                    })
                    .collect(),
            )
        }
        (Val::Filters(x), Val::Filters(y)) => {
            let ds = |f: &[Filter]| match f {
                [] => Some(None),
                [Filter::DropShadow(s)] => Some(Some(*s)),
                _ => None,
            };
            match (ds(x), ds(y)) {
                (Some(p), Some(q)) => {
                    let (p, q) = match (p, q) {
                        (Some(p), Some(q)) => (p, q),
                        (None, Some(q)) => (clear_shadow(&q), q),
                        (Some(p), None) => (p, clear_shadow(&p)),
                        (None, None) => return Val::Filters(Vec::new()),
                    };
                    Val::Filters(vec![Filter::DropShadow(lerp_shadow(&p, &q, t))])
                }
                _ => snap(),
            }
        }
        (Val::Vis(x), Val::Vis(y)) => Val::Vis(if t <= 0.0 { *x } else if t >= 1.0 { *y } else { *x || *y }),
        _ => snap(),
    }
}

struct Running {
    from: Val,
    to: Val,
    start: f64,
    dur: f64,
    delay: f64,
    easing: Easing,
}

impl Running {
    fn progress(&self, now: f64) -> f32 {
        let e = now - self.start - self.delay;
        if e < 0.0 {
            0.0
        } else if self.dur <= 0.0 {
            1.0
        } else {
            (e / self.dur).min(1.0) as f32
        }
    }
    fn value(&self, now: f64) -> Val {
        let p = self.progress(now);
        let t = if p <= 0.0 || p >= 1.0 { p } else { self.easing.apply(p) };
        // Before the delay ends the old value shows; `visibility` uses the raw progress.
        if now - self.start < self.delay {
            return self.from.clone();
        }
        lerp(&self.from, &self.to, t)
    }
    fn done(&self, now: f64) -> bool {
        now - self.start >= self.delay + self.dur
    }
}

struct AnimRun {
    name: String,
    start: f64,
    /// Elapsed time frozen by `animation-play-state: paused`.
    paused_at: Option<f64>,
    layout: bool,
}

#[derive(Default)]
struct BoxState {
    target: HashMap<Prop, Val>,
    running: HashMap<Prop, Running>,
    anims: Vec<AnimRun>,
}

pub struct Animator {
    state: HashMap<(NodeId, Part), BoxState>,
    /// `(target, type)` DOM events to dispatch: `transitionrun` / `transitionend`.
    pub events: Vec<(NodeId, &'static str)>,
    /// Multiplies every transition/animation duration and delay (settings "Kecepatan animasi").
    pub scale: f64,
}

impl Default for Animator {
    fn default() -> Self {
        Self { state: HashMap::new(), events: Vec::new(), scale: 1.0 }
    }
}

fn covers(list: &[super::cascade::Transition], p: Prop) -> Option<&super::cascade::Transition> {
    list.iter().rev().find(|t| t.prop == "all" || p.names().contains(&t.prop.as_str()))
}

impl Animator {
    /// Returns styles with transitions/animations at time `now` (ms) applied.
    pub fn apply(&mut self, cascade: &Cascade, dom: &crate::dom::Dom, styles: &[Option<Rc<ElemStyle>>], now: f64) -> Vec<Option<Rc<ElemStyle>>> {
        let mut out = styles.to_vec();
        let mut seen: Vec<(NodeId, Part)> = Vec::with_capacity(self.state.len());
        let mut rendered = vec![false; styles.len()];
        let mut stack = vec![dom.html];
        while let Some(n) = stack.pop() {
            let Some(node) = dom.node(n) else { continue };
            let shown = styles.get(n as usize).and_then(Option::as_ref)
                .is_none_or(|s| s.base.display != super::cascade::Display::None);
            if shown {
                if let Some(r) = rendered.get_mut(n as usize) { *r = true; }
                stack.extend(node.children.iter().copied());
            }
        }
        for (i, es) in styles.iter().enumerate() {
            let Some(es) = es else { continue };
            if !rendered[i] {
                continue;
            }
            let n = i as NodeId;
            let idle = |s: &Style| s.transitions.is_empty() && s.animations.is_empty();
            if idle(&es.base) && es.before.as_ref().is_none_or(idle) && es.after.as_ref().is_none_or(idle)
                && ![Part::Main, Part::Before, Part::After].iter().any(|p| self.state.contains_key(&(n, *p)))
            {
                continue;
            }
            let mut changed = false;
            let mut new = (**es).clone();
            for (part, st) in [(Part::Main, Some(&mut new.base)), (Part::Before, new.before.as_mut()), (Part::After, new.after.as_mut())] {
                let Some(st) = st else { continue };
                if part != Part::Main && st.display == super::cascade::Display::None {
                    continue;
                }
                seen.push((n, part));
                if self.step(cascade, (n, part), st, now) {
                    changed = true;
                }
            }
            if changed {
                out[i] = Some(Rc::new(new));
            }
        }
        // Boxes no longer rendered lose their state (no transition when they reappear).
        if seen.len() != self.state.len() {
            let keep: std::collections::HashSet<_> = seen.into_iter().collect();
            self.state.retain(|k, _| keep.contains(k));
        }
        out
    }

    /// Updates one box; mutates `st` to the animated values. Returns whether `st` changed.
    fn step(&mut self, cascade: &Cascade, key: (NodeId, Part), st: &mut Style, now: f64) -> bool {
        let scale = self.scale;
        let bs = self.state.entry(key).or_default();
        let mut changed = false;
        for p in PROPS {
            let v = get(st, p);
            match bs.target.get(&p) {
                Some(old) if *old != v => {
                    let current = match bs.running.get(&p) {
                        Some(r) => r.value(now),
                        None => old.clone(),
                    };
                    match covers(&st.transitions, p).filter(|t| t.dur + t.delay.max(0.0) > 0.0) {
                        Some(t) if current != v => {
                            bs.running.insert(
                                p,
                                Running { from: current, to: v.clone(), start: now, dur: t.dur as f64 * scale, delay: t.delay as f64 * scale, easing: t.easing.clone() },
                            );
                            self.events.push((key.0, "transitionrun"));
                        }
                        _ => {
                            bs.running.remove(&p);
                        }
                    }
                    bs.target.insert(p, v);
                }
                Some(_) => {}
                None => {
                    bs.target.insert(p, v);
                }
            }
        }
        let mut ended = Vec::new();
        for (p, r) in bs.running.iter() {
            if r.done(now) {
                ended.push(*p);
            } else {
                set(st, *p, r.value(now));
                changed = true;
            }
        }
        for p in ended {
            bs.running.remove(&p);
            self.events.push((key.0, "transitionend"));
        }

        // ---- keyframe animations
        let list = st.animations.clone();
        let mut runs: Vec<AnimRun> = Vec::new();
        for a in list.iter() {
            if a.name.is_empty() || a.name == "none" || a.dur <= 0.0 || !cascade.keyframes.contains_key(&a.name) {
                continue;
            }
            let prev = bs.anims.iter().position(|r| r.name == a.name);
            let mut run = match prev {
                Some(i) => bs.anims.remove(i),
                None => AnimRun { name: a.name.clone(), start: now, paused_at: None, layout: keyframes_affect_layout(cascade, &a.name) },
            };
            let mut elapsed = now - run.start - a.delay as f64 * scale;
            if a.paused {
                let e = *run.paused_at.get_or_insert(elapsed);
                elapsed = e;
            } else if let Some(e) = run.paused_at.take() {
                run.start = now - a.delay as f64 * scale - e;
                elapsed = e;
            }
            let dur = a.dur as f64 * scale;
            let total = if a.count.is_finite() { dur * a.count as f64 } else { f64::INFINITY };
            if elapsed >= 0.0 && elapsed < total {
                let iter = (elapsed / dur).floor();
                let mut p = ((elapsed - iter * dur) / dur) as f32;
                if a.alternate && (iter as i64) % 2 == 1 {
                    p = 1.0 - p;
                }
                keyframe(cascade, &a.name, &a.easing, p, st);
                changed = true;
                runs.push(run);
            } else if elapsed < 0.0 || a.paused {
                runs.push(run);
            }
            // finished runs are dropped but remembered so they do not restart:
            else {
                runs.push(AnimRun { start: f64::NEG_INFINITY, ..run });
            }
        }
        bs.anims = runs;
        // Finished animations never need frames; keep `active()` honest.
        let all_finished = bs.anims.iter().all(|r| r.start == f64::NEG_INFINITY);
        if all_finished && !bs.anims.is_empty() {
            // Keep the entries (so they do not restart) but mark this box as idle by storing
            // them in a form `active()` ignores.
            for r in bs.anims.iter_mut() {
                r.paused_at = Some(f64::INFINITY);
            }
        }
        changed
    }
}

fn keyframe(cascade: &Cascade, name: &str, easing: &Easing, at: f32, st: &mut Style) {
    let Some(kf) = cascade.keyframes.get(name) else { return };
    let base = st.clone();
    // Values per property at each keyframe offset.
    let mut tracks: HashMap<Prop, Vec<(f32, Val)>> = HashMap::new();
    for (off, decls) in &kf.frames {
        let mut s = base.clone();
        let mut touched = Vec::new();
        for d in decls {
            let v = if d.value.contains("var(") { subst(&d.value, &base.vars, 0).unwrap_or_default() } else { d.value.clone() };
            if apply(&mut s, &base, &d.name, v.trim()) {
                touched.extend(PROPS.iter().copied().filter(|p| p.names().contains(&d.name.as_str())));
            }
        }
        for p in touched {
            tracks.entry(p).or_default().push((*off, get(&s, p)));
        }
    }
    for (p, mut frames) in tracks {
        frames.sort_by(|a, b| a.0.total_cmp(&b.0));
        frames.dedup_by(|a, b| a.0 == b.0);
        if frames[0].0 > 0.0 {
            frames.insert(0, (0.0, get(&base, p)));
        }
        if frames.last().unwrap().0 < 1.0 {
            frames.push((1.0, get(&base, p)));
        }
        let i = frames.iter().position(|f| f.0 > at).unwrap_or(frames.len() - 1).max(1);
        let (a, b) = (&frames[i - 1], &frames[i]);
        let local = if b.0 > a.0 { ((at - a.0) / (b.0 - a.0)).clamp(0.0, 1.0) } else { 1.0 };
        set(st, p, lerp(&a.1, &b.1, easing.apply(local)));
    }
}

impl BoxState {
    fn live(&self) -> bool {
        !self.running.is_empty() || self.anims.iter().any(|r| r.paused_at.is_none() && r.start != f64::NEG_INFINITY)
    }
}

const LAYOUT_PROPS: [Prop; 6] = [Prop::Width, Prop::Height, Prop::FlexGrow, Prop::ColumnGap, Prop::RowGap, Prop::GridCols];

fn keyframes_affect_layout(cascade: &Cascade, name: &str) -> bool {
    cascade.keyframes.get(name).is_some_and(|kf| kf.frames.iter().any(|(_, decls)|
        decls.iter().any(|d| LAYOUT_PROPS.iter().any(|p| p.names().contains(&d.name.as_str())))))
}

impl Animator {
    /// Needs another frame soon.
    pub fn busy(&self) -> bool {
        self.state.values().any(|b| b.live())
    }
    pub fn layout_busy(&self) -> bool {
        self.state.values().any(|b| b.running.keys().any(|p| LAYOUT_PROPS.contains(p))
            || b.anims.iter().any(|r| r.layout && r.paused_at.is_none() && r.start != f64::NEG_INFINITY))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::Dom;

    #[test]
    fn hidden_ancestor_animations_idle() {
        let c = Cascade::new(".bars { display: none } .bars i { animation: pulse 1s infinite } @keyframes pulse { from { opacity: 0 } to { opacity: 1 } }");
        let d = Dom::from_index();
        let mut a = Animator::default();
        a.apply(&c, &d, &c.compute(&d), 0.0);
        assert!(!a.busy(), "hidden bars must not request frames");
    }

    #[test]
    fn layout_busy_only_for_layout_props() {
        let c = Cascade::new(".ambient { opacity: 0; transition: opacity 1s } body[data-state=open] .ambient { opacity: 1 } #island { width: 100px; transition: width 1s }");
        let mut d = Dom::from_index();
        let mut a = Animator::default();
        a.apply(&c, &d, &c.compute(&d), 0.0);
        d.set_attr(d.body, "data-state", "open");
        a.apply(&c, &d, &c.compute(&d), 0.0);
        a.apply(&c, &d, &c.compute(&d), 100.0);
        assert!(a.busy() && !a.layout_busy());
        d.style_set(d.by_id("island").unwrap(), "width", "200px");
        a.apply(&c, &d, &c.compute(&d), 100.0);
        assert!(a.layout_busy());
    }

    #[test]
    fn transform_shorter_list_keeps_percentage_prefix() {
        let center = Tf::Translate(Lp::pct(-50.0), Lp::ZERO);
        let closed = [center, Tf::Translate(Lp::ZERO, Lp::px(-8.0)), Tf::Scale(0.96, 0.96)];
        let open = [center];
        for (a, b) in [(&closed[..], &open[..]), (&open[..], &closed[..])] {
            let mid = lerp_tf(a, b, 0.5);
            assert_eq!(mid.len(), 3);
            assert_eq!(mid[0], center);
            assert_eq!(mid[1], Tf::Translate(Lp::ZERO, Lp::px(-4.0)));
            let Tf::Scale(x, y) = mid[2] else { panic!("expected scale, got {:?}", mid[2]) };
            assert!((x - 0.98).abs() < 1e-6 && (y - 0.98).abs() < 1e-6);
        }
    }

    #[test]
    fn transform_matrix_fallback_keeps_compatible_prefix() {
        let center = Tf::Translate(Lp::pct(-50.0), Lp::ZERO);
        let mid = lerp_tf(&[center, Tf::Scale(0.96, 0.96)], &[center, Tf::Rotate(0.0)], 0.5);
        assert_eq!(mid.len(), 2);
        assert_eq!(mid[0], center);
        let Tf::Matrix(m) = mid[1] else { panic!("expected matrix, got {:?}", mid[1]) };
        assert!((m[0] - 0.98).abs() < 1e-6 && (m[3] - 0.98).abs() < 1e-6);
        assert_eq!([m[1], m[2], m[4], m[5]], [0.0; 4]);
    }

    #[test]
    fn island_width_transitions_with_spring() {
        let c = Cascade::new(include_str!("../../ui/style.css"));
        let mut d = Dom::from_index();
        let island = d.by_id("island").unwrap() as usize;
        let mut a = Animator::default();
        let _ = a.apply(&c, &d, &c.compute(&d), 0.0);
        d.set_attr(d.body, "data-state", "open");
        d.style_set(d.body, "--w", "460px");
        let st = c.compute(&d);
        let w = |o: &Vec<Option<Rc<ElemStyle>>>| o[island].as_ref().unwrap().base.width.lp().unwrap().px;
        let o = a.apply(&c, &d, &st, 0.0);
        assert_eq!(w(&o), 168.0);
        assert!(a.events.contains(&(island as u32, "transitionrun")));
        let mid = w(&a.apply(&c, &d, &st, 200.0));
        assert!(mid > 168.0 && mid < 460.0 + 10.0, "{mid}");
        let peak = (0..=620).step_by(10).map(|t| w(&a.apply(&c, &d, &st, t as f64))).fold(0.0f32, f32::max);
        assert!(peak > 460.0, "spring must overshoot: {peak}");
        assert_eq!(w(&a.apply(&c, &d, &st, 700.0)), 460.0);
        assert!(a.events.contains(&(island as u32, "transitionend")));
    }

    #[test]
    fn layer_visibility_delay_and_keyframes() {
        let c = Cascade::new(include_str!("../../ui/style.css"));
        let mut d = Dom::from_index();
        let row = d.by_id("row").unwrap() as usize;
        let mut a = Animator::default();
        let _ = a.apply(&c, &d, &c.compute(&d), 0.0);
        d.set_attr(d.body, "data-state", "open");
        let st = c.compute(&d);
        let vis = |o: &Vec<Option<Rc<ElemStyle>>>| o[row].as_ref().unwrap().base.visible;
        assert!(vis(&a.apply(&c, &d, &st, 10.0)), "row stays visible during visibility delay");
        assert!(!vis(&a.apply(&c, &d, &st, 260.0)), "hidden after .2s");

        // `.eye.blink` keyframe at 50% squashes scaleY to .1
        let eye = d.query_str(d.html, "#mascot .eye", false)[0];
        d.add_class(eye, "blink");
        let st = c.compute(&d);
        let _ = a.apply(&c, &d, &st, 1000.0);
        let tf = a.apply(&c, &d, &st, 1070.0)[eye as usize].as_ref().unwrap().base.transform.clone();
        assert!(tf.iter().any(|t| matches!(t, Tf::Scale(_, y) if (*y - 0.1).abs() < 0.05)), "{tf:?}");
    }
}
