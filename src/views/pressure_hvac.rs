// Combined "Pressure + HVAC (24h)" screen.
//
// Top:    Current pressure readouts (BME local + OWM remote)
// Middle: 24h pressure graph (solid BME, thin OWM overlay)
// Bottom: Compact HVAC 24h summary (heat/cool runtime, cycles, etc.)

use embedded_graphics::{
    mono_font::MonoTextStyle,
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Line, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle},
    text::{Alignment, Text},
};
use profont::PROFONT_14_POINT;

use crate::framebuffer::Framebuffer;
use crate::hvac::HvacState;
use crate::layout::*;
use crate::views::AppState;

// Graph colors
const COLOR_BME: Rgb565 = rgb(120, 220, 160);  // green for local sensor
const COLOR_OWM: Rgb565 = rgb(140, 160, 255);  // blue-purple for remote
const COLOR_HEAT: Rgb565 = rgb(255, 140, 60);  // warm orange
const COLOR_COOL: Rgb565 = rgb(80, 180, 255);  // cool blue
const COLOR_WARN: Rgb565 = rgb(255, 200, 60);  // yellow for short-cycle warnings
const GRAPH_GRID: Rgb565 = rgb(40, 48, 58);
const GRAPH_BG: Rgb565 = rgb(16, 20, 28);

pub fn draw(fb: &mut Framebuffer, state: &AppState) {
    let (screen_w, screen_h) = screen_size(state.orientation);
    let landscape = state.orientation.is_landscape();
    fb.clear_color(BG_HVAC);
    draw_hline(fb, HEADER_LINE_Y, LINE_COLOR_3);

    let header_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_HEADER);
    let label_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_TERTIARY);

    // ── Header ──────────────────────────────────────────────────────
    Text::new("Pressure + HVAC (24h)", Point::new(14, 24), header_style)
        .draw(fb)
        .ok();

    // ── Current pressure readouts ───────────────────────────────────
    let readout_y = 48;

    // BME280 (local)
    if let Some(p) = state
        .indoor_pressure
        .or_else(|| state.pressure_history.latest_bme())
    {
        let txt = format!("{:.1} hPa", p);
        let bme_style = MonoTextStyle::new(&PROFONT_14_POINT, COLOR_BME);
        Text::new("BME:", Point::new(14, readout_y), label_style)
            .draw(fb)
            .ok();
        Text::new(&txt, Point::new(56, readout_y), bme_style)
            .draw(fb)
            .ok();
    } else {
        Text::new("BME: Unavailable", Point::new(14, readout_y), label_style)
            .draw(fb)
            .ok();
    }

    // OpenWeather (remote)
    let owm_x = if landscape { 240 } else { 14 };
    let owm_y = if landscape { readout_y } else { readout_y + 16 };
    let live_owm = state
        .current_weather
        .as_ref()
        .and_then(|cw| (cw.pressure_hpa > 0).then_some(cw.pressure_hpa as f32));
    if let Some(p) = live_owm.or_else(|| state.pressure_history.latest_owm()) {
        let txt = format!("{:.0} hPa", p);
        let owm_style = MonoTextStyle::new(&PROFONT_14_POINT, COLOR_OWM);
        Text::new("OWM:", Point::new(owm_x, owm_y), label_style)
            .draw(fb)
            .ok();
        Text::new(&txt, Point::new(owm_x + 42, owm_y), owm_style)
            .draw(fb)
            .ok();
    } else {
        Text::new("OWM: Unavailable", Point::new(owm_x, owm_y), label_style)
            .draw(fb)
            .ok();
    }

    // Delta readout (right-aligned in landscape, or separate line in portrait)
    let delta_x = screen_w - 14;
    let delta_y = if landscape { readout_y } else { readout_y + 32 };
    // 12 samples = last hour at 5-min cadence; also used to normalize BME to sea-level on graph
    let bme_offset = state.pressure_history.delta_owm_bme_recent(12);
    if let Some(delta) = bme_offset {
        let sign = if delta >= 0.0 { "+" } else { "" };
        let txt = format!("Delta: {}{:.1}", sign, delta);
        Text::with_alignment(&txt, Point::new(delta_x, delta_y), label_style, Alignment::Right)
            .draw(fb)
            .ok();
    }

    // ── Pressure graph ──────────────────────────────────────────────
    let graph_top = if landscape { 64 } else { 84 };
    let graph_x = 44;
    let graph_w = screen_w - graph_x - 14;
    let hvac_box_h = if landscape { 72 } else { 90 };
    let graph_h = screen_h - graph_top - hvac_box_h - 40; // leave room for labels + HVAC box + hint
    let graph_h = graph_h.max(60); // minimum useful height

    // Graph background
    let bg_style = PrimitiveStyleBuilder::new().fill_color(GRAPH_BG).build();
    Rectangle::new(
        Point::new(graph_x, graph_top),
        Size::new(graph_w as u32, graph_h as u32),
    )
    .into_styled(bg_style)
    .draw(fb)
    .ok();

    // Grid lines (3 horizontal)
    let grid_style = PrimitiveStyle::with_stroke(GRAPH_GRID, 1);
    for i in 1..4 {
        let gy = graph_top + (graph_h * i) / 4;
        Line::new(
            Point::new(graph_x, gy),
            Point::new(graph_x + graph_w, gy),
        )
        .into_styled(grid_style)
        .draw(fb)
        .ok();
    }

    let phist = &state.pressure_history;
    let total_samples = phist.len();

    if total_samples >= 2 {
        let bme_pts_raw = phist.bme_series();
        let owm_pts = phist.owm_series();

        // Normalize BME to sea-level by adding the OWM-BME offset so both lines
        // share the same Y baseline. Falls back to raw values until offset is known.
        let bme_pts: Vec<(usize, f32)> = if let Some(off) = bme_offset {
            bme_pts_raw.iter().map(|&(i, v)| (i, v + off)).collect()
        } else {
            bme_pts_raw
        };

        // Compute Y range from the (possibly normalized) combined data with padding.
        let y_range_opt = {
            let mut lo = f32::INFINITY;
            let mut hi = f32::NEG_INFINITY;
            for &(_, v) in bme_pts.iter().chain(owm_pts.iter()) {
                if v.is_finite() {
                    if v < lo { lo = v; }
                    if v > hi { hi = v; }
                }
            }
            if lo.is_finite() && hi.is_finite() { Some((lo - 0.5, hi + 0.5)) } else { None }
        };

        if let Some((y_min, y_max)) = y_range_opt {
            let y_range = (y_max - y_min).max(0.1);

            // Draw OWM line first (thinner, behind BME)
            if owm_pts.len() >= 2 {
                draw_indexed_line(
                    fb,
                    &owm_pts,
                    total_samples,
                    graph_x, graph_top, graph_w, graph_h,
                    y_min, y_range,
                    COLOR_OWM,
                    1,
                );
            }

            // Draw BME line (thicker, on top) using normalized values
            if bme_pts.len() >= 2 {
                draw_indexed_line(
                    fb,
                    &bme_pts,
                    total_samples,
                    graph_x, graph_top, graph_w, graph_h,
                    y_min, y_range,
                    COLOR_BME,
                    2,
                );
            }

            // Y-axis labels (top and bottom of range)
            let axis_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_TERTIARY);
            let top_label = format!("{:.0}", y_max);
            let bot_label = format!("{:.0}", y_min);
            Text::with_alignment(
                &top_label,
                Point::new(graph_x - 4, graph_top + 10),
                axis_style,
                Alignment::Right,
            )
            .draw(fb)
            .ok();
            Text::with_alignment(
                &bot_label,
                Point::new(graph_x - 4, graph_top + graph_h - 4),
                axis_style,
                Alignment::Right,
            )
            .draw(fb)
            .ok();
        }
    } else {
        let msg_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_TERTIARY);
        Text::with_alignment(
            "Collecting pressure data...",
            Point::new(graph_x + graph_w / 2, graph_top + graph_h / 2),
            msg_style,
            Alignment::Center,
        )
        .draw(fb)
        .ok();
    }

    // X-axis labels
    let x_label_y = graph_top + graph_h + 14;
    Text::new("-24h", Point::new(graph_x, x_label_y), label_style)
        .draw(fb)
        .ok();
    Text::with_alignment(
        "Now",
        Point::new(graph_x + graph_w, x_label_y),
        label_style,
        Alignment::Right,
    )
    .draw(fb)
    .ok();

    // Min/max labels (compact, between graph and HVAC box)
    let minmax_y = x_label_y + 2;
    if let Some((blo, bhi)) = phist.bme_min_max() {
        let (blo, bhi) = if let Some(off) = bme_offset {
            (blo + off, bhi + off)
        } else {
            (blo, bhi)
        };
        let txt = format!("BME {:.0}-{:.0}", blo, bhi);
        let s = MonoTextStyle::new(&PROFONT_14_POINT, COLOR_BME);
        Text::with_alignment(
            &txt,
            Point::new(graph_x + graph_w / 3, minmax_y),
            s,
            Alignment::Center,
        )
        .draw(fb)
        .ok();
    }
    if let Some((olo, ohi)) = phist.owm_min_max() {
        let txt = format!("OWM {:.0}-{:.0}", olo, ohi);
        let s = MonoTextStyle::new(&PROFONT_14_POINT, COLOR_OWM);
        Text::with_alignment(
            &txt,
            Point::new(graph_x + 2 * graph_w / 3, minmax_y),
            s,
            Alignment::Center,
        )
        .draw(fb)
        .ok();
    }

    // ── HVAC 24h Summary box ────────────────────────────────────────
    let hvac_y = screen_h - hvac_box_h - 18;
    draw_hline(fb, hvac_y - 2, LINE_COLOR_3);

    let hvac_header_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_HEADER);
    Text::new("HVAC 24h Summary", Point::new(14, hvac_y + 14), hvac_header_style)
        .draw(fb)
        .ok();

    let hvac = &state.hvac;
    if hvac.history_count() < 10 {
        Text::new(
            "(collecting data...)",
            Point::new(14, hvac_y + 32),
            label_style,
        )
        .draw(fb)
        .ok();
    } else {
        let stats = hvac.stats();
        let h = &stats.heat;
        let c = &stats.cool;

        // Current state indicator
        let (state_color, state_label) = match hvac.state() {
            HvacState::Heating => (COLOR_HEAT, "HEATING"),
            HvacState::Cooling => (COLOR_COOL, "COOLING"),
            HvacState::Idle => (rgb(140, 148, 160), "IDLE"),
        };
        let state_style = MonoTextStyle::new(&PROFONT_14_POINT, state_color);
        Text::with_alignment(
            state_label,
            Point::new(screen_w - 14, hvac_y + 14),
            state_style,
            Alignment::Right,
        )
        .draw(fb)
        .ok();

        // Heat line
        let heat_y = hvac_y + 32;
        let heat_style = MonoTextStyle::new(&PROFONT_14_POINT, COLOR_HEAT);
        if h.cycles > 0 {
            let txt = format!(
                "Heat: {}h{:02}m | {} cyc | avg {:.0}m | max {}m",
                h.total_minutes / 60,
                h.total_minutes % 60,
                h.cycles,
                h.avg_cycle_mins,
                h.longest_cycle_mins,
            );
            Text::new(&txt, Point::new(14, heat_y), heat_style)
                .draw(fb)
                .ok();
        } else {
            Text::new("Heat: --", Point::new(14, heat_y), heat_style)
                .draw(fb)
                .ok();
        }

        // Cool line
        let cool_y = heat_y + 18;
        let cool_style = MonoTextStyle::new(&PROFONT_14_POINT, COLOR_COOL);
        if c.cycles > 0 {
            let txt = format!(
                "Cool: {}h{:02}m | {} cyc | avg {:.0}m | max {}m",
                c.total_minutes / 60,
                c.total_minutes % 60,
                c.cycles,
                c.avg_cycle_mins,
                c.longest_cycle_mins,
            );
            Text::new(&txt, Point::new(14, cool_y), cool_style)
                .draw(fb)
                .ok();
        } else {
            Text::new("Cool: --", Point::new(14, cool_y), cool_style)
                .draw(fb)
                .ok();
        }

        // Short-cycle warning (if any)
        let short_total = h.short_cycles + c.short_cycles;
        if short_total > 0 {
            let warn_y = cool_y + 18;
            let warn_style = MonoTextStyle::new(&PROFONT_14_POINT, COLOR_WARN);
            let txt = format!("Short cycles (<4m): {}", short_total);
            Text::new(&txt, Point::new(14, warn_y), warn_style)
                .draw(fb)
                .ok();
        }
    }

    // ── Bottom hint ─────────────────────────────────────────────────
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

/// Draw a line graph from indexed sparse points (skipping gaps).
/// `points` is `(sample_index, value)` sorted by index.
/// `total` is the full ring buffer count for X-axis scaling.
#[allow(clippy::too_many_arguments)]
fn draw_indexed_line(
    fb: &mut Framebuffer,
    points: &[(usize, f32)],
    total: usize,
    gx: i32, gy: i32, gw: i32, gh: i32,
    y_min: f32, y_range: f32,
    color: Rgb565,
    stroke: u32,
) {
    if points.len() < 2 || total < 2 {
        return;
    }
    let line_style = PrimitiveStyle::with_stroke(color, stroke);
    let max_idx = total.saturating_sub(1).max(1) as i32;

    for pair in points.windows(2) {
        let (i0, v0) = pair[0];
        let (i1, v1) = pair[1];
        let x0 = gx + (i0 as i32 * gw) / max_idx;
        let x1 = gx + (i1 as i32 * gw) / max_idx;
        let y0 = (gy + gh - ((v0 - y_min) / y_range * gh as f32) as i32).clamp(gy, gy + gh);
        let y1 = (gy + gh - ((v1 - y_min) / y_range * gh as f32) as i32).clamp(gy, gy + gh);

        Line::new(Point::new(x0, y0), Point::new(x1, y1))
            .into_styled(line_style)
            .draw(fb)
            .ok();
    }
}
