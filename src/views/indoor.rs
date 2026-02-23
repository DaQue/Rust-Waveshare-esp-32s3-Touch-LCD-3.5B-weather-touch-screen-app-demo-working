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
        (12, 168, screen_h - 230)
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

    // Get contiguous slices for graph rendering
    let temp_slice = state.indoor_temp_history.as_slices();
    let temp_data: Vec<f32> = temp_slice.0.iter().chain(temp_slice.1.iter()).copied().collect();
    let hum_slice = state.indoor_hum_history.as_slices();
    let hum_data: Vec<f32> = hum_slice.0.iter().chain(hum_slice.1.iter()).copied().collect();

    // Draw temperature line
    if temp_data.len() >= 2 {
        draw_line_graph(
            fb,
            &temp_data,
            graph_x, graph_y, graph_w, graph_h,
            GRAPH_TEMP_COLOR,
        );
    }

    // Draw humidity line
    if hum_data.len() >= 2 {
        draw_line_graph(
            fb,
            &hum_data,
            graph_x, graph_y, graph_w, graph_h,
            GRAPH_HUM_COLOR,
        );
    }

    // Y-axis labels (auto-scale based on data)
    let axis_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_TERTIARY);
    if !temp_data.is_empty() {
        let (min_v, max_v) = data_range(&temp_data);
        let top_label = format!("{:.0}", max_v);
        let bot_label = format!("{:.0}", min_v);
        Text::with_alignment(&top_label, Point::new(graph_x - 4, graph_y + 10), axis_style, Alignment::Right)
            .draw(fb).ok();
        Text::with_alignment(&bot_label, Point::new(graph_x - 4, graph_y + graph_h - 4), axis_style, Alignment::Right)
            .draw(fb).ok();
    }
    if !hum_data.is_empty() {
        let (min_v, max_v) = data_range(&hum_data);
        let top_label = format!("{:.0}%", max_v);
        let bot_label = format!("{:.0}%", min_v);
        Text::with_alignment(
            &top_label,
            Point::new(graph_x + graph_w - 4, graph_y + 10),
            axis_style,
            Alignment::Right,
        )
        .draw(fb)
        .ok();
        Text::with_alignment(
            &bot_label,
            Point::new(graph_x + graph_w - 4, graph_y + graph_h - 4),
            axis_style,
            Alignment::Right,
        )
        .draw(fb)
        .ok();
    }

    // X-axis label
    let samples = state.indoor_temp_history.len().max(state.indoor_hum_history.len());
    let minutes = (samples as u32 * 5) / 60; // 5s per sample
    let time_label = if minutes > 0 {
        format!("Last {} min", minutes)
    } else {
        "Collecting...".to_string()
    };
    Text::with_alignment(
        &time_label,
        Point::new(graph_x + graph_w / 2, graph_y + graph_h + 18),
        axis_style,
        Alignment::Center,
    )
    .draw(fb)
    .ok();

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
        "(swipe <-/-> or tap header to switch pages)",
        Point::new(screen_w / 2, screen_h - 4),
        hint_style,
        Alignment::Center,
    )
    .draw(fb)
    .ok();
}

/// Get min/max of data with padding, ignoring outliers via IQR filtering.
fn data_range(data: &[f32]) -> (f32, f32) {
    let clean: Vec<f32> = data.iter().copied().filter(|v| v.is_finite()).collect();
    if clean.is_empty() {
        return (0.0, 1.0);
    }
    let mut sorted = clean;
    sorted.sort_unstable_by(|a, b| a.total_cmp(b));
    let n = sorted.len();
    let q1 = sorted[n / 4];
    let q3 = sorted[(3 * n) / 4];
    let iqr = (q3 - q1).max(1.0);
    let lo = q1 - 1.5 * iqr;
    let hi = q3 + 1.5 * iqr;
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for &v in &sorted {
        if v >= lo && v <= hi {
            if v < min { min = v; }
            if v > max { max = v; }
        }
    }
    if min > max {
        // All values were outliers — use median
        min = sorted[n / 2] - 1.0;
        max = sorted[n / 2] + 1.0;
    }
    let pad = ((max - min) * 0.1).max(1.0);
    (min - pad, max + pad)
}

/// Draw a line graph of data points within the given rectangle.
fn draw_line_graph(
    fb: &mut Framebuffer,
    data: &[f32],
    x: i32, y: i32, w: i32, h: i32,
    color: Rgb565,
) {
    let clean: Vec<f32> = data.iter().copied().filter(|v| v.is_finite()).collect();
    if clean.len() < 2 {
        return;
    }

    let (min_v, max_v) = data_range(&clean);
    let range = (max_v - min_v).max(0.01);
    let line_style = PrimitiveStyle::with_stroke(color, 2);

    let n = clean.len();
    for i in 1..n {
        let x1 = x + ((i - 1) as i32 * w) / (n - 1) as i32;
        let x2 = x + (i as i32 * w) / (n - 1) as i32;
        let y1 = (y + h - ((clean[i - 1] - min_v) / range * h as f32) as i32).clamp(y, y + h);
        let y2 = (y + h - ((clean[i] - min_v) / range * h as f32) as i32).clamp(y, y + h);

        Line::new(Point::new(x1, y1), Point::new(x2, y2))
            .into_styled(line_style)
            .draw(fb)
            .ok();
    }
}
