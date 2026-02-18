use embedded_graphics::{
    mono_font::MonoTextStyle,
    prelude::*,
    text::{Alignment, Text},
};
use profont::{PROFONT_24_POINT, PROFONT_14_POINT, PROFONT_12_POINT, PROFONT_10_POINT};
use crate::framebuffer::Framebuffer;
use crate::layout::*;
use crate::views::AppState;

pub fn draw(fb: &mut Framebuffer, state: &AppState) {
    fb.clear_color(BG_FORECAST);
    draw_hline(fb, HEADER_LINE_Y, LINE_COLOR_1);

    if state.forecast_hourly_open {
        draw_hourly(fb, state);
    } else {
        draw_daily(fb, state);
    }

    // Bottom hint
    let hint_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_BOTTOM);
    Text::with_alignment(
        "(swipe <-/-> or tap header to switch pages)",
        Point::new(SCREEN_W / 2, SCREEN_H - 4),
        hint_style,
        Alignment::Center,
    )
    .draw(fb)
    .ok();
}

fn draw_daily(fb: &mut Framebuffer, state: &AppState) {
    let header_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_HEADER);
    Text::new("Forecast", Point::new(14, 24), header_style)
        .draw(fb)
        .ok();

    // "> Main" nav on right
    let nav_style = MonoTextStyle::new(&PROFONT_12_POINT, TEXT_CONDITION);
    Text::with_alignment(
        "Main >",
        Point::new(SCREEN_W - 10, 24),
        nav_style,
        Alignment::Right,
    )
    .draw(fb)
    .ok();

    let card_w = SCREEN_W - 2 * CARD_MARGIN;
    let row_h = 60;
    let row_stride = 66;
    let row_y_base = 38;

    // Draw 4 row cards
    for i in 0..FORECAST_ROWS {
        let y = row_y_base + (i as i32) * row_stride;
        draw_card(
            fb,
            CARD_MARGIN, y,
            card_w, row_h,
            CARD_RADIUS as u32,
            CARD_FILL_FORECAST, CARD_BORDER_FORECAST, 1,
        );
    }

    if let Some(forecast) = &state.forecast {
        for (i, row) in forecast.rows.iter().take(FORECAST_ROWS).enumerate() {
            let y = row_y_base + (i as i32) * row_stride;

            // Icon (36x36)
            row.icon.draw_36(fb, CARD_MARGIN + 8, y + 12);

            // Title (weekday)
            let title_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_SECONDARY);
            Text::new(&row.title, Point::new(CARD_MARGIN + 52, y + 24), title_style)
                .draw(fb)
                .ok();

            // Detail line
            let detail_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_DETAIL);
            Text::new(&row.detail, Point::new(CARD_MARGIN + 52, y + 44), detail_style)
                .draw(fb)
                .ok();

            // Temperature (right side)
            let temp_style = MonoTextStyle::new(&PROFONT_24_POINT, TEXT_PRIMARY);
            Text::with_alignment(
                &row.temp_text,
                Point::new(SCREEN_W - CARD_MARGIN - 14, y + 38),
                temp_style,
                Alignment::Right,
            )
            .draw(fb)
            .ok();
        }
    } else {
        let placeholder_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_DETAIL);
        Text::new("No forecast data", Point::new(60, SCREEN_H / 2), placeholder_style)
            .draw(fb)
            .ok();
    }
}

fn draw_hourly(fb: &mut Framebuffer, state: &AppState) {
    let header_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_HEADER);

    // Get day title
    let day_title = state.forecast.as_ref()
        .and_then(|fc| fc.rows.get(state.forecast_hourly_day))
        .map(|r| r.title.as_str())
        .unwrap_or("Hourly");

    Text::new(day_title, Point::new(14, 24), header_style)
        .draw(fb)
        .ok();

    // "X Close" nav on right
    let nav_style = MonoTextStyle::new(&PROFONT_12_POINT, TEXT_CONDITION);
    Text::with_alignment(
        "Main >",
        Point::new(SCREEN_W - 10, 24),
        nav_style,
        Alignment::Right,
    )
    .draw(fb)
    .ok();

    let card_w = SCREEN_W - 2 * CARD_MARGIN;
    let row_h = 60;
    let row_stride = 66;
    let row_y_base = 38;
    let visible_rows = 4;

    if let Some(forecast) = &state.forecast {
        if let Some(day) = forecast.days.get(state.forecast_hourly_day) {
            let scroll = state.forecast_hourly_scroll.min(
                day.entries.len().saturating_sub(visible_rows)
            );

            for (vi, entry) in day.entries.iter().skip(scroll).take(visible_rows).enumerate() {
                let y = row_y_base + (vi as i32) * row_stride;

                draw_card(
                    fb,
                    CARD_MARGIN, y,
                    card_w, row_h,
                    CARD_RADIUS as u32,
                    CARD_FILL_FORECAST, CARD_BORDER_FORECAST, 1,
                );

                // Icon (36x36)
                entry.icon.draw_36(fb, CARD_MARGIN + 8, y + 12);

                // Time label (e.g. "Tue 12AM")
                let time_label = format!("{} {}", day_title, entry.time_text);
                let title_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_SECONDARY);
                Text::new(&time_label, Point::new(CARD_MARGIN + 52, y + 24), title_style)
                    .draw(fb)
                    .ok();

                // Detail (feels + wind)
                let detail_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_DETAIL);
                Text::new(&entry.detail, Point::new(CARD_MARGIN + 52, y + 44), detail_style)
                    .draw(fb)
                    .ok();

                // Temperature (right)
                let temp_style = MonoTextStyle::new(&PROFONT_24_POINT, TEXT_PRIMARY);
                Text::with_alignment(
                    &entry.temp_text,
                    Point::new(SCREEN_W - CARD_MARGIN - 14, y + 38),
                    temp_style,
                    Alignment::Right,
                )
                .draw(fb)
                .ok();
            }

            // Scroll indicator
            if day.entries.len() > visible_rows {
                let indicator_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_TERTIARY);
                let indicator = format!(
                    "{}-{} of {}  (swipe up/dn)",
                    scroll + 1,
                    (scroll + visible_rows).min(day.entries.len()),
                    day.entries.len()
                );
                Text::with_alignment(
                    &indicator,
                    Point::new(SCREEN_W / 2, SCREEN_H - 18),
                    indicator_style,
                    Alignment::Center,
                )
                .draw(fb)
                .ok();
            }
        }
    }
}
