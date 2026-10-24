//! Music page layout.
use crate::app::App;
use crate::gfx::icons::{self, CTL_NEXT, CTL_PAUSE, CTL_PLAY, CTL_PREV};
use crate::gfx::text::TextStyle;
use crate::gfx::Gfx;
use crate::logic::fmt_track;
use crate::ui::{button_logic, id_from_str, Frame, Rect};

pub fn layout(app: &mut App, gfx: &mut Gfx, f: &mut Frame, rect: Rect) {
    let m = &app.media;
    let title = if m.title.is_empty() { "Tidak ada musik" } else { &m.title };
    let artist = if m.artist.is_empty() { "Blip Notch" } else { &m.artist };

    unsafe {
        // Title & artist
        if let Ok(brush) = gfx.solid(1.0, 1.0, 1.0, 0.95) {
            let st_title = TextStyle {
                size: 13.0,
                weight: 600,
                ellipsis: true,
                ..Default::default()
            };
            let lay_title = app.text.layout(title, &st_title, rect.w - 120.0, 20.0);
            crate::gfx::text::Text::draw(&gfx.dc, &lay_title, rect.x + 20.0, rect.y + 12.0, &brush);

            if let Ok(mist) = gfx.solid(0.6, 0.65, 0.75, 1.0) {
                let st_art = TextStyle {
                    size: 11.5,
                    weight: 400,
                    ellipsis: true,
                    ..Default::default()
                };
                let lay_art = app.text.layout(artist, &st_art, rect.w - 120.0, 18.0);
                crate::gfx::text::Text::draw(&gfx.dc, &lay_art, rect.x + 20.0, rect.y + 32.0, &mist);
            }
        }

        // Progress line
        let prog_rect = Rect::new(rect.x + 20.0, rect.y + 56.0, rect.w - 40.0, 4.0);
        if let Ok(bg) = gfx.solid(1.0, 1.0, 1.0, 0.1) {
            let geom = crate::gfx::svg::rounded_rect(prog_rect.x, prog_rect.y, prog_rect.w, prog_rect.h, 2.0);
            if let Ok(g) = crate::gfx::svg::build_geometry(&gfx.factory, &geom, true) {
                gfx.dc.FillGeometry(&g, &bg, None);
            }
        }
        let pct = if m.duration > 0.0 {
            (m.position / m.duration).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        if pct > 0.0 {
            if let Ok(fill) = gfx.solid(0.3, 0.75, 1.0, 0.9) {
                let fill_geom = crate::gfx::svg::rounded_rect(prog_rect.x, prog_rect.y, prog_rect.w * pct, prog_rect.h, 2.0);
                if let Ok(g) = crate::gfx::svg::build_geometry(&gfx.factory, &fill_geom, true) {
                    gfx.dc.FillGeometry(&g, &fill, None);
                }
            }
        }

        // Time indicators
        let pos_str = fmt_track(m.position);
        let dur_str = fmt_track(m.duration);
        if let Ok(mist) = gfx.solid(0.5, 0.55, 0.65, 1.0) {
            let st_time = TextStyle {
                size: 10.0,
                weight: 400,
                tabular: true,
                ..Default::default()
            };
            let lay_pos = app.text.layout(&pos_str, &st_time, 50.0, 16.0);
            crate::gfx::text::Text::draw(&gfx.dc, &lay_pos, rect.x + 20.0, rect.y + 64.0, &mist);

            let lay_dur = app.text.layout(&dur_str, &st_time, 50.0, 16.0);
            crate::gfx::text::Text::draw(&gfx.dc, &lay_dur, rect.x + rect.w - 60.0, rect.y + 64.0, &mist);
        }

        // Media controls (Prev, Play/Pause, Next)
        let cx = rect.x + rect.w / 2.0;
        let cy = rect.y + 76.0;

        let btn_prev_rect = Rect::new(cx - 50.0, cy - 12.0, 24.0, 24.0);
        let res_prev = button_logic(&app.ui, id_from_str("m_prev", 0), btn_prev_rect, f, Some("Sebelumnya"));
        if res_prev.clicked {
            crate::media::media_cmd("prev".into(), None);
        }
        if let Ok(b) = gfx.solid(1.0, 1.0, 1.0, if res_prev.hovered { 1.0 } else { 0.7 }) {
            let _ = icons::draw(&gfx.dc, &gfx.factory, &CTL_PREV, btn_prev_rect.x, btn_prev_rect.y, 24.0, &b, None);
        }

        let btn_play_rect = Rect::new(cx - 14.0, cy - 14.0, 28.0, 28.0);
        let res_play = button_logic(&app.ui, id_from_str("m_play", 0), btn_play_rect, f, Some(if m.playing { "Jeda" } else { "Putar" }));
        if res_play.clicked {
            crate::media::media_cmd("toggle".into(), None);
        }
        let icon_play = if m.playing { &CTL_PAUSE } else { &CTL_PLAY };
        if let Ok(b) = gfx.solid(1.0, 1.0, 1.0, if res_play.hovered { 1.0 } else { 0.85 }) {
            let _ = icons::draw(&gfx.dc, &gfx.factory, icon_play, btn_play_rect.x, btn_play_rect.y, 28.0, &b, None);
        }

        let btn_next_rect = Rect::new(cx + 26.0, cy - 12.0, 24.0, 24.0);
        let res_next = button_logic(&app.ui, id_from_str("m_next", 0), btn_next_rect, f, Some("Berikutnya"));
        if res_next.clicked {
            crate::media::media_cmd("next".into(), None);
        }
        if let Ok(b) = gfx.solid(1.0, 1.0, 1.0, if res_next.hovered { 1.0 } else { 0.7 }) {
            let _ = icons::draw(&gfx.dc, &gfx.factory, &CTL_NEXT, btn_next_rect.x, btn_next_rect.y, 24.0, &b, None);
        }
    }
}
