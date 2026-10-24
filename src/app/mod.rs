//! Main application state machine, event handling, and rendering.
pub mod pages;

use std::time::{Duration, Instant};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::*;

use crate::bus::{self, DictatePhase, Event, HudEvent, MediaInfo};
use crate::gfx::anim::{Easing, Tween};
use crate::gfx::text::Text;
use crate::gfx::Gfx;
use crate::logic::Msg;
use crate::store::{Saver, Store};
use crate::ui::{button_logic, id_from_str, Frame, Rect, UiState};
use pages::PageId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Idle,
    Live,
    Alert,
    Open,
    Answer,
    Listening,
    Thinking,
    Voice,
    Hud,
}

pub struct App {
    pub state: State,
    pub store: Store,
    pub saver: Saver,
    pub ui: UiState,
    pub text: Text,
    pub media: MediaInfo,
    pub active_page: PageId,
    pub pinned: bool,
    pub hover: bool,
    pub compact: bool,

    // Island shape animated properties
    pub tween_w: Tween,
    pub tween_h: Tween,
    pub tween_r: Tween,
    pub tween_e: Tween,

    // Alert & answer
    pub alert_text: String,
    pub alert_until: Instant,
    pub answer_text: String,

    // Mascot
    pub eye_blink: bool,
    pub last_blink: Instant,
}

impl App {
    pub fn new() -> Self {
        let now = Instant::now();
        let store = Store::load();
        let active_page = PageId::from_str(&store.page);
        App {
            state: State::Idle,
            store,
            saver: Saver::new(),
            ui: UiState::default(),
            text: Text::new().expect("directwrite factory"),
            media: MediaInfo::default(),
            active_page,
            pinned: false,
            hover: false,
            compact: false,
            tween_w: Tween::settled(168.0, now),
            tween_h: Tween::settled(36.0, now),
            tween_r: Tween::settled(16.0, now),
            tween_e: Tween::settled(11.0, now),
            alert_text: String::new(),
            alert_until: now,
            answer_text: String::new(),
            eye_blink: false,
            last_blink: now,
        }
    }

    pub fn set_target_size(&mut self, w: f32, h: f32, r: f32, e: f32, ms: u64, easing: Easing) {
        let now = Instant::now();
        self.tween_w.retarget(w, now, ms, 0, easing);
        self.tween_h.retarget(h, now, ms, 0, easing);
        self.tween_r.retarget(r, now, ms, 0, easing);
        self.tween_e.retarget(e, now, ms, 0, easing);
        crate::hover::set_rect(w as f64, h as f64);
    }

    pub fn go(&mut self, next: State) {
        if self.state == next {
            return;
        }
        self.state = next;
        self.apply_size();
    }

    pub fn apply_size(&mut self) {
        let (w, h, r, e, ms, easing) = match self.state {
            State::Idle => {
                if self.compact {
                    (120.0, 24.0, 12.0, 8.0, 460, Easing::EaseClose)
                } else {
                    (168.0, 36.0, 16.0, 11.0, 460, Easing::EaseClose)
                }
            }
            State::Live => {
                if self.compact {
                    (160.0, 24.0, 12.0, 8.0, 460, Easing::EaseClose)
                } else {
                    (220.0, 36.0, 16.0, 11.0, 460, Easing::EaseClose)
                }
            }
            State::Alert => (260.0, 36.0, 16.0, 11.0, 460, Easing::EaseClose),
            State::Open => (460.0, 176.0, 24.0, 14.0, 640, Easing::Spring),
            State::Answer => (420.0, 160.0, 22.0, 13.0, 640, Easing::Spring),
            State::Listening => (260.0, 48.0, 24.0, 13.0, 640, Easing::Spring),
            State::Thinking => (180.0, 42.0, 21.0, 12.0, 460, Easing::EaseClose),
            State::Voice => (320.0, 180.0, 24.0, 14.0, 640, Easing::Spring),
            State::Hud => (220.0, 44.0, 22.0, 12.0, 640, Easing::Spring),
        };
        self.set_target_size(w, h, r, e, ms, easing);
    }

    pub fn handle_events(&mut self) {
        let events = bus::drain();
        for ev in events {
            match ev {
                Event::Hover(h) => {
                    self.hover = h;
                    if h {
                        if self.state == State::Idle || self.state == State::Live {
                            self.go(State::Open);
                        }
                    } else if !self.pinned && self.state == State::Open {
                        self.go(if self.media.playing { State::Live } else { State::Idle });
                    }
                }
                Event::Cursor { x, y } => {
                    self.ui.pointer_pos = (x as f32, y as f32);
                }
                Event::Compact(c) => {
                    self.compact = c;
                    self.apply_size();
                }
                Event::Media(m) => {
                    let was_playing = self.media.playing;
                    self.media = m;
                    if self.media.playing && !was_playing && self.state == State::Idle {
                        self.go(State::Live);
                    } else if !self.media.playing && was_playing && self.state == State::Live {
                        self.go(State::Idle);
                    }
                }
                Event::Hotkey => {
                    if self.state == State::Open {
                        self.go(State::Idle);
                    } else {
                        self.go(State::Open);
                    }
                }
                Event::UiShow => {
                    self.go(State::Open);
                }
                Event::UiHide => {
                    self.go(State::Idle);
                }
                Event::UiAsk(text) => {
                    self.send_chat(text);
                }
                Event::Agent { agent, state, .. } => {
                    let msg = format!("{agent} {state}");
                    self.show_alert(&msg, 5000);
                }
                Event::ChatChunk { delta, .. } => {
                    self.answer_text.push_str(&delta);
                    if self.state != State::Answer && self.state != State::Open {
                        self.go(State::Answer);
                    }
                }
                Event::ChatDone { id: _, model: _, aborted } => {
                    if !aborted && !self.answer_text.is_empty() {
                        let content = std::mem::take(&mut self.answer_text);
                        self.store.chat.push(Msg {
                            role: "assistant".into(),
                            content,
                        });
                        self.saver.mark_dirty();
                    }
                }
                Event::ChatError { error, .. } => {
                    self.show_alert(&error, 3000);
                }
                Event::SttResult(text) => {
                    if !text.is_empty() {
                        self.send_chat(text);
                    }
                }
                _ => {}
            }
        }
    }

    pub fn send_chat(&mut self, text: String) {
        self.store.chat.push(Msg {
            role: "user".into(),
            content: text.clone(),
        });
        self.saver.mark_dirty();
        self.answer_text.clear();
        self.go(State::Thinking);

        let id = format!("{:x}", crate::time::now_ms());
        let messages = self
            .store
            .chat
            .iter()
            .map(|m| serde_json::json!({ "role": m.role, "content": m.content }))
            .collect();
        let memory = self.store.memory.join("\n");
        crate::ai::chat_send(id, messages, Some(false), Some(memory));
    }

    pub fn show_alert(&mut self, text: &str, dur_ms: u64) {
        self.alert_text = text.to_string();
        self.alert_until = Instant::now() + Duration::from_millis(dur_ms);
        self.go(State::Alert);
    }

    pub fn update(&mut self) {
        let now = Instant::now();
        if self.state == State::Alert && now >= self.alert_until {
            self.go(if self.media.playing { State::Live } else { State::Idle });
        }
        if now.saturating_duration_since(self.last_blink) >= Duration::from_secs(4) {
            self.eye_blink = true;
            if now.saturating_duration_since(self.last_blink) >= Duration::from_millis(4200) {
                self.eye_blink = false;
                self.last_blink = now;
            }
        }
        if self.saver.due(now) {
            self.saver.flush(&self.store);
        }
    }

    pub fn render(&mut self, hwnd: HWND, gfx: &mut Gfx, scale: f32) {
        self.handle_events();
        self.update();

        let now = Instant::now();
        let w = self.tween_w.value(now);
        let h = self.tween_h.value(now);
        let r = self.tween_r.value(now);
        let e = self.tween_e.value(now);

        let left = (640.0 - w) / 2.0;

        gfx.begin(scale);

        // Draw island body with anti-aliasing
        if let (Ok(path), Ok(brush)) = (
            crate::gfx::island_path(&gfx.factory, left, w, h, r, e),
            gfx.solid(0.0, 0.0, 0.0, 1.0),
        ) {
            unsafe { gfx.dc.FillGeometry(&path, &brush, None) };
        }

        let mut frame = Frame::default();
        let island_rect = Rect::new(left, 0.0, w, h);

        match self.state {
            State::Idle => {
                self.render_mascot(gfx, left + w / 2.0, h / 2.0);
            }
            State::Live => {
                self.render_live_row(gfx, &mut frame, island_rect);
            }
            State::Alert => {
                self.render_alert_row(gfx, island_rect);
            }
            State::Open => {
                self.render_open_panel(gfx, &mut frame, island_rect);
            }
            _ => {
                self.render_mascot(gfx, left + w / 2.0, h / 2.0);
            }
        }

        let (x, y, _, _) = crate::win::window::top_centre(scale as f64);
        if gfx.present(hwnd, x, y).is_err() {
            let _ = gfx.rebuild();
        }
    }

    fn render_mascot(&mut self, gfx: &mut Gfx, cx: f32, cy: f32) {
        unsafe {
            if let Ok(b) = gfx.solid(0.3, 0.79, 0.94, 1.0) {
                let eye_h = if self.eye_blink { 2.0 } else { 12.0 };
                let r1 = crate::gfx::svg::rounded_rect(cx - 7.0, cy - eye_h / 2.0, 5.0, eye_h, 2.5);
                let r2 = crate::gfx::svg::rounded_rect(cx + 2.0, cy - eye_h / 2.0, 5.0, eye_h, 2.5);
                if let Ok(g1) = crate::gfx::svg::build_geometry(&gfx.factory, &r1, true) {
                    gfx.dc.FillGeometry(&g1, &b, None);
                }
                if let Ok(g2) = crate::gfx::svg::build_geometry(&gfx.factory, &r2, true) {
                    gfx.dc.FillGeometry(&g2, &b, None);
                }
            }
        }
    }

    fn render_live_row(&mut self, gfx: &mut Gfx, _f: &mut Frame, rect: Rect) {
        unsafe {
            self.render_mascot(gfx, rect.x + 20.0, rect.y + rect.h / 2.0);
            if let Ok(brush) = gfx.solid(1.0, 1.0, 1.0, 0.95) {
                let st = crate::gfx::text::TextStyle {
                    size: 12.0,
                    weight: 600,
                    ellipsis: true,
                    ..Default::default()
                };
                let lay = self.text.layout(&self.media.title, &st, rect.w - 50.0, 18.0);
                crate::gfx::text::Text::draw(&gfx.dc, &lay, rect.x + 36.0, rect.y + 9.0, &brush);
            }
        }
    }

    fn render_alert_row(&mut self, gfx: &mut Gfx, rect: Rect) {
        unsafe {
            if let Ok(brush) = gfx.solid(1.0, 0.8, 0.3, 1.0) {
                let st = crate::gfx::text::TextStyle {
                    size: 12.0,
                    weight: 600,
                    ellipsis: true,
                    ..Default::default()
                };
                let lay = self.text.layout(&self.alert_text, &st, rect.w - 20.0, 18.0);
                crate::gfx::text::Text::draw(&gfx.dc, &lay, rect.x + 10.0, rect.y + 9.0, &brush);
            }
        }
    }

    fn render_open_panel(&mut self, gfx: &mut Gfx, f: &mut Frame, rect: Rect) {
        // Header (28px)
        let hdr_rect = Rect::new(rect.x, rect.y, rect.w, 28.0);
        unsafe {
            if let Ok(brush) = gfx.solid(1.0, 1.0, 1.0, 0.9) {
                let st = crate::gfx::text::TextStyle {
                    size: 12.0,
                    weight: 600,
                    ..Default::default()
                };
                let lay = self.text.layout(self.active_page.title(), &st, 100.0, 20.0);
                crate::gfx::text::Text::draw(&gfx.dc, &lay, hdr_rect.x + 20.0, hdr_rect.y + 6.0, &brush);
            }

            // Pin button
            let pin_rect = Rect::new(hdr_rect.x + hdr_rect.w - 36.0, hdr_rect.y + 5.0, 18.0, 18.0);
            let res_pin = button_logic(&self.ui, id_from_str("hdr_pin", 0), pin_rect, f, Some("Sematkan"));
            if res_pin.clicked {
                self.pinned = !self.pinned;
            }
            let pin_color = if self.pinned {
                (0.3, 0.79, 0.94, 1.0)
            } else if res_pin.hovered {
                (1.0, 1.0, 1.0, 1.0)
            } else {
                (0.6, 0.65, 0.75, 0.8)
            };
            if let Ok(b) = gfx.solid(pin_color.0, pin_color.1, pin_color.2, pin_color.3) {
                let _ = crate::gfx::icons::draw(&gfx.dc, &gfx.factory, &crate::gfx::icons::PIN, pin_rect.x, pin_rect.y, 16.0, &b, None);
            }
        }

        // Active page content
        let content_rect = Rect::new(rect.x, rect.y + 28.0, rect.w, rect.h - 60.0);
        match self.active_page {
            PageId::Today => pages::today::layout(self, gfx, f, content_rect),
            PageId::Music => pages::music::layout(self, gfx, f, content_rect),
            PageId::Focus => pages::focus::layout(self, gfx, f, content_rect),
            PageId::Todo => pages::todo::layout(self, gfx, f, content_rect),
            PageId::Water => pages::water::layout(self, gfx, f, content_rect),
            PageId::Shelf => pages::shelf::layout(self, gfx, f, content_rect),
            PageId::Chat => pages::chat::layout(self, gfx, f, content_rect),
        }

        // Bottom nav tabs (32px)
        let nav_rect = Rect::new(rect.x, rect.y + rect.h - 32.0, rect.w, 32.0);
        self.render_page_nav(gfx, f, nav_rect);
    }

    fn render_page_nav(&mut self, gfx: &mut Gfx, f: &mut Frame, rect: Rect) {
        let tabs = [
            (PageId::Today, &crate::gfx::icons::NAV_TODAY),
            (PageId::Music, &crate::gfx::icons::NAV_MUSIC),
            (PageId::Focus, &crate::gfx::icons::NAV_FOCUS),
            (PageId::Todo, &crate::gfx::icons::NAV_TODO),
            (PageId::Water, &crate::gfx::icons::NAV_WATER),
            (PageId::Shelf, &crate::gfx::icons::NAV_SHELF),
            (PageId::Chat, &crate::gfx::icons::NAV_CHAT),
        ];

        let tab_w = (rect.w - 36.0) / tabs.len() as f32;
        let mut x = rect.x + 18.0;

        for (p, icon) in tabs {
            let is_active = self.active_page == p;
            let tab_rect = Rect::new(x, rect.y + 3.0, tab_w - 4.0, 26.0);
            let res = button_logic(&self.ui, id_from_str("nav_tab", p as usize), tab_rect, f, Some(p.title()));
            if res.clicked {
                self.active_page = p;
                self.store.page = p.as_str().to_string();
                self.saver.mark_dirty();
            }

            unsafe {
                if is_active {
                    if let Ok(b) = gfx.solid(0.1, 0.72, 1.0, 0.2) {
                        let geom = crate::gfx::svg::rounded_rect(tab_rect.x, tab_rect.y, tab_rect.w, tab_rect.h, 10.0);
                        if let Ok(g) = crate::gfx::svg::build_geometry(&gfx.factory, &geom, true) {
                            gfx.dc.FillGeometry(&g, &b, None);
                        }
                    }
                }
                let icon_color = if is_active {
                    (0.3, 0.8, 1.0, 1.0)
                } else if res.hovered {
                    (1.0, 1.0, 1.0, 1.0)
                } else {
                    (0.55, 0.6, 0.7, 0.8)
                };
                if let Ok(b) = gfx.solid(icon_color.0, icon_color.1, icon_color.2, icon_color.3) {
                    let _ = crate::gfx::icons::draw(&gfx.dc, &gfx.factory, icon, tab_rect.x + (tab_rect.w - 16.0) / 2.0, tab_rect.y + 5.0, 16.0, &b, None);
                }
            }

            x += tab_w;
        }
    }
}
