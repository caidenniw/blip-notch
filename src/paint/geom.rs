//! World transforms, clip chains and paint order shared by the painter, hit-testing and
//! `getBoundingClientRect`.
use crate::css::cascade::{Display, ElemStyle, Position, Style};
use crate::css::value::{Lp, Tf};
use crate::dom::{Dom, NodeId};
use crate::layout::{BoxGeom, Kid, Layout, Part};
use std::collections::HashMap;
use std::rc::Rc;

/// 2D affine map, column-vector convention: `x' = a x + c y + e`, `y' = b x + d y + f`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
}

impl Affine {
    pub const ID: Affine = Affine { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: 0.0, f: 0.0 };
    pub fn translate(x: f32, y: f32) -> Affine {
        Affine { e: x, f: y, ..Affine::ID }
    }
    pub fn scale(x: f32, y: f32) -> Affine {
        Affine { a: x, d: y, ..Affine::ID }
    }
    pub fn rotate(deg: f32) -> Affine {
        let (s, c) = deg.to_radians().sin_cos();
        Affine { a: c, b: s, c: -s, d: c, e: 0.0, f: 0.0 }
    }
    /// `self ∘ o` (apply `o` first).
    pub fn mul(self, o: Affine) -> Affine {
        Affine {
            a: self.a * o.a + self.c * o.b,
            b: self.b * o.a + self.d * o.b,
            c: self.a * o.c + self.c * o.d,
            d: self.b * o.c + self.d * o.d,
            e: self.a * o.e + self.c * o.f + self.e,
            f: self.b * o.e + self.d * o.f + self.f,
        }
    }
    pub fn apply(self, x: f32, y: f32) -> (f32, f32) {
        (self.a * x + self.c * y + self.e, self.b * x + self.d * y + self.f)
    }
    pub fn invert(self) -> Option<Affine> {
        let det = self.a * self.d - self.b * self.c;
        if det.abs() < 1e-9 {
            return None;
        }
        let i = 1.0 / det;
        Some(Affine {
            a: self.d * i,
            b: -self.b * i,
            c: -self.c * i,
            d: self.a * i,
            e: (self.c * self.f - self.d * self.e) * i,
            f: (self.b * self.e - self.a * self.f) * i,
        })
    }
    pub fn to_d2d(self) -> windows_numerics::Matrix3x2 {
        windows_numerics::Matrix3x2 { M11: self.a, M12: self.b, M21: self.c, M22: self.d, M31: self.e, M32: self.f }
    }
    /// Axis-aligned bounds of the mapped rectangle.
    pub fn bounds(self, x: f32, y: f32, w: f32, h: f32) -> (f32, f32, f32, f32) {
        let pts = [self.apply(x, y), self.apply(x + w, y), self.apply(x, y + h), self.apply(x + w, y + h)];
        let (mut l, mut t, mut r, mut b) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for (px, py) in pts {
            l = l.min(px);
            t = t.min(py);
            r = r.max(px);
            b = b.max(py);
        }
        (l, t, r - l, b - t)
    }
}

pub fn tf_matrix(list: &[Tf], w: f32, h: f32) -> Affine {
    let mut m = Affine::ID;
    for t in list {
        m = m.mul(match *t {
            Tf::Translate(x, y) => Affine::translate(x.resolve(w), y.resolve(h)),
            Tf::Scale(x, y) => Affine::scale(x, y),
            Tf::Rotate(d) => Affine::rotate(d),
            Tf::Matrix([a, b, c, d, e, f]) => Affine { a, b, c, d, e, f },
        });
    }
    m
}

/// Element-local transform in document space (transform-origin, `translate`, `transform`).
pub fn local_transform(st: &Style, g: &BoxGeom) -> Affine {
    let has_tr = st.translate != (Lp::ZERO, Lp::ZERO);
    if st.transform.is_empty() && !has_tr {
        return Affine::ID;
    }
    let ox = g.x + st.origin.0.resolve(g.w);
    let oy = g.y + st.origin.1.resolve(g.h);
    Affine::translate(ox, oy)
        .mul(Affine::translate(st.translate.0.resolve(g.w), st.translate.1.resolve(g.h)))
        .mul(tf_matrix(&st.transform, g.w, g.h))
        .mul(Affine::translate(-ox, -oy))
}

/// Clip region: rounded rectangle in the space of `m`.
#[derive(Clone, Debug)]
pub struct Clip {
    pub m: Affine,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub radii: [(f32, f32); 4],
}

impl Clip {
    pub fn contains(&self, px: f32, py: f32) -> bool {
        let Some(inv) = self.m.invert() else { return false };
        let (x, y) = inv.apply(px, py);
        rrect_contains(self.x, self.y, self.w, self.h, &self.radii, x, y)
    }
}

pub fn rrect_contains(x: f32, y: f32, w: f32, h: f32, r: &[(f32, f32); 4], px: f32, py: f32) -> bool {
    if px < x || py < y || px > x + w || py > y + h {
        return false;
    }
    let corner = |cx: f32, cy: f32, rx: f32, ry: f32| {
        if rx <= 0.0 || ry <= 0.0 {
            return true;
        }
        let dx = (px - cx) / rx;
        let dy = (py - cy) / ry;
        dx * dx + dy * dy <= 1.0
    };
    let (tl, tr, br, bl) = (r[0], r[1], r[2], r[3]);
    if px < x + tl.0 && py < y + tl.1 {
        return corner(x + tl.0, y + tl.1, tl.0, tl.1);
    }
    if px > x + w - tr.0 && py < y + tr.1 {
        return corner(x + w - tr.0, y + tr.1, tr.0, tr.1);
    }
    if px > x + w - br.0 && py > y + h - br.1 {
        return corner(x + w - br.0, y + h - br.1, br.0, br.1);
    }
    if px < x + bl.0 && py > y + h - bl.1 {
        return corner(x + bl.0, y + h - bl.1, bl.0, bl.1);
    }
    true
}

/// CSS border radii resolved and clamped for a `w`×`h` box.
pub fn radii(st: &Style, w: f32, h: f32) -> [(f32, f32); 4] {
    let mut r = st.radius.map(|(a, b)| (a.resolve(w).max(0.0), b.resolve(h).max(0.0)));
    let mut f: f32 = 1.0;
    let sum = |a: f32, b: f32, len: f32| if a + b > 0.0 { len / (a + b) } else { 1.0 };
    f = f.min(sum(r[0].0, r[1].0, w)).min(sum(r[3].0, r[2].0, w)).min(sum(r[0].1, r[3].1, h)).min(sum(r[1].1, r[2].1, h));
    if f < 1.0 {
        for x in r.iter_mut() {
            *x = (x.0 * f, x.1 * f);
        }
    }
    r
}

/// Radii shrunk by the border widths (padding-box curve).
pub fn inner_radii(r: [(f32, f32); 4], b: [f32; 4]) -> [(f32, f32); 4] {
    let (t, ri, bo, l) = (b[0], b[1], b[2], b[3]);
    [
        ((r[0].0 - l).max(0.0), (r[0].1 - t).max(0.0)),
        ((r[1].0 - ri).max(0.0), (r[1].1 - t).max(0.0)),
        ((r[2].0 - ri).max(0.0), (r[2].1 - bo).max(0.0)),
        ((r[3].0 - l).max(0.0), (r[3].1 - bo).max(0.0)),
    ]
}

/// Child roots owned by a box; contexts hoist descendants across noncontext ancestors.
#[derive(Default)]
pub struct Schedule {
    pub children: Vec<(NodeId, Part)>,
    pub negative: usize,
}

fn establishes_context(st: &Style) -> bool {
    st.opacity < 1.0 || !st.transform.is_empty() || st.translate != (Lp::ZERO, Lp::ZERO)
        || st.mask.is_some() || !st.filter.is_empty()
        || (st.position != Position::Static && st.z_index.is_some())
}

#[derive(Default)]
pub struct Geom {
    pub world: HashMap<(NodeId, Part), Affine>,
    /// Ancestor overflow clips of each box (excluding its own).
    pub clips: HashMap<(NodeId, Part), Rc<Vec<Clip>>>,
    /// Boxes in the same stacking order consumed by the painter.
    pub order: Vec<(NodeId, Part)>,
    pub schedule: HashMap<(NodeId, Part), Schedule>,
}

pub fn compute(dom: &Dom, styles: &[Option<Rc<ElemStyle>>], layout: &Layout) -> Geom {
    let mut g = Geom::default();
    walk(dom, styles, layout, dom.html, Affine::ID, Rc::new(Vec::new()), &mut g);
    stacking(styles, (dom.html, Part::Main), &mut g);
    flatten((dom.html, Part::Main), &mut g);
    g
}

fn walk(
    dom: &Dom,
    styles: &[Option<Rc<ElemStyle>>],
    layout: &Layout,
    n: NodeId,
    parent: Affine,
    clips: Rc<Vec<Clip>>,
    out: &mut Geom,
) {
    let Some(es) = styles.get(n as usize).and_then(|s| s.as_deref()) else { return };
    let st = &es.base;
    if st.display == Display::None {
        return;
    }
    let Some(bx) = layout.boxes.get(&(n, Part::Main)) else { return };
    let world = parent.mul(local_transform(st, bx));
    out.world.insert((n, Part::Main), world);
    out.clips.insert((n, Part::Main), clips.clone());
    let mut children = Vec::new();
    let (sx, sy) = dom.scroll_of(n);
    let child_world = if sx != 0.0 || sy != 0.0 { world.mul(Affine::translate(-sx, -sy)) } else { world };
    let child_clips = if st.clips() || matches!(dom.tag(n), "input" | "textarea") {
        let (px, py, pw, ph) = bx.padding_box();
        let mut v = (*clips).clone();
        v.push(Clip { m: world, x: px, y: py, w: pw, h: ph, radii: inner_radii(radii(st, bx.w, bx.h), bx.border) });
        Rc::new(v)
    } else {
        clips
    };
    let part = |p: Part, ps: Option<&Style>, out: &mut Geom, children: &mut Vec<(NodeId, Part)>| {
        if let Some(pb) = layout.boxes.get(&(n, p)) {
            if ps.is_some_and(|s| s.display == Display::None) { return; }
            let w = match ps {
                Some(s) => child_world.mul(local_transform(s, pb)),
                None => child_world,
            };
            out.world.insert((n, p), w);
            out.clips.insert((n, p), child_clips.clone());
            children.push((n, p));
        }
    };
    if let Some(leaf) = layout.leaves.get(&(n, Part::Main)) {
        if let Some(c) = leaf.caret {
            walk(dom, styles, layout, c, child_world, child_clips.clone(), out);
            if out.world.contains_key(&(c, Part::Main)) { children.push((c, Part::Main)); }
        }
    }
    for k in layout.kids.get(&n).map(Vec::as_slice).unwrap_or(&[]) {
        match *k {
            Kid::Elem(c) => {
                walk(dom, styles, layout, c, child_world, child_clips.clone(), out);
                if out.world.contains_key(&(c, Part::Main)) { children.push((c, Part::Main)); }
            }
            Kid::Part(p @ Part::Before) => part(p, es.before.as_ref(), out, &mut children),
            Kid::Part(p @ Part::After) => part(p, es.after.as_ref(), out, &mut children),
            Kid::Part(p) => part(p, None, out, &mut children),
        }
    }
    out.schedule.insert((n, Part::Main), Schedule { children, negative: 0 });
}

fn box_style(styles: &[Option<Rc<ElemStyle>>], key: (NodeId, Part)) -> Option<&Style> {
    let es = styles.get(key.0 as usize)?.as_deref()?;
    match key.1 {
        Part::Main => Some(&es.base),
        Part::Before => es.before.as_ref(),
        Part::After => es.after.as_ref(),
        _ => None,
    }
}

/// Gather only until an actual context boundary. Positioned auto boxes join the zero
/// phase but their descendant contexts still belong to the nearest actual context.
fn gather(
    styles: &[Option<Rc<ElemStyle>>], key: (NodeId, Part), out: &mut Geom,
    deferred: &mut Vec<(i32, (NodeId, Part))>,
) -> bool {
    let st = box_style(styles, key);
    if st.is_some_and(establishes_context) {
        let z = st.filter(|s| s.position != Position::Static).and_then(|s| s.z_index).unwrap_or(0);
        deferred.push((z, key));
        stacking(styles, key, out);
        return false;
    }
    let positioned = st.is_some_and(|s| s.position != Position::Static);
    if positioned { deferred.push((0, key)); }
    let mut schedule = out.schedule.remove(&key).unwrap_or_default();
    schedule.children.retain(|&child| gather(styles, child, out, deferred));
    out.schedule.insert(key, schedule);
    !positioned
}

fn stacking(styles: &[Option<Rc<ElemStyle>>], key: (NodeId, Part), out: &mut Geom) {
    let mut schedule = out.schedule.remove(&key).unwrap_or_default();
    let mut deferred = Vec::new();
    schedule.children.retain(|&child| gather(styles, child, out, &mut deferred));
    deferred.sort_by_key(|&(z, _)| z);
    let negative = deferred.partition_point(|&(z, _)| z < 0);
    let mut children = Vec::with_capacity(schedule.children.len() + deferred.len());
    children.extend(deferred[..negative].iter().map(|&(_, key)| key));
    children.append(&mut schedule.children);
    children.extend(deferred[negative..].iter().map(|&(_, key)| key));
    out.schedule.insert(key, Schedule { children, negative });
}

fn flatten(key: (NodeId, Part), out: &mut Geom) {
    if !out.world.contains_key(&key) { return; }
    out.order.push(key);
    // Borrow order and schedules separately: no per-frame child list copies.
    fn children(key: (NodeId, Part), schedules: &HashMap<(NodeId, Part), Schedule>, order: &mut Vec<(NodeId, Part)>) {
        if let Some(schedule) = schedules.get(&key) {
            for &child in &schedule.children {
                order.push(child);
                children(child, schedules, order);
            }
        }
    }
    children(key, &out.schedule, &mut out.order);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn affine_roundtrip() {
        let m = Affine::translate(10.0, 5.0).mul(Affine::rotate(30.0)).mul(Affine::scale(2.0, 0.5));
        let (x, y) = m.apply(3.0, 4.0);
        let (bx, by) = m.invert().unwrap().apply(x, y);
        assert!((bx - 3.0).abs() < 1e-4 && (by - 4.0).abs() < 1e-4);
    }

    #[test]
    fn island_translate_centres() {
        use crate::css::cascade::Cascade;
        let d = Dom::from_index();
        let st = Cascade::new(include_str!("../../ui/style.css")).compute(&d);
        let mut t = crate::gfx::text::Text::new().unwrap();
        let l = crate::layout::compute(&d, &st, &mut t, (crate::layout::VIEW_W, crate::layout::VIEW_H));
        let g = compute(&d, &st, &l);
        let island = d.by_id("island").unwrap();
        let b = l.boxes[&(island, Part::Main)];
        let (x, _, w, _) = g.world[&(island, Part::Main)].bounds(b.x, b.y, b.w, b.h);
        assert_eq!((x, w), (236.0, 168.0));
    }
}
