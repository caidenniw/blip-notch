//! Pointer, wheel, keyboard and text editing for the native DOM (Chromium-like defaults).
use crate::css::cascade::{Display, Overflow};
use crate::dom::{self, NodeId};
use crate::engine::Engine;
use crate::gfx::text;
use crate::layout::Part;
use serde_json::json;

#[derive(Default)]
pub struct Input {
    pub hover: Vec<NodeId>,
    down: Option<(NodeId, f32, f32)>,
    last_click: Option<(f64, f32, f32, NodeId)>,
    clicks: u32,
    active: Vec<NodeId>,
    /// Editing state of the focused control: (anchor, focus) UTF-16, value at focus time.
    sel: (u32, u32),
    focus_value: String,
    /// Read-only text selection: (element, part, anchor, focus).
    ro_sel: Option<(NodeId, Part, u32, u32)>,
    dragging_sel: bool,
    /// Scrollbar thumb drag: (element, start y, start scrollTop).
    thumb: Option<(NodeId, f32, f32)>,
    pub caret_on: bool,
    pub caret_t0: f64,
    pub pos: (f32, f32),
    undo: Vec<(String, (u32, u32))>,
    redo: Vec<(String, (u32, u32))>,
    last_edit: f64,
}

#[derive(Clone, Copy, Default)]
pub struct Mods {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

fn is_editable(e: &Engine, n: NodeId) -> bool {
    matches!(e.page.borrow().dom.tag(n), "input" | "textarea")
}

fn focusable(e: &Engine, n: NodeId) -> bool {
    let p = e.page.borrow();
    matches!(p.dom.tag(n), "input" | "textarea" | "button") || p.dom.has_attr(n, "tabindex")
}

/// Topmost box under `(x, y)` in CSS px. Pseudo-element boxes target their owner.
pub fn hit_test(e: &Engine, x: f32, y: f32) -> Option<NodeId> {
    let p = e.page.borrow();
    for key in p.geom.order.iter().rev() {
        let (n, part) = *key;
        let Some(es) = p.styles.get(n as usize).and_then(|s| s.as_deref()) else { continue };
        let st = match part {
            Part::Before => es.before.as_ref(),
            Part::After => es.after.as_ref(),
            _ => Some(&es.base),
        };
        let Some(st) = st else { continue };
        if !st.pointer_events || !st.visible || st.display == Display::None || es.base.display == Display::None {
            continue;
        }
        // Ancestors with `display:none`/`opacity` excluded by geom already; effective visibility via styles.
        let (Some(g), Some(m)) = (p.layout.boxes.get(key), p.geom.world.get(key)) else { continue };
        let Some(inv) = m.invert() else { continue };
        let (lx, ly) = inv.apply(x, y);
        if lx < g.x || ly < g.y || lx > g.x + g.w || ly > g.y + g.h {
            continue;
        }
        let r = crate::paint::geom::radii(st, g.w, g.h);
        if !crate::paint::geom::rrect_contains(g.x, g.y, g.w, g.h, &r, lx, ly) {
            continue;
        }
        if let Some(clips) = p.geom.clips.get(key) {
            if !clips.iter().all(|c| c.contains(x, y)) {
                continue;
            }
        }
        return Some(n);
    }
    None
}

fn chain(e: &Engine, n: Option<NodeId>) -> Vec<NodeId> {
    let p = e.page.borrow();
    let mut v = Vec::new();
    let mut cur = n;
    while let Some(x) = cur {
        v.push(x);
        cur = p.dom.parent(x);
    }
    v
}

fn mouse(x: f32, y: f32, buttons: u32, m: Mods) -> serde_json::Value {
    json!({ "bubbles": true, "cancelable": true, "clientX": x, "clientY": y, "buttons": buttons,
            "ctrlKey": m.ctrl, "shiftKey": m.shift, "altKey": m.alt, "pointerId": 1, "isPrimary": true })
}
pub fn utf16_unit(pending: &mut Option<u16>, unit: u16) -> Option<char> {
    if (0xD800..=0xDBFF).contains(&unit) {
        *pending = Some(unit);
        return None;
    }
    if (0xDC00..=0xDFFF).contains(&unit) {
        let high = pending.take()?;
        return char::decode_utf16([high, unit]).next()?.ok();
    }
    *pending = None;
    char::from_u32(unit as u32)
}


impl Input {
    fn set_flag(e: &Engine, old: &[NodeId], new: &[NodeId], flag: u8) {
        let mut p = e.page.borrow_mut();
        for &n in old {
            if !new.contains(&n) {
                p.dom.set_state(n, flag, false);
            }
        }
        for &n in new {
            p.dom.set_state(n, flag, true);
        }
    }

    pub fn leave(&mut self, e: &Engine) {
        let old = std::mem::take(&mut self.hover);
        Self::set_flag(e, &old, &[], dom::HOVER);
    }

    pub fn move_to(&mut self, e: &Engine, x: f32, y: f32, buttons: u32, m: Mods) {
        self.pos = (x, y);
        if let Some((n, y0, top0)) = self.thumb {
            let g = e.page.borrow().layout.boxes.get(&(n, Part::Main)).copied();
            if let Some(g) = g {
                let ratio = g.scroll_h() / g.client_h().max(1.0);
                e.page.borrow_mut().set_scroll_top(n, top0 + (y - y0) * ratio);
            }
            return;
        }
        let target = hit_test(e, x, y);
        let new = chain(e, target);
        let old = std::mem::replace(&mut self.hover, new.clone());
        if old != new {
            Self::set_flag(e, &old, &new, dom::HOVER);
        }
        if self.dragging_sel {
            self.drag_select(e, x, y);
        }
        let cap = e.page.borrow().capture;
        if let Some(t) = cap.or(target) {
            e.js.fire(t, "pointermove", &mouse(x, y, buttons, m));
            e.run_calls();
        }
    }

    /// Returns the cursor kind: 0 arrow, 1 hand, 2 I-beam.
    pub fn cursor(&self, e: &Engine) -> u8 {
        let Some(&n) = self.hover.first() else { return 0 };
        let p = e.page.borrow();
        if matches!(p.dom.tag(n), "input" | "textarea") {
            return 2;
        }
        match p.styles.get(n as usize).and_then(|s| s.as_ref()) {
            Some(s) if s.base.cursor_pointer => 1,
            _ => 0,
        }
    }

    pub fn down(&mut self, e: &Engine, x: f32, y: f32, m: Mods) {
        self.pos = (x, y);
        // Scrollbar thumb takes the press without JS events.
        if let Some(n) = self.thumb_at(e, x, y) {
            let top = e.page.borrow().dom.scroll_of(n).1;
            self.thumb = Some((n, y, top));
            return;
        }
        let target = hit_test(e, x, y);
        self.down = target.map(|t| (t, x, y));
        let ok = match target {
            Some(t) => e.js.fire(t, "pointerdown", &mouse(x, y, 1, m)),
            None => e.js.fire_doc("pointerdown", &mouse(x, y, 1, m)),
        };
        e.run_calls();
        let ch = chain(e, target);
        Self::set_flag(e, &self.active.clone(), &ch, dom::ACTIVE);
        self.active = ch.clone();
        if ok {
            let f = ch.iter().copied().find(|&n| focusable(e, n));
            self.set_focus(e, f, false);
            self.ro_sel = None;
            if let Some(f) = f.filter(|&f| is_editable(e, f)) {
                let pos = self.text_pos(e, f, Part::Main, x, y).unwrap_or(0);
                self.sel = if m.shift && e.page.borrow().focus == Some(f) { (self.sel.0, pos) } else { (pos, pos) };
                self.dragging_sel = true;
            } else if let Some(t) = target {
                let sel_ok = e.page.borrow().styles.get(t as usize).and_then(|s| s.as_ref()).is_some_and(|s| s.base.select_text);
                if sel_ok && e.page.borrow().layout.leaves.contains_key(&(t, Part::Main)) {
                    let pos = self.text_pos(e, t, Part::Main, x, y).unwrap_or(0);
                    self.ro_sel = Some((t, Part::Main, pos, pos));
                    self.dragging_sel = true;
                }
            }
            self.sync_overlay(e);
        }
    }

    pub fn up(&mut self, e: &Engine, x: f32, y: f32, m: Mods, dbl_ms: f64) {
        self.dragging_sel = false;
        if self.thumb.take().is_some() {
            return;
        }
        let target = hit_test(e, x, y);
        let cap = e.page.borrow_mut().capture.take();
        if let Some(t) = cap.or(target) {
            e.js.fire(t, "pointerup", &mouse(x, y, 0, m));
            e.run_calls();
        }
        let active = std::mem::take(&mut self.active);
        Self::set_flag(e, &active, &[], dom::ACTIVE);
        let Some((d, _, _)) = self.down.take() else { return };
        let Some(t) = target else { return };
        // click target: nearest common ancestor of down and up targets
        let up_chain = chain(e, Some(t));
        let Some(&common) = chain(e, Some(d)).iter().find(|n| up_chain.contains(n)) else { return };
        e.js.fire(common, "click", &json!({ "bubbles": true, "cancelable": true, "clientX": x, "clientY": y, "ctrlKey": m.ctrl, "shiftKey": m.shift, "detail": 1 }));
        e.run_calls();
        let now = e.now();
        let near = matches!(self.last_click, Some((t0, x0, y0, n0)) if now - t0 <= dbl_ms && (x - x0).abs() <= 4.0 && (y - y0).abs() <= 4.0 && n0 == common);
        self.clicks = if near { self.clicks + 1 } else { 1 };
        self.last_click = Some((now, x, y, common));
        if self.clicks == 2 {
            e.js.fire(common, "dblclick", &json!({ "bubbles": true, "cancelable": true, "clientX": x, "clientY": y, "detail": 2 }));
            e.run_calls();
            self.select_word(e, x, y);
        } else if self.clicks == 3 {
            self.select_all_text(e);
        }
    }

    pub fn cancel(&mut self, e: &Engine) {
        let target = e.page.borrow().capture.or(self.down.map(|d| d.0));
        if let Some(t) = target {
            e.js.fire(t, "pointercancel", &json!({ "bubbles": true, "pointerId": 1 }));
            e.run_calls();
        }
        self.down = None;
        self.thumb = None;
        self.dragging_sel = false;
        let active = std::mem::take(&mut self.active);
        Self::set_flag(e, &active, &[], dom::ACTIVE);
        e.page.borrow_mut().capture = None;
    }

    fn select_all_text(&mut self, e: &Engine) {
        if let Some(f) = e.page.borrow().focus.filter(|&f| is_editable(e, f)) {
            self.sel = (0, text::utf16_len(e.page.borrow().dom.value(f)));
        } else if let Some((n, part, _, _)) = self.ro_sel {
            let len = e.page.borrow().layout.leaves.get(&(n, part)).map_or(0, |l| text::utf16_len(&l.text));
            self.ro_sel = Some((n, part, 0, len));
        }
        self.sync_overlay(e);
    }

    pub fn wheel(&mut self, e: &Engine, x: f32, y: f32, dx: f32, dy: f32) {
        let Some(t) = hit_test(e, x, y) else { return };
        let ok = e.js.fire(t, "wheel", &json!({ "bubbles": true, "cancelable": true, "clientX": x, "clientY": y, "deltaY": dy, "deltaX": dx }));
        e.run_calls();
        if !ok {
            return;
        }
        // Default: scroll nearest scrollable ancestor.
        let mut p = e.page.borrow_mut();
        for n in chain_ref(&p.dom, t) {
            let Some(es) = p.styles.get(n as usize).and_then(|s| s.clone()) else { continue };
            let Some(g) = p.layout.boxes.get(&(n, Part::Main)).copied() else { continue };
            if dy != 0.0 && es.base.overflow_y == Overflow::Auto && g.scroll_h() > g.client_h() + 0.5 {
                let from = p.dom.scroll_of(n).1;
                let max = (g.scroll_h() - g.client_h()).max(0.0);
                let to = (p.smooth.get(&n).map(|s| s.1).unwrap_or(from) + dy).clamp(0.0, max);
                let now = p.now;
                if p.reduced_motion {
                    p.smooth.remove(&n);
                    p.set_scroll_top(n, to);
                } else {
                    p.smooth.insert(n, (from, to, now, 150.0));
                }
                p.dirty = true;
                return;
            }
            if dy == 0.0 && dx != 0.0 && es.base.overflow_x == Overflow::Auto && g.scroll_w() > g.client_w() + 0.5 {
                let e2 = p.dom.scroll.entry(n).or_insert((0.0, 0.0));
                e2.0 = (e2.0 + dx).clamp(0.0, (g.scroll_w() - g.client_w()).max(0.0));
                p.dirty = true;
                return;
            }
        }
    }

    fn thumb_at(&self, e: &Engine, x: f32, y: f32) -> Option<NodeId> {
        let p = e.page.borrow();
        for key in p.geom.order.iter().rev() {
            if key.1 != Part::Main {
                continue;
            }
            let Some(es) = p.styles.get(key.0 as usize).and_then(|s| s.as_ref()) else { continue };
            if es.base.overflow_y != Overflow::Auto || !es.base.visible {
                continue;
            }
            let (Some(g), Some(m)) = (p.layout.boxes.get(key), p.geom.world.get(key)) else { continue };
            if g.scroll_h() <= g.client_h() + 0.5 {
                continue;
            }
            let Some(inv) = m.invert() else { continue };
            let (lx, ly) = inv.apply(x, y);
            let (px, py, pw, ph) = g.padding_box();
            if lx >= px + pw - 6.0 && lx <= px + pw && ly >= py && ly <= py + ph {
                return Some(key.0);
            }
        }
        None
    }

    // ------------------------------------------------------------------ focus / text

    /// Moves focus; fires blur/change on the old element. `visible` = keyboard focus.
    pub fn set_focus(&mut self, e: &Engine, n: Option<NodeId>, visible: bool) {
        let old = e.page.borrow().focus;
        if old == n {
            return;
        }
        self.undo.clear();
        self.redo.clear();
        if let Some(o) = old {
            let changed = is_editable(e, o) && e.page.borrow().dom.value(o) != self.focus_value;
            {
                let mut p = e.page.borrow_mut();
                p.focus = None;
                let anc = chain_ref(&p.dom, o);
                for a in anc {
                    p.dom.set_state(a, dom::FOCUS | dom::FOCUS_VISIBLE | dom::FOCUS_WITHIN, false);
                }
            }
            if changed {
                e.js.fire(o, "change", &json!({ "bubbles": true }));
            }
            e.js.fire(o, "blur", &json!({}));
            e.run_calls();
        }
        if let Some(n) = n.filter(|&n| e.page.borrow().dom.connected(n)) {
            let mut p = e.page.borrow_mut();
            p.focus = Some(n);
            p.dom.set_state(n, dom::FOCUS, true);
            let fv = visible || matches!(p.dom.tag(n), "input" | "textarea");
            p.dom.set_state(n, dom::FOCUS_VISIBLE, fv);
            for a in chain_ref(&p.dom, n).into_iter().skip(1) {
                p.dom.set_state(a, dom::FOCUS_WITHIN, true);
            }
            self.focus_value = p.dom.value(n).to_string();
            let len = text::utf16_len(p.dom.value(n));
            self.sel = (len, len);
            drop(p);
            e.js.fire(n, "focus", &json!({}));
            e.run_calls();
        }
        self.caret_t0 = e.now();
        self.sync_overlay(e);
    }

    /// Applies `el.focus()` / `el.blur()` requests made by JS.
    pub fn apply_focus_requests(&mut self, e: &Engine) {
        let reqs = std::mem::take(&mut e.page.borrow_mut().focus_req);
        for r in reqs {
            self.set_focus(e, r, false);
        }
    }

    fn text_pos(&self, e: &Engine, n: NodeId, part: Part, x: f32, y: f32) -> Option<u32> {
        let p = e.page.borrow();
        let leaf = p.layout.leaves.get(&(n, part))?;
        let g = p.layout.boxes.get(&(n, part))?;
        let m = p.geom.world.get(&(n, part))?.invert()?;
        let (lx, ly) = m.apply(x, y);
        let (sx, sy) = p.dom.scroll_of(n);
        let (cx, cy, _, _) = g.content_box();
        if leaf.placeholder {
            return Some(0);
        }
        Some(text::hit_test_caret(&leaf.layout, lx - cx - leaf.dx + sx, ly - cy - leaf.dy + sy))
    }

    pub fn caret_screen(&self, e: &Engine) -> Option<(f32, f32, f32)> {
        let p = e.page.borrow();
        let f = p.focus.filter(|&f| matches!(p.dom.tag(f), "input" | "textarea"))?;
        let leaf = p.layout.leaves.get(&(f, Part::Main))?;
        let g = p.layout.boxes.get(&(f, Part::Main))?;
        let m = p.geom.world.get(&(f, Part::Main))?;
        let (x, y, h) = text::caret_rect(&leaf.layout, self.sel.1);
        let (cx, cy, _, _) = g.content_box();
        let (sx, sy) = p.dom.scroll_of(f);
        let (wx, wy) = m.apply(cx + leaf.dx + x - sx, cy + leaf.dy + y - sy);
        Some((wx, wy, h))
    }

    fn drag_select(&mut self, e: &Engine, x: f32, y: f32) {
        let f = e.page.borrow().focus;
        if let Some(f) = f.filter(|&f| is_editable(e, f)) {
            if let Some(pos) = self.text_pos(e, f, Part::Main, x, y) {
                self.sel.1 = pos;
            }
        } else if let Some((n, part, a, _)) = self.ro_sel {
            if let Some(pos) = self.text_pos(e, n, part, x, y) {
                self.ro_sel = Some((n, part, a, pos));
            }
        }
        self.sync_overlay(e);
    }

    fn select_word(&mut self, e: &Engine, x: f32, y: f32) {
        let (n, part, editable) = match e.page.borrow().focus {
            Some(f) if matches!(e.page.borrow().dom.tag(f), "input" | "textarea") => (f, Part::Main, true),
            _ => match self.ro_sel {
                Some((n, p, _, _)) => (n, p, false),
                None => return,
            },
        };
        let Some(pos) = self.text_pos(e, n, part, x, y) else { return };
        let s: Vec<u16> = {
            let p = e.page.borrow();
            match p.layout.leaves.get(&(n, part)) {
                Some(l) if !l.placeholder => l.text.encode_utf16().collect(),
                _ => return,
            }
        };
        let word = |c: u16| char::from_u32(c as u32).is_some_and(|ch| ch.is_alphanumeric() || ch == '_');
        let mut a = pos as usize;
        let mut b = pos as usize;
        while a > 0 && word(s[a - 1]) {
            a -= 1;
        }
        while b < s.len() && word(s[b]) {
            b += 1;
        }
        if editable {
            self.sel = (a as u32, b as u32);
        } else {
            self.ro_sel = Some((n, part, a as u32, b as u32));
        }
        self.sync_overlay(e);
    }

    pub fn sync_overlay(&self, e: &Engine) -> crate::paint::Overlay {
        let p = e.page.borrow();
        let mut o = crate::paint::Overlay::default();
        if let Some(f) = p.focus.filter(|&f| matches!(p.dom.tag(f), "input" | "textarea")) {
            let (a, b) = self.sel;
            if a != b {
                o.sel = Some((f, Part::Main, a.min(b), a.max(b) - a.min(b)));
            } else if self.caret_on {
                o.caret = Some((f, b));
            }
        } else if let Some((n, part, a, b)) = self.ro_sel {
            if a != b {
                o.sel = Some((n, part, a.min(b), a.max(b) - a.min(b)));
            }
        }
        o
    }

    /// Caret blink phase; returns true when it flipped.
    pub fn blink(&mut self, e: &Engine) -> bool {
        let focused_edit = e.page.borrow().focus.is_some_and(|f| is_editable(e, f));
        let on = focused_edit && ((e.now() - self.caret_t0) / 500.0).floor() as i64 % 2 == 0;
        let flip = on != self.caret_on;
        self.caret_on = on;
        flip
    }

    pub fn caret_active(&self, e: &Engine) -> bool {
        e.page.borrow().focus.is_some_and(|f| is_editable(e, f))
    }

    // ------------------------------------------------------------------ keyboard

    pub fn context_target(&self, e: &Engine, x: f32, y: f32) -> bool {
        let hit = hit_test(e, x, y);
        let focus = e.page.borrow().focus.filter(|&f| is_editable(e, f));
        focus.is_some_and(|f| hit.is_some_and(|h| e.page.borrow().dom.is_inclusive_descendant(h, f))) || self.ro_text(e).is_some()
    }

    pub fn context_state(&self, e: &Engine) -> [bool; 4] {
        if let Some(f) = e.page.borrow().focus.filter(|&f| is_editable(e, f)) {
            let selected = self.sel.0 != self.sel.1;
            let filled = !e.page.borrow().dom.value(f).is_empty();
            return [selected, selected, true, filled];
        }
        let selected = self.ro_text(e).is_some();
        [false, selected, false, self.ro_sel.is_some()]
    }

    pub fn context_command(&mut self, e: &Engine, id: u32) {
        if let Some(f) = e.page.borrow().focus.filter(|&f| is_editable(e, f)) {
            let key = match id { 1 => "x", 2 => "c", 3 => "v", 4 => "a", _ => return };
            self.edit_key(e, f, key, Mods { ctrl: true, ..Default::default() });
        } else if id == 2 {
            if let Some(t) = self.ro_text(e) {
                let _ = arboard::Clipboard::new().and_then(|mut c| c.set_text(t));
            }
        } else if id == 4 {
            self.select_all_text(e);
        }
    }
    /// `keydown`; returns true when handled by the page.
    pub fn key(&mut self, e: &Engine, key: &str, m: Mods) {
        let target = e.page.borrow().focus.unwrap_or(e.page.borrow().dom.body);
        let ok = e.js.fire(target, "keydown", &json!({ "bubbles": true, "cancelable": true, "key": key, "ctrlKey": m.ctrl, "shiftKey": m.shift, "altKey": m.alt, "metaKey": false }));
        e.run_calls();
        self.apply_focus_requests(e);
        if !ok {
            return;
        }
        let f = e.page.borrow().focus;
        if key == "Tab" {
            self.tab(e, m.shift);
            return;
        }
        if let Some(f) = f.filter(|&f| is_editable(e, f)) {
            self.edit_key(e, f, key, m);
        } else if m.ctrl && key.eq_ignore_ascii_case("c") {
            if let Some(t) = self.ro_text(e) {
                let _ = arboard::Clipboard::new().and_then(|mut c| c.set_text(t));
            }
        } else if (key == "Enter" || key == " ") && f.is_some_and(|f| e.page.borrow().dom.tag(f) == "button") {
            let f = f.unwrap();
            e.js.fire(f, "click", &json!({ "bubbles": true, "cancelable": true }));
            e.run_calls();
        }
    }

    fn ro_text(&self, e: &Engine) -> Option<String> {
        let (n, part, a, b) = self.ro_sel?;
        let p = e.page.borrow();
        let l = p.layout.leaves.get(&(n, part))?;
        let u: Vec<u16> = l.text.encode_utf16().collect();
        let (a, b) = (a.min(b) as usize, a.max(b) as usize);
        (a != b).then(|| String::from_utf16_lossy(&u[a.min(u.len())..b.min(u.len())]))
    }

    fn tab(&mut self, e: &Engine, back: bool) {
        let list: Vec<NodeId> = {
            let p = e.page.borrow();
            p.geom
                .order
                .iter()
                .filter(|k| k.1 == Part::Main)
                .map(|k| k.0)
                .filter(|&n| {
                    matches!(p.dom.tag(n), "input" | "textarea" | "button")
                        && p.styles.get(n as usize).and_then(|s| s.as_ref()).is_some_and(|s| s.base.visible && s.base.pointer_events)
                })
                .collect()
        };
        if list.is_empty() {
            return;
        }
        let cur = e.page.borrow().focus.and_then(|f| list.iter().position(|&x| x == f));
        let next = match (cur, back) {
            (Some(i), false) => (i + 1) % list.len(),
            (Some(i), true) => (i + list.len() - 1) % list.len(),
            (None, false) => 0,
            (None, true) => list.len() - 1,
        };
        self.set_focus(e, Some(list[next]), true);
    }

    fn value16(e: &Engine, f: NodeId) -> Vec<u16> {
        e.page.borrow().dom.value(f).encode_utf16().collect()
    }

    fn normalize_sel(&mut self, v: &[u16]) {
        self.sel = (utf16_boundary(v, self.sel.0), utf16_boundary(v, self.sel.1));
    }

    fn replace_sel(&mut self, e: &Engine, f: NodeId, ins: &str) {
        let mut v = Self::value16(e, f);
        self.normalize_sel(&v);
        let now = e.now();
        let single = ins.chars().count() == 1;
        if !(single && now - self.last_edit < 1000.0 && !self.undo.is_empty()) {
            self.undo.push((String::from_utf16_lossy(&v), self.sel));
            if self.undo.len() > 100 { self.undo.remove(0); }
        }
        self.redo.clear();
        self.last_edit = now;
        let (a, b) = (self.sel.0.min(self.sel.1) as usize, self.sel.0.max(self.sel.1) as usize);
        let ins16: Vec<u16> = ins.encode_utf16().collect();
        v.splice(a..b, ins16.iter().copied());
        let s = String::from_utf16_lossy(&v);
        e.page.borrow_mut().dom.set_value(f, &s);
        let c = (a + ins16.len()) as u32;
        self.sel = (c, c);
        self.caret_t0 = e.now();
        e.js.fire(f, "input", &json!({ "bubbles": true }));
        e.run_calls();
        self.scroll_caret_into_view(e, f);
        self.sync_overlay(e);
    }

    fn scroll_caret_into_view(&self, e: &Engine, f: NodeId) {
        // Recompute layout so the caret rect is current, then keep it inside the control.
        e.refresh(false);
        let mut p = e.page.borrow_mut();
        let (Some(leaf), Some(g)) = (p.layout.leaves.get(&(f, Part::Main)).cloned(), p.layout.boxes.get(&(f, Part::Main)).copied()) else { return };
        let (cx, cy, h) = text::caret_rect(&leaf.layout, self.sel.1);
        let (_, _, cw, ch) = g.content_box();
        let (sx, sy) = p.dom.scroll_of(f);
        let nx = if cx - sx > cw { cx - cw + 2.0 } else if cx < sx { cx } else { sx };
        let ny = if cy + h - sy > ch { cy + h - ch } else if cy < sy { cy } else { sy };
        p.dom.scroll.insert(f, (nx.max(0.0), ny.max(0.0)));
    }

    pub fn char(&mut self, e: &Engine, c: char) {
        let Some(f) = e.page.borrow().focus.filter(|&f| is_editable(e, f)) else { return };
        if (c as u32) < 0x20 {
            return;
        }
        let number = e.page.borrow().dom.attr(f, "type") == Some("number");
        if number && !(c.is_ascii_digit() || c == '-' || c == '.') {
            return;
        }
        self.replace_sel(e, f, &c.to_string());
    }

    pub fn insert_text(&mut self, e: &Engine, s: &str) {
        let Some(f) = e.page.borrow().focus.filter(|&f| is_editable(e, f)) else { return };
        let textarea = e.page.borrow().dom.tag(f) == "textarea";
        let text: String = s.chars().filter(|&c| c >= ' ' || textarea && c == '\n').collect();
        if !text.is_empty() { self.replace_sel(e, f, &text); }
    }

    fn restore(&mut self, e: &Engine, f: NodeId, redo: bool) {
        let item = if redo { self.redo.pop() } else { self.undo.pop() };
        let Some((value, sel)) = item else { return };
        let current = (e.page.borrow().dom.value(f).to_string(), self.sel);
        if redo { self.undo.push(current) } else { self.redo.push(current) }
        e.page.borrow_mut().dom.set_value(f, &value);
        self.sel = sel;
        e.js.fire(f, "input", &json!({ "bubbles": true }));
        e.run_calls();
    }

    fn edit_key(&mut self, e: &Engine, f: NodeId, key: &str, m: Mods) {
        let v = Self::value16(e, f);
        self.normalize_sel(&v);
        let len = v.len() as u32;
        let (a, b) = self.sel;
        let (lo, hi) = (a.min(b), a.max(b));
        let word_left = |mut i: usize| {
            while i > 0 && v[i - 1] == b' ' as u16 {
                i -= 1;
            }
            while i > 0 && v[i - 1] != b' ' as u16 {
                i -= 1;
            }
            i as u32
        };
        let word_right = |mut i: usize| {
            while i < v.len() && v[i] == b' ' as u16 {
                i += 1;
            }
            while i < v.len() && v[i] != b' ' as u16 {
                i += 1;
            }
            i as u32
        };
        let mv = |me: &mut Self, to: u32| {
            me.sel = if m.shift { (a, to) } else { (to, to) };
            me.caret_t0 = e.now();
        };
        let textarea = e.page.borrow().dom.tag(f) == "textarea";
        match key {
            "Backspace" => {
                if lo == hi && lo > 0 {
                    self.sel = (if m.ctrl { word_left(lo as usize) } else { utf16_prev(&v, lo) }, lo);
                }
                if self.sel.0 != self.sel.1 {
                    self.replace_sel(e, f, "");
                }
            }
            "Delete" => {
                if lo == hi && hi < len {
                    self.sel = (lo, if m.ctrl { word_right(hi as usize) } else { utf16_next(&v, hi) });
                }
                if self.sel.0 != self.sel.1 {
                    self.replace_sel(e, f, "");
                }
            }
            "ArrowLeft" => mv(self, if m.ctrl { word_left(b as usize) } else if lo != hi && !m.shift { lo } else { utf16_prev(&v, b) }),
            "ArrowRight" => mv(self, if m.ctrl { word_right(b as usize) } else if lo != hi && !m.shift { hi } else { utf16_next(&v, b) }),
            "ArrowUp" | "ArrowDown" if textarea => {
                let to = {
                    let p = e.page.borrow();
                    p.layout.leaves.get(&(f, Part::Main)).map_or(b, |leaf| {
                        let (cx, cy, h) = text::caret_rect(&leaf.layout, b);
                        let y = if key == "ArrowUp" { cy - h * 0.5 } else { cy + h * 1.5 };
                        let mut metrics = windows::Win32::Graphics::DirectWrite::DWRITE_TEXT_METRICS::default();
                        let height = unsafe { leaf.layout.GetMetrics(&mut metrics) }.map_or(h, |_| metrics.height);
                        if y < 0.0 { 0 } else if y > height { len } else { text::hit_test_caret(&leaf.layout, cx, y) }
                    })
                };
                mv(self, to)
            }
            "Home" | "ArrowUp" => mv(self, 0),
            "End" | "ArrowDown" => mv(self, len),
            k if m.ctrl && (k.eq_ignore_ascii_case("y") || (m.shift && k.eq_ignore_ascii_case("z"))) => self.restore(e, f, true),
            k if m.ctrl && k.eq_ignore_ascii_case("z") => self.restore(e, f, false),
            "Enter" if textarea => self.replace_sel(e, f, "\n"),
            "Enter" => {
                if e.page.borrow().dom.value(f) != self.focus_value {
                    self.focus_value = e.page.borrow().dom.value(f).to_string();
                    e.js.fire(f, "change", &json!({ "bubbles": true }));
                    e.run_calls();
                }
            }
            k if m.ctrl && k.eq_ignore_ascii_case("a") => self.sel = (0, len),
            k if m.ctrl && (k.eq_ignore_ascii_case("c") || k.eq_ignore_ascii_case("x")) => {
                if lo != hi && e.page.borrow().dom.attr(f, "type") != Some("password") {
                    let t = String::from_utf16_lossy(&v[lo as usize..hi as usize]);
                    let copied = arboard::Clipboard::new().and_then(|mut c| c.set_text(t));
                    if k.eq_ignore_ascii_case("x") && copied.is_ok() {
                        self.replace_sel(e, f, "");
                    }
                }
            }
            k if m.ctrl && k.eq_ignore_ascii_case("v") => {
                if let Ok(t) = arboard::Clipboard::new().and_then(|mut c| c.get_text()) {
                    let t = if textarea { t.replace("\r\n", "\n") } else { t.replace(['\r', '\n'], " ") };
                    self.replace_sel(e, f, &t);
                }
            }
            _ => {}
        }
        self.sync_overlay(e);
    }
}

/// Clamp stale DOM offsets and never leave a caret inside a surrogate pair.
fn utf16_boundary(v: &[u16], pos: u32) -> u32 {
    let mut i = (pos as usize).min(v.len());
    if i > 0 && i < v.len() && (0xD800..=0xDBFF).contains(&v[i - 1]) && (0xDC00..=0xDFFF).contains(&v[i]) {
        i -= 1;
    }
    i as u32
}

fn utf16_prev(v: &[u16], pos: u32) -> u32 {
    utf16_boundary(v, pos.saturating_sub(1))
}

fn utf16_next(v: &[u16], pos: u32) -> u32 {
    let i = (pos as usize + 1).min(v.len());
    let boundary = utf16_boundary(v, i as u32);
    if boundary < i as u32 { (i + 1) as u32 } else { boundary }
}

fn chain_ref(d: &dom::Dom, n: NodeId) -> Vec<NodeId> {
    let mut v = vec![n];
    let mut cur = n;
    while let Some(p) = d.parent(cur) {
        v.push(p);
        cur = p;
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn boot(s: &str) -> Engine {
        let extra = vec![
            ("fixed.js", include_str!("../tools/ref/fixed.js").to_string()),
            ("mock.js", include_str!("../tools/ref/mock.js").to_string()),
            ("scenario", s.to_string()),
        ];
        Engine::boot(&crate::engine::Doc::ISLAND, crate::js::storage::Storage::memory(), 0.0, true, &extra)
    }
    fn run(e: &Engine, inp: &mut Input, until: f64) {
        let mut t = e.now();
        while t < until {
            t += 16.0;
            e.pump(t, true);
            inp.apply_focus_requests(e);
        }
    }
    fn centre(e: &Engine, sel: &str) -> (f32, f32) {
        let p = e.page.borrow();
        let n = p.dom.query_str(p.dom.html, sel, false)[0];
        let g = p.layout.boxes[&(n, Part::Main)];
        let m = p.geom.world[&(n, Part::Main)];
        let (x, y, w, h) = m.bounds(g.x, g.y, g.w, g.h);
        (x + w / 2.0, y + h / 2.0)
    }

    /// Click the pin (toggle class), then type into the chat box and press Enter.
    #[test]
    fn click_and_type() {
        let e = boot("__seedStorage({'blip.page':'chat','blip.pageAt':Date.now()}); setTimeout(() => __emit('hover', true), 100);");
        let mut inp = Input::default();
        run(&e, &mut inp, 1500.0);
        let (x, y) = centre(&e, "#pinBtn");
        let pin = e.page.borrow().dom.by_id("pinBtn").unwrap();
        let hit = hit_test(&e, x, y).unwrap();
        assert!(e.page.borrow().dom.is_inclusive_descendant(hit, pin), "hit {hit}");
        inp.down(&e, x, y, Mods::default());
        inp.up(&e, x, y, Mods::default(), 500.0);
        assert!(e.page.borrow().dom.has_class(pin, "on"));

        let (x, y) = centre(&e, "#input");
        inp.down(&e, x, y, Mods::default());
        inp.up(&e, x, y, Mods::default(), 500.0);
        let input = e.page.borrow().dom.by_id("input").unwrap();
        assert_eq!(e.page.borrow().focus, Some(input));
        for c in "halo".chars() {
            inp.char(&e, c);
        }
        inp.key(&e, "Backspace", Mods::default());
        assert_eq!(e.page.borrow().dom.value(input), "hal");
        inp.key(&e, "Enter", Mods::default());
        run(&e, &mut inp, 2000.0);
        let p = e.page.borrow();
        let msgs = p.dom.by_id("messages").unwrap();
        assert_eq!(p.dom.text_content(p.dom.element_children(msgs)[0]), "hal");
        assert_eq!(p.dom.value(input), "");
        drop(p);

        let ctrl = Mods { ctrl: true, ..Mods::default() };
        inp.key(&e, "ArrowLeft", ctrl);
        assert_eq!(inp.sel, (0, 0));
        for c in "again".chars() {
            inp.char(&e, c);
        }
        inp.key(&e, "Enter", Mods::default());
        inp.key(&e, "Backspace", ctrl);
        assert_eq!(e.page.borrow().dom.value(input), "");
        assert_eq!(inp.sel, (0, 0));
    }

    #[test]
    fn emoji_navigation_and_deletion_preserve_utf16_pairs() {
        let e = boot("");
        let mut inp = Input::default();
        let input = e.page.borrow().dom.by_id("input").unwrap();
        inp.set_focus(&e, Some(input), true);
        for c in "a😀b".chars() {
            inp.char(&e, c);
        }
        inp.key(&e, "ArrowLeft", Mods::default());
        assert_eq!(inp.sel, (3, 3));
        inp.key(&e, "ArrowLeft", Mods::default());
        assert_eq!(inp.sel, (1, 1));
        inp.key(&e, "ArrowRight", Mods::default());
        assert_eq!(inp.sel, (3, 3));
        inp.key(&e, "Backspace", Mods::default());
        assert_eq!(e.page.borrow().dom.value(input), "ab");
        inp.char(&e, '😀');
        inp.key(&e, "ArrowLeft", Mods::default());
        inp.key(&e, "Delete", Mods::default());
        assert_eq!(e.page.borrow().dom.value(input), "ab");
    }

    #[test]
    fn changed_value_normalizes_selection_before_editing() {
        let e = boot("");
        let mut inp = Input::default();
        let input = e.page.borrow().dom.by_id("input").unwrap();
        inp.set_focus(&e, Some(input), true);
        inp.char(&e, 'x');
        e.page.borrow_mut().dom.set_value(input, "😀");
        inp.char(&e, 'a');
        assert_eq!(e.page.borrow().dom.value(input), "a😀");
        inp.key(&e, "End", Mods::default());
        e.page.borrow_mut().dom.set_value(input, "z");
        inp.key(&e, "Backspace", Mods::default());
        assert_eq!(e.page.borrow().dom.value(input), "");
    }

    /// Wheel over the open panel switches page (app.js handler, 320 ms throttle).
    #[test]
    fn wheel_switches_page() {
        let e = boot("__seedStorage({'blip.page':'today','blip.pageAt':Date.now()}); setTimeout(() => __emit('hover', true), 100);");
        let mut inp = Input::default();
        run(&e, &mut inp, 1500.0);
        let (x, y) = centre(&e, ".hdr");
        inp.wheel(&e, x, y, 0.0, 100.0);
        run(&e, &mut inp, 1600.0);
        assert_eq!(e.page.borrow().dom.attr(e.page.borrow().dom.body, "data-page"), Some("focus"));
    }

    /// Spam-clicking one tab must stay on that page: tabs never move under the cursor.
    #[test]
    fn spam_click_tab_stays() {
        let e = boot("__seedStorage({'blip.page':'today','blip.pageAt':Date.now()}); setTimeout(() => __emit('hover', true), 100);");
        let mut inp = Input::default();
        run(&e, &mut inp, 1500.0);
        let (x, y) = centre(&e, ".page-tab[data-p=\"water\"]");
        let mut t = 1500.0;
        for _ in 0..4 {
            inp.down(&e, x, y, Mods::default());
            inp.up(&e, x, y, Mods::default(), 0.0);
            t += 60.0;
            run(&e, &mut inp, t);
        }
        run(&e, &mut inp, t + 1000.0);
        assert_eq!(e.page.borrow().dom.attr(e.page.borrow().dom.body, "data-page"), Some("water"));
    }

    #[test]
    fn surrogate_pairs_combine() {
        let mut pending = None;
        assert_eq!(utf16_unit(&mut pending, 0xD83D), None);
        assert_eq!(utf16_unit(&mut pending, 0xDE00), Some('😀'));
        assert_eq!(utf16_unit(&mut pending, 0xDE00), None);
    }

    #[test]
    fn undo_restores_value() {
        let e = boot("");
        let mut inp = Input::default();
        let input = e.page.borrow().dom.by_id("input").unwrap();
        inp.set_focus(&e, Some(input), true);
        for c in "abc".chars() { inp.char(&e, c); }
        let ctrl = Mods { ctrl: true, ..Mods::default() };
        inp.key(&e, "z", ctrl);
        assert_eq!(e.page.borrow().dom.value(input), "");
        inp.key(&e, "y", ctrl);
        assert_eq!(e.page.borrow().dom.value(input), "abc");
    }

    #[test]
    fn shift_click_extends() {
        let e = boot("__seedStorage({'blip.page':'chat','blip.pageAt':Date.now()}); setTimeout(() => __emit('hover', true), 100);");
        let mut inp = Input::default();
        run(&e, &mut inp, 1500.0);
        let input = e.page.borrow().dom.by_id("input").unwrap();
        let (x, y) = centre(&e, "#input");
        inp.down(&e, x, y, Mods::default());
        inp.up(&e, x, y, Mods::default(), 500.0);
        for c in "hello".chars() { inp.char(&e, c); }
        run(&e, &mut inp, 1600.0);
        let (left, end) = {
            let p = e.page.borrow();
            let g = p.layout.boxes[&(input, Part::Main)];
            let m = p.geom.world[&(input, Part::Main)];
            let leaf = &p.layout.leaves[&(input, Part::Main)];
            let (cx, cy, _, ch) = g.content_box();
            let (ex, _, _) = text::caret_rect(&leaf.layout, 5);
            (m.apply(cx + leaf.dx, cy + ch / 2.0), m.apply(cx + leaf.dx + ex, cy + ch / 2.0))
        };
        inp.down(&e, left.0, left.1, Mods::default());
        inp.up(&e, left.0, left.1, Mods::default(), 0.0);
        inp.down(&e, end.0, end.1, Mods { shift: true, ..Mods::default() });
        assert_eq!(inp.sel, (0, 5));
    }

    #[test]
    fn capture_cancel_clears_state() {
        let e = boot("__seedStorage({'blip.page':'chat','blip.pageAt':Date.now()}); setTimeout(() => __emit('hover', true), 100);");
        let mut inp = Input::default();
        run(&e, &mut inp, 1500.0);
        let (x, y) = centre(&e, "#pinBtn");
        inp.down(&e, x, y, Mods::default());
        e.page.borrow_mut().capture = inp.down.map(|d| d.0);
        inp.cancel(&e);
        assert_eq!(e.page.borrow().capture, None);
    }
}
