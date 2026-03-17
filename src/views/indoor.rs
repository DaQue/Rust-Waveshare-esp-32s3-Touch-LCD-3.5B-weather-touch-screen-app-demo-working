use embedded_graphics::{
    mono_font::MonoTextStyle,
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Line, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle},
    text::{Alignment, Text},
};
use profont::{PROFONT_14_POINT, PROFONT_24_POINT};

use crate::framebuffer::Framebuffer;
use crate::layout::*;
use crate::views::AppState;

// Graph colors
const GRAPH_TEMP_COLOR: Rgb565 = rgb(255, 140, 60);   // warm orange
const GRAPH_HUM_COLOR: Rgb565 = rgb(80, 180, 255);    // cool blue
const GRAPH_GRID_COLOR: Rgb565 = rgb(40, 48, 58);     // subtle grid
const GRAPH_BG: Rgb565 = rgb(16, 20, 28);             // dark graph area

pub fn draw(fb: &mut Framebuffer, state: &AppState) {
    let (screen_w, screen_h) = screen_size(state.orientation);
    fb.clear_color(BG_INDOOR);
    draw_hline(fb, HEADER_LINE_Y, LINE_COLOR_3);

    // Header
    let header_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_HEADER);
    Text::new("Indoor", Point::new(14, 24), header_style)
        .draw(fb)
        .ok();

    // Current readings
    let reading_y = 52;
    let primary_style = MonoTextStyle::new(&PROFONT_24_POINT, TEXT_PRIMARY);
    let label_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_TERTIARY);

    if state.indoor_temp.is_none() && state.indoor_humidity.is_none() {
        let warn_color = crate::layout::rgb(255, 80, 80);
        let warn_style = MonoTextStyle::new(&PROFONT_14_POINT, warn_color);
        Text::new("BME280 sensor unavailable", Point::new(14, reading_y), warn_style)
            .draw(fb)
            .ok();
        Text::new("(check I2C wiring)", Point::new(14, reading_y + 18), label_style)
            .draw(fb)
            .ok();
    }

    if let Some(temp) = state.indoor_temp {
        let t = if state.use_celsius {
            format!("{:.1}°C", (temp - 32.0) * 5.0 / 9.0)
        } else {
            format!("{:.1}°F", temp)
        };
        let x = 20;
        let y = reading_y;
        Text::new(&t, Point::new(x, y), primary_style)
            .draw(fb)
            .ok();
        Text::new("TEMP", Point::new(x, y + 20), label_style)
            .draw(fb)
            .ok();
    }

    if let Some(hum) = state.indoor_humidity {
        let t = format!("{:.0}%", hum);
        let (x, y) = if state.orientation.is_portrait() {
            (20, reading_y + 34)
        } else {
            (200, reading_y)
        };
        Text::new(&t, Point::new(x, y), primary_style)
            .draw(fb)
            .ok();
        Text::new("HUMIDITY", Point::new(x, y + 20), label_style)
            .draw(fb)
            .ok();
    }

    if let Some(press) = state.indoor_pressure {
        let t = format!("{:.0}", press);
        let (x, y) = if state.orientation.is_portrait() {
            (20, reading_y + 68)
        } else {
            (350, reading_y)
        };
        Text::new(&t, Point::new(x, y), primary_style)
            .draw(fb)
            .ok();
        Text::new("hPa", Point::new(x, y + 20), label_style)
            .draw(fb)
            .ok();
    }

    // Graph area
    let (graph_x, graph_y, graph_h) = if state.orientation.is_portrait() {
        (46, 168, screen_h - 230)  // 46 gives Y-axis labels room (right-aligned at x=42)
    } else {
        (50, 88, 180)
    };
    let graph_w = screen_w - graph_x - 16;

    // Graph background
    let bg_style = PrimitiveStyleBuilder::new().fill_color(GRAPH_BG).build();
    Rectangle::new(
        Point::new(graph_x, graph_y),
        Size::new(graph_w as u32, graph_h as u32),
    )
    .into_styled(bg_style)
    .draw(fb)
    .ok();

    // Horizontal grid lines (4 lines)
    let grid_style = PrimitiveStyle::with_stroke(GRAPH_GRID_COLOR, 1);
    for i in 1..4 {
        let gy = graph_y + (graph_h * i) / 4;
        Line::new(
            Point::new(graph_x, gy),
            Point::new(graph_x + graph_w, gy),
        )
        .into_styled(grid_style)
        .draw(fb)
        .ok();
    }

    // Select short (2h) or long (24h) dataset based on toggle flag
    let axis_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_TERTIARY);
    if state.plot_range_short {
        let (temp_s0, temp_s1) = state.indoor_temp_history.as_slices();
        let (hum_s0,  hum_s1)  = state.indoor_hum_history.as_slices();
        let temp_len = state.indoor_temp_history.len();
        let hum_len  = state.indoor_hum_history.len();
        let samples  = temp_len.max(hum_len);
        let time_label = if samples > 0 { format!("~{}m", (samples as u32 * 15) / 60) }
                         else           { "Collecting...".to_string() };
        draw_graph_data(fb, temp_s0, temp_s1, temp_len, hum_s0, hum_s1, hum_len,
                        graph_x, graph_y, graph_w, graph_h, axis_style, "-2h", &time_label);
    } else {
        let (temp_s0, temp_s1) = state.indoor_temp_hist_long.as_slices();
        let (hum_s0,  hum_s1)  = state.indoor_hum_hist_long.as_slices();
        let temp_len = state.indoor_temp_hist_long.len();
        let hum_len  = state.indoor_hum_hist_long.len();
        let samples  = temp_len.max(hum_len);
        let time_label = if samples > 0 { format!("~{}h", (samples as u32 * 3) / 60) }
                         else           { "Collecting...".to_string() };
        draw_graph_data(fb, temp_s0, temp_s1, temp_len, hum_s0, hum_s1, hum_len,
                        graph_x, graph_y, graph_w, graph_h, axis_style, "-24h", &time_label);
    }

    // Range label (Now) on right side of x-axis
    Text::with_alignment(
        "Now",
        Point::new(graph_x + graph_w, graph_y + graph_h + 14),
        axis_style,
        Alignment::Right,
    ).draw(fb).ok();

    // Legend
    let legend_y = graph_y + graph_h + 36;
    let legend_style = MonoTextStyle::new(&PROFONT_14_POINT, GRAPH_TEMP_COLOR);
    Text::new("-- Temp", Point::new(graph_x, legend_y), legend_style)
        .draw(fb).ok();
    let legend_style2 = MonoTextStyle::new(&PROFONT_14_POINT, GRAPH_HUM_COLOR);
    Text::new("-- Humidity", Point::new(graph_x + 138, legend_y), legend_style2)
        .draw(fb).ok();

    // Bottom hint
    let hint_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_BOTTOM);
    Text::with_alignment(
        "(tap graph: 2h/24h | swipe <-/-> to switch pages)",
        Point::new(screen_w / 2, screen_h - 4),
        hint_style,
        Alignment::Center,
    )
    .draw(fb)
    .ok();
}

/// Draw temp + humidity line graphs and axis labels for one time window.
#[allow(clippy::too_many_arguments)]
fn draw_graph_data(
    fb: &mut Framebuffer,
    temp_s0: &[f32], temp_s1: &[f32], temp_len: usize,
    hum_s0:  &[f32], hum_s1:  &[f32], hum_len:  usize,
    graph_x: i32, graph_y: i32, graph_w: i32, graph_h: i32,
    axis_style: MonoTextStyle<Rgb565>,
    left_label: &str, center_label: &str,
) {
    if temp_len >= 2 {
        draw_line_graph(fb, temp_s0, temp_s1, graph_x, graph_y, graph_w, graph_h, GRAPH_TEMP_COLOR);
    }
    if hum_len >= 2 {
        draw_line_graph(fb, hum_s0, hum_s1, graph_x, graph_y, graph_w, graph_h, GRAPH_HUM_COLOR);
    }
    if temp_len > 0 {
        let (min_v, max_v) = data_range(temp_s0, temp_s1);
        Text::with_alignment(&format!("{:.0}", max_v), Point::new(graph_x - 4, graph_y + 10),          axis_style, Alignment::Right).draw(fb).ok();
        Text::with_alignment(&format!("{:.0}", min_v), Point::new(graph_x - 4, graph_y + graph_h - 4), axis_style, Alignment::Right).draw(fb).ok();
    }
    if hum_len > 0 {
        let (min_v, max_v) = data_range(hum_s0, hum_s1);
        Text::with_alignment(&format!("{:.0}%", max_v), Point::new(graph_x + graph_w - 4, graph_y + 10),          axis_style, Alignment::Right).draw(fb).ok();
        Text::with_alignment(&format!("{:.0}%", min_v), Point::new(graph_x + graph_w - 4, graph_y + graph_h - 4), axis_style, Alignment::Right).draw(fb).ok();
    }
    Text::new(left_label, Point::new(graph_x, graph_y + graph_h + 14), axis_style).draw(fb).ok();
    Text::with_alignment(center_label, Point::new(graph_x + graph_w / 2, graph_y + graph_h + 18), axis_style, Alignment::Center).draw(fb).ok();
}

/// Get min/max of data with 10% padding. Zero allocations — scans both slices directly.
fn data_range(s0: &[f32], s1: &[f32]) -> (f32, f32) {
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for &v in s0.iter().chain(s1.iter()) {
        if v.is_finite() {
            if v < min { min = v; }
            if v > max { max = v; }
        }
    }
    if !min.is_finite() {
        return (0.0, 1.0);
    }
    let pad = ((max - min) * 0.1).max(1.0);
    (min - pad, max + pad)
}

/// Draw a line graph from two VecDeque slices. Zero allocations — iterates directly.
#[allow(clippy::too_many_arguments)]
fn draw_line_graph(
    fb: &mut Framebuffer,
    s0: &[f32], s1: &[f32],
    x: i32, y: i32, w: i32, h: i32,
    color: Rgb565,
) {
    // Count finite values for x-axis scaling
    let n: usize = s0.iter().chain(s1.iter()).filter(|v| v.is_finite()).count();
    if n < 2 { return; }

    let (min_v, max_v) = data_range(s0, s1);
    let range = (max_v - min_v).max(0.01);
    let line_style = PrimitiveStyle::with_stroke(color, 2);

    let mut prev: Option<(i32, i32)> = None;
    let mut idx: usize = 0;
    for &v in s0.iter().chain(s1.iter()) {
        if !v.is_finite() { continue; }
        let px = x + (idx as i32 * w) / (n - 1) as i32;
        let py = (y + h - ((v - min_v) / range * h as f32) as i32).clamp(y, y + h);
        if let Some((px1, py1)) = prev {
            Line::new(Point::new(px1, py1), Point::new(px, py))
                .into_styled(line_style)
                .draw(fb)
                .ok();
        }
        prev = Some((px, py));
        idx += 1;
    }
}
