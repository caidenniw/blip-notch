//! Chat page layout.
use crate::app::App;
use crate::gfx::icons::{self, MIC, SEND};
use crate::gfx::text::TextStyle;
use crate::gfx::Gfx;
use crate::ui::{button_logic, id_from_str, Frame, Rect};

pub fn layout(app: &mut App, gfx: &mut Gfx, f: &mut Frame, rect: Rect) {
    let chat = &app.store.chat;

    unsafe {
        // Render last message if any
        if let Some(last) = chat.last() {
            let is_user = last.role == "user";
            let msg_rect = Rect::new(rect.x + 20.0, rect.y + 10.0, rect.w - 40.0, 36.0);
            if let Ok(b) = gfx.solid(1.0, 1.0, 1.0, if is_user { 0.12 } else { 0.06 }) {
                let geom = crate::gfx::svg::rounded_rect(msg_rect.x, msg_rect.y, msg_rect.w, msg_rect.h, 8.0);
                if let Ok(g) = crate::gfx::svg::build_geometry(&gfx.factory, &geom, true) {
                    gfx.dc.FillGeometry(&g, &b, None);
                }
            }
            if let Ok(brush) = gfx.solid(1.0, 1.0, 1.0, 0.95) {
                let st = TextStyle { size: 12.0, weight: 400, ellipsis: true, ..Default::default() };
                let lay = app.text.layout(&last.content, &st, msg_rect.w - 16.0, 24.0);
                crate::gfx::text::Text::draw(&gfx.dc, &lay, msg_rect.x + 8.0, msg_rect.y + 8.0, &brush);
            }
        } else {
            if let Ok(mist) = gfx.solid(0.6, 0.65, 0.75, 0.7) {
                let st = TextStyle { size: 12.0, weight: 400, ..Default::default() };
                let lay = app.text.layout("Mulai percakapan dengan Blip...", &st, rect.w - 40.0, 20.0);
                crate::gfx::text::Text::draw(&gfx.dc, &lay, rect.x + 20.0, rect.y + 16.0, &mist);
            }
        }

        // Composer box
        let comp_rect = Rect::new(rect.x + 20.0, rect.y + rect.h - 44.0, rect.w - 40.0, 32.0);
        if let Ok(b) = gfx.solid(1.0, 1.0, 1.0, 0.08) {
            let geom = crate::gfx::svg::rounded_rect(comp_rect.x, comp_rect.y, comp_rect.w, comp_rect.h, 16.0);
            if let Ok(g) = crate::gfx::svg::build_geometry(&gfx.factory, &geom, true) {
                gfx.dc.FillGeometry(&g, &b, None);
            }
        }

        let input_rect = Rect::new(comp_rect.x + 12.0, comp_rect.y + 6.0, comp_rect.w - 74.0, 20.0);
        let id_input = id_from_str("chat_input", 0);
        f.register(id_input, input_rect, None);

        let text_content = app.ui.text_edits.get(&id_input).map(|e| e.text.clone()).unwrap_or_default();
        if text_content.is_empty() {
            if let Ok(mist) = gfx.solid(0.5, 0.55, 0.65, 0.8) {
                let st = TextStyle { size: 12.0, weight: 400, ..Default::default() };
                let lay = app.text.layout("Tulis atau tekan Ctrl Kanan...", &st, input_rect.w, input_rect.h);
                crate::gfx::text::Text::draw(&gfx.dc, &lay, input_rect.x, input_rect.y, &mist);
            }
        } else {
            if let Ok(brush) = gfx.solid(1.0, 1.0, 1.0, 1.0) {
                let st = TextStyle { size: 12.0, weight: 400, ..Default::default() };
                let lay = app.text.layout(&text_content, &st, input_rect.w, input_rect.h);
                crate::gfx::text::Text::draw(&gfx.dc, &lay, input_rect.x, input_rect.y, &brush);
            }
        }

        // Mic & Send buttons
        let mic_rect = Rect::new(comp_rect.x + comp_rect.w - 56.0, comp_rect.y + 4.0, 24.0, 24.0);
        let res_mic = button_logic(&app.ui, id_from_str("chat_mic", 0), mic_rect, f, Some("Bicara"));
        if res_mic.clicked {
            let _ = crate::rec::rec_start("chat".into());
        }
        if let Ok(b) = gfx.solid(1.0, 1.0, 1.0, if res_mic.hovered { 1.0 } else { 0.7 }) {
            let _ = icons::draw(&gfx.dc, &gfx.factory, &MIC, mic_rect.x, mic_rect.y, 24.0, &b, None);
        }

        let send_rect = Rect::new(comp_rect.x + comp_rect.w - 28.0, comp_rect.y + 4.0, 24.0, 24.0);
        let res_send = button_logic(&app.ui, id_from_str("chat_send", 0), send_rect, f, Some("Kirim"));
        let mut to_send = None;
        if res_send.clicked {
            if let Some(edit) = app.ui.text_edits.get_mut(&id_input) {
                let t = edit.text.trim().to_string();
                if !t.is_empty() {
                    edit.text.clear();
                    edit.cursor = 0;
                    to_send = Some(t);
                }
            }
        }
        if let Some(t) = to_send {
            app.send_chat(t);
        }
        if let Ok(b) = gfx.solid(0.3, 0.8, 1.0, if res_send.hovered { 1.0 } else { 0.8 }) {
            let _ = icons::draw(&gfx.dc, &gfx.factory, &SEND, send_rect.x, send_rect.y, 24.0, &b, None);
        }
    }
}
