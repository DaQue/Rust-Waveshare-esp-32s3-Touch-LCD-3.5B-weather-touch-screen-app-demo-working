use embedded_graphics::{
    mono_font::MonoTextStyle,
    prelude::*,
    text::{Alignment, Text},
};
use profont::{PROFONT_10_POINT, PROFONT_12_POINT, PROFONT_14_POINT, PROFONT_24_POINT};

use crate::framebuffer::Framebuffer;
use crate::layout::*;
use crate::views::AppState;
use crate::weather::AlertKind;

/// Approximate character width in pixels for PROFONT_10_POINT.
const CHAR_W_10PT: i32 = 6;
/// Line height for PROFONT_10_POINT body text.
const LINE_H_10PT: i32 = 14;

pub fn draw(fb: &mut Framebuffer, state: &AppState) {
    let (screen_w, screen_h) = screen_size(state.orientation);

    // Background
    fb.clear_color(BG_WARNING);

    let alert = match state.weather_alerts.first() {
        Some(a) => a,
        None => {
            // No alerts — shouldn't be on this view, draw fallback
            let style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_WARNING_BODY);
            Text::with_alignment(
                "No active warnings",
                Point::new(screen_w / 2, screen_h / 2),
                style,
                Alignment::Center,
            )
            .draw(fb)
            .ok();
            return;
        }
    };

    let kind = alert.kind();
    let title_color = match kind {
        AlertKind::Warning => TEXT_WARNING_TITLE,
        AlertKind::Watch => rgb(255, 230, 120),
        AlertKind::Advisory => rgb(255, 200, 110),
        AlertKind::Other => TEXT_PRIMARY,
    };

    // ── Header: severity badge + count ──
    let badge_style = MonoTextStyle::new(&PROFONT_24_POINT, title_color);
    let count = state.weather_alerts.len();
    let badge_text = if count > 1 {
        format!("{} ({} active)", alert.kind().as_str().to_ascii_uppercase(), count)
    } else {
        alert.kind().as_str().to_ascii_uppercase()
    };
    Text::with_alignment(
        &badge_text,
        Point::new(screen_w / 2, 34),
        badge_style,
        Alignment::Center,
    )
    .draw(fb)
    .ok();

    // Separator line
    draw_hline(fb, 42, rgb(180, 40, 40));

    // ── Event name ──
    let event_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_WARNING_BODY);
    Text::with_alignment(
        &alert.event,
        Point::new(screen_w / 2, 60),
        event_style,
        Alignment::Center,
    )
    .draw(fb)
    .ok();

    // ── Headline ──
    let headline_style = MonoTextStyle::new(&PROFONT_12_POINT, TEXT_WARNING_BODY);
    let max_headline_chars = ((screen_w - 20) / 8) as usize; // ~8px per char at 12pt
    let headline = if alert.headline.len() > max_headline_chars {
        format!("{}...", &alert.headline[..max_headline_chars.saturating_sub(3)])
    } else {
        alert.headline.clone()
    };
    Text::new(&headline, Point::new(10, 78), headline_style)
        .draw(fb)
        .ok();

    // ── Silence button zone (bottom third) ──
    let button_h = screen_h / 3;
    let button_y = screen_h - button_h;

    let (btn_fill, btn_border, btn_text) = if state.warning_active {
        (CARD_FILL_SILENCE, CARD_BORDER_SILENCE, "TAP TO SILENCE")
    } else {
        (CARD_FILL_SILENCED, CARD_BORDER_SILENCED, "SILENCED")
    };

    draw_card(
        fb,
        4,
        button_y,
        screen_w - 8,
        button_h - 4,
        12,
        btn_fill,
        btn_border,
        2,
    );

    let btn_style = MonoTextStyle::new(&PROFONT_24_POINT, TEXT_SILENCE_BUTTON);
    Text::with_alignment(
        btn_text,
        Point::new(screen_w / 2, button_y + button_h / 2 + 6),
        btn_style,
        Alignment::Center,
    )
    .draw(fb)
    .ok();

    if !state.warning_active {
        let hint_style = MonoTextStyle::new(&PROFONT_10_POINT, rgb(160, 200, 160));
        Text::with_alignment(
            "swipe left/right to exit",
            Point::new(screen_w / 2, button_y + button_h / 2 + 26),
            hint_style,
            Alignment::Center,
        )
        .draw(fb)
        .ok();
    }

    // ── Description + instruction text area (between headline and button) ──
    let text_top = 90;
    let text_bottom = button_y - 6;
    let text_area_h = text_bottom - text_top;

    if text_area_h < LINE_H_10PT {
        return; // Not enough space
    }

    let max_chars = ((screen_w - 20) / CHAR_W_10PT) as usize;
    let body_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_WARNING_BODY);
    let dim_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_DETAIL);

    // Build all text lines
    let mut all_lines: Vec<(String, bool)> = Vec::new(); // (text, is_dim)

    // Description
    if !alert.description.is_empty() {
        for line in crate::layout::word_wrap(&alert.description, max_chars) {
            all_lines.push((line, false));
        }
    }

    // Blank separator
    if !alert.description.is_empty() && !alert.instruction.is_empty() {
        all_lines.push((String::new(), false));
    }

    // Instruction
    if !alert.instruction.is_empty() {
        all_lines.push(("ACTION:".to_string(), false));
        for line in crate::layout::word_wrap(&alert.instruction, max_chars) {
            all_lines.push((line, false));
        }
    }

    // Metadata at bottom
    all_lines.push((String::new(), true));
    all_lines.push((
        format!("Severity: {}  Urgency: {}", alert.severity, alert.urgency),
        true,
    ));
    if !alert.expires.is_empty() {
        all_lines.push((
            format!("Expires: {}", crate::weather::format_alert_expiry(&alert.expires)),
            true,
        ));
    }

    let visible_lines = (text_area_h / LINE_H_10PT) as usize;
    let scroll = state.warning_scroll.min(all_lines.len().saturating_sub(visible_lines));

    for (i, (line, is_dim)) in all_lines.iter().skip(scroll).take(visible_lines).enumerate() {
        let y = text_top + (i as i32) * LINE_H_10PT + LINE_H_10PT;
        if line.is_empty() {
            continue;
        }
        let style = if *is_dim { dim_style } else { body_style };
        Text::new(line, Point::new(10, y), style)
            .draw(fb)
            .ok();
    }

    // Scroll indicator
    if all_lines.len() > visible_lines {
        let indicator_style = MonoTextStyle::new(&PROFONT_10_POINT, rgb(120, 80, 80));
        let indicator = format!(
            "[{}/{}]",
            scroll + 1,
            all_lines.len().saturating_sub(visible_lines) + 1
        );
        Text::with_alignment(
            &indicator,
            Point::new(screen_w - 8, text_top + LINE_H_10PT),
            indicator_style,
            Alignment::Right,
        )
        .draw(fb)
        .ok();
    }
}
