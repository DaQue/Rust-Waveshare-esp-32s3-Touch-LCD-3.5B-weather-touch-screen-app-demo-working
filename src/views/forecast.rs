use crate::framebuffer::Framebuffer;
use crate::layout::*;
use crate::views::AppState;
use embedded_graphics::{
    mono_font::MonoTextStyle,
    prelude::*,
    text::{Alignment, Text},
};
use profont::{PROFONT_10_POINT, PROFONT_12_POINT, PROFONT_14_POINT, PROFONT_24_POINT};

pub fn draw(fb: &mut Framebuffer, state: &AppState) {
    let (screen_w, screen_h) = screen_size(state.orientation);
    fb.clear_color(BG_FORECAST);
    draw_hline(fb, HEADER_LINE_Y, LINE_COLOR_1);

    draw_daily(fb, state);

    // Bottom hint
    let hint_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_BOTTOM);
    Text::with_alignment(
        "(swipe <-/-> or tap header to switch pages)",
        Point::new(screen_w / 2, screen_h - 4),
        hint_style,
        Alignment::Center,
    )
    .draw(fb)
    .ok();
}

fn draw_daily(fb: &mut Framebuffer, state: &AppState) {
    let (screen_w, screen_h) = screen_size(state.orientation);
    let header_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_HEADER);
    Text::new("Forecast", Point::new(14, 24), header_style)
        .draw(fb)
        .ok();

    // "> Main" nav on right
    let nav_style = MonoTextStyle::new(&PROFONT_12_POINT, TEXT_CONDITION);
    Text::with_alignment(
        "Main >",
        Point::new(screen_w - 10, 24),
        nav_style,
        Alignment::Right,
    )
    .draw(fb)
    .ok();

    let card_w = screen_w - 2 * CARD_MARGIN;
    let portrait = state.orientation.is_portrait();
    // Portrait: scale rows to fill 480px screen.  Landscape: compact 60px rows.
    let row_h = if portrait {
        (screen_h - 38 - 20) / FORECAST_ROWS as i32 - 6
    } else {
        60
    };
    let row_stride = row_h + 6;
    let row_y_base = 38;

    // Draw row cards
    for i in 0..FORECAST_ROWS {
        let y = row_y_base + (i as i32) * row_stride;
        draw_card(
            fb,
            CARD_MARGIN,
            y,
            card_w,
            row_h,
            CARD_RADIUS as u32,
            CARD_FILL_FORECAST,
            CARD_BORDER_FORECAST,
            1,
        );
    }

    if let Some(forecast) = &state.forecast {
        let title_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_SECONDARY);
        let detail_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_DETAIL);
        let temp_style = MonoTextStyle::new(&PROFONT_24_POINT, TEXT_PRIMARY);
        for (i, row) in forecast.rows.iter().take(FORECAST_ROWS).enumerate() {
            let y = row_y_base + (i as i32) * row_stride;
            // Proportional offsets — work for any row_h ≥ 60
            let icon_dy = row_h / 5; // ~20% from top
            let title_dy = row_h * 2 / 5; // ~40%  (PROFONT_14 baseline=13)
            let detail_dy = row_h * 4 / 5; // ~80%  (PROFONT_10 baseline=9)
            let temp_dy = (row_h * 3 / 5).max(38); // ~60%  (PROFONT_24 baseline=24)

            row.icon.draw_36(fb, CARD_MARGIN + 8, y + icon_dy);

            Text::new(
                &row.title,
                Point::new(CARD_MARGIN + 52, y + title_dy),
                title_style,
            )
            .draw(fb)
            .ok();
            Text::new(
                &row.detail,
                Point::new(CARD_MARGIN + 52, y + detail_dy),
                detail_style,
            )
            .draw(fb)
            .ok();
            Text::with_alignment(
                &row.temp_text,
                Point::new(screen_w - CARD_MARGIN - 14, y + temp_dy),
                temp_style,
                Alignment::Right,
            )
            .draw(fb)
            .ok();
        }
    } else {
        let placeholder_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_DETAIL);
        Text::new(
            "No forecast data",
            Point::new(60, screen_h / 2),
            placeholder_style,
        )
        .draw(fb)
        .ok();
    }
}
