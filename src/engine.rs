//! The UI engine: DOM + CSS + animations + layout + JS, driven by a clock.
use crate::css::anim::Animator;
use crate::js::bridge::{self, Outcome};
use crate::js::{storage::Storage, Js, Page, Shared};
use crate::paint::{Painter, Scene};
use crate::layout::Part;
use std::cell::RefCell;
use std::rc::Rc;

pub struct Engine {
    pub page: Shared,
    pub js: Js,
    pub painter: Rc<RefCell<Painter>>,
    pub anim: Rc<RefCell<Animator>>,
    pub overlay: crate::paint::Overlay,
}

/// One UI document: markup, stylesheet, scripts (in order) and viewport size.
pub struct Doc {
    pub html: &'static str,
    pub css: &'static str,
    pub scripts: &'static [(&'static str, &'static str)],
    pub view: (f32, f32),
}

impl Doc {
    pub const ISLAND: Doc = Doc {
        html: include_str!("../ui/index.html"),
        css: include_str!("../ui/style.css"),
        scripts: &[("lib.js", include_str!("../ui/lib.js")), ("app.js", include_str!("../ui/app.js"))],
        view: (crate::win::window::WIN_W as f32, crate::win::window::WIN_H as f32),
    };
    pub const SETTINGS: Doc = Doc {
        html: include_str!("../ui/settings.html"),
        css: include_str!("../ui/settings.css"),
        scripts: &[("settings.js", include_str!("../ui/settings.js"))],
        view: (crate::win::window::SET_W as f32, crate::win::window::SET_H as f32),
    };
}

/// Style → transitions → layout → geometry at the page's current time.
fn reflow(page: &mut Page, anim: &RefCell<Animator>, painter: &RefCell<Painter>) {
    page.cascade.reduced_motion = page.reduced_motion;
    if page.computed_generation != page.dom.style_gen {
        page.computed_styles = page.cascade.compute(&page.dom);
        page.computed_generation = page.dom.style_gen;
    }
    let styles = anim.borrow_mut().apply(&page.cascade, &page.dom, &page.computed_styles, page.now);
    let layout = {
        let mut p = painter.borrow_mut();
        let layout = crate::layout::compute(&page.dom, &styles, p.text(), page.view);
        p.text().gc();
        layout
    };
    // Browser scroll offsets shrink with the scrollable extent after resize/replacement.
    for (&id, offset) in page.dom.scroll.iter_mut() {
        if let Some(g) = layout.boxes.get(&(id, crate::layout::Part::Main)) {
            offset.0 = offset.0.clamp(0.0, (g.scroll_w() - g.client_w()).max(0.0));
            offset.1 = offset.1.clamp(0.0, (g.scroll_h() - g.client_h()).max(0.0));
        }
    }
    page.geom = crate::paint::geom::compute(&page.dom, &styles, &layout);
    page.layout = layout;
    page.styles = styles;
    page.laid_out = (page.dom.style_gen, page.dom.text_gen);
}

impl Engine {
    /// Builds the DOM and runs `prelude.js`, the bridge (unless `mocked`), `extra` scripts
    /// (snapshot/test mode), then the document's scripts.
    pub fn boot(doc: &Doc, storage: Storage, now: f64, mocked: bool, extra: &[(&str, String)]) -> Engine {
        let mut page = Page::new(storage, doc.html, doc.css, doc.view);
        page.now = now;
        page.reduced_motion = crate::win::window::reduced_motion();
        let page = Rc::new(RefCell::new(page));
        let painter = Rc::new(RefCell::new(Painter::new()));
        let anim = Rc::new(RefCell::new(Animator::default()));
        {
            let (a, p) = (anim.clone(), painter.clone());
            page.borrow_mut().reflow = Some(Box::new(move |pg: &mut Page| reflow(pg, &a, &p)));
        }
        let js = Js::new(page.clone()).expect("QuickJS");
        let e = Engine { page, js, painter, anim, overlay: Default::default() };
        e.js.eval("prelude.js", include_str!("js/prelude.js"));
        if !mocked {
            e.js.eval("bridge", bridge::BRIDGE_JS);
        }
        for (name, src) in extra {
            e.js.eval(name, src);
        }
        for (name, src) in doc.scripts {
            e.js.eval(name, src);
        }
        e.run_calls();
        e
    }

    pub fn now(&self) -> f64 {
        self.page.borrow().now
    }

    /// Executes `invoke` calls queued by JS.
    pub fn run_calls(&self) {
        loop {
            let calls = std::mem::take(&mut self.page.borrow_mut().calls);
            if calls.is_empty() {
                return;
            }
            for (id, cmd, args) in calls {
                match bridge::dispatch(id, &cmd, &args) {
                    Outcome::Now(r) => self.resolve(id, r),
                    Outcome::Later => {}
                    Outcome::DragOut(paths) => {
                        // Store for the window procedure (OLE modal loop must not nest in JS).
                        DRAG.with(|d| d.borrow_mut().push_back((id, paths)));
                        crate::win::window::post_drag(crate::bus::hwnd());
                    }
                }
            }
        }
    }

    pub fn resolve(&self, id: u32, r: Result<serde_json::Value, String>) {
        let j = match r {
            Ok(v) => serde_json::json!([id, true, v]),
            Err(e) => serde_json::json!([id, false, e]),
        };
        self.js.call("__resolve", &j.to_string());
    }

    pub fn emit(&self, name: &str, payload: &serde_json::Value) {
        self.js.call("__emit", &serde_json::json!([name, payload]).to_string());
        self.run_calls();
    }

    /// Delivers one bus event to JS.
    pub fn deliver(&self, e: crate::bus::Event) {
        if let crate::bus::Event::Resolve { id, result } = e {
            self.resolve(id, result);
            self.run_calls();
            return;
        }
        let pair = {
            let mut p = self.page.borrow_mut();
            let page = &mut *p;
            let mut put = |b: Vec<u8>| page.images.put(b);
            bridge::event_json(e, &mut put)
        };
        if let Some((name, payload)) = pair {
            self.emit(name, &payload);
        }
    }

    /// Advances the clock to `now`: timers, rAF (when `frame`), smooth scroll, then style/layout.
    pub fn pump(&self, now: f64, frame: bool) {
        self.page.borrow_mut().now = now;
        self.js.run_timers();
        self.run_calls();
        if frame {
            self.js.frame();
            self.run_calls();
        }
        self.page.borrow_mut().tick_smooth();
        self.refresh(frame);
    }

    /// Recomputes style/animations/layout and dispatches transition events. Non-frame calls
    /// during an animation skip the animated pass: the next `WM_FRAME` paints it.
    pub fn refresh(&self, frame: bool) {
        {
            let mut p = self.page.borrow_mut();
            let dirty = p.laid_out != (p.dom.style_gen, p.dom.text_gen);
            let (busy, layout_busy) = { let a = self.anim.borrow(); (a.busy(), a.layout_busy()) };
            if dirty || (frame && layout_busy) {
                reflow(&mut p, &self.anim, &self.painter);
            } else if frame && busy {
                p.styles = self.anim.borrow_mut().apply(&p.cascade, &p.dom, &p.computed_styles, p.now);
                p.geom = crate::paint::geom::compute(&p.dom, &p.styles, &p.layout);
            } else if !busy {
                // Scroll changes affect world transforms, not taffy layout.
                p.geom = crate::paint::geom::compute(&p.dom, &p.styles, &p.layout);
            }
        }
        let events = std::mem::take(&mut self.anim.borrow_mut().events);
        let mut seen = std::collections::HashSet::new();
        for (id, ty) in events {
            if seen.insert((id, ty)) {
                self.js.fire(id, ty, &serde_json::json!({ "bubbles": true }));
            }
        }
        if !seen.is_empty() {
            self.run_calls();
            let dirty = { let p = self.page.borrow(); p.laid_out != (p.dom.style_gen, p.dom.text_gen) };
            if dirty {
                let mut p = self.page.borrow_mut();
                reflow(&mut p, &self.anim, &self.painter);
            }
        }
    }

    /// Something time-driven is still in progress (transition, animation, rAF, smooth scroll).
    pub fn animating(&self) -> bool {
        let p = self.page.borrow();
        p.want_frame || !p.smooth.is_empty() || self.anim.borrow().busy()
    }

    /// Logical `(x0, y0, x1, y1)` world bounds of element `#id` and its pseudo-elements.
    pub fn rect_of(&self, id: &str) -> Option<(f32, f32, f32, f32)> {
        let p = self.page.borrow();
        let n = p.dom.by_id(id)?;
        let mut out: Option<(f32, f32, f32, f32)> = None;
        for part in [Part::Main, Part::Before, Part::After] {
            let (Some(b), Some(m)) = (p.layout.boxes.get(&(n, part)), p.geom.world.get(&(n, part))) else { continue };
            let (x, y, w, h) = m.bounds(b.x, b.y, b.w, b.h);
            out = Some(match out {
                Some(o) => (o.0.min(x), o.1.min(y), o.2.max(x + w), o.3.max(y + h)),
                None => (x, y, x + w, y + h),
            });
        }
        out
    }

    /// Applies the motion settings and forces a full restyle.
    pub fn set_motion(&self, reduced: bool, scale: f64) {
        let mut p = self.page.borrow_mut();
        p.reduced_motion = reduced;
        self.anim.borrow_mut().scale = scale;
        p.computed_generation = u64::MAX;
        p.laid_out = (u64::MAX, u64::MAX);
        p.dirty = true;
    }

    pub fn next_timer(&self) -> Option<f64> {
        self.js.next_timer()
    }

    pub fn paint(&self, gfx: &crate::gfx::Gfx, scale: f32) {
        let mut p = self.page.borrow_mut();
        let mut painter = self.painter.borrow_mut();
        // Images are owned by the page (JS resolves `src` through it); lend them to the painter.
        std::mem::swap(&mut painter.images, &mut p.images);
        {
            let scene = Scene { dom: &p.dom, styles: &p.styles, layout: &p.layout, geom: &p.geom, overlay: &self.overlay };
            painter.paint(gfx, &scene, scale);
        }
        std::mem::swap(&mut painter.images, &mut p.images);
    }

    pub fn snapshot(&self, gfx: &mut crate::gfx::Gfx, path: &std::path::Path) -> windows::core::Result<()> {
        let mut p = self.page.borrow_mut();
        let mut painter = self.painter.borrow_mut();
        std::mem::swap(&mut painter.images, &mut p.images);
        let r = {
            let scene = Scene { dom: &p.dom, styles: &p.styles, layout: &p.layout, geom: &p.geom, overlay: &self.overlay };
            crate::paint::render_to_png(gfx, &mut painter, &scene, 1.0, p.view, path)
        };
        std::mem::swap(&mut painter.images, &mut p.images);
        r
    }
}

thread_local! {
    pub static DRAG: RefCell<std::collections::VecDeque<(u32, Vec<String>)>> = RefCell::new(Default::default());
}
