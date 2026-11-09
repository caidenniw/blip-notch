//! QuickJS host running the original `ui/lib.js` + `ui/app.js` against the native DOM.
pub mod bridge;
pub mod storage;
#[cfg(test)]
mod tests;

use crate::css::cascade::{serialize, Cascade, ElemStyle};
use crate::dom::{Dom, MutKind, NodeId};
use crate::layout::{Layout, Part};
use crate::paint::geom::Geom;
use rquickjs::function::{Opt, Rest};
use rquickjs::{CatchResultExt, CaughtError, Context, Ctx, Function, Object, Runtime, Value};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

/// Engine-wide state visible to the JS bindings.
pub struct Page {
    pub dom: Dom,
    pub cascade: Cascade,
    /// Animated styles of the last frame (what getComputedStyle reports).
    pub styles: Vec<Option<Rc<ElemStyle>>>,
    /// Unanimated cascade, invalidated only by DOM style generation.
    pub computed_styles: Vec<Option<Rc<ElemStyle>>>,
    pub computed_generation: u64,
    pub layout: Layout,
    pub geom: Geom,
    /// `style_gen`/`text_gen` the layout was computed for.
    pub laid_out: (u64, u64),
    /// Forces a style+layout pass (`offsetWidth` etc. on a dirty tree).
    pub reflow: Option<Box<dyn FnMut(&mut Page)>>,
    pub now: f64,
    /// Pending JS timers: id -> due time (ms on `now` clock).
    pub timers: BTreeMap<u32, f64>,
    pub want_frame: bool,
    pub focus: Option<NodeId>,
    /// Focus/blur requests from JS (`el.focus()`), applied by the input layer.
    pub focus_req: Vec<Option<NodeId>>,
    pub capture: Option<NodeId>,
    pub storage: storage::Storage,
    pub images: crate::paint::images::ImageStore,
    /// Smooth scroll targets: id -> (from, to, start, duration).
    pub smooth: HashMap<NodeId, (f32, f32, f64, f64)>,
    pub reduced_motion: bool,
    /// Bridge calls queued by JS: (promise id, command, JSON args).
    pub calls: Vec<(u32, String, String)>,
    pub dirty: bool,
    /// Viewport size (logical px) the root is laid out in.
    pub view: (f32, f32),
}

pub type Shared = Rc<RefCell<Page>>;

impl Page {
    pub fn new(storage: storage::Storage, html: &str, css: &str, view: (f32, f32)) -> Page {
        Page {
            dom: crate::dom::html::parse_document(html),
            cascade: Cascade::new(css),
            styles: Vec::new(),
            computed_styles: Vec::new(),
            computed_generation: 0,
            layout: Layout::default(),
            geom: Geom::default(),
            laid_out: (0, 0),
            reflow: None,
            now: 0.0,
            timers: BTreeMap::new(),
            want_frame: false,
            focus: None,
            focus_req: Vec::new(),
            capture: None,
            storage,
            images: Default::default(),
            smooth: HashMap::new(),
            reduced_motion: false,
            calls: Vec::new(),
            dirty: true,
            view,
        }
    }

    fn ensure_layout(&mut self) {
        if self.laid_out != (self.dom.style_gen, self.dom.text_gen) {
            if let Some(mut f) = self.reflow.take() {
                f(self);
                self.reflow = Some(f);
            }
        }
    }

    pub fn box_of(&mut self, id: NodeId) -> Option<crate::layout::BoxGeom> {
        self.ensure_layout();
        self.layout.boxes.get(&(id, Part::Main)).copied()
    }

    /// Nearest positioned ancestor (offsetParent), or body.
    fn offset_parent(&self, id: NodeId) -> Option<NodeId> {
        let mut p = self.dom.parent(id);
        while let Some(n) = p {
            if n == self.dom.body {
                return Some(n);
            }
            let st = self.styles.get(n as usize).and_then(|s| s.as_ref());
            if st.is_some_and(|s| s.base.position != crate::css::cascade::Position::Static) {
                return Some(n);
            }
            p = self.dom.parent(n);
        }
        None
    }

    pub fn max_scroll(&mut self, id: NodeId) -> f32 {
        self.box_of(id).map(|g| (g.scroll_h() - g.client_h()).max(0.0)).unwrap_or(0.0)
    }

    pub fn set_scroll_top(&mut self, id: NodeId, v: f32) {
        let v = v.clamp(0.0, self.max_scroll(id)).round();
        let e = self.dom.scroll.entry(id).or_insert((0.0, 0.0));
        if e.1 != v {
            e.1 = v;
            self.dirty = true;
        }
    }

    /// Advances smooth scrolls; returns whether any is still running.
    pub fn tick_smooth(&mut self) -> bool {
        let now = self.now;
        let list: Vec<_> = self.smooth.iter().map(|(k, v)| (*k, *v)).collect();
        for (id, (from, to, start, dur)) in list {
            let t = ((now - start) / dur).clamp(0.0, 1.0) as f32;
            let e = crate::gfx::anim::Easing::Bezier(0.25, 0.1, 0.25, 1.0).apply(t);
            self.set_scroll_top(id, from + (to - from) * e);
            if t >= 1.0 {
                self.smooth.remove(&id);
            }
        }
        !self.smooth.is_empty()
    }
}

/// QuickJS runtime + context bound to a `Page`.
pub struct Js {
    pub rt: Runtime,
    pub ctx: Context,
    pub page: Shared,
}

fn log_err(e: CaughtError) {
    crate::conf::log(&format!("js error: {e}"));
}

impl Js {
    pub fn new(page: Shared) -> rquickjs::Result<Js> {
        let rt = Runtime::new()?;
        rt.set_memory_limit(256 << 20);
        let ctx = Context::full(&rt)?;
        ctx.with(|c| install(&c, &page))?;
        Ok(Js { rt, ctx, page })
    }

    /// Evaluates a global script; errors are logged, never fatal.
    pub fn eval(&self, name: &str, src: &str) {
        self.ctx.with(|c| {
            let mut opts = rquickjs::context::EvalOptions::default();
            opts.global = true;
            opts.strict = false;
            if let Err(e) = c.eval_with_options::<(), _>(src, opts).catch(&c) {
                crate::conf::log(&format!("js error in {name}: {e}"));
            }
        });
        self.settle();
    }

    /// Calls a global function with JSON-encoded args: `name(...args)`.
    pub fn call(&self, name: &str, args_json: &str) -> bool {
        let r = self.ctx.with(|c| -> bool {
            let f: Function = match c.globals().get(name) {
                Ok(f) => f,
                Err(_) => return true,
            };
            let args: Value = match c.json_parse(args_json) {
                Ok(v) => v,
                Err(_) => return true,
            };
            let arr = match args.into_array() {
                Some(a) => a,
                None => return true,
            };
            let items: Vec<Value> = arr.iter().filter_map(|v| v.ok()).collect();
            let mut a = rquickjs::function::Args::new(c.clone(), items.len());
            for v in items {
                let _ = a.push_arg(v);
            }
            match f.call_arg::<Value>(a).catch(&c) {
                Ok(v) => v.as_bool().unwrap_or(true),
                Err(e) => {
                    log_err(e);
                    true
                }
            }
        });
        self.settle();
        r
    }

    /// Runs promise jobs, then delivers MutationObserver records and freed-node notices.
    pub fn settle(&self) {
        for _ in 0..4 {
            loop {
                match self.rt.execute_pending_job() {
                    Ok(true) => {}
                    Ok(false) => break,
                    Err(e) => {
                        crate::conf::log(&format!("js job error: {e:?}"));
                    }
                }
            }
            let (muts, freed) = {
                let mut p = self.page.borrow_mut();
                (p.dom.take_mutations(), p.dom.take_freed())
            };
            if !freed.is_empty() {
                let j = serde_json::to_string(&[freed]).unwrap();
                self.call_raw("__forget", &j);
            }
            if muts.is_empty() {
                break;
            }
            let has = self.ctx.with(|c| {
                c.globals().get::<_, Function>("__hasMutationObservers").ok().and_then(|f| f.call::<_, bool>(()).ok()).unwrap_or(false)
            });
            if !has {
                break;
            }
            let recs: Vec<(NodeId, &str)> = muts
                .iter()
                .map(|m| {
                    (m.target, match m.kind {
                        MutKind::ChildList => "childList",
                        MutKind::CharacterData => "characterData",
                        MutKind::Attributes => "attributes",
                    })
                })
                .collect();
            self.call_raw("__mutations", &serde_json::to_string(&[recs]).unwrap());
        }
    }

    fn call_raw(&self, name: &str, args_json: &str) {
        self.ctx.with(|c| {
            let Ok(f) = c.globals().get::<_, Function>(name) else { return };
            let Ok(Some(arr)) = c.json_parse(args_json).map(|v| v.into_array()) else { return };
            let items: Vec<Value> = arr.iter().filter_map(|v| v.ok()).collect();
            let mut a = rquickjs::function::Args::new(c.clone(), items.len());
            for v in items {
                let _ = a.push_arg(v);
            }
            if let Err(e) = f.call_arg::<()>(a).catch(&c) {
                log_err(e);
            }
        });
    }

    /// Fires due timers (in deadline order); returns how many ran.
    pub fn run_timers(&self) -> usize {
        let mut ran = 0;
        loop {
            let due = {
                let p = self.page.borrow();
                let now = p.now;
                p.timers.iter().filter(|(_, &t)| t <= now).min_by(|a, b| a.1.total_cmp(b.1).then(a.0.cmp(b.0))).map(|(&id, _)| id)
            };
            let Some(id) = due else { break };
            self.page.borrow_mut().timers.remove(&id);
            self.call("__timer", &format!("[{id}]"));
            ran += 1;
            if ran > 10_000 {
                break;
            }
        }
        ran
    }

    pub fn next_timer(&self) -> Option<f64> {
        self.page.borrow().timers.values().copied().fold(None, |a, t| Some(a.map_or(t, |a: f64| a.min(t))))
    }

    /// Runs requestAnimationFrame callbacks.
    pub fn frame(&self) {
        let ts = {
            let mut p = self.page.borrow_mut();
            if !p.want_frame {
                return;
            }
            p.want_frame = false;
            p.now
        };
        self.call("__frame", &format!("[{ts}]"));
    }

    /// Dispatches a DOM event; returns false when `preventDefault` was called.
    pub fn fire(&self, target: NodeId, ty: &str, init: &serde_json::Value) -> bool {
        let j = serde_json::to_string(&serde_json::json!([target, ty, init])).unwrap();
        self.call("__fire", &j)
    }

    /// `document`-targeted event.
    pub fn fire_doc(&self, ty: &str, init: &serde_json::Value) -> bool {
        let j = serde_json::to_string(&serde_json::json!([-3, ty, init])).unwrap();
        self.call("__fire", &j)
    }
}

fn arg_id(v: f64) -> Option<NodeId> {
    (v >= 0.0).then_some(v as NodeId)
}

fn install<'js>(c: &Ctx<'js>, page: &Shared) -> rquickjs::Result<()> {
    let d = Object::new(c.clone())?;
    macro_rules! f {
        ($name:literal, $p:ident, |$($a:ident : $t:ty),*| $body:expr) => {{
            let $p = page.clone();
            d.set($name, Function::new(c.clone(), move |$($a: $t),*| { #[allow(unused_mut)] let mut $p = $p.borrow_mut(); $body })?)?;
        }};
    }
    // ---- tree
    f!("body", p, | | p.dom.body as f64);
    f!("root", p, | | p.dom.html as f64);
    f!("create", p, |tag: String| p.dom.create_element(&tag) as f64);
    f!("createText", p, |s: String| p.dom.create_text(&s) as f64);
    f!("isText", p, |id: f64| p.dom.text(id as NodeId).is_some());
    f!("tag", p, |id: f64| p.dom.tag(id as NodeId).to_string());
    f!("text", p, |id: f64| p.dom.text_content(id as NodeId));
    f!("setText", p, |id: f64, s: String| p.dom.set_text_content(id as NodeId, &s));
    f!("html", p, |id: f64, s: String| p.dom.set_inner_html(id as NodeId, &s));
    f!("append", p, |pa: f64, ch: f64| p.dom.append_child(pa as NodeId, ch as NodeId));
    f!("insertBefore", p, |pa: f64, ch: f64, r: f64| p.dom.insert_before(pa as NodeId, ch as NodeId, arg_id(r)));
    f!("remove", p, |id: f64| p.dom.remove(id as NodeId));
    f!("replace", p, |a: f64, b: f64| p.dom.replace_with(a as NodeId, b as NodeId));
    f!("parent", p, |id: f64| p.dom.parent(id as NodeId).map(|x| x as f64).unwrap_or(-1.0));
    f!("children", p, |id: f64| p.dom.element_children(id as NodeId).into_iter().map(|x| x as f64).collect::<Vec<_>>());
    f!("childNodes", p, |id: f64| p.dom.children(id as NodeId).iter().map(|&x| x as f64).collect::<Vec<_>>());
    f!("connected", p, |id: f64| p.dom.connected(id as NodeId));
    f!("contains", p, |a: f64, b: f64| p.dom.exists(b as NodeId) && p.dom.is_inclusive_descendant(b as NodeId, a as NodeId));
    // ---- attributes / classes / style
    f!("attr", p, |id: f64, n: String| p.dom.attr(id as NodeId, &n).map(str::to_string));
    f!("setAttr", p, |id: f64, n: String, v: String| p.dom.set_attr(id as NodeId, &n, &v));
    f!("rmAttr", p, |id: f64, n: String| p.dom.remove_attr(id as NodeId, &n));
    f!("classHas", p, |id: f64, k: String| p.dom.has_class(id as NodeId, &k));
    f!("classAdd", p, |id: f64, k: String| p.dom.add_class(id as NodeId, &k));
    f!("classRemove", p, |id: f64, k: String| p.dom.remove_class(id as NodeId, &k));
    f!("classToggle", p, |id: f64, k: String, force: f64| p.dom.toggle_class(id as NodeId, &k, if force < 0.0 { None } else { Some(force > 0.0) }));
    f!("styleSet", p, |id: f64, k: String, v: String| p.dom.style_set(id as NodeId, &k, &v));
    f!("styleGet", p, |id: f64, k: String| p.dom.style_get(id as NodeId, &k).unwrap_or("").to_string());
    f!("styleRm", p, |id: f64, k: String| p.dom.style_remove(id as NodeId, &k));
    // ---- queries
    f!("query", p, |root: f64, sel: String, all: bool| p.dom.query_str(root as NodeId, &sel, all).into_iter().map(|x| x as f64).collect::<Vec<_>>());
    f!("matches", p, |id: f64, sel: String| crate::css::select::parse_selector_list(&sel).is_some_and(|s| p.dom.matches(id as NodeId, &s)));
    f!("closest", p, |id: f64, sel: String| crate::css::select::parse_selector_list(&sel)
        .and_then(|s| p.dom.closest(id as NodeId, &s))
        .map(|x| x as f64)
        .unwrap_or(-1.0));
    // ---- layout
    f!("offset", p, |id: f64| {
        let id = id as NodeId;
        match p.box_of(id) {
            Some(g) => {
                let op = p.offset_parent(id).and_then(|o| p.layout.boxes.get(&(o, Part::Main)).copied());
                let (ox, oy) = op.map(|o| (o.x + o.border[3], o.y + o.border[0])).unwrap_or((0.0, 0.0));
                vec![(g.y - oy).round() as f64, (g.x - ox).round() as f64, g.w.round() as f64, g.h.round() as f64]
            }
            None => vec![0.0; 4],
        }
    });
    f!("scroll", p, |id: f64| {
        let id = id as NodeId;
        let top = p.dom.scroll_of(id).1;
        match p.box_of(id) {
            Some(g) => vec![top as f64, g.scroll_h().round() as f64, g.client_h().round() as f64, g.scroll_w() as f64, g.client_w().round() as f64],
            None => vec![top as f64, 0.0, 0.0, 0.0, 0.0],
        }
    });
    f!("setScrollTop", p, |id: f64, v: f64, smooth: bool| {
        let id = id as NodeId;
        if smooth && !p.reduced_motion {
            let from = p.dom.scroll_of(id).1;
            let to = (v as f32).clamp(0.0, p.max_scroll(id));
            let now = p.now;
            p.smooth.insert(id, (from, to, now, 250.0));
            p.dirty = true;
        } else {
            p.smooth.remove(&id);
            p.set_scroll_top(id, v as f32);
        }
    });
    f!("rect", p, |id: f64| {
        let id = id as NodeId;
        let Some(g) = p.box_of(id) else { return vec![0.0; 4] };
        let m = p.geom.world.get(&(id, Part::Main)).copied().unwrap_or(crate::paint::geom::Affine::ID);
        let (x, y, w, h) = m.bounds(g.x, g.y, g.w, g.h);
        vec![x as f64, y as f64, w as f64, h as f64]
    });
    f!("computed", p, |id: f64, pseudo: String, prop: String| {
        let id = id as NodeId;
        p.ensure_layout();
        let part = match pseudo.as_str() {
            "::before" | ":before" => Part::Before,
            "::after" | ":after" => Part::After,
            _ => Part::Main,
        };
        let Some(es) = p.styles.get(id as usize).and_then(|s| s.clone()) else { return String::new() };
        let st = match part {
            Part::Before => es.before.clone(),
            Part::After => es.after.clone(),
            _ => Some(es.base.clone()),
        };
        let Some(st) = st else { return String::new() };
        let g = p.layout.boxes.get(&(id, part)).copied().unwrap_or_default();
        serialize(&st, &prop, g.w, g.h)
    });
    // ---- forms / focus / capture
    f!("value", p, |id: f64| p.dom.value(id as NodeId).to_string());
    f!("setValue", p, |id: f64, v: String| p.dom.set_value(id as NodeId, &v));
    f!("focus", p, |id: f64| p.focus_req.push(Some(id as NodeId)));
    f!("blur", p, |id: f64| {
        if p.focus == Some(id as NodeId) {
            p.focus_req.push(None)
        }
    });
    f!("active", p, | | p.focus.map(|x| x as f64).unwrap_or(-1.0));
    f!("capture", p, |id: f64| p.capture = arg_id(id));
    f!("captured", p, | | p.capture.map(|x| x as f64).unwrap_or(-1.0));
    // ---- images
    f!("imgPixels", p, |src: String| p.images.bytes(&src).and_then(|b| crate::gfx::image::tint_pixels(&b)).map(|v| v.into_iter().map(|x| x as f64).collect::<Vec<_>>()));
    // ---- timers / frames
    f!("timer", p, |id: f64, ms: f64| {
        let due = p.now + ms.max(0.0);
        p.timers.insert(id as u32, due);
    });
    f!("clearTimer", p, |id: f64| {
        p.timers.remove(&(id as u32));
    });
    f!("wantFrame", p, | | p.want_frame = true);
    f!("now", p, | | p.now);
    f!("reducedMotion", p, | | p.reduced_motion);
    // ---- storage
    f!("lsGet", p, |k: String| p.storage.get(&k));
    f!("lsSet", p, |k: String, v: String| p.storage.set(&k, &v));
    f!("lsRm", p, |k: String| p.storage.remove(&k));
    f!("lsClear", p, | | p.storage.clear());
    f!("lsKeys", p, | | p.storage.keys());
    // ---- misc
    f!("log", _p, |s: String| crate::conf::log(&format!("js: {s}")));
    {
        let p = page.clone();
        d.set(
            "invoke",
            Function::new(c.clone(), move |id: f64, cmd: String, args: Opt<String>, _rest: Rest<Value>| {
                p.borrow_mut().calls.push((id as u32, cmd, args.0.unwrap_or_else(|| "{}".into())));
            })?,
        )?;
    }
    c.globals().set("__dom", d)?;
    Ok(())
}
