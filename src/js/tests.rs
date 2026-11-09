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
    boot_with("", scenario)
}

fn boot_with(pre: &str, scenario: &str) -> crate::engine::Engine {
    let extra = vec![
        ("lang", pre.to_string()),
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

#[test]
fn task_reminders_select_remove_and_reject_past() {
    let e = boot(r#"
setTimeout(() => { document.querySelector('#todoIn').value = 'Review @work'; document.body.setAttribute('data-probe', String(Date.now() + 15 * 60000)); __click('#todo15'); __click('#todoAdd'); }, 100);
setTimeout(() => { document.querySelector('#todoIn').value = 'No reminder'; __click('#todo60'); __click('#todoRemindClear'); __click('#todoAdd'); }, 200);
setTimeout(() => { __click('#todoRemind'); __click('#todoCustom'); document.querySelector('#todoYear').value = '2026'; document.querySelector('#todoMonth').value = '10'; document.querySelector('#todoDay').value = '24'; document.querySelector('#todoHour').value = '8'; document.querySelector('#todoMinute').value = '0'; __click('#todoSet'); }, 300);
"#);
    run(&e, 500.0);
    let p = e.page.borrow();
    let tasks: serde_json::Value = serde_json::from_str(&p.storage.get("blip.todos").unwrap()).unwrap();
    assert_eq!(tasks[0]["text"], "No reminder");
    assert!(tasks[0]["dueAt"].is_null());
    assert_eq!(tasks[1]["text"], "Review @work");
    assert_eq!(tasks[1]["dueAt"].as_i64().unwrap(), probe(&e).parse::<i64>().unwrap());
    let d = &p.dom;
    assert_eq!(d.text_content(d.by_id("todoReminderError").unwrap()), "Choose a valid date and a future time.");
    assert!(d.attr(d.by_id("todoPicker").unwrap(), "hidden").is_none());
}

#[test]
fn task_custom_reminder_keeps_explicit_date() {
    let e = boot(r#"
setTimeout(() => {
  __click('#todoRemind'); __click('#todoCustom');
  for (const [id, value] of [['todoYear','2027'],['todoMonth','2'],['todoDay','30'],['todoHour','14'],['todoMinute','30']]) document.querySelector('#' + id).value = value;
  __click('#todoSet');
  document.body.setAttribute('data-probe', document.querySelector('#todoRemind').getAttribute('aria-expanded'));
  document.querySelector('#todoDay').value = '28'; __click('#todoSet');
  document.querySelector('#todoIn').value = 'Appointment'; __click('#todoAdd');
}, 100);
"#);
    run(&e, 300.0);
    assert_eq!(probe(&e), "true");
    let p = e.page.borrow();
    let tasks: serde_json::Value = serde_json::from_str(&p.storage.get("blip.todos").unwrap()).unwrap();
    let expected = boot("setTimeout(() => document.body.setAttribute('data-probe', String(new Date(2027, 1, 28, 14, 30).getTime())), 100);");
    run(&expected, 200.0);
    assert_eq!(tasks[0]["dueAt"].as_i64().unwrap(), probe(&expected).parse::<i64>().unwrap());
}

#[test]
fn settings_page_toggle_commits_once() {
    let e = boot_settings(r#"
setTimeout(() => __emit('prefs', { pagesOff: [] }), 100);
setTimeout(() => __click('.navb[data-s="pages"]'), 150);
setTimeout(() => __click('.sw[data-k="p.page.todo"]'), 200);
setTimeout(() => document.body.setAttribute('data-probe', JSON.stringify(__lastInvoke.pref_set)), 300);
"#);
    run(&e, 400.0);
    let value: serde_json::Value = serde_json::from_str(&probe(&e)).unwrap();
    assert_eq!(value, serde_json::json!({"key": "pagesOff", "value": ["todo"]}));
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
    assert_eq!(d.text_content(d.by_id("icDate").unwrap()), "Sat, 24 Oct");
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
    assert!(d.text_content(d.by_id("tdGreet").unwrap()).starts_with("Good"));
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

fn boot_settings(scenario: &str) -> crate::engine::Engine {
    let extra = vec![
        ("fixed.js", include_str!("../../tools/ref/fixed.js").to_string()),
        ("mock.js", include_str!("../../tools/ref/mock.js").to_string()),
        ("scenario", scenario.to_string()),
    ];
    crate::engine::Engine::boot(&crate::engine::Doc::SETTINGS, crate::js::storage::Storage::memory(), 0.0, true, &extra)
}

fn probe(e: &crate::engine::Engine) -> String {
    let p = e.page.borrow();
    p.dom.attr(p.dom.body, "data-probe").unwrap_or("").to_string()
}

/// Settings window: nav switches section, a choice commits through `config_set`.
#[test]
fn settings_page_commits_choice() {
    let e = boot_settings(r#"
setTimeout(() => __click('.navb[data-s="notch"]'), 300);
setTimeout(() => __click('.seg[data-k="c.ui.animSpeed"] button[data-v="fast"]'), 400);
setTimeout(() => document.body.setAttribute('data-probe', __lastInvoke.config_set ? __lastInvoke.config_set.conf.ui.animSpeed : 'none'), 600);
"#);
    run(&e, 800.0);
    assert_eq!(probe(&e), "fast");
    let p = e.page.borrow();
    assert_eq!(p.dom.attr(p.dom.body, "data-sec"), Some("notch"));
    let sec = |s: &str| p.dom.query_str(p.dom.html, &format!("section.sec[data-s=\"{s}\"]"), false)[0];
    assert!(p.layout.boxes.get(&(sec("notch"), crate::layout::Part::Main)).is_some_and(|b| b.w > 0.0));
    assert!(!p.layout.boxes.contains_key(&(sec("ai"), crate::layout::Part::Main)));
}

fn ai_connect_scenario(model: &str) -> String {
    format!(r#"
setTimeout(() => __click('.navb[data-s="ai"]'), 300);
setTimeout(() => {{ const m = document.querySelector('#aiModel'); m.value = '{model}'; m.dispatchEvent(new Event('input')); document.querySelector('#bkNew').value = 'bad-model'; }}, 350);
setTimeout(() => __click('#bkAdd'), 400);
setTimeout(() => __click('#connect'), 450);
setTimeout(() => document.body.setAttribute('data-probe', (__lastInvoke.config_set ? __lastInvoke.config_set.conf.router.models.join(',') : 'none') + '|' + document.querySelector('#conn').dataset.st + '|' + document.querySelector('.bk .dot').className), 900);
"#)
}

#[test]
fn settings_ai_connect_saves_on_success() {
    let e = boot_settings(&ai_connect_scenario("gpt-x"));
    run(&e, 1000.0);
    assert_eq!(probe(&e), "gpt-x,bad-model|ok|dot bad");
}

#[test]
fn settings_ai_connect_fail_keeps_config() {
    let e = boot_settings(&ai_connect_scenario("bad-primary"));
    run(&e, 1000.0);
    assert!(probe(&e).starts_with("none|fail"), "{}", probe(&e));
}

/// Hidden pages drop their tab and Today row; Files off ignores dropped files.
#[test]
fn island_hidden_page_rows() {
    let e = boot("__seedStorage({'blip.page':'today','blip.pageAt':Date.now()});
        setTimeout(() => __emit('pref-set', {key:'pagesOff', value:['todo','shelf']}), 100);
        setTimeout(() => __emit('hover', true), 200);
        setTimeout(() => __emit('shelf:drop', ['C:/x.txt']), 1600);");
    run(&e, 1500.0);
    {
        let p = e.page.borrow();
        let d = &p.dom;
        assert_eq!(d.element_children(d.by_id("pageNav").unwrap()).len(), 4);
        let row = d.query_str(d.html, ".tdrow[data-go=\"todo\"]", false)[0];
        assert!(d.attr(row, "hidden").is_some());
    }
    run(&e, 2000.0);
    let p = e.page.borrow();
    let shelf = p.storage.get("blip.shelf").unwrap_or_default();
    assert!(shelf.is_empty() || shelf == "[]", "{shelf}");
}

/// Every key used by the UI exists, and `en`/`id` carry the same key set.
#[test]
fn i18n_keys_complete() {
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();
    let missing: String = ctx.with(|ctx| {
        ctx.globals().set("__lang_srcs", vec![
            include_str!("../../ui/index.html"), include_str!("../../ui/settings.html"), include_str!("../../ui/app.js"),
            include_str!("../../ui/settings.js"), include_str!("../../ui/lib.js"),
        ]).unwrap();
        ctx.eval::<(), _>(include_str!("../../ui/i18n.js")).catch(&ctx).unwrap();
        ctx.eval::<String, _>(
            r#"(() => {
              const en = Object.keys(STR.en).sort(), id = Object.keys(STR.id).sort();
              if (JSON.stringify(en) !== JSON.stringify(id)) return 'key sets differ: ' + en.filter(k => !id.includes(k)).concat(id.filter(k => !en.includes(k))).join(',');
              const used = new Set();
              for (const src of __lang_srcs) {
                for (const m of src.matchAll(/data-i18n(?:-title|-aria|-ph)?="([^"]+)"/g)) used.add(m[1]);
                for (const m of src.matchAll(/\b(?:t|tr)\('([^']+)'/g)) if (!m[1].endsWith('.')) used.add(m[1]);
                for (const m of src.matchAll(/'((?:intent|greet|nudge)\.[a-zA-Z.]+)'/g)) used.add(m[1]);
              }
              return [...used].filter(k => !(k in STR.en)).join(',');
            })()"#,
        ).catch(&ctx).unwrap()
    });
    assert_eq!(missing, "");
}

#[test]
fn island_switches_to_indonesian() {
    let e = boot_with("globalThis.__LANG='id';", "");
    run(&e, 500.0);
    let p = e.page.borrow();
    let d = &p.dom;
    assert_eq!(d.text_content(d.by_id("icDate").unwrap()), "Sab, 24 Okt");
    assert_eq!(d.text_content(d.by_id("pageTitle").unwrap()), "Obrolan");
}
