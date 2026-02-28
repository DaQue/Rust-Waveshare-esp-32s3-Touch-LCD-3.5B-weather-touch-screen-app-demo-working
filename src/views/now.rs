use embedded_graphics::{
    mono_font::MonoTextStyle,
    prelude::*,
    text::{Alignment, Text},
};
use profont::{PROFONT_10_POINT, PROFONT_12_POINT, PROFONT_14_POINT, PROFONT_24_POINT};

use crate::framebuffer::Framebuffer;
use crate::layout::*;
use crate::weather::AlertKind;
use crate::views::AppState;

fn f_to_c(f: f32) -> f32 {
    (f - 32.0) * 5.0 / 9.0
}

pub fn draw(fb: &mut Framebuffer, state: &AppState) {
    let (screen_w, screen_h) = screen_size(state.orientation);
    fb.clear_color(BG_NOW);
    draw_hline(fb, HEADER_LINE_Y, LINE_COLOR_1);

    // Header: time (left), city (center), status (right)
    let header_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_HEADER);
    Text::new(&state.time_text, Point::new(10, 24), header_style)
        .draw(fb)
        .ok();

    let top_alert_kind = state.weather_alerts.first().map(|a| a.kind());

    if let Some(cw) = &state.current_weather {
        let city_text = format!("{}, {}", cw.city, cw.country);
        Text::with_alignment(
            &city_text,
            Point::new(screen_w / 2, 24),
            header_style,
            Alignment::Center,
        )
        .draw(fb)
        .ok();
    }

    let status_color = match top_alert_kind {
        Some(AlertKind::Warning) => rgb(255, 90, 90),
        Some(AlertKind::Watch) => rgb(255, 225, 80),
        Some(AlertKind::Advisory) => rgb(255, 180, 80),
        _ => TEXT_STATUS,
    };
    let status_style = MonoTextStyle::new(&PROFONT_10_POINT, status_color);
    Text::with_alignment(
        &state.status_text,
        Point::new(screen_w - 8, 24),
        status_style,
        Alignment::Right,
    )
    .draw(fb)
    .ok();
    if state.weather_stale {
        let stale_style = MonoTextStyle::new(&PROFONT_10_POINT, rgb(255, 225, 80));
        Text::with_alignment(
            "WX STALE",
            Point::new(screen_w - 8, 14),
            stale_style,
            Alignment::Right,
        )
        .draw(fb)
        .ok();
    }

    let card_top = 36;
    let card_h = if state.orientation.is_portrait() { 178 } else { 148 };
    draw_card(
        fb,
        CARD_MARGIN,
        card_top,
        screen_w - 2 * CARD_MARGIN,
        card_h,
        CARD_RADIUS as u32,
        CARD_FILL_NOW,
        CARD_BORDER_NOW,
        1,
    );

    let icon = state.current_weather.as_ref().map(|w| w.icon).unwrap_or_default();
    icon.draw_80(fb, 18, card_top + 10);

    if let Some(cw) = &state.current_weather {
        let (temp, feels, unit) = if state.use_celsius {
            (f_to_c(cw.temp_f), f_to_c(cw.feels_f), "C")
        } else {
            (cw.temp_f, cw.feels_f, "F")
        };

        let temp_text = format!("{:.0}°{}", temp, unit);
        let temp_style = MonoTextStyle::new(&PROFONT_24_POINT, TEXT_PRIMARY);
        let feels_text = format!("FEELS {:.0}°", feels);
        let feels_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_CONDITION);
        let cond_style = MonoTextStyle::new(&PROFONT_12_POINT, TEXT_DETAIL);
        let stats_text = format!(
            "Hum {}%  Wind {:.0}mph  {} hPa",
            cw.humidity, cw.wind_mph, cw.pressure_hpa
        );
        let stats_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_TERTIARY);
        let hint_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_BOTTOM);

        Text::new(&temp_text, Point::new(120, card_top + 46), temp_style).draw(fb).ok();
        Text::new(&feels_text, Point::new(120, card_top + 72), feels_style).draw(fb).ok();
        Text::new(&cw.condition, Point::new(120, card_top + 94), cond_style).draw(fb).ok();

        if state.orientation.is_portrait() {
            Text::new(&stats_text, Point::new(18, card_top + 124), stats_style).draw(fb).ok();
            let hint = if state.weather_alerts.is_empty() {
                "(tap temp = F/C)   (tap icon = refresh)"
            } else {
                "(tap temp = F/C)   (tap icon = alerts)"
            };
            Text::new(hint, Point::new(18, card_top + 144), hint_style)
                .draw(fb)
                .ok();
        } else {
            Text::new(&stats_text, Point::new(18, card_top + 120), stats_style).draw(fb).ok();
            let hint = if state.weather_alerts.is_empty() {
                "(tap temp = F/C)   (tap icon = refresh)"
            } else {
                "(tap temp = F/C)   (tap icon = alerts)"
            };
            Text::new(hint, Point::new(18, card_top + 138), hint_style)
                .draw(fb)
                .ok();
        }
    } else {
        let no_data_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_DETAIL);
        Text::new("Waiting for weather data...", Point::new(120, card_top + 60), no_data_style)
            .draw(fb)
            .ok();
    }

    let strip_y = card_top + card_h + 6;
    let indoor_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_TERTIARY);
    let mut indoor_text = String::from("Indoor:");
    if let Some(temp) = state.indoor_temp {
        let t = if state.use_celsius { f_to_c(temp) } else { temp };
        indoor_text += &format!("  {:.1}°", t);
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

    let fc_top = strip_y + 18;
    let fc_h = screen_h - fc_top - 18;

    draw_card(
        fb,
        CARD_MARGIN,
        fc_top,
        screen_w - 2 * CARD_MARGIN,
        fc_h,
        10,
        CARD_FILL_FORECAST_PREVIEW,
        CARD_BORDER_FORECAST_PREVIEW,
        1,
    );

    // "Forecast >" label top-left of card
    let label_style = MonoTextStyle::new(&PROFONT_12_POINT, TEXT_HEADER);
    Text::new("Forecast >", Point::new(CARD_MARGIN + 8, fc_top + 14), label_style)
        .draw(fb)
        .ok();

    if let Some(fc) = &state.forecast {
        let day_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_TERTIARY);
        let temp_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_SECONDARY);

        if state.orientation.is_portrait() {
            let rows = 4i32;
            let row_h = (fc_h - 24).max(40) / rows;
            for (i, row) in fc.rows.iter().take(rows as usize).enumerate() {
                let y = fc_top + 24 + (i as i32) * row_h;
                row.icon.draw_36(fb, CARD_MARGIN + 10, y);
                Text::new(&row.title, Point::new(CARD_MARGIN + 54, y + 14), day_style)
                    .draw(fb)
                    .ok();
                let temp_label = format!("{}°", row.temp_f);
                Text::with_alignment(
                    &temp_label,
                    Point::new(screen_w - CARD_MARGIN - 10, y + 22),
                    temp_style,
                    Alignment::Right,
                )
                .draw(fb)
                .ok();
            }
        } else {
            let col_count = 4;
            let col_w = (screen_w - 2 * CARD_MARGIN) / col_count;
            for (i, row) in fc.rows.iter().take(col_count as usize).enumerate() {
                let cx = CARD_MARGIN + (i as i32) * col_w + col_w / 2;
                Text::with_alignment(
                    &row.title,
                    Point::new(cx, fc_top + 30),
                    day_style,
                    Alignment::Center,
                )
                .draw(fb)
                .ok();
                row.icon.draw_36(fb, cx - 18, fc_top + 34);
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
    }

    if state.now_alerts_open {
        draw_alert_overlay(fb, state, screen_w, screen_h);
    }

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

fn draw_alert_overlay(fb: &mut Framebuffer, state: &AppState, screen_w: i32, screen_h: i32) {
    if state.weather_alerts.is_empty() {
        return;
    }
    let alert = &state.weather_alerts[0];
    let (fill, border, title_color) = match alert.kind() {
        AlertKind::Warning => (rgb(60, 12, 12), rgb(180, 40, 40), rgb(255, 130, 130)),
        AlertKind::Watch => (rgb(60, 52, 12), rgb(180, 150, 32), rgb(255, 230, 120)),
        AlertKind::Advisory => (rgb(60, 36, 12), rgb(190, 110, 40), rgb(255, 200, 110)),
        AlertKind::Other => (rgb(18, 24, 36), rgb(63, 75, 95), TEXT_PRIMARY),
    };
    let y = if state.orientation.is_portrait() { 224 } else { 190 };
    let h = screen_h - y - 8;
    draw_card(fb, CARD_MARGIN, y, screen_w - 2 * CARD_MARGIN, h, 10, fill, border, 1);

    let title_style = MonoTextStyle::new(&PROFONT_14_POINT, title_color);
    let body_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_PRIMARY);
    let dim_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_TERTIARY);

    // Title: kind + count
    let count_text = if state.weather_alerts.len() > 1 {
        format!("{} ({} active)", alert.kind().as_str(), state.weather_alerts.len())
    } else {
        alert.kind().as_str().to_string()
    };
    Text::new(&count_text, Point::new(CARD_MARGIN + 8, y + 14), title_style).draw(fb).ok();

    // Event name
    Text::new(&alert.event, Point::new(CARD_MARGIN + 8, y + 30), body_style).draw(fb).ok();

    // Headline - word-wrapped, up to 2 lines
    let char_w = 6i32; // PROFONT_10_POINT ~6px/char
    let line_h = 14i32;
    let max_chars = ((screen_w - 2 * CARD_MARGIN - 16) / char_w) as usize;
    let bottom_reserve = y + h - 20;
    let mut row_y = y + 44;
    for line in crate::layout::word_wrap(&alert.headline, max_chars).iter().take(2) {
        if row_y + line_h > bottom_reserve { break; }
        Text::new(line, Point::new(CARD_MARGIN + 8, row_y), body_style).draw(fb).ok();
        row_y += line_h;
    }

    // Description bullets (lines starting with '*')
    row_y += 2;
    for raw in alert.description.lines() {
        if row_y + line_h > bottom_reserve { break; }
        let raw = raw.trim();
        if !raw.starts_with('*') { continue; }
        let content = raw.trim_start_matches('*').trim();
        let line = if content.len() + 2 > max_chars {
            format!("- {}...", &content[..max_chars.saturating_sub(5)])
        } else {
            format!("- {}", content)
        };
        Text::new(&line, Point::new(CARD_MARGIN + 8, row_y), dim_style).draw(fb).ok();
        row_y += line_h;
    }

    // Bottom row: expiry left, close hint right-aligned
    let exp_text = format!("exp {}", crate::weather::format_alert_expiry(&alert.expires));
    Text::new(&exp_text, Point::new(CARD_MARGIN + 8, y + h - 8), dim_style).draw(fb).ok();
    Text::with_alignment(
        "(tap to close)",
        Point::new(screen_w - CARD_MARGIN - 8, y + h - 8),
        dim_style,
        Alignment::Right,
    )
    .draw(fb)
    .ok();
}
