//! Today page layout.
use crate::app::App;
use crate::gfx::text::TextStyle;
use crate::gfx::Gfx;
use crate::logic::greeting;
use crate::time::{id_date_short, local_now};
use crate::ui::{Frame, Rect};

pub fn layout(app: &mut App, gfx: &mut Gfx, f: &mut Frame, rect: Rect) {
    let loc = local_now();
    let greet = format!("{}, {}", greeting(loc.hour), "Deni");
    let time_str = format!("{:02}:{:02}", loc.hour, loc.minute);
    let date_str = id_date_short(loc.year, loc.month, loc.day, loc.weekday);

    unsafe {
        if let Ok(brush) = gfx.solid(1.0, 1.0, 1.0, 0.95) {
            let style_greet = TextStyle {
                size: 13.0,
                weight: 600,
                ..Default::default()
            };
            let lay = app.text.layout(&greet, &style_greet, rect.w - 40.0, 20.0);
            crate::gfx::text::Text::draw(&gfx.dc, &lay, rect.x + 20.0, rect.y + 10.0, &brush);

            let style_time = TextStyle {
                size: 28.0,
                weight: 700,
                tabular: true,
                ..Default::default()
            };
            let lay_time = app.text.layout(&time_str, &style_time, 200.0, 36.0);
            crate::gfx::text::Text::draw(&gfx.dc, &lay_time, rect.x + 20.0, rect.y + 32.0, &brush);

            if let Ok(mist) = gfx.solid(0.6, 0.65, 0.75, 1.0) {
                let style_date = TextStyle {
                    size: 12.0,
                    weight: 400,
                    ..Default::default()
                };
                let lay_date = app.text.layout(&date_str, &style_date, 200.0, 20.0);
                crate::gfx::text::Text::draw(&gfx.dc, &lay_date, rect.x + 120.0, rect.y + 44.0, &mist);
            }
        }

        // Summary cards
        let today_focus = app.store.focus_log.get(&crate::time::day_key_now()).copied().unwrap_or(0);
        let water_today: u32 = app
            .store
            .water
            .log
            .get(&crate::time::day_key_now())
            .map(|v| v.iter().map(|e| e.ml).sum())
            .unwrap_or(0);

        let sub = format!(
            "Fokus: {} sesi  |  Air: {}/{} ml",
            today_focus, water_today, app.store.water.goal
        );

        if let Ok(mist) = gfx.solid(0.65, 0.7, 0.8, 0.9) {
            let style_sub = TextStyle {
                size: 11.5,
                weight: 400,
                ..Default::default()
            };
            let lay_sub = app.text.layout(&sub, &style_sub, rect.w - 40.0, 20.0);
            crate::gfx::text::Text::draw(&gfx.dc, &lay_sub, rect.x + 20.0, rect.y + 76.0, &mist);
        }
    }
}
