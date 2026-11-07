use rquickjs::{CatchResultExt, Context, Function, Runtime};
use std::cell::RefCell;
use std::rc::Rc;

/// Runs the original `ui/app.test.js` against `ui/lib.js` inside QuickJS.
#[test]
fn lib_js_tests() {
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();
    let out = Rc::new(RefCell::new(String::new()));
    ctx.with(|ctx| {
        let sink = out.clone();
        let log = Function::new(ctx.clone(), move |s: String| sink.borrow_mut().push_str(&s)).unwrap();
        ctx.globals().set("__log", log).unwrap();
        ctx.eval::<(), _>(
            r#"
            var console = { log: (...a) => __log(a.join(' ')) };
            var module = { exports: {} };
            var assertShim = {
              strictEqual(a, b) { if (a !== b) throw new Error('strictEqual: ' + a + ' !== ' + b); },
              deepStrictEqual(a, b) {
                const x = JSON.stringify(a), y = JSON.stringify(b);
                if (x !== y) throw new Error('deepStrictEqual: ' + x + ' !== ' + y);
              },
            };
            var require = n => n === 'assert' ? assertShim : module.exports;
            "#,
        )
        .unwrap();
        ctx.eval::<(), _>(include_str!("../../ui/lib.js")).catch(&ctx).unwrap();
        // app.test.js starts with 'use strict' + top-level const; wrap so it gets its own scope.
        let src = format!("(function(){{{}}})()", include_str!("../../ui/app.test.js"));
        ctx.eval::<(), _>(src).catch(&ctx).unwrap();
    });
    assert_eq!(out.borrow().as_str(), "ok");
}

fn boot(scenario: &str) -> crate::engine::Engine {
    let extra = vec![
        ("fixed.js", include_str!("../../tools/ref/fixed.js").to_string()),
        ("mock.js", include_str!("../../tools/ref/mock.js").to_string()),
        ("scenario", scenario.to_string()),
    ];
    crate::engine::Engine::boot(&crate::engine::Doc::ISLAND, crate::js::storage::Storage::memory(), 0.0, true, &extra)
}

fn run(e: &crate::engine::Engine, until: f64) {
    let mut t = e.now();
    while t < until {
        t += 16.0;
        e.pump(t, true);
    }
}

/// app.js boots against the native DOM, renders its page nav and the idle clock.
#[test]
fn app_boots_idle() {
    let e = boot("");
    run(&e, 500.0);
    let p = e.page.borrow();
    let d = &p.dom;
    let ic = d.by_id("icTime").unwrap();
    assert_eq!(d.text_content(ic), "09:41");
    assert_eq!(d.text_content(d.by_id("icDate").unwrap()), "Sab, 24 Okt");
    // music page hidden without media: 6 tabs
    assert_eq!(d.element_children(d.by_id("pageNav").unwrap()).len(), 6);
    assert_eq!(d.attr(d.body, "data-state"), Some("idle"));
    assert_eq!(d.style_get(d.body, "--w"), Some("168px"));
    assert_eq!(d.style_get(d.body, "--h"), Some("32px"));
}

/// Hover opens the panel on the stored page; layout reaches the open size after the spring.
#[test]
fn hover_opens_today() {
    let e = boot("__seedStorage({'blip.page':'today','blip.pageAt':Date.now()}); setTimeout(() => __emit('hover', true), 100);");
    run(&e, 1500.0);
    let p = e.page.borrow();
    let d = &p.dom;
    assert_eq!(d.attr(d.body, "data-state"), Some("open"));
    assert_eq!(d.attr(d.body, "data-page"), Some("today"));
    let island = d.by_id("island").unwrap();
    let g = p.layout.boxes[&(island, crate::layout::Part::Main)];
    assert_eq!((g.w, g.h), (460.0, 150.0));
    assert!(d.text_content(d.by_id("tdGreet").unwrap()).starts_with("Selamat"));
}

/// Chat bubbles stream: the MutationObserver + rAF path resizes the panel.
#[test]
fn chat_streams_into_bubble() {
    let e = boot(
        "__seedStorage({'blip.page':'chat','blip.pageAt':Date.now()});
         setTimeout(() => __emit('hover', true), 100);
         setTimeout(() => { const i = document.querySelector('#input'); i.value = 'halo apa kabar'; __click('#send'); }, 900);
         setTimeout(() => __emit('chat:chunk', { id: __lastInvoke.chat_send.id, delta: 'Baik, terima kasih!' }), 1200);
         setTimeout(() => __emit('chat:done', { id: __lastInvoke.chat_send.id }), 1400);",
    );
    run(&e, 2500.0);
    let p = e.page.borrow();
    let d = &p.dom;
    let msgs = d.by_id("messages").unwrap();
    let kids = d.element_children(msgs);
    assert_eq!(kids.len(), 2);
    assert_eq!(d.text_content(kids[1]), "Baik, terima kasih!");
    assert!(p.storage.get("blip.chat").unwrap().contains("terima kasih"));
}

#[test]
fn local_time_matches_windows() {
    let e = boot("");
    e.js.eval("t", "globalThis.__t = [new Date(2026,9,24,9,41).getHours(), new Date(2026,0,1).getTimezoneOffset()]");
    let (h, off) = e.js.ctx.with(|c| {
        let v: Vec<i32> = c.globals().get("__t").unwrap();
        (v[0], v[1])
    });
    assert_eq!(h, 9);
    let mut tz = windows::Win32::System::Time::TIME_ZONE_INFORMATION::default();
    unsafe { windows::Win32::System::Time::GetTimeZoneInformation(&mut tz) };
    assert_eq!(off, tz.Bias, "QuickJS tz offset vs Windows bias");
}

fn ids(e: &crate::engine::Engine, id: &str) -> Vec<crate::dom::NodeId> {
    let p = e.page.borrow();
    p.dom.element_children(p.dom.by_id(id).unwrap())
}

#[test]
fn nav_tabs_survive_page_switch() {
    let e = boot("__seedStorage({'blip.page':'today','blip.pageAt':Date.now()}); setTimeout(() => __emit('hover', true), 100); setTimeout(() => __click('.page-tab[data-p=\"focus\"]'), 1500);");
    run(&e, 1400.0);
    let before = ids(&e, "pageNav");
    run(&e, 2500.0);
    assert_eq!(ids(&e, "pageNav"), before);
    let p = e.page.borrow();
    let tabs = p.dom.query_str(p.dom.html, ".page-tab", true);
    let tab = |id| *tabs.iter().find(|&&n| p.dom.attr(n, "data-p") == Some(id)).unwrap();
    assert!(p.dom.has_class(tab("focus"), "on"));
    assert!(!p.dom.has_class(tab("today"), "on"));
}

#[test]
fn chat_history_not_rebuilt_on_reopen() {
    let e = boot("__seedStorage({'blip.page':'chat','blip.pageAt':Date.now(),'blip.chat':[{role:'user',content:'a'},{role:'assistant',content:'b'},{role:'user',content:'c'},{role:'assistant',content:'d'}]}); setTimeout(() => __emit('hover', true), 100); setTimeout(() => __emit('hover', false), 1500); setTimeout(() => __emit('hover', true), 3500);");
    run(&e, 1400.0);
    let before = ids(&e, "messages");
    run(&e, 5000.0);
    let after = ids(&e, "messages");
    assert_eq!(after, before);
    let p = e.page.borrow();
    assert!(after.iter().all(|&n| p.dom.has_class(n, "settled")));
}

#[test]
fn todo_toggle_keeps_row() {
    let e = boot("__seedStorage({'blip.page':'todo','blip.pageAt':Date.now(),'blip.todos':[{id:1,text:'a',done:false}]}); setTimeout(() => __emit('hover', true), 100); setTimeout(() => __click('.chk'), 1500);");
    run(&e, 1400.0);
    let before = { let p = e.page.borrow(); p.dom.query_str(p.dom.html, ".trow", false)[0] };
    run(&e, 1700.0);
    let p = e.page.borrow();
    let after = p.dom.query_str(p.dom.html, ".trow", false)[0];
    assert_eq!(after, before);
    assert!(p.dom.has_class(after, "done"));
}

#[test]
fn idle_is_quiet() {
    let e = boot("");
    run(&e, 2000.0);
    assert!(!e.animating());
}

/// Settings window: nav switches section, a choice commits through `config_set`.
#[test]
fn settings_page_commits_choice() {
    const SRC: &str = r#"
setTimeout(() => __click('.navb[data-s="sistem"]'), 300);
setTimeout(() => __click('.seg[data-k="c.ui.animSpeed"] button[data-v="fast"]'), 400);
setTimeout(() => document.body.setAttribute('data-probe', __lastInvoke.config_set ? __lastInvoke.config_set.conf.ui.animSpeed : 'none'), 600);
"#;
    let extra = vec![
        ("fixed.js", include_str!("../../tools/ref/fixed.js").to_string()),
        ("mock.js", include_str!("../../tools/ref/mock.js").to_string()),
        ("scenario", SRC.to_string()),
    ];
    let e = crate::engine::Engine::boot(&crate::engine::Doc::SETTINGS, crate::js::storage::Storage::memory(), 0.0, true, &extra);
    run(&e, 800.0);
    let p = e.page.borrow();
    assert_eq!(p.dom.attr(p.dom.body, "data-probe"), Some("fast"));
    assert_eq!(p.dom.attr(p.dom.body, "data-sec"), Some("sistem"));
    let sec = |s: &str| p.dom.query_str(p.dom.html, &format!("section.sec[data-s=\"{s}\"]"), false)[0];
    assert!(p.layout.boxes.get(&(sec("sistem"), crate::layout::Part::Main)).is_some_and(|b| b.w > 0.0));
    assert!(!p.layout.boxes.contains_key(&(sec("ai"), crate::layout::Part::Main)));
}
