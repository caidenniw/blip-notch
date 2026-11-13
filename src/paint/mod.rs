//! Painter: draws the styled, laid-out DOM with Direct2D (anti-aliased geometry, grayscale text).
pub mod geom;
pub mod images;

use crate::css::cascade::{BorderStyle, Display, ElemStyle, Overflow, Paint, Style};
use crate::css::value::{Bg, Color, Filter, Gradient, Stop};
use crate::dom::{Dom, NodeId};
use crate::gfx::text::{self, Text};
use crate::gfx::Gfx;
use crate::layout::{BoxGeom, Layout, Part};
use geom::{inner_radii, radii, Affine, Geom};
use std::collections::HashMap;
use std::mem::ManuallyDrop;
use std::rc::Rc;
use windows::core::Interface;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows_numerics::Vector2;

/// Everything one frame needs.
pub struct Scene<'a> {
    pub dom: &'a Dom,
    pub styles: &'a [Option<Rc<ElemStyle>>],
    pub layout: &'a Layout,
    pub geom: &'a Geom,
    pub overlay: &'a Overlay,
}

/// Editing overlays owned by the input layer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Overlay {
    /// Blinking caret: (editable element, UTF-16 index), drawn when `Some`.
    pub caret: Option<(NodeId, u32)>,
    /// Selection highlight: (element, part, start, len) in UTF-16 units.
    pub sel: Option<(NodeId, Part, u32, u32)>,
}

const SEL: Color = Color { r: 51.0 / 255.0, g: 144.0 / 255.0, b: 1.0, a: 0.6 };
const THUMB: Color = Color { r: 1.0, g: 1.0, b: 1.0, a: 0.14 };
/// Layer content bounds: the whole canvas with margin. Huge values (D2D's "infinite" rect)
/// overflow once combined with a non-identity scale and corrupt the opacity-brush mapping.
const INF: D2D_RECT_F = D2D_RECT_F { left: -4096.0, top: -4096.0, right: 8192.0, bottom: 8192.0 };

fn col(c: Color) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r: c.r, g: c.g, b: c.b, a: c.a }
}
fn rect(x: f32, y: f32, w: f32, h: f32) -> D2D_RECT_F {
    D2D_RECT_F { left: x, top: y, right: x + w, bottom: y + h }
}
fn v2(x: f32, y: f32) -> Vector2 {
    Vector2 { X: x, Y: y }
}

enum Pushed {
    Layer,
    Axis,
}

#[derive(Default)]
pub struct Painter {
    pub text: Option<Text>,
    pub images: images::ImageStore,
    bitmaps: HashMap<String, Option<ID2D1Bitmap1>>,
    brushes: HashMap<(usize, u32), ID2D1SolidColorBrush>,
    paths: HashMap<String, Option<ID2D1Geometry>>,
    strokes: HashMap<(bool, bool, bool), ID2D1StrokeStyle>,
    aux: Vec<ID2D1DeviceContext>,
    depth: usize,
    factory: Option<ID2D1Factory1>,
    base: Option<Affine>,
    scale: f32,
}

impl Painter {
    pub fn new() -> Painter {
        Painter { text: Text::new().ok(), scale: 1.0, ..Default::default() }
    }

    pub fn text(&mut self) -> &mut Text {
        self.text.get_or_insert_with(|| Text::new().expect("DirectWrite"))
    }

    /// Drops device-dependent resources (after `Gfx::rebuild`).
    pub fn reset_device(&mut self) {
        self.bitmaps.clear();
        self.brushes.clear();
        self.aux.clear();
        self.paths.clear();
        self.strokes.clear();
    }

    /// Paints the scene into `gfx.dc` (between `Gfx::begin` and `present`/`end`).
    pub fn paint(&mut self, gfx: &Gfx, s: &Scene, scale: f32) {
        if self.factory.as_ref().map(|f| f.as_raw()) != Some(gfx.factory.as_raw()) {
            self.reset_device();
            self.factory = Some(gfx.factory.clone());
        }
        while self.aux.len() < 4 {
            match gfx.aux() {
                Ok(a) => self.aux.push(a),
                Err(_) => break,
            }
        }
        self.scale = scale;
        self.base = Some(Affine::scale(scale, scale));
        self.depth = 0;
        let dc = gfx.dc.clone();
        self.node(&dc, s, s.dom.html);
        unsafe { dc.SetTransform(&Affine::scale(scale, scale).to_d2d()) };
    }

    fn f(&self) -> &ID2D1Factory1 {
        self.factory.as_ref().unwrap()
    }
    fn m(&self, s: &Scene, key: (NodeId, Part)) -> Option<Affine> {
        Some(self.base?.mul(*s.geom.world.get(&key)?))
    }

    // ------------------------------------------------------------------ recording / effects

    fn record(&mut self, f: impl FnOnce(&mut Painter, &ID2D1DeviceContext)) -> Option<ID2D1CommandList> {
        let aux = self.aux.get(self.depth)?.clone();
        let cl = unsafe { aux.CreateCommandList() }.ok()?;
        self.depth += 1;
        unsafe {
            aux.SetTarget(&cl);
            aux.BeginDraw();
        }
        f(self, &aux);
        unsafe {
            let _ = aux.EndDraw(None, None);
            aux.SetTarget(None::<&ID2D1Image>);
        }
        self.depth -= 1;
        unsafe { cl.Close() }.ok()?;
        Some(cl)
    }

    fn blur(dc: &ID2D1DeviceContext, input: &ID2D1Image, sigma: f32) -> Option<ID2D1Image> {
        unsafe {
            let e = dc.CreateEffect(&CLSID_D2D1GaussianBlur).ok()?;
            e.SetInput(0, input, true);
            e.SetValue(D2D1_GAUSSIANBLUR_PROP_STANDARD_DEVIATION.0 as u32, D2D1_PROPERTY_TYPE_FLOAT, &sigma.to_le_bytes()).ok()?;
            e.GetOutput().ok()
        }
    }

    fn shadow(dc: &ID2D1DeviceContext, input: &ID2D1Image, sigma: f32, c: Color) -> Option<ID2D1Image> {
        unsafe {
            let e = dc.CreateEffect(&CLSID_D2D1Shadow).ok()?;
            e.SetInput(0, input, true);
            e.SetValue(D2D1_SHADOW_PROP_BLUR_STANDARD_DEVIATION.0 as u32, D2D1_PROPERTY_TYPE_FLOAT, &sigma.to_le_bytes()).ok()?;
            let mut v = Vec::with_capacity(16);
            for x in [c.r, c.g, c.b, c.a] {
                v.extend_from_slice(&x.to_le_bytes());
            }
            e.SetValue(D2D1_SHADOW_PROP_COLOR.0 as u32, D2D1_PROPERTY_TYPE_VECTOR4, &v).ok()?;
            e.GetOutput().ok()
        }
    }

    fn draw_image(dc: &ID2D1DeviceContext, img: &ID2D1Image, off: (f32, f32)) {
        unsafe {
            dc.SetTransform(&Affine::ID.to_d2d());
            dc.DrawImage(img, Some(&v2(off.0, off.1)), None, D2D1_INTERPOLATION_MODE_LINEAR, D2D1_COMPOSITE_MODE_SOURCE_OVER);
        }
    }

    /// Renders `f` into a command list and draws it through CSS `filter`s.
    fn filtered(&mut self, dc: &ID2D1DeviceContext, filters: &[Filter], f: impl FnOnce(&mut Painter, &ID2D1DeviceContext)) {
        let Some(cl) = self.record(f) else { return };
        let Ok(mut cur) = cl.cast::<ID2D1Image>() else { return };
        let k = self.scale;
        for flt in filters {
            match *flt {
                Filter::Blur(r) if r > 0.0 => {
                    if let Some(o) = Self::blur(dc, &cur, r * k) {
                        cur = o;
                    }
                }
                Filter::DropShadow(sh) => {
                    if let Some(o) = Self::shadow(dc, &cur, sh.blur / 2.0 * k, sh.color) {
                        Self::draw_image(dc, &o, (sh.x * k, sh.y * k));
                    }
                }
                _ => {}
            }
        }
        Self::draw_image(dc, &cur, (0.0, 0.0));
    }

    // ------------------------------------------------------------------ geometry helpers

    fn rrect(&self, x: f32, y: f32, w: f32, h: f32, r: &[(f32, f32); 4]) -> Option<ID2D1Geometry> {
        let w = w.max(0.0);
        let h = h.max(0.0);
        unsafe {
            if r.iter().all(|&(a, b)| a <= 0.0 || b <= 0.0) {
                return self.f().CreateRectangleGeometry(&rect(x, y, w, h)).ok()?.cast().ok();
            }
            let g = self.f().CreatePathGeometry().ok()?;
            let sk = g.Open().ok()?;
            let arc = |px: f32, py: f32, (rx, ry): (f32, f32)| {
                if rx > 0.0 && ry > 0.0 {
                    sk.AddArc(&D2D1_ARC_SEGMENT {
                        point: v2(px, py),
                        size: D2D_SIZE_F { width: rx, height: ry },
                        rotationAngle: 0.0,
                        sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                        arcSize: D2D1_ARC_SIZE_SMALL,
                    });
                } else {
                    sk.AddLine(v2(px, py));
                }
            };
            let [tl, tr, br, bl] = *r;
            let z = |c: (f32, f32)| if c.0 > 0.0 && c.1 > 0.0 { c } else { (0.0, 0.0) };
            let (tl, tr, br, bl) = (z(tl), z(tr), z(br), z(bl));
            sk.BeginFigure(v2(x + tl.0, y), D2D1_FIGURE_BEGIN_FILLED);
            sk.AddLine(v2(x + w - tr.0, y));
            arc(x + w, y + tr.1, tr);
            sk.AddLine(v2(x + w, y + h - br.1));
            arc(x + w - br.0, y + h, br);
            sk.AddLine(v2(x + bl.0, y + h));
            arc(x, y + h - bl.1, bl);
            sk.AddLine(v2(x, y + tl.1));
            arc(x + tl.0, y, tl);
            sk.EndFigure(D2D1_FIGURE_END_CLOSED);
            sk.Close().ok()?;
            g.cast().ok()
        }
    }

    fn stroke_style(&mut self, round_cap: bool, round_join: bool, dashed: bool) -> Option<ID2D1StrokeStyle> {
        if let Some(s) = self.strokes.get(&(round_cap, round_join, dashed)) {
            return Some(s.clone());
        }
        let cap = if round_cap { D2D1_CAP_STYLE_ROUND } else { D2D1_CAP_STYLE_FLAT };
        let props = D2D1_STROKE_STYLE_PROPERTIES {
            startCap: cap,
            endCap: cap,
            dashCap: D2D1_CAP_STYLE_FLAT,
            lineJoin: if round_join { D2D1_LINE_JOIN_ROUND } else { D2D1_LINE_JOIN_MITER },
            miterLimit: 4.0,
            dashStyle: if dashed { D2D1_DASH_STYLE_CUSTOM } else { D2D1_DASH_STYLE_SOLID },
            dashOffset: 0.0,
        };
        let dashes = [3.0f32, 3.0];
        let f: &ID2D1Factory = self.f();
        let s = unsafe { f.CreateStrokeStyle(&props, if dashed { Some(&dashes[..]) } else { None }) }.ok()?;
        self.strokes.insert((round_cap, round_join, dashed), s.clone());
        Some(s)
    }

    fn solid(&mut self, dc: &ID2D1DeviceContext, c: Color) -> Option<ID2D1SolidColorBrush> {
        let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
        let rgba = byte(c.r) | byte(c.g) << 8 | byte(c.b) << 16 | byte(c.a) << 24;
        let key = (dc.as_raw() as usize, rgba);
        if let Some(brush) = self.brushes.get(&key) { return Some(brush.clone()); }
        let brush = unsafe { dc.CreateSolidColorBrush(&col(c), None) }.ok()?;
        if self.brushes.len() >= 512 { self.brushes.clear(); }
        self.brushes.insert(key, brush.clone());
        Some(brush)
    }

    fn fill(dc: &ID2D1DeviceContext, g: &ID2D1Geometry, b: &ID2D1Brush) {
        unsafe { dc.FillGeometry(g, b, None::<&ID2D1Brush>) };
    }

    fn push_layer(dc: &ID2D1DeviceContext, mask: Option<ID2D1Geometry>, opacity: f32, brush: Option<ID2D1Brush>) {
        let p = D2D1_LAYER_PARAMETERS1 {
            contentBounds: INF,
            geometricMask: ManuallyDrop::new(mask),
            maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
            maskTransform: Affine::ID.to_d2d(),
            opacity,
            opacityBrush: ManuallyDrop::new(brush),
            layerOptions: D2D1_LAYER_OPTIONS1_NONE,
        };
        unsafe { dc.PushLayer(&p, None::<&ID2D1Layer>) };
        drop(ManuallyDrop::into_inner(p.geometricMask));
        drop(ManuallyDrop::into_inner(p.opacityBrush));
    }

    // ------------------------------------------------------------------ gradients

    fn resolve_stops(stops: &[Stop], len: f32) -> Vec<D2D1_GRADIENT_STOP> {
        let n = stops.len();
        let mut pos: Vec<Option<f32>> = stops.iter().map(|s| s.pos.map(|p| if len > 0.0 { p.resolve(len) / len } else { 0.0 })).collect();
        if pos[0].is_none() {
            pos[0] = Some(0.0);
        }
        if pos[n - 1].is_none() {
            pos[n - 1] = Some(1.0);
        }
        let mut i = 1;
        while i < n {
            if pos[i].is_none() {
                let j = (i..n).find(|&j| pos[j].is_some()).unwrap();
                let (a, b) = (pos[i - 1].unwrap(), pos[j].unwrap());
                for (k, p) in pos.iter_mut().enumerate().take(j).skip(i) {
                    *p = Some(a + (b - a) * (k - i + 1) as f32 / (j - i + 1) as f32);
                }
                i = j;
            }
            i += 1;
        }
        let mut last = f32::MIN;
        stops
            .iter()
            .zip(pos)
            .map(|(s, p)| {
                let p = p.unwrap().max(last);
                last = p;
                D2D1_GRADIENT_STOP { position: p, color: col(s.color) }
            })
            .collect()
    }

    fn stop_collection(dc: &ID2D1DeviceContext, st: &[D2D1_GRADIENT_STOP]) -> Option<ID2D1GradientStopCollection1> {
        unsafe {
            dc.CreateGradientStopCollection(
                st,
                D2D1_COLOR_SPACE_SRGB,
                D2D1_COLOR_SPACE_SRGB,
                D2D1_BUFFER_PRECISION_8BPC_UNORM,
                D2D1_EXTEND_MODE_CLAMP,
                D2D1_COLOR_INTERPOLATION_MODE_PREMULTIPLIED,
            )
        }
        .ok()
    }

    /// Brush for a linear/radial gradient painted over the box `(x, y, w, h)`.
    fn gradient_brush(dc: &ID2D1DeviceContext, g: &Gradient, x: f32, y: f32, w: f32, h: f32) -> Option<ID2D1Brush> {
        unsafe {
            match g {
                Gradient::Linear { angle, stops } => {
                    let a = angle.to_radians();
                    let (sn, cs) = a.sin_cos();
                    let len = (w * sn).abs() + (h * cs).abs();
                    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
                    let (dx, dy) = (sn * len / 2.0, -cs * len / 2.0);
                    let coll = Self::stop_collection(dc, &Self::resolve_stops(stops, len))?;
                    let props = D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES { startPoint: v2(cx - dx, cy - dy), endPoint: v2(cx + dx, cy + dy) };
                    dc.CreateLinearGradientBrush(&props, None, &coll).ok()?.cast().ok()
                }
                Gradient::Radial { at, stops } => {
                    let (cx, cy) = (x + at.0.resolve(w), y + at.1.resolve(h));
                    let r = [(x, y), (x + w, y), (x, y + h), (x + w, y + h)]
                        .iter()
                        .map(|&(px, py)| ((px - cx).powi(2) + (py - cy).powi(2)).sqrt())
                        .fold(0.0f32, f32::max);
                    let coll = Self::stop_collection(dc, &Self::resolve_stops(stops, r))?;
                    let props = D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES {
                        center: v2(cx, cy),
                        gradientOriginOffset: v2(0.0, 0.0),
                        radiusX: r,
                        radiusY: r,
                    };
                    dc.CreateRadialGradientBrush(&props, None, &coll).ok()?.cast().ok()
                }
                Gradient::Conic { .. } => None,
            }
        }
    }

    // ------------------------------------------------------------------ box decoration

    fn decor(&mut self, dc: &ID2D1DeviceContext, m: Affine, st: &Style, g: &BoxGeom) {
        let r = radii(st, g.w, g.h);
        let set = |dc: &ID2D1DeviceContext| unsafe { dc.SetTransform(&m.to_d2d()) };
        set(dc);
        // box-shadow: first shadow on top, so paint in reverse.
        for sh in st.shadows.iter().rev().filter(|s| !s.inset && s.color.a > 0.0) {
            let sp = sh.spread;
            let rr = r.map(|(a, b)| if a > 0.0 { (a + sp, b + sp) } else { (0.0, 0.0) });
            let Some(geom) = self.rrect(g.x - sp + sh.x, g.y - sp + sh.y, g.w + 2.0 * sp, g.h + 2.0 * sp, &rr) else { continue };
            if sh.blur <= 0.0 {
                if let Some(b) = self.solid(dc, sh.color) {
                    Self::fill(dc, &geom, &b.cast().unwrap());
                }
                continue;
            }
            let c = sh.color;
            let rec = self.record(|me, a| unsafe {
                a.SetTransform(&m.to_d2d());
                if let Some(b) = me.solid(a, c) {
                    Self::fill(a, &geom, &b.cast().unwrap());
                }
            });
            let k = (m.a * m.d - m.b * m.c).abs().sqrt();
            if let Some(img) = rec.and_then(|cl| cl.cast::<ID2D1Image>().ok()).and_then(|i| Self::blur(dc, &i, sh.blur / 2.0 * k)) {
                Self::draw_image(dc, &img, (0.0, 0.0));
            }
            set(dc);
        }
        // background (border-box)
        match &st.background {
            Bg::None => {}
            Bg::Color(c) if c.a <= 0.0 => {}
            Bg::Color(c) => {
                let geom = self.rrect(g.x, g.y, g.w, g.h, &r);
                let brush = self.solid(dc, *c);
                if let (Some(geom), Some(b)) = (geom, brush) {
                    Self::fill(dc, &geom, &b.cast().unwrap());
                }
            }
            Bg::Image(Gradient::Conic { stops }) => {
                let Some(geom) = self.rrect(g.x, g.y, g.w, g.h, &r) else { return };
                let (c1, p) = (stops[0].color, stops[0].pos.map(|l| l.pct / 100.0).unwrap_or(0.0).clamp(0.0, 1.0));
                let c2 = stops.last().unwrap().color;
                if let Some(b) = self.solid(dc, c2) {
                    Self::fill(dc, &geom, &b.cast().unwrap());
                }
                if p > 0.0 {
                    Self::push_layer(dc, Some(geom.clone()), 1.0, None);
                    let (cx, cy) = (g.x + g.w / 2.0, g.y + g.h / 2.0);
                    let rad = (g.w.max(g.h)) * 0.75;
                    let wedge = self.wedge(cx, cy, rad, p);
                    let brush = self.solid(dc, c1);
                    if let (Some(w), Some(b)) = (wedge, brush) {
                        Self::fill(dc, &w, &b.cast().unwrap());
                    }
                    unsafe { dc.PopLayer() };
                }
            }
            Bg::Image(gr) => {
                if let (Some(geom), Some(b)) = (self.rrect(g.x, g.y, g.w, g.h, &r), Self::gradient_brush(dc, gr, g.x, g.y, g.w, g.h)) {
                    Self::fill(dc, &geom, &b);
                }
            }
        }
        // border
        let bw = g.border;
        if bw.iter().any(|&w| w > 0.0) {
            let c = st.border_color[0].unwrap_or(st.color);
            if c.a <= 0.0 {
                return;
            }
            let Some(b) = self.solid(dc, c) else { return };
            if st.border_style[0] == BorderStyle::Dashed {
                let w = bw[0];
                let rr = r.map(|(a, bb)| ((a - w / 2.0).max(0.0), (bb - w / 2.0).max(0.0)));
                if let (Some(geom), Some(ss)) = (self.rrect(g.x + w / 2.0, g.y + w / 2.0, g.w - w, g.h - w, &rr), self.stroke_style(false, false, true)) {
                    unsafe { dc.DrawGeometry(&geom, &b, w, &ss) };
                }
            } else {
                let (ix, iy, iw, ih) = g.padding_box();
                let outer = self.rrect(g.x, g.y, g.w, g.h, &r);
                let inner = self.rrect(ix, iy, iw, ih, &inner_radii(r, bw));
                if let (Some(o), Some(i)) = (outer, inner) {
                    if let Ok(grp) = unsafe { self.f().CreateGeometryGroup(D2D1_FILL_MODE_ALTERNATE, &[Some(o), Some(i)]) } {
                        Self::fill(dc, &grp.cast().unwrap(), &b.cast().unwrap());
                    }
                }
            }
        }
    }

    /// Pie slice from 12 o'clock, clockwise, covering fraction `p`.
    fn wedge(&self, cx: f32, cy: f32, r: f32, p: f32) -> Option<ID2D1Geometry> {
        unsafe {
            if p >= 0.9999 {
                return self.f().CreateEllipseGeometry(&D2D1_ELLIPSE { point: v2(cx, cy), radiusX: r, radiusY: r }).ok()?.cast().ok();
            }
            let a = p * std::f32::consts::TAU;
            let g = self.f().CreatePathGeometry().ok()?;
            let sk = g.Open().ok()?;
            sk.BeginFigure(v2(cx, cy), D2D1_FIGURE_BEGIN_FILLED);
            sk.AddLine(v2(cx, cy - r));
            sk.AddArc(&D2D1_ARC_SEGMENT {
                point: v2(cx + r * a.sin(), cy - r * a.cos()),
                size: D2D_SIZE_F { width: r, height: r },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: if p > 0.5 { D2D1_ARC_SIZE_LARGE } else { D2D1_ARC_SIZE_SMALL },
            });
            sk.EndFigure(D2D1_FIGURE_END_CLOSED);
            sk.Close().ok()?;
            g.cast().ok()
        }
    }

    // ------------------------------------------------------------------ tree walk

    fn node(&mut self, dc: &ID2D1DeviceContext, s: &Scene, n: NodeId) {
        let Some(es) = s.styles.get(n as usize).and_then(|x| x.as_deref()) else { return };
        let st = &es.base;
        if st.display == Display::None || st.opacity <= 0.002 || !s.geom.world.contains_key(&(n, Part::Main)) {
            return;
        }
        if st.filter.is_empty() {
            self.node_body(dc, s, n, es);
        } else {
            self.filtered(dc, &st.filter, |p, d| p.node_body(d, s, n, es));
        }
    }


    fn node_body(&mut self, dc: &ID2D1DeviceContext, s: &Scene, n: NodeId, es: &ElemStyle) {
        let st = &es.base;
        let Some(m) = self.m(s, (n, Part::Main)) else { return };
        let Some(g) = s.layout.boxes.get(&(n, Part::Main)).copied() else { return };
        let tag = s.dom.tag(n);
        let layer = st.opacity < 1.0 || st.mask.is_some();
        if layer {
            unsafe { dc.SetTransform(&m.to_d2d()) };
            let brush = st.mask.as_ref().and_then(|gr| Self::gradient_brush(dc, gr, g.x, g.y, g.w, g.h));
            Self::push_layer(dc, None, st.opacity, brush);
        }
        let schedule = s.geom.schedule.get(&(n, Part::Main));
        if st.visible {
            self.decor(dc, m, st, &g);
            match tag {
                "img" => self.image(dc, s, n, m, st, &g),
                "svg" => self.svg(dc, s, n, m, &g),
                _ => {}
            }
        }
        let clip = st.clips() || matches!(tag, "input" | "textarea");
        let pushed = if clip { self.push_clip(dc, m, st, &g) } else { None };
        let active_clips = s.geom.clips.get(&(n, Part::Main)).map_or(0, |c| c.len()) + usize::from(clip);
        if let Some(schedule) = schedule {
            for &key in &schedule.children[..schedule.negative] {
                self.scheduled_box(dc, s, key, active_clips);
            }
        }
        let (sx, sy) = s.dom.scroll_of(n);
        let mc = m.mul(Affine::translate(-sx, -sy));
        if st.visible {
            let ph = es.placeholder.unwrap_or(Color { a: 0.5, ..Color::WHITE });
            self.leaf(dc, s, mc, (n, Part::Main), st, ph);
        }
        if let Some(schedule) = schedule {
            for &key in &schedule.children[schedule.negative..] {
                self.scheduled_box(dc, s, key, active_clips);
            }
        }
        match pushed {
            Some(Pushed::Layer) => unsafe { dc.PopLayer() },
            Some(Pushed::Axis) => unsafe { dc.PopAxisAlignedClip() },
            None => {}
        }
        if st.visible {
            self.scrollbars(dc, s, n, m, st, &g);
            if let Some((w, c)) = st.outline {
                {
                    let o = st.outline_offset + w / 2.0;
                    let r = radii(st, g.w, g.h).map(|(a, b)| if a > 0.0 { (a + o, b + o) } else { (0.0, 0.0) });
                    let geom = self.rrect(g.x - o, g.y - o, g.w + 2.0 * o, g.h + 2.0 * o, &r);
                    let brush = self.solid(dc, c);
                    if let (Some(geom), Some(b)) = (geom, brush) {
                        unsafe {
                            dc.SetTransform(&m.to_d2d());
                            dc.DrawGeometry(&geom, &b, w, None::<&ID2D1StrokeStyle>);
                        }
                    }
                }
            }
        }
        if layer {
            unsafe { dc.PopLayer() };
        }
    }

    /// Deferred roots retain DOM overflow clips; replay only ancestors no longer active.
    fn scheduled_box(&mut self, dc: &ID2D1DeviceContext, s: &Scene, key: (NodeId, Part), active_clips: usize) {
        if let Some(c) = s.geom.clips.get(&key).and_then(|c| c.get(active_clips)) {
            let m = self.base.unwrap_or(Affine::ID).mul(c.m);
            let pushed = self.push_clip_shape(dc, m, c.x, c.y, c.w, c.h, &c.radii);
            self.scheduled_box(dc, s, key, active_clips + 1);
            match pushed {
                Some(Pushed::Layer) => unsafe { dc.PopLayer() },
                Some(Pushed::Axis) => unsafe { dc.PopAxisAlignedClip() },
                None => {}
            }
        } else if key.1 == Part::Main {
            self.node(dc, s, key.0);
        } else if let Some(es) = s.styles.get(key.0 as usize).and_then(|x| x.as_deref()) {
            self.part(dc, s, key.0, key.1, es);
        }
    }

    fn part(&mut self, dc: &ID2D1DeviceContext, s: &Scene, n: NodeId, p: Part, es: &ElemStyle) {
        let Some(m) = self.m(s, (n, p)) else { return };
        let ps = match p {
            Part::Before => es.before.as_ref(),
            Part::After => es.after.as_ref(),
            _ => None,
        };
        let Some(ps) = ps else {
            // anonymous text run of `n`
            if es.base.visible {
                self.leaf(dc, s, m, (n, p), &es.base, Color::WHITE);
            }
            return;
        };
        if ps.display == Display::None || ps.opacity <= 0.002 {
            return;
        }
        let body = move |me: &mut Painter, d: &ID2D1DeviceContext| {
            let Some(g) = s.layout.boxes.get(&(n, p)).copied() else { return };
            let layer = ps.opacity < 1.0 || ps.mask.is_some();
            if layer {
                unsafe { d.SetTransform(&m.to_d2d()) };
                let brush = ps.mask.as_ref().and_then(|gr| Self::gradient_brush(d, gr, g.x, g.y, g.w, g.h));
                Self::push_layer(d, None, ps.opacity, brush);
            }
            if ps.visible {
                me.decor(d, m, ps, &g);
                me.leaf(d, s, m, (n, p), ps, Color::WHITE);
            }
            if layer {
                unsafe { d.PopLayer() };
            }
        };
        if ps.filter.is_empty() {
            body(self, dc);
        } else {
            self.filtered(dc, &ps.filter, body);
        }
    }

    fn push_clip(&self, dc: &ID2D1DeviceContext, m: Affine, st: &Style, g: &BoxGeom) -> Option<Pushed> {
        let (x, y, w, h) = g.padding_box();
        let r = inner_radii(radii(st, g.w, g.h), g.border);
        self.push_clip_shape(dc, m, x, y, w, h, &r)
    }

    fn push_clip_shape(&self, dc: &ID2D1DeviceContext, m: Affine, x: f32, y: f32, w: f32, h: f32, r: &[(f32, f32); 4]) -> Option<Pushed> {
        unsafe { dc.SetTransform(&m.to_d2d()) };
        if r.iter().all(|&(a, b)| a <= 0.0 || b <= 0.0) && m.b == 0.0 && m.c == 0.0 {
            unsafe { dc.PushAxisAlignedClip(&rect(x, y, w, h), D2D1_ANTIALIAS_MODE_PER_PRIMITIVE) };
            return Some(Pushed::Axis);
        }
        let geom = self.rrect(x, y, w, h, r)?;
        Self::push_layer(dc, Some(geom), 1.0, None);
        Some(Pushed::Layer)
    }

    fn scrollbars(&mut self, dc: &ID2D1DeviceContext, s: &Scene, n: NodeId, m: Affine, st: &Style, g: &BoxGeom) {
        let (sx, sy) = s.dom.scroll_of(n);
        let (px, py, pw, ph) = g.padding_box();
        let vertical = st.overflow_y == Overflow::Auto && g.scroll_h() > g.client_h() + 0.5;
        let horizontal = st.overflow_x == Overflow::Auto && g.scroll_w() > g.client_w() + 0.5;
        if !vertical && !horizontal { return; }
        let Some(b) = self.solid(dc, THUMB) else { return };
        unsafe { dc.SetTransform(&m.to_d2d()) };
        let draw = |x: f32, y: f32, w: f32, h: f32| {
            let rr = D2D1_ROUNDED_RECT { rect: rect(x, y, w, h), radiusX: 2.5, radiusY: 2.5 };
            unsafe { dc.FillRoundedRectangle(&rr, &b) };
        };
        if vertical {
            let total = g.scroll_h();
            let th = (ph * ph / total).max(18.0).min(ph);
            let ty = py + (ph - th) * (sy / (total - ph).max(1.0)).clamp(0.0, 1.0);
            draw(px + pw - 5.0, ty, 5.0, th);
        }
        if horizontal {
            let total = g.scroll_w();
            let tw = (pw * pw / total).max(18.0).min(pw);
            let tx = px + (pw - tw) * (sx / (total - pw).max(1.0)).clamp(0.0, 1.0);
            draw(tx, py + ph - 5.0, tw, 5.0);
        }
    }

    // ------------------------------------------------------------------ content

    fn leaf(&mut self, dc: &ID2D1DeviceContext, s: &Scene, m: Affine, key: (NodeId, Part), st: &Style, ph: Color) {
        let Some(leaf) = s.layout.leaves.get(&key) else { return };
        let Some(g) = s.layout.boxes.get(&key).copied() else { return };
        let (cx, cy, _, _) = g.content_box();
        let (x, y) = (cx + leaf.dx, cy + leaf.dy);
        let c = if leaf.placeholder { ph } else { st.color };
        unsafe { dc.SetTransform(&m.to_d2d()) };
        if let Some((sn, sp, a, len)) = s.overlay.sel {
            if (sn, sp) == key && len > 0 && !leaf.placeholder {
                if let Some(b) = self.solid(dc, SEL) {
                    for (rx, ry, rw, rh) in text::range_rects(&leaf.layout, a, len) {
                        unsafe { dc.FillRectangle(&rect(x + rx, y + ry, rw, rh), &b) };
                    }
                }
            }
        }
        if !leaf.text.is_empty() && c.a > 0.0 {
            for sh in st.text_shadow.iter().rev().filter(|s| s.color.a > 0.0) {
                let lay = leaf.layout.clone();
                let rec = self.record(|me, a| unsafe {
                    a.SetTransform(&m.to_d2d());
                    if let Some(b) = me.solid(a, Color::BLACK) {
                        a.DrawTextLayout(v2(x, y), &lay, &b, D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT);
                    }
                });
                if let Some(img) = rec.and_then(|cl| cl.cast::<ID2D1Image>().ok()).and_then(|i| Self::shadow(dc, &i, sh.blur / 2.0 * self.scale, sh.color)) {
                    Self::draw_image(dc, &img, (sh.x * self.scale, sh.y * self.scale));
                }
                unsafe { dc.SetTransform(&m.to_d2d()) };
            }
            if let Some(b) = self.solid(dc, c) {
                unsafe { dc.DrawTextLayout(v2(x, y), &leaf.layout, &b, D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT) };
            }
        }
        if let Some((cn, pos)) = s.overlay.caret {
            if cn == key.0 && key.1 == Part::Main {
                let (kx, ky, kh) = if leaf.placeholder || leaf.text.is_empty() {
                    let (_, _, h) = text::caret_rect(&leaf.layout, 0);
                    let ax = match leaf.style.align {
                        text::Align::Center => g.content_box().2 / 2.0,
                        text::Align::Right => g.content_box().2,
                        text::Align::Left => 0.0,
                    };
                    (ax, 0.0, h)
                } else {
                    text::caret_rect(&leaf.layout, pos)
                };
                if let Some(b) = self.solid(dc, st.color) {
                    unsafe { dc.FillRectangle(&rect(x + kx, y + ky, 1.0, kh), &b) };
                }
            }
        }
    }

    fn bitmap(&mut self, dc: &ID2D1DeviceContext, src: &str) -> Option<ID2D1Bitmap1> {
        if !self.bitmaps.contains_key(src) {
            let b = self.images.bytes(src).and_then(|bytes| crate::gfx::image::decode(dc, &bytes));
            if self.bitmaps.len() > 16 {
                self.bitmaps.clear();
            }
            self.bitmaps.insert(src.to_string(), b);
        }
        self.bitmaps.get(src).cloned().flatten()
    }

    fn image(&mut self, dc: &ID2D1DeviceContext, s: &Scene, n: NodeId, m: Affine, st: &Style, g: &BoxGeom) {
        let Some(src) = s.dom.attr(n, "src").filter(|x| !x.is_empty()) else { return };
        let Some(bmp) = self.bitmap(dc, src) else { return };
        let (x, y, w, h) = g.content_box();
        let sz = unsafe { bmp.GetSize() };
        if sz.width <= 0.0 || sz.height <= 0.0 || w <= 0.0 || h <= 0.0 {
            return;
        }
        let srcr = if st.cover {
            let k = (w / sz.width).max(h / sz.height);
            let (sw, sh) = (w / k, h / k);
            rect((sz.width - sw) / 2.0, (sz.height - sh) / 2.0, sw, sh)
        } else {
            rect(0.0, 0.0, sz.width, sz.height)
        };
        let r = radii(st, g.w, g.h);
        unsafe { dc.SetTransform(&m.to_d2d()) };
        let rounded = r.iter().any(|&(a, b)| a > 0.0 && b > 0.0);
        if rounded {
            if let Some(geom) = self.rrect(g.x, g.y, g.w, g.h, &r) {
                Self::push_layer(dc, Some(geom), 1.0, None);
            }
        }
        unsafe { dc.DrawBitmap(&bmp, Some(&rect(x, y, w, h)), 1.0, D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC, Some(&srcr), None) };
        if rounded {
            unsafe { dc.PopLayer() };
        }
    }

    fn path_geom(&mut self, key: String, segs: impl FnOnce() -> Vec<crate::gfx::svg::Seg>) -> Option<ID2D1Geometry> {
        if !self.paths.contains_key(&key) {
            let g = crate::gfx::svg::build_geometry(self.f(), &segs(), true).ok().and_then(|g| g.cast().ok());
            if self.paths.len() > 512 {
                self.paths.clear();
            }
            self.paths.insert(key.clone(), g);
        }
        self.paths.get(&key).cloned().flatten()
    }

    fn svg(&mut self, dc: &ID2D1DeviceContext, s: &Scene, n: NodeId, m: Affine, g: &BoxGeom) {
        let (x, y, w, h) = g.content_box();
        let vb: Vec<f32> = s
            .dom
            .attr(n, "viewBox")
            .map(|v| v.split([' ', ',']).filter_map(|t| t.parse().ok()).collect())
            .unwrap_or_default();
        let (vx, vy, vw, vh) = if vb.len() == 4 { (vb[0], vb[1], vb[2], vb[3]) } else { (0.0, 0.0, w, h) };
        if vw <= 0.0 || vh <= 0.0 {
            return;
        }
        let k = (w / vw).min(h / vh);
        let ox = x + (w - vw * k) / 2.0;
        let oy = y + (h - vh * k) / 2.0;
        let mm = m.mul(Affine::translate(ox, oy)).mul(Affine::scale(k, k)).mul(Affine::translate(-vx, -vy));
        unsafe { dc.SetTransform(&mm.to_d2d()) };
        for &c in s.dom.children(n) {
            let Some(cs) = s.styles.get(c as usize).and_then(|x| x.as_deref()) else { continue };
            let cs = &cs.base;
            if cs.display == Display::None {
                continue;
            }
            let num = |a: &str| s.dom.attr(c, a).and_then(|v| v.trim().parse::<f32>().ok()).unwrap_or(0.0);
            let geom = match s.dom.tag(c) {
                "path" => {
                    let d = s.dom.attr(c, "d").unwrap_or("").to_string();
                    self.path_geom(format!("p{d}"), || crate::gfx::svg::parse_svg_path(&d))
                }
                "circle" => {
                    let (cx, cy, r) = (num("cx"), num("cy"), num("r"));
                    self.path_geom(format!("c{cx},{cy},{r}"), || crate::gfx::svg::circle(cx, cy, r))
                }
                "rect" => {
                    let (rx0, ry0, rw, rh, rr) = (num("x"), num("y"), num("width"), num("height"), num("rx"));
                    self.path_geom(format!("r{rx0},{ry0},{rw},{rh},{rr}"), || crate::gfx::svg::rounded_rect(rx0, ry0, rw, rh, rr))
                }
                _ => None,
            };
            let Some(geom) = geom else { continue };
            let paint = |attr: &str, css: Paint| -> Option<Color> {
                let p = match s.dom.attr(c, attr) {
                    Some("none") => None,
                    Some(v) if v.eq_ignore_ascii_case("currentcolor") => Some(None),
                    Some(v) => crate::css::value::color(v, cs.color).map(Some),
                    None => css,
                };
                p.map(|x| x.unwrap_or(cs.color))
            };
            if let Some(fc) = paint("fill", cs.fill) {
                if let Some(b) = self.solid(dc, fc) {
                    Self::fill(dc, &geom, &b.cast().unwrap());
                }
            }
            if let Some(sc) = paint("stroke", cs.stroke) {
                let sw = s.dom.attr(c, "stroke-width").and_then(|v| v.parse().ok()).unwrap_or(cs.stroke_width);
                let cap = s.dom.attr(c, "stroke-linecap").map(|v| v == "round").unwrap_or(cs.round_cap);
                let join = s.dom.attr(c, "stroke-linejoin").map(|v| v == "round").unwrap_or(cs.round_join);
                if let (Some(b), Some(ss)) = (self.solid(dc, sc), self.stroke_style(cap, join, false)) {
                    unsafe { dc.DrawGeometry(&geom, &b, sw, &ss) };
                }
            }
        }
    }
}

/// Renders the 640x460 CSS-px frame at `scale` into a PNG (snapshot/parity mode).
pub fn render_to_png(gfx: &mut Gfx, painter: &mut Painter, s: &Scene, scale: f32, view: (f32, f32), path: &std::path::Path) -> windows::core::Result<()> {
    gfx.resize((view.0 * scale).round() as u32, (view.1 * scale).round() as u32)?;
    gfx.begin(scale, None);
    painter.paint(gfx, s, scale);
    gfx.end()?;
    let px = gfx.read_rgba()?;
    crate::gfx::image::write_png(path, gfx.w, gfx.h, &px)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css::cascade::Cascade;

    #[test]
    fn filtered_pseudo_preserves_style_and_pixels() {
        unsafe {
            windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED).ok().unwrap();
        }
        let mut d = Dom::new();
        d.set_inner_html(d.body, "<div id='panel'></div><div id='sibling'></div>");
        let styles = Cascade::new("*{margin:0;padding:0;border:0}html,body{width:100px;height:100px}#panel{position:absolute;left:10px;top:10px;width:40px;height:40px;background:white;overflow:hidden;opacity:.8;filter:drop-shadow(2px 2px 1px black)}#panel::before{content:'';position:absolute;left:25px;top:0;width:30px;height:30px;background:#ff0000;filter:blur(1px)}#sibling{position:absolute;left:70px;top:10px;width:20px;height:30px;background:#0000ff}").compute(&d);
        let mut painter = Painter::new();
        let layout = crate::layout::compute(&d, &styles, painter.text(), (100.0, 100.0));
        let geom = geom::compute(&d, &styles, &layout);
        let overlay = Overlay::default();
        let scene = Scene { dom: &d, styles: &styles, layout: &layout, geom: &geom, overlay: &overlay };
        let mut gfx = Gfx::new(100, 100).unwrap();
        let render = |painter: &mut Painter| {
            gfx.begin(1.0, None);
            painter.paint(&gfx, &scene, 1.0);
            gfx.end().unwrap();
            gfx.read_rgba().unwrap()
        };
        let first = render(&mut painter);
        let at = |x: usize, y: usize| &first[(y * 100 + x) * 4..(y * 100 + x) * 4 + 4];
        let red = at(42, 20);
        assert!(red[0] > 180 && red[1] < 30 && red[2] < 30, "pseudo: {red:?}");
        assert!(at(62, 20)[0] < 10, "overflow clip");
        assert_eq!(at(80, 20), &[0, 0, 255, 255]);
        assert_eq!(first, render(&mut painter));
        if let Some(path) = std::env::var_os("BLIP_FILTER_FIXTURE") {
            render_to_png(&mut gfx, &mut painter, &scene, 1.0, (100.0, 100.0), std::path::Path::new(&path)).unwrap();
        }
    }

    #[test]
    fn stacking_descendants_cross_only_noncontexts_and_keep_clips() {
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED);
        }
        for isolation in ["", "opacity:.99", "transform:translateX(0px)", "translate:1px 0", "filter:blur(0px)", "mask-image:linear-gradient(black,black)", "position:relative;z-index:0"] {
            let mut d = Dom::new();
            d.set_inner_html(d.body, "<div id='wrap'><div id='high'></div><div id='low'></div></div><div id='middle'></div>");
            let css = format!("html,body {{ margin:0; width:100px; height:100px }} #wrap {{ width:30px;height:30px;overflow:hidden;{isolation} }} #high,#low,#middle {{ position:absolute;left:0;top:0;width:60px;height:60px }} #high {{ background:#ff0000;z-index:9 }} #low {{ background:#0000ff;z-index:-9 }} #middle {{ background:#008000;z-index:1 }}");
            let styles = Cascade::new(&css).compute(&d);
            let mut painter = Painter::new();
            let layout = crate::layout::compute(&d, &styles, painter.text(), (crate::layout::VIEW_W, crate::layout::VIEW_H));
            let geom = geom::compute(&d, &styles, &layout);
            let high = (d.by_id("high").unwrap(), Part::Main);
            let low = (d.by_id("low").unwrap(), Part::Main);
            let middle = (d.by_id("middle").unwrap(), Part::Main);
            assert_eq!(styles[middle.0 as usize].as_ref().unwrap().base.background, Bg::Color(Color::rgba8(0, 128, 0, 1.0)));
            let pos = |key| geom.order.iter().position(|&k| k == key).unwrap();
            assert!(pos(low) < pos(middle));
            assert_eq!(pos(high) > pos(middle), isolation.is_empty(), "{isolation}");
            assert!(geom.clips[&high].iter().all(|c| c.contains(10.0, 10.0)));
            assert!(geom.clips[&high].iter().any(|c| !c.contains(40.0, 10.0)));
            let gfx = Gfx::new(100, 100).unwrap();
            let overlay = Overlay::default();
            let scene = Scene { dom: &d, styles: &styles, layout: &layout, geom: &geom, overlay: &overlay };
            gfx.begin(1.0, None);
            painter.paint(&gfx, &scene, 1.0);
            gfx.end().unwrap();
            let px = gfx.read_rgba().unwrap();
            let at = |x: usize| &px[(10 * 100 + x) * 4..(10 * 100 + x) * 4 + 4];
            if isolation.is_empty() {
                assert_eq!(at(10), &[255, 0, 0, 255]);
            } else {
                assert_eq!(at(10), &[0, 128, 0, 255], "{isolation}");
            }
            assert_eq!(at(40), &[0, 128, 0, 255], "deferred overflow clip: {isolation}");
        }
    }

    #[test]
    fn stacking_sorts_numeric_z_and_paints_context_background_first() {
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED);
        }
        for (z, expected) in [("-1", [0, 128, 0, 255]), ("-3", [255, 0, 0, 255]), ("-2", [0, 128, 0, 255])] {
            let mut d = Dom::new();
            d.set_inner_html(d.body, "<div id='wrap'><div id='red'></div></div><div id='green'></div>");
            let css = format!("html {{ background:black }} html,body {{ margin:0;width:100px;height:100px }} #wrap {{ width:50px;height:50px }} #red,#green {{ position:absolute;left:0;top:0;width:50px;height:50px }} #red {{ background:#ff0000;z-index:-2 }} #green {{ background:#008000;z-index:{z} }}");
            let styles = Cascade::new(&css).compute(&d);
            let mut painter = Painter::new();
            let layout = crate::layout::compute(&d, &styles, painter.text(), (crate::layout::VIEW_W, crate::layout::VIEW_H));
            let geom = geom::compute(&d, &styles, &layout);
            let red = (d.by_id("red").unwrap(), Part::Main);
            let green = (d.by_id("green").unwrap(), Part::Main);
            assert_eq!(styles[green.0 as usize].as_ref().unwrap().base.background, Bg::Color(Color::rgba8(0, 128, 0, 1.0)));
            let top = if z == "-3" { red } else { green };
            assert_eq!(geom.order.iter().rev().find(|&&k| k == red || k == green), Some(&top));
            let gfx = Gfx::new(100, 100).unwrap();
            let overlay = Overlay::default();
            let scene = Scene { dom: &d, styles: &styles, layout: &layout, geom: &geom, overlay: &overlay };
            gfx.begin(1.0, None);
            painter.paint(&gfx, &scene, 1.0);
            gfx.end().unwrap();
            let px = gfx.read_rgba().unwrap();
            assert_eq!(&px[(10 * 100 + 10) * 4..(10 * 100 + 10) * 4 + 4], &expected, "z-index:{z}");
        }
    }

    /// Idle notch: opaque black pill centred at the top, anti-aliased bottom corners and ears.
    #[test]
    fn idle_pixels() {
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED);
        }
        let d = Dom::from_index();
        let st = Cascade::new(include_str!("../../ui/style.css")).compute(&d);
        let mut painter = Painter::new();
        let l = crate::layout::compute(&d, &st, painter.text(), (crate::layout::VIEW_W, crate::layout::VIEW_H));
        let g = geom::compute(&d, &st, &l);
        let mut gfx = Gfx::new(640, 460).unwrap();
        let ov = Overlay::default();
        let scene = Scene { dom: &d, styles: &st, layout: &l, geom: &g, overlay: &ov };
        let p = std::env::temp_dir().join("blip_idle_test.png");
        render_to_png(&mut gfx, &mut painter, &scene, 1.0, (crate::layout::VIEW_W, crate::layout::VIEW_H), &p).unwrap();
        let px = gfx.read_rgba().unwrap();
        let at = |x: usize, y: usize| &px[(y * 640 + x) * 4..(y * 640 + x) * 4 + 4];
        assert_eq!(at(320, 2)[3], 255, "inside pill");
        assert_eq!(at(10, 10)[3], 0, "outside");
        assert_eq!(at(232, 0)[3], 255, "ear top row is black");
        // bottom-left corner of the 16px radius has partial coverage somewhere along the arc
        let partial = (0..16).any(|i| {
            let a = at(236 + i, 35 - i / 3)[3];
            a > 0 && a < 255
        });
        assert!(partial, "anti-aliased corner");
        let _ = std::fs::remove_file(p);
    }

    /// At a DPI scale != 1 the masked `.ambient` layer must stay faint (it was painted as an
    /// opaque purple block when the layer bounds were D2D's "infinite" rect).
    #[test]
    fn masked_layer_at_125_percent() {
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED);
        }
        let mut d = Dom::from_index();
        d.set_attr(d.body, "data-state", "open");
        for (k, v) in [("--w", "460px"), ("--h", "150px"), ("--ow", "460px"), ("--oh", "150px")] {
            d.style_set(d.body, k, v);
        }
        let st = Cascade::new(include_str!("../../ui/style.css")).compute(&d);
        let mut painter = Painter::new();
        let l = crate::layout::compute(&d, &st, painter.text(), (crate::layout::VIEW_W, crate::layout::VIEW_H));
        let g = geom::compute(&d, &st, &l);
        let mut gfx = Gfx::new(800, 575).unwrap();
        let ov = Overlay::default();
        let scene = Scene { dom: &d, styles: &st, layout: &l, geom: &g, overlay: &ov };
        let p = std::env::temp_dir().join("blip_scale_test.png");
        render_to_png(&mut gfx, &mut painter, &scene, 1.25, (crate::layout::VIEW_W, crate::layout::VIEW_H), &p).unwrap();
        let px = gfx.read_rgba().unwrap();
        // CSS (540, 100): right part of the open island, ambient opacity .18 under a fading mask.
        let i = ((100.0 * 1.25) as usize * 800 + (540.0 * 1.25) as usize) * 4;
        let (r, gg, b) = (px[i], px[i + 1], px[i + 2]);
        assert!(r < 60 && gg < 60 && b < 90, "ambient too strong: {r},{gg},{b}");
        let _ = std::fs::remove_file(p);
    }
}
