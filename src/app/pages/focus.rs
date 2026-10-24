//! Focus page layout.
use crate::app::App;
use crate::gfx::text::TextStyle;
use crate::gfx::Gfx;
use crate::logic::fmt;
use crate::ui::{button_logic, id_from_str, Frame, Rect};

pub fn layout(app: &mut App, gfx: &mut Gfx, f: &mut Frame, rect: Rect) {
    let focus = &app.store.focus;
    let time_str = fmt(focus.remaining);
    let phase_title = match focus.phase.as_str() {
        "short" => "Istirahat Pendek",
        "long" => "Istirahat Panjang",
        _ => "Sesi Fokus",
    };

    unsafe {
        if let Ok(brush) = gfx.solid(1.0, 1.0, 1.0, 0.95) {
            let st_ph = TextStyle {
                size: 13.0,
                weight: 600,
                ..Default::default()
            };
            let lay_ph = app.text.layout(phase_title, &st_ph, rect.w - 40.0, 20.0);
            crate::gfx::text::Text::draw(&gfx.dc, &lay_ph, rect.x + 20.0, rect.y + 12.0, &brush);

            let st_time = TextStyle {
                size: 32.0,
                weight: 700,
                tabular: true,
                ..Default::default()
            };
            let lay_time = app.text.layout(&time_str, &st_time, 200.0, 40.0);
            crate::gfx::text::Text::draw(&gfx.dc, &lay_time, rect.x + 20.0, rect.y + 36.0, &brush);
        }

        // Action buttons: Start/Pause, Reset, Skip
        let is_running = focus.running_until.is_some();
        let btn_toggle_rect = Rect::new(rect.x + rect.w - 140.0, rect.y + 38.0, 60.0, 28.0);
        let res_toggle = button_logic(&app.ui, id_from_str("f_toggle", 0), btn_toggle_rect, f, None);
        if res_toggle.clicked {
            if is_running {
                app.store.focus.running_until = None;
            } else {
                app.store.focus.running_until = Some(crate::time::now_ms() + (focus.remaining as i64 * 1000));
            }
            app.saver.mark_dirty();
        }

        if let Ok(b) = gfx.solid(0.2, 0.6, 1.0, if res_toggle.hovered { 0.9 } else { 0.7 }) {
            let geom = crate::gfx::svg::rounded_rect(btn_toggle_rect.x, btn_toggle_rect.y, btn_toggle_rect.w, btn_toggle_rect.h, 6.0);
            if let Ok(g) = crate::gfx::svg::build_geometry(&gfx.factory, &geom, true) {
                gfx.dc.FillGeometry(&g, &b, None);
            }
            if let Ok(fg) = gfx.solid(1.0, 1.0, 1.0, 1.0) {
                let lbl = if is_running { "Jeda" } else { "Mulai" };
                let st = TextStyle { size: 12.0, weight: 600, ..Default::default() };
                let lay = app.text.layout(lbl, &st, btn_toggle_rect.w, btn_toggle_rect.h);
                crate::gfx::text::Text::draw(&gfx.dc, &lay, btn_toggle_rect.x + 14.0, btn_toggle_rect.y + 6.0, &fg);
            }
        }
    }
}
