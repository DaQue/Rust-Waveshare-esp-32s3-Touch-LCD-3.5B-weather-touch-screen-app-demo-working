use embedded_graphics::{
    mono_font::MonoTextStyle,
    prelude::*,
    primitives::{Triangle, PrimitiveStyle, Rectangle, PrimitiveStyleBuilder},
    text::{Alignment, Text},
};
use profont::{PROFONT_10_POINT, PROFONT_12_POINT, PROFONT_14_POINT, PROFONT_18_POINT, PROFONT_24_POINT};

use crate::framebuffer::Framebuffer;
use crate::layout::*;
use crate::weather::AlertKind;
use crate::views::AppState;

fn f_to_c(f: f32) -> f32 {
    (f - 32.0) * 5.0 / 9.0
}

/// Returns 1 (rising), -1 (falling), or 0 (steady/insufficient data).
fn outdoor_temp_trend(history: &crate::psbox::PsramRing) -> i8 {
    let len = history.len();
    if len < 2 { return 0; }
    let diff = history[len - 1] - history[len - 2];
    if diff > 0.5 { 1 } else if diff < -0.5 { -1 } else { 0 }
}

fn draw_temp_trend(fb: &mut Framebuffer, trend: i8, cx: i32, cy: i32) {
    let (color, pts) = match trend {
        1 => (
            rgb(100, 220, 100), // green — rising
            [Point::new(cx, cy - 9), Point::new(cx - 8, cy + 7), Point::new(cx + 8, cy + 7)],
        ),
        -1 => (
            rgb(100, 160, 255), // blue — falling
            [Point::new(cx, cy + 9), Point::new(cx - 8, cy - 7), Point::new(cx + 8, cy - 7)],
        ),
        _ => (
            rgb(140, 140, 140), // gray — steady; right-pointing
            [Point::new(cx + 9, cy), Point::new(cx - 7, cy - 7), Point::new(cx - 7, cy + 7)],
        ),
    };
    Triangle::new(pts[0], pts[1], pts[2])
        .into_styled(PrimitiveStyle::with_fill(color))
        .draw(fb).ok();
}


/// Draw WiFi signal bars + RSSI dBm value, right-aligned at (x_right, y_base).
fn draw_wifi_signal(fb: &mut Framebuffer, x_right: i32, y_base: i32, rssi: Option<i8>) {
    // 4 bars: width 3px, gap 2px, heights [5,8,11,14] — right-aligned before rssi text
    let text_reserve = 24i32; // space for "-90" + padding
    let bar_w = 3i32;
    let bar_gap = 2i32;
    let bars_total_w = 4 * bar_w + 3 * bar_gap; // 18px
    let bars_x = x_right - text_reserve - 4 - bars_total_w;

    let bars_active: usize = match rssi {
        None => 0,
        Some(r) if r >= -60 => 4,
        Some(r) if r >= -70 => 3,
        Some(r) if r >= -80 => 2,
        Some(_) => 1,
    };

    let heights = [5i32, 8, 11, 14];
    for (i, &h) in heights.iter().enumerate() {
        let x = bars_x + i as i32 * (bar_w + bar_gap);
        let color = if i < bars_active { rgb(100, 200, 100) } else { rgb(45, 55, 70) };
        let style = PrimitiveStyleBuilder::new().fill_color(color).build();
        Rectangle::new(Point::new(x, y_base - h), Size::new(bar_w as u32, h as u32))
            .into_styled(style)
            .draw(fb).ok();
    }

    let rssi_text = match rssi {
        Some(r) => format!("{}", r),
        None => "--".to_string(),
    };
    let rssi_style = MonoTextStyle::new(&PROFONT_10_POINT, rgb(150, 160, 175));
    Text::with_alignment(&rssi_text, Point::new(x_right, y_base), rssi_style, Alignment::Right)
        .draw(fb).ok();
}

pub fn draw(fb: &mut Framebuffer, state: &AppState) {
    let (screen_w, screen_h) = screen_size(state.orientation);
    fb.clear_color(BG_NOW);
    draw_hline(fb, HEADER_LINE_Y, LINE_COLOR_1);

    // Header: time (left), city (center), status (right)
    // In portrait (320px wide) PROFONT_14 "12:34 PM" = 72px and city can start
    // at ~70px → use PROFONT_12 (7px/char → 56px) to avoid overlap.
    let time_style = if state.orientation.is_portrait() {
        MonoTextStyle::new(&PROFONT_12_POINT, TEXT_HEADER)
    } else {
        MonoTextStyle::new(&PROFONT_14_POINT, TEXT_HEADER)
    };
    Text::new(&state.time_text, Point::new(10, 24), time_style)
        .draw(fb)
        .ok();

    let top_alert_kind = state.weather_alerts.first().map(|a| a.kind());

    if let Some(cw) = &state.current_weather {
        // Alert color tint on city name when alerts active
        let city_color = match top_alert_kind {
            Some(AlertKind::Warning) => rgb(255, 130, 130),
            Some(AlertKind::Watch) => rgb(255, 230, 120),
            Some(AlertKind::Advisory) => rgb(255, 200, 110),
            _ => TEXT_HEADER,
        };
        // Portrait: city also uses PROFONT_12 to avoid overlap with time text.
        // PROFONT_14 (9px/char) at 20 chars = 180px centered at 160 → starts at 70,
        // only 4px from time text end (~66px). PROFONT_12 (7px/char) → starts at 90.
        let city_style = if state.orientation.is_portrait() {
            MonoTextStyle::new(&PROFONT_12_POINT, city_color)
        } else {
            MonoTextStyle::new(&PROFONT_14_POINT, city_color)
        };
        Text::with_alignment(
            &cw.city,
            Point::new(screen_w / 2, 24),
            city_style,
            Alignment::Center,
        )
        .draw(fb)
        .ok();
    }

    // WiFi signal bars + RSSI, right-aligned in header
    draw_wifi_signal(fb, screen_w - 8, 24, state.wifi_rssi);

    if state.weather_stale {
        let stale_style = MonoTextStyle::new(&PROFONT_10_POINT, rgb(255, 225, 80));
        Text::with_alignment(
            "STALE",
            Point::new(screen_w - 8, 14),
            stale_style,
            Alignment::Right,
        )
        .draw(fb)
        .ok();
    }

    let card_top = 36;
    let card_h = if state.orientation.is_portrait() { 156 } else { 140 };
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
    icon.draw_80(fb, 16, card_top + 8);

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
        let cond_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_DETAIL);
        let hint_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_BOTTOM);

        if state.orientation.is_portrait() {
            // Portrait weather card: icon left, temp/feels/condition right
            // card_top=36, card_h=178 → inner y range 37..213
            Text::new(&temp_text, Point::new(106, card_top + 46), temp_style).draw(fb).ok();
            let trend = outdoor_temp_trend(&state.outdoor_temp_history);
            // PROFONT_24 char width=16; baseline=24 → cell_top=card_top+22 ≥ 37+border ✓
            let triangle_x = 106 + temp_text.chars().count() as i32 * 16 + 10;
            draw_temp_trend(fb, trend, triangle_x, card_top + 36);
            Text::new(&feels_text, Point::new(106, card_top + 72), feels_style).draw(fb).ok();
            Text::new(&cw.condition, Point::new(106, card_top + 94), cond_style).draw(fb).ok();

            let hint = if state.weather_alerts.is_empty() {
                "(tap icon=refresh)  (tap temp=F/C)"
            } else {
                "(tap icon=alerts)  (tap temp=F/C)"
            };
            Text::new(hint, Point::new(18, card_top + card_h - 14), hint_style).draw(fb).ok();
        } else {
            // Landscape: new layout — weather left/middle, Indoor right panel
            const DIVIDER_X: i32 = 298;

            // Temp + trend (middle column)
            // baseline=24 for PROFONT_24, so cell_top = baseline_y - 24; need cell_top >= 37 (inside border)
            // card_top+34=70 → cell_top=46, 10px inside card top ✓
            Text::new(&temp_text, Point::new(106, card_top + 34), temp_style).draw(fb).ok();
            let trend = outdoor_temp_trend(&state.outdoor_temp_history);
            // PROFONT_24 char width = 16px
            let triangle_x = 106 + temp_text.chars().count() as i32 * 16 + 10;
            draw_temp_trend(fb, trend, triangle_x, card_top + 24);

            // Feels like + condition
            Text::new(&feels_text, Point::new(106, card_top + 56), feels_style).draw(fb).ok();
            Text::new(&cw.condition, Point::new(106, card_top + 76), cond_style).draw(fb).ok();

            // Hint row at card bottom
            let hint = if state.weather_alerts.is_empty() {
                "(tap icon=refresh)  (tap temp=F/C)"
            } else {
                "(tap icon=alerts)  (tap temp=F/C)"
            };
            Text::new(hint, Point::new(CARD_MARGIN + 6, card_top + card_h - 13), hint_style)
                .draw(fb).ok();

            // Vertical divider
            draw_vline(fb, DIVIDER_X, card_top + 10, card_top + card_h - 10, LINE_COLOR_1);

            // Indoor right panel
            let ind_x = DIVIDER_X + 10;
            let ind_style_label = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_HEADER);
            let ind_style_temp  = MonoTextStyle::new(&PROFONT_18_POINT, TEXT_PRIMARY);
            let ind_style_small = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_TERTIARY);

            // "Indoor": push down to card_top+22 so text top aligns with icon top (card_top+8)
            Text::new("Indoor", Point::new(ind_x, card_top + 22), ind_style_label).draw(fb).ok();

            if let Some(t) = state.indoor_temp {
                let t_disp = if state.use_celsius { f_to_c(t) } else { t };
                let unit_c = if state.use_celsius { "C" } else { "F" };
                let t_text = format!("{:.1}°{}", t_disp, unit_c);
                Text::new(&t_text, Point::new(ind_x, card_top + 47), ind_style_temp).draw(fb).ok();
            }
            if let Some(h) = state.indoor_humidity {
                let h_text = format!("{:.0}% RH", h);
                Text::new(&h_text, Point::new(ind_x, card_top + 79), ind_style_small).draw(fb).ok();
            }
            if let Some(p) = state.indoor_pressure {
                let correction = state.pressure_history.delta_owm_bme_stable().unwrap_or(0.0);
                let p_text = format!("{:.1} hPa", p + correction);
                Text::new(&p_text, Point::new(ind_x, card_top + 95), ind_style_small).draw(fb).ok();
            }
            // Pressure trend indicator (3h/1h/30m) — hide when sensor offline
            if state.indoor_pressure.is_some() {
                if let Some((delta, label)) = state.pressure_history.pressure_trend() {
                    let sign = if delta >= 0.0 { "+" } else { "" };
                    let trend_text = format!("{}: {}{:.1}", label, sign, delta);
                    Text::new(&trend_text, Point::new(ind_x, card_top + 111), ind_style_small)
                        .draw(fb).ok();
                }
            }
        }
    } else {
        let no_data_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_DETAIL);
        Text::new("Waiting for weather data...", Point::new(106, card_top + 60), no_data_style)
            .draw(fb)
            .ok();
    }

    // ── Portrait: indoor card + forecast card ─────────────────────────────────
    if state.orientation.is_portrait() {
        let ind_card_y = card_top + card_h + 6;  // 36+156+6 = 198
        let ind_card_h = 60;
        draw_card(fb, CARD_MARGIN, ind_card_y, screen_w - 2 * CARD_MARGIN, ind_card_h, 8,
                  CARD_FILL_INDOOR, CARD_BORDER_INDOOR, 1);

        // Indoor card content: row1 label+temp+RH, row2 pressure+trend
        // PROFONT_14 baseline=13: cell_top = y-13; row1 y=216 → cell_top=203 ≥ 199+1 ✓
        let ind_label = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_HEADER);
        let ind_small = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_TERTIARY);
        Text::new("Indoor", Point::new(CARD_MARGIN + 8, ind_card_y + 18), ind_label).draw(fb).ok();

        let mut ind_x2 = CARD_MARGIN + 70;
        if let Some(t) = state.indoor_temp {
            let t_disp = if state.use_celsius { f_to_c(t) } else { t };
            let unit_c = if state.use_celsius { "C" } else { "F" };
            let t_text = format!("{:.1}°{}", t_disp, unit_c);
            Text::new(&t_text, Point::new(ind_x2, ind_card_y + 18), ind_label).draw(fb).ok();
            ind_x2 += t_text.chars().count() as i32 * 10 + 8;
        }
        if let Some(h) = state.indoor_humidity {
            let h_text = format!("{:.0}% RH", h);
            Text::new(&h_text, Point::new(ind_x2, ind_card_y + 18), ind_small).draw(fb).ok();
        }
        // Row 2: pressure + trend
        // PROFONT_10 baseline=9: row2 y=ind_card_y+38 → cell_top=29 ≥ 1 ✓
        if let Some(p) = state.indoor_pressure {
            let correction = state.pressure_history.delta_owm_bme_stable().unwrap_or(0.0);
            let p_text = format!("{:.1} hPa", p + correction);
            Text::new(&p_text, Point::new(CARD_MARGIN + 8, ind_card_y + 48), ind_small).draw(fb).ok();
        }
        if state.indoor_pressure.is_some() {
            if let Some((delta, label)) = state.pressure_history.pressure_trend() {
                let sign = if delta >= 0.0 { "+" } else { "" };
                let trend_text = format!("{}h: {}{:.1}", label.trim_end_matches('h'), sign, delta);
                Text::new(&trend_text, Point::new(CARD_MARGIN + 100, ind_card_y + 48), ind_small).draw(fb).ok();
            }
        }

        // Forecast card
        let fc_card_y = ind_card_y + ind_card_h + 6;  // 198+60+6 = 264
        let fc_card_h = screen_h - fc_card_y - 20;    // 480-264-20 = 196
        draw_card(fb, CARD_MARGIN, fc_card_y, screen_w - 2 * CARD_MARGIN, fc_card_h, 10,
                  CARD_FILL_FORECAST_PREVIEW, CARD_BORDER_FORECAST_PREVIEW, 1);

        if let Some(fc) = &state.forecast {
            let day_style  = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_TERTIARY);
            let temp_style = MonoTextStyle::new(&PROFONT_18_POINT, TEXT_PRIMARY);
            let cond_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_DETAIL);
            let rows = 4i32;
            let row_h = fc_card_h / rows;  // ~49px
            for (i, row) in fc.rows.iter().take(rows as usize).enumerate() {
                // Center the row content vertically within row_h
                let ry = fc_card_y + (i as i32) * row_h + row_h / 2;
                // icon: 36px, centered on ry-18
                row.icon.draw_36(fb, CARD_MARGIN + 8, ry - 18);
                // Day name: PROFONT_14 baseline=13; baseline_y=ry-5
                Text::new(&row.title, Point::new(CARD_MARGIN + 52, ry - 5), day_style).draw(fb).ok();
                // Condition: PROFONT_10 below day name
                Text::new(&row.condition, Point::new(CARD_MARGIN + 52, ry + 10), cond_style).draw(fb).ok();
                // Temp right-aligned: PROFONT_18 baseline=17; baseline_y=ry+9
                let temp_label = format!("{}°", row.temp_f);
                Text::with_alignment(
                    &temp_label,
                    Point::new(screen_w - CARD_MARGIN - 8, ry + 9),
                    temp_style,
                    Alignment::Right,
                ).draw(fb).ok();
            }
        }
    } else {
        // ── Landscape: single forecast strip ──────────────────────────────────
        let fc_top = card_top + card_h + 6;
        let fc_h   = screen_h - fc_top - 16;

        draw_card(fb, CARD_MARGIN, fc_top, screen_w - 2 * CARD_MARGIN, fc_h, 10,
                  CARD_FILL_FORECAST_PREVIEW, CARD_BORDER_FORECAST_PREVIEW, 1);

        if let Some(fc) = &state.forecast {
            let day_style  = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_TERTIARY);
            let temp_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_SECONDARY);
            let cond_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_DETAIL);
            let fc_label_style = MonoTextStyle::new(&PROFONT_12_POINT, TEXT_HEADER);
            Text::new("Forecast >", Point::new(CARD_MARGIN + 8, fc_top + 14), fc_label_style)
                .draw(fb).ok();
            let col_w = (screen_w - 2 * CARD_MARGIN) / 4;
            // Vertical layout within the strip (fc_h ≈ 122px):
            //   Day name   baseline y = fc_top + 20
            //   Icon top            y = fc_top + 28  (48px → bottom at fc_top+76)
            //   Temp       baseline y = fc_top + 91  (PROFONT_14 h=17, 7px gap below icon)
            //   Condition  baseline y = fc_top + 106 (PROFONT_10 h=12, 3px gap)
            const ICON_TOP:  i32 = 28;
            const TEMP_BASE: i32 = 91;
            const COND_BASE: i32 = 106;
            for (i, row) in fc.rows.iter().take(4).enumerate() {
                let cx = CARD_MARGIN + (i as i32) * col_w + col_w / 2;
                Text::with_alignment(&row.title, Point::new(cx, fc_top + 20), day_style, Alignment::Center)
                    .draw(fb).ok();
                row.icon.draw_48(fb, cx - 24, fc_top + ICON_TOP);
                let temp_label = format!("{}°", row.temp_f);
                Text::with_alignment(&temp_label, Point::new(cx, fc_top + TEMP_BASE), temp_style, Alignment::Center)
                    .draw(fb).ok();
                Text::with_alignment(&row.condition, Point::new(cx, fc_top + COND_BASE), cond_style, Alignment::Center)
                    .draw(fb).ok();
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
        // Draw "- " prefix and content as separate draws to avoid format! in the
        // hot render path.  Use char-aware truncation to avoid panicking on
        // multi-byte UTF-8 characters (degree signs, smart quotes, em-dashes).
        let char_limit = max_chars.saturating_sub(2); // reserve "- "
        let content_chars = content.chars().count();
        if content_chars <= char_limit {
            Text::new("- ", Point::new(CARD_MARGIN + 8, row_y), dim_style).draw(fb).ok();
            Text::new(content, Point::new(CARD_MARGIN + 8 + 12, row_y), dim_style).draw(fb).ok();
        } else {
            let end = content.char_indices()
                .nth(char_limit.saturating_sub(3))
                .map(|(i, _)| i)
                .unwrap_or(content.len());
            Text::new("- ", Point::new(CARD_MARGIN + 8, row_y), dim_style).draw(fb).ok();
            Text::new(&content[..end], Point::new(CARD_MARGIN + 8 + 12, row_y), dim_style).draw(fb).ok();
            Text::new("...", Point::new(CARD_MARGIN + 8 + 12 + (char_limit as i32 - 3) * 6, row_y), dim_style).draw(fb).ok();
        }
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
