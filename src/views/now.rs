use embedded_graphics::{
    mono_font::MonoTextStyle,
    prelude::*,
    text::{Alignment, Text},
};
use profont::{PROFONT_10_POINT, PROFONT_12_POINT, PROFONT_14_POINT, PROFONT_24_POINT};

use crate::framebuffer::Framebuffer;
use crate::layout::*;
use crate::views::AppState;

pub fn draw(fb: &mut Framebuffer, state: &AppState) {
    fb.clear_color(BG_NOW);
    draw_hline(fb, HEADER_LINE_Y, LINE_COLOR_1);

    // Header: time (left), city (center), status (right)
    let header_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_HEADER);
    Text::new(&state.time_text, Point::new(10, 24), header_style)
        .draw(fb)
        .ok();

    if let Some(cw) = &state.current_weather {
        let city_text = format!("{}, {}", cw.city, cw.country);
        Text::with_alignment(
            &city_text,
            Point::new(SCREEN_W / 2, 24),
            header_style,
            Alignment::Center,
        )
        .draw(fb)
        .ok();
    }

    let status_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_STATUS);
    Text::with_alignment(
        &state.status_text,
        Point::new(SCREEN_W - 8, 24),
        status_style,
        Alignment::Right,
    )
    .draw(fb)
    .ok();

    // Main weather card
    let card_top = 36;
    let card_h = 148;
    draw_card(
        fb,
        CARD_MARGIN, card_top,
        SCREEN_W - 2 * CARD_MARGIN, card_h,
        CARD_RADIUS as u32,
        CARD_FILL_NOW, CARD_BORDER_NOW, 1,
    );

    // Weather icon (80x80)
    let icon = state
        .current_weather
        .as_ref()
        .map(|w| w.icon)
        .unwrap_or_default();
    icon.draw_80(fb, 18, card_top + 10);

    if let Some(cw) = &state.current_weather {
        // Large temperature
        let temp_text = format!("{:.0}°", cw.temp_f);
        let temp_style = MonoTextStyle::new(&PROFONT_24_POINT, TEXT_PRIMARY);
        Text::new(&temp_text, Point::new(120, card_top + 46), temp_style)
            .draw(fb)
            .ok();

        // "FEELS XX°"
        let feels_text = format!("FEELS {:.0}°", cw.feels_f);
        let feels_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_CONDITION);
        Text::new(&feels_text, Point::new(120, card_top + 72), feels_style)
            .draw(fb)
            .ok();

        // Condition description
        let cond_style = MonoTextStyle::new(&PROFONT_12_POINT, TEXT_DETAIL);
        Text::new(&cw.condition, Point::new(120, card_top + 94), cond_style)
            .draw(fb)
            .ok();

        // Stats line
        let stats_text = format!(
            "Hum {}%  Wind {:.0}mph  {} hPa",
            cw.humidity, cw.wind_mph, cw.pressure_hpa
        );
        let stats_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_TERTIARY);
        Text::new(&stats_text, Point::new(18, card_top + 120), stats_style)
            .draw(fb)
            .ok();

        // Tap hint
        let hint_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_BOTTOM);
        Text::new(
            "(tap temp = F/C)   (tap icon = refresh)",
            Point::new(18, card_top + 138),
            hint_style,
        )
        .draw(fb)
        .ok();
    } else {
        let no_data_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_DETAIL);
        Text::new("Waiting for weather data...", Point::new(120, card_top + 60), no_data_style)
            .draw(fb)
            .ok();
    }

    // Indoor info strip (compact single line below main card)
    let strip_y = card_top + card_h + 6;
    let indoor_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_TERTIARY);
    let mut indoor_text = String::from("Indoor:");
    if let Some(temp) = state.indoor_temp {
        indoor_text += &format!("  {:.1}°", temp);
    }
    if let Some(hum) = state.indoor_humidity {
        indoor_text += &format!("  {:.0}%RH", hum);
    }
    if let Some(press) = state.indoor_pressure {
        indoor_text += &format!("  {:.0}hPa", press);
    }
    Text::new(&indoor_text, Point::new(CARD_MARGIN + 4, strip_y + 10), indoor_style)
        .draw(fb)
        .ok();

    // Forecast: 4 days side-by-side across full width
    let fc_top = strip_y + 18;
    let fc_h = SCREEN_H - fc_top - 18;
    let col_count = 4;
    let col_w = (SCREEN_W - 2 * CARD_MARGIN) / col_count;

    draw_card(
        fb,
        CARD_MARGIN, fc_top,
        SCREEN_W - 2 * CARD_MARGIN, fc_h,
        10,
        CARD_FILL_FORECAST_PREVIEW, CARD_BORDER_FORECAST_PREVIEW, 1,
    );

    // "Forecast >" label top-left of card
    let label_style = MonoTextStyle::new(&PROFONT_12_POINT, TEXT_HEADER);
    Text::new("Forecast >", Point::new(CARD_MARGIN + 8, fc_top + 14), label_style)
        .draw(fb)
        .ok();

    if let Some(fc) = &state.forecast {
        let day_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_TERTIARY);
        let temp_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_SECONDARY);

        for (i, row) in fc.rows.iter().take(col_count as usize).enumerate() {
            let cx = CARD_MARGIN + (i as i32) * col_w + col_w / 2;

            // Day name centered
            Text::with_alignment(
                &row.title,
                Point::new(cx, fc_top + 30),
                day_style,
                Alignment::Center,
            )
            .draw(fb)
            .ok();

            // Icon (36x36) centered
            row.icon.draw_36(fb, cx - 18, fc_top + 34);

            // Temperature centered below icon
            let temp_label = format!("{}°", row.temp_f);
            Text::with_alignment(
                &temp_label,
                Point::new(cx, fc_top + fc_h - 6),
                temp_style,
                Alignment::Center,
            )
            .draw(fb)
            .ok();
        }
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
