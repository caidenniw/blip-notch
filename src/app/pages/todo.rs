//! Todo page layout.
use crate::app::App;
use crate::gfx::icons::{self, CHECK, CLOSE};
use crate::gfx::text::TextStyle;
use crate::gfx::Gfx;
use crate::ui::{button_logic, id_from_str, Frame, Rect};

pub fn layout(app: &mut App, gfx: &mut Gfx, f: &mut Frame, rect: Rect) {
    let todos = &app.store.todos;
    let open_count = todos.iter().filter(|t| !t.done).count();
    let header_text = format!("Tugas ({})", open_count);

    unsafe {
        if let Ok(brush) = gfx.solid(1.0, 1.0, 1.0, 0.95) {
            let st = TextStyle { size: 13.0, weight: 600, ..Default::default() };
            let lay = app.text.layout(&header_text, &st, rect.w - 40.0, 20.0);
            crate::gfx::text::Text::draw(&gfx.dc, &lay, rect.x + 20.0, rect.y + 10.0, &brush);
        }

        // Render top few todo items
        let mut y = rect.y + 34.0;
        let mut clicked_toggle = None;
        let mut clicked_remove = None;

        for (idx, item) in todos.iter().enumerate().take(3) {
            let row_rect = Rect::new(rect.x + 20.0, y, rect.w - 40.0, 22.0);
            let chk_rect = Rect::new(row_rect.x, row_rect.y + 2.0, 16.0, 16.0);
            let res_chk = button_logic(&app.ui, id_from_str("td_chk", idx), chk_rect, f, None);
            if res_chk.clicked {
                clicked_toggle = Some(idx);
            }

            if let Ok(b) = gfx.solid(1.0, 1.0, 1.0, if item.done { 0.8 } else { 0.2 }) {
                let geom = crate::gfx::svg::rounded_rect(chk_rect.x, chk_rect.y, chk_rect.w, chk_rect.h, 4.0);
                if let Ok(g) = crate::gfx::svg::build_geometry(&gfx.factory, &geom, !item.done) {
                    if item.done {
                        gfx.dc.FillGeometry(&g, &b, None);
                        if let Ok(fg) = gfx.solid(0.0, 0.0, 0.0, 1.0) {
                            let _ = icons::draw(&gfx.dc, &gfx.factory, &CHECK, chk_rect.x, chk_rect.y, 16.0, &fg, None);
                        }
                    } else {
                        gfx.dc.DrawGeometry(&g, &b, 1.5, None);
                    }
                }
            }

            let text_color = if item.done { (0.4, 0.45, 0.55, 1.0) } else { (0.9, 0.9, 0.95, 1.0) };
            if let Ok(tb) = gfx.solid(text_color.0, text_color.1, text_color.2, text_color.3) {
                let st_txt = TextStyle { size: 12.0, weight: 400, ellipsis: true, ..Default::default() };
                let lay_txt = app.text.layout(&item.text, &st_txt, row_rect.w - 50.0, 20.0);
                crate::gfx::text::Text::draw(&gfx.dc, &lay_txt, row_rect.x + 24.0, row_rect.y + 2.0, &tb);
            }

            // Remove button
            let del_rect = Rect::new(row_rect.x + row_rect.w - 18.0, row_rect.y + 2.0, 16.0, 16.0);
            let res_del = button_logic(&app.ui, id_from_str("td_del", idx), del_rect, f, Some("Hapus"));
            if res_del.clicked {
                clicked_remove = Some(idx);
            }
            if let Ok(db) = gfx.solid(1.0, 0.4, 0.4, if res_del.hovered { 0.9 } else { 0.4 }) {
                let _ = icons::draw(&gfx.dc, &gfx.factory, &CLOSE, del_rect.x, del_rect.y, 16.0, &db, None);
            }

            y += 24.0;
        }

        if let Some(i) = clicked_toggle {
            if let Some(item) = app.store.todos.get_mut(i) {
                item.done = !item.done;
                app.saver.mark_dirty();
            }
        }
        if let Some(i) = clicked_remove {
            if i < app.store.todos.len() {
                app.store.todos.remove(i);
                app.saver.mark_dirty();
            }
        }
    }
}
