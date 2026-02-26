use embedded_graphics::{
    mono_font::MonoTextStyle,
    pixelcolor::Rgb565,
    prelude::*,
    text::{Alignment, Text},
};
use profont::{PROFONT_14_POINT, PROFONT_24_POINT};

use crate::framebuffer::Framebuffer;
use crate::hvac::HvacState;
use crate::layout::*;
use crate::views::AppState;

const COLOR_HEAT: Rgb565 = rgb(255, 140, 60);   // warm orange
const COLOR_COOL: Rgb565 = rgb(80, 180, 255);   // cool blue
const COLOR_IDLE: Rgb565 = rgb(140, 148, 160);   // gray

pub fn draw(fb: &mut Framebuffer, state: &AppState) {
    let (screen_w, screen_h) = screen_size(state.orientation);
    fb.clear_color(BG_HVAC);
    draw_hline(fb, HEADER_LINE_Y, LINE_COLOR_3);

    // Header
    let header_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_HEADER);
    Text::new("HVAC (24h)", Point::new(14, 24), header_style)
        .draw(fb)
        .ok();

    let label_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_TERTIARY);
    let body_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_PRIMARY);

    let hvac = &state.hvac;

    // Check if we have enough data
    if hvac.history_count() < 120 {
        let msg_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_TERTIARY);
        Text::with_alignment(
            "(collecting data...)",
            Point::new(screen_w / 2, 80),
            msg_style,
            Alignment::Center,
        )
        .draw(fb)
        .ok();

        let samples = hvac.history_count();
        let detail = format!("{}/120 samples", samples);
        Text::with_alignment(
            &detail,
            Point::new(screen_w / 2, 100),
            msg_style,
            Alignment::Center,
        )
        .draw(fb)
        .ok();
    }

    // Current state (always show)
    let now_ms = crate::now_ms();
    let current = hvac.state();
    let (state_color, state_text) = match current {
        HvacState::Heating => (COLOR_HEAT, "HEATING"),
        HvacState::Cooling => (COLOR_COOL, "COOLING"),
        HvacState::Idle => (COLOR_IDLE, "IDLE"),
    };

    let big_style = MonoTextStyle::new(&PROFONT_24_POINT, state_color);
    Text::new(state_text, Point::new(14, 68), big_style)
        .draw(fb)
        .ok();

    let dur_secs = hvac.state_duration_secs(now_ms);
    let dur_m = dur_secs / 60;
    let dur_s = dur_secs % 60;
    let dur_text = format!("for {}m {}s", dur_m, dur_s);
    Text::new(&dur_text, Point::new(200, 68), label_style)
        .draw(fb)
        .ok();

    // Stats sections
    if hvac.history_count() >= 60 {
        let stats = hvac.stats();
        let section_y = 100;

        // HEAT section
        let heat_color_style = MonoTextStyle::new(&PROFONT_14_POINT, COLOR_HEAT);
        Text::new("HEAT", Point::new(14, section_y), heat_color_style)
            .draw(fb)
            .ok();

        let h = &stats.heat;
        if h.cycles > 0 {
            let line1 = format!(
                "Runtime: {}h {}m  Cycles: {}",
                h.total_minutes / 60,
                h.total_minutes % 60,
                h.cycles
            );
            Text::new(&line1, Point::new(14, section_y + 18), body_style)
                .draw(fb)
                .ok();

            let line2 = format!(
                "Avg: {:.0}m  Longest: {}m",
                h.avg_cycle_mins, h.longest_cycle_mins
            );
            Text::new(&line2, Point::new(14, section_y + 36), body_style)
                .draw(fb)
                .ok();

            if h.short_cycles > 0 {
                let warn = format!("Short cycles: {}", h.short_cycles);
                let warn_style = MonoTextStyle::new(&PROFONT_14_POINT, rgb(255, 200, 60));
                Text::new(&warn, Point::new(14, section_y + 54), warn_style)
                    .draw(fb)
                    .ok();
            }
        } else {
            Text::new("No heating detected", Point::new(14, section_y + 18), label_style)
                .draw(fb)
                .ok();
        }

        // COOL section
        let cool_y = section_y + 76;
        let cool_color_style = MonoTextStyle::new(&PROFONT_14_POINT, COLOR_COOL);
        Text::new("COOL", Point::new(14, cool_y), cool_color_style)
            .draw(fb)
            .ok();

        let c = &stats.cool;
        if c.cycles > 0 {
            let line1 = format!(
                "Runtime: {}h {}m  Cycles: {}",
                c.total_minutes / 60,
                c.total_minutes % 60,
                c.cycles
            );
            Text::new(&line1, Point::new(14, cool_y + 18), body_style)
                .draw(fb)
                .ok();

            let line2 = format!(
                "Avg: {:.0}m  Longest: {}m",
                c.avg_cycle_mins, c.longest_cycle_mins
            );
            Text::new(&line2, Point::new(14, cool_y + 36), body_style)
                .draw(fb)
                .ok();

            if c.short_cycles > 0 {
                let warn = format!("Short cycles: {}", c.short_cycles);
                let warn_style = MonoTextStyle::new(&PROFONT_14_POINT, rgb(255, 200, 60));
                Text::new(&warn, Point::new(14, cool_y + 54), warn_style)
                    .draw(fb)
                    .ok();
            }
        } else {
            Text::new("No cooling detected", Point::new(14, cool_y + 18), label_style)
                .draw(fb)
                .ok();
        }

        // History duration
        let hist_text = format!(
            "History: {}h {}m",
            stats.history_minutes / 60,
            stats.history_minutes % 60
        );
        Text::with_alignment(
            &hist_text,
            Point::new(screen_w - 14, section_y),
            MonoTextStyle::new(&PROFONT_14_POINT, TEXT_TERTIARY),
            Alignment::Right,
        )
        .draw(fb)
        .ok();
    }

    // Bottom hint
    let hint_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_BOTTOM);
    Text::with_alignment(
        "(swipe <-/-> or tap header to switch pages)",
        Point::new(screen_w / 2, screen_h - 4),
        hint_style,
        Alignment::Center,
    )
    .draw(fb)
    .ok();
}
