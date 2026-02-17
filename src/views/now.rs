use embedded_graphics::{
    mono_font::MonoTextStyle,
    prelude::*,
    text::Text,
};
use profont::{PROFONT_10_POINT, PROFONT_12_POINT, PROFONT_14_POINT, PROFONT_24_POINT};

use crate::framebuffer::Framebuffer;
use crate::layout::*;
use crate::views::AppState;

pub fn draw(fb: &mut Framebuffer, state: &AppState) {
    // 1. Fill background
    fb.clear_color(BG_NOW);

    // 2. Header lines
    draw_hline(fb, HEADER_LINE_Y, LINE_COLOR_1);
    draw_hline(fb, HEADER_LINE2_Y, LINE_COLOR_2);

    // 3. Main card
    draw_card(
        fb,
        CARD_MARGIN,
        NOW_CARD_Y,
        SCREEN_W - 2 * CARD_MARGIN,
        NOW_CARD_H,
        CARD_RADIUS as u32,
        CARD_FILL_NOW,
        CARD_BORDER_NOW,
        2,
    );

    // 4. Weather icon (80x80 BMP)
    let icon = state
        .current_weather
        .as_ref()
        .map(|w| w.icon)
        .unwrap_or_default();
    icon.draw_80(fb, NOW_ICON_X, NOW_CARD_Y + 10);

    // 5. Current weather text
    if let Some(cw) = &state.current_weather {
        let temp_text = format!("{:.0}°F", cw.temp_f);
        let temp_style = MonoTextStyle::new(&PROFONT_24_POINT, TEXT_PRIMARY);
        Text::new(&temp_text, Point::new(NOW_TEMP_X, 100), temp_style)
            .draw(fb)
            .ok();

        let cond_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_CONDITION);
        Text::new(&cw.condition, Point::new(NOW_CONDITION_X, NOW_CONDITION_Y), cond_style)
            .draw(fb)
            .ok();

        let desc_style = MonoTextStyle::new(&PROFONT_12_POINT, TEXT_WEATHER);
        let desc = format!("{:.0}°F / {}", cw.feels_f, icon.label());
        Text::new(&desc, Point::new(NOW_WEATHER_X, NOW_WEATHER_Y), desc_style)
            .draw(fb)
            .ok();
    }

    // 6. Time text in header
    let time_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_HEADER);
    Text::new(&state.time_text, Point::new(14, 26), time_style)
        .draw(fb)
        .ok();

    // 7. Status text (right area of header)
    let status_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_STATUS);
    Text::new(&state.status_text, Point::new(SCREEN_W - 240, 26), status_style)
        .draw(fb)
        .ok();

    // 8. Divider line
    draw_hline(fb, NOW_DIVIDER_Y, LINE_COLOR_2);

    // 9. Preview cards (3 forecast day previews)
    let card_w = (SCREEN_W - 2 * CARD_MARGIN - 2 * PREVIEW_GAP) / 3;
    let preview_style = MonoTextStyle::new(&PROFONT_12_POINT, TEXT_TERTIARY);

    for i in 0..PREVIEW_DAYS {
        let x = CARD_MARGIN + i as i32 * (card_w + PREVIEW_GAP);
        draw_card(
            fb,
            x,
            PREVIEW_Y,
            card_w,
            PREVIEW_H,
            10,
            CARD_FILL_FORECAST_PREVIEW,
            CARD_BORDER_FORECAST_PREVIEW,
            1,
        );

        // Draw preview text from forecast if available
        if let Some(fc) = &state.forecast {
            if let Some(row) = fc.rows.get(i) {
                let label = format!("{} {}°", row.title, row.temp_f);
                Text::new(&label, Point::new(x + 8, PREVIEW_Y + 30), preview_style)
                    .draw(fb)
                    .ok();

                // Small icon in preview card (if card tall enough)
                row.icon.draw_36(fb, x + card_w - 42, PREVIEW_Y + 7);
            }
        }
    }

    // 10. Stats lines
    let detail_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_DETAIL);
    if let Some(cw) = &state.current_weather {
        let line1 = format!(
            "Hum {}%  Wind {:.0}mph  Feels {:.0}°F  {} hPa",
            cw.humidity, cw.wind_mph, cw.feels_f, cw.pressure_hpa
        );
        Text::new(&line1, Point::new(NOW_STATS_X, NOW_STATS_Y1), detail_style)
            .draw(fb)
            .ok();

        let line2 = format!("{}, {}", cw.city, cw.country);
        Text::new(&line2, Point::new(NOW_STATS_X, NOW_STATS_Y2), detail_style)
            .draw(fb)
            .ok();
    } else {
        Text::new("No weather data", Point::new(NOW_STATS_X, NOW_STATS_Y1), detail_style)
            .draw(fb)
            .ok();
    }

}
