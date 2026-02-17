use embedded_graphics::{
    mono_font::MonoTextStyle,
    prelude::*,
    primitives::{PrimitiveStyleBuilder, Rectangle},
    text::Text,
};
use profont::{PROFONT_24_POINT, PROFONT_14_POINT, PROFONT_10_POINT};
use crate::framebuffer::Framebuffer;
use crate::layout::*;
use crate::views::AppState;

pub fn draw(fb: &mut Framebuffer, state: &AppState) {
    // Fill background
    let bg_style = PrimitiveStyleBuilder::new().fill_color(BG_FORECAST).build();
    Rectangle::new(Point::zero(), Size::new(SCREEN_W as u32, SCREEN_H as u32))
        .into_styled(bg_style)
        .draw(fb)
        .ok();

    // Header line
    draw_hline(fb, HEADER_LINE_Y, LINE_COLOR_1);

    // Header text
    let header_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_HEADER);
    Text::new("Forecast", Point::new(14, 26), header_style)
        .draw(fb)
        .ok();

    // Draw 4 row cards
    let card_w = SCREEN_W - 20;
    for i in 0..FORECAST_ROWS {
        let y = FORECAST_ROW_Y_BASE + (i as i32) * FORECAST_ROW_STRIDE;
        draw_card(
            fb,
            CARD_MARGIN, y,
            card_w, FORECAST_ROW_H,
            CARD_RADIUS as u32,
            CARD_FILL_FORECAST, CARD_BORDER_FORECAST, 1,
        );
    }

    if let Some(forecast) = &state.forecast {
        for (i, row) in forecast.rows.iter().take(FORECAST_ROWS).enumerate() {
            let y = FORECAST_ROW_Y_BASE + (i as i32) * FORECAST_ROW_STRIDE;

            // Icon (36x36 BMP)
            row.icon.draw_36(fb, FORECAST_ICON_X, y + 7);

            // Title (weekday)
            let title_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_SECONDARY);
            Text::new(&row.title, Point::new(FORECAST_TITLE_X, y + 22), title_style)
                .draw(fb)
                .ok();

            // Detail line
            let detail_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_DETAIL);
            Text::new(&row.detail, Point::new(FORECAST_DETAIL_X, y + 44), detail_style)
                .draw(fb)
                .ok();

            // Temperature
            let temp_style = MonoTextStyle::new(&PROFONT_24_POINT, TEXT_PRIMARY);
            Text::new(&row.temp_text, Point::new(SCREEN_W - 90, y + 30), temp_style)
                .draw(fb)
                .ok();
        }
    } else {
        // No forecast data placeholder
        let placeholder_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_DETAIL);
        Text::new("No forecast data", Point::new(60, SCREEN_H / 2), placeholder_style)
            .draw(fb)
            .ok();
    }

    // Bottom text
    let bottom_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_BOTTOM);
    Text::new(&state.bottom_text, Point::new(BOTTOM_X, BOTTOM_Y), bottom_style)
        .draw(fb)
        .ok();
}
