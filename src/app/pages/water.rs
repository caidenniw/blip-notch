//! Water page layout.
use crate::app::App;
use crate::gfx::text::TextStyle;
use crate::gfx::Gfx;
use crate::logic::WaterEntry;
use crate::ui::{button_logic, id_from_str, Frame, Rect};

pub fn layout(app: &mut App, gfx: &mut Gfx, f: &mut Frame, rect: Rect) {
    let current_ml: u32 = app
        .store
        .water
        .log
        .get(&crate::time::day_key_now())
        .map(|v| v.iter().map(|e| e.ml).sum())
        .unwrap_or(0);
    let goal_ml = app.store.water.goal;
    let cup = app.store.water.cup;
    let big_str = format!("{:.2} / {:.1} L", current_ml as f64 / 1000.0, goal_ml as f64 / 1000.0);
    let pct = (current_ml as f32 / goal_ml.max(1) as f32).clamp(0.0, 1.0);
    let pct_str = format!("{}% hari ini", (pct * 100.0).round() as u32);

    unsafe {
        if let Ok(brush) = gfx.solid(1.0, 1.0, 1.0, 0.95) {
            let st_big = TextStyle { size: 19.0, weight: 700, tabular: true, ..Default::default() };
            let lay_big = app.text.layout(&big_str, &st_big, 200.0, 24.0);
            crate::gfx::text::Text::draw(&gfx.dc, &lay_big, rect.x + 20.0, rect.y + 12.0, &brush);

            if let Ok(mist) = gfx.solid(0.6, 0.65, 0.75, 1.0) {
                let st_pct = TextStyle { size: 11.0, weight: 400, ..Default::default() };
                let lay_pct = app.text.layout(&pct_str, &st_pct, 150.0, 16.0);
                crate::gfx::text::Text::draw(&gfx.dc, &lay_pct, rect.x + 20.0, rect.y + 36.0, &mist);
            }
        }

        // Progress bar
        let bar_rect = Rect::new(rect.x + 20.0, rect.y + 56.0, rect.w - 40.0, 6.0);
        if let Ok(bg) = gfx.solid(1.0, 1.0, 1.0, 0.1) {
            let geom = crate::gfx::svg::rounded_rect(bar_rect.x, bar_rect.y, bar_rect.w, bar_rect.h, 3.0);
            if let Ok(g) = crate::gfx::svg::build_geometry(&gfx.factory, &geom, true) {
                gfx.dc.FillGeometry(&g, &bg, None);
            }
        }
        if pct > 0.0 {
            if let Ok(fill) = gfx.solid(0.3, 0.8, 1.0, 0.9) {
                let fill_geom = crate::gfx::svg::rounded_rect(bar_rect.x, bar_rect.y, bar_rect.w * pct, bar_rect.h, 3.0);
                if let Ok(g) = crate::gfx::svg::build_geometry(&gfx.factory, &fill_geom, true) {
                    gfx.dc.FillGeometry(&g, &fill, None);
                }
            }
        }

        // Add drink button "+250 ml"
        let btn_add_rect = Rect::new(rect.x + rect.w - 120.0, rect.y + 16.0, 80.0, 26.0);
        let res_add = button_logic(&app.ui, id_from_str("w_add", 0), btn_add_rect, f, None);
        if res_add.clicked {
            let key = crate::time::day_key_now();
            let entry = WaterEntry { t: crate::time::now_ms(), ml: cup };
            app.store.water.log.entry(key).or_default().push(entry);
            app.saver.mark_dirty();
        }

        if let Ok(b) = gfx.solid(0.2, 0.65, 0.95, if res_add.hovered { 0.9 } else { 0.7 }) {
            let geom = crate::gfx::svg::rounded_rect(btn_add_rect.x, btn_add_rect.y, btn_add_rect.w, btn_add_rect.h, 6.0);
            if let Ok(g) = crate::gfx::svg::build_geometry(&gfx.factory, &geom, true) {
                gfx.dc.FillGeometry(&g, &b, None);
            }
            if let Ok(fg) = gfx.solid(1.0, 1.0, 1.0, 1.0) {
                let lbl = format!("+{} ml", cup);
                let st = TextStyle { size: 12.0, weight: 600, ..Default::default() };
                let lay = app.text.layout(&lbl, &st, btn_add_rect.w, btn_add_rect.h);
                crate::gfx::text::Text::draw(&gfx.dc, &lay, btn_add_rect.x + 12.0, btn_add_rect.y + 5.0, &fg);
            }
        }
    }
}
