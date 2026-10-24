//! Shelf page layout.
use crate::app::App;
use crate::gfx::icons::{self, FILE_FOLDER, FILE_GENERIC};
use crate::gfx::text::TextStyle;
use crate::gfx::Gfx;
use crate::logic::fmt_size;
use crate::ui::{button_logic, id_from_str, Frame, Rect};

pub fn layout(app: &mut App, gfx: &mut Gfx, f: &mut Frame, rect: Rect) {
    let shelf = &app.store.shelf;

    unsafe {
        if shelf.is_empty() {
            // Drop zone placeholder
            let zone_rect = Rect::new(rect.x + 20.0, rect.y + 16.0, rect.w - 40.0, rect.h - 32.0);
            if let Ok(b) = gfx.solid(1.0, 1.0, 1.0, 0.05) {
                let geom = crate::gfx::svg::rounded_rect(zone_rect.x, zone_rect.y, zone_rect.w, zone_rect.h, 8.0);
                if let Ok(g) = crate::gfx::svg::build_geometry(&gfx.factory, &geom, true) {
                    gfx.dc.FillGeometry(&g, &b, None);
                }
            }

            if let Ok(brush) = gfx.solid(1.0, 1.0, 1.0, 0.8) {
                let st = TextStyle { size: 13.0, weight: 600, ..Default::default() };
                let lay = app.text.layout("Tarik file ke sini", &st, zone_rect.w, 20.0);
                crate::gfx::text::Text::draw(&gfx.dc, &lay, zone_rect.x + zone_rect.w / 2.0 - 50.0, zone_rect.y + 20.0, &brush);
            }
            if let Ok(mist) = gfx.solid(0.6, 0.65, 0.75, 0.8) {
                let st = TextStyle { size: 11.0, weight: 400, ..Default::default() };
                let lay = app.text.layout("atau tempel file dari clipboard", &st, zone_rect.w, 20.0);
                crate::gfx::text::Text::draw(&gfx.dc, &lay, zone_rect.x + zone_rect.w / 2.0 - 75.0, zone_rect.y + 40.0, &mist);
            }
        } else {
            let mut x = rect.x + 20.0;
            let mut remove_idx = None;

            for (idx, item) in shelf.iter().enumerate().take(4) {
                let card_rect = Rect::new(x, rect.y + 12.0, 78.0, 68.0);
                let res = button_logic(&app.ui, id_from_str("sh_card", idx), card_rect, f, Some(&item.name));
                if res.clicked {
                    let _ = crate::sys::open_path(item.path.clone());
                }

                if let Ok(b) = gfx.solid(1.0, 1.0, 1.0, if res.hovered { 0.12 } else { 0.06 }) {
                    let geom = crate::gfx::svg::rounded_rect(card_rect.x, card_rect.y, card_rect.w, card_rect.h, 6.0);
                    if let Ok(g) = crate::gfx::svg::build_geometry(&gfx.factory, &geom, true) {
                        gfx.dc.FillGeometry(&g, &b, None);
                    }
                }

                // File icon
                let icon = if item.dir { &FILE_FOLDER } else { &FILE_GENERIC };
                if let Ok(b) = gfx.solid(0.4, 0.8, 1.0, 0.8) {
                    let _ = icons::draw(&gfx.dc, &gfx.factory, icon, card_rect.x + 27.0, card_rect.y + 8.0, 24.0, &b, None);
                }

                // File name & size
                if let Ok(b) = gfx.solid(1.0, 1.0, 1.0, 0.9) {
                    let st = TextStyle { size: 10.0, weight: 500, ellipsis: true, ..Default::default() };
                    let lay = app.text.layout(&item.name, &st, card_rect.w - 8.0, 16.0);
                    crate::gfx::text::Text::draw(&gfx.dc, &lay, card_rect.x + 4.0, card_rect.y + 36.0, &b);
                }
                if let Ok(mist) = gfx.solid(0.5, 0.55, 0.65, 0.8) {
                    let sz = fmt_size(item.size);
                    let st = TextStyle { size: 9.0, weight: 400, tabular: true, ..Default::default() };
                    let lay = app.text.layout(&sz, &st, card_rect.w - 8.0, 14.0);
                    crate::gfx::text::Text::draw(&gfx.dc, &lay, card_rect.x + 4.0, card_rect.y + 50.0, &mist);
                }

                x += 84.0;
            }

            if let Some(i) = remove_idx {
                if i < app.store.shelf.len() {
                    app.store.shelf.remove(i);
                    app.saver.mark_dirty();
                }
            }
        }
    }
}
