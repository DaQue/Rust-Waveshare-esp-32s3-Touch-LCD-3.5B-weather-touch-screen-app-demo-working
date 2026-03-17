use embedded_graphics::{
    mono_font::MonoTextStyle,
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, Line, Rectangle, PrimitiveStyle},
    text::{Alignment, Text},
};
use profont::{PROFONT_18_POINT, PROFONT_14_POINT, PROFONT_12_POINT, PROFONT_10_POINT};
use crate::framebuffer::Framebuffer;
use crate::layout::*;
use crate::views::AppState;

// Icon colors
const SUN_COLOR: Rgb565    = rgb(255, 210, 80);
const BAR_COLOR: Rgb565    = rgb(80,  170, 220);
const GEAR_COLOR: Rgb565   = rgb(160, 175, 195);

/// Which group button was tapped, if any.
pub enum NavTap {
    Weather,
    Sensors,
    System,
}

/// Layout for the three group buttons.
/// Returns [(x, y, w, h); 3] for Weather, Sensors, System.
fn button_rects(orientation: Orientation) -> [(i32, i32, i32, i32); 3] {
    if orientation.is_landscape() {
        let bw = 148;
        let bh = 260;
        let by = 38;
        [
            (8,   by, bw,     bh),
            (164, by, bw,     bh),
            (320, by, bw + 4, bh),
        ]
    } else {
        let bw = 304;
        let bh = 126;
        [
            (8, 40,  bw, bh),
            (8, 174, bw, bh),
            (8, 308, bw, bh),
        ]
    }
}

/// Return which group button was tapped, if any.
pub fn hit_test(x: i16, y: i16, orientation: Orientation) -> Option<NavTap> {
    let rects = button_rects(orientation);
    for (i, (bx, by, bw, bh)) in rects.iter().enumerate() {
        if x >= *bx as i16 && x < (*bx + *bw) as i16
            && y >= *by as i16 && y < (*by + *bh) as i16
        {
            return Some(match i {
                0 => NavTap::Weather,
                1 => NavTap::Sensors,
                _ => NavTap::System,
            });
        }
    }
    None
}

/// Sun icon: filled circle + 8 rays.
fn draw_sun(fb: &mut Framebuffer, cx: i32, cy: i32) {
    let style = PrimitiveStyle::with_fill(SUN_COLOR);
    let ray = PrimitiveStyle::with_stroke(SUN_COLOR, 2);

    // Core circle
    Circle::new(Point::new(cx - 11, cy - 11), 22)
        .into_styled(style)
        .draw(fb).ok();

    // 8 rays (cardinal + diagonal)
    let rays: [(i32, i32, i32, i32); 8] = [
        (cx,      cy - 14, cx,      cy - 22), // N
        (cx + 10, cy - 10, cx + 16, cy - 16), // NE
        (cx + 14, cy,      cx + 22, cy     ), // E
        (cx + 10, cy + 10, cx + 16, cy + 16), // SE
        (cx,      cy + 14, cx,      cy + 22), // S
        (cx - 10, cy + 10, cx - 16, cy + 16), // SW
        (cx - 14, cy,      cx - 22, cy     ), // W
        (cx - 10, cy - 10, cx - 16, cy - 16), // NW
    ];
    for (x1, y1, x2, y2) in rays {
        Line::new(Point::new(x1, y1), Point::new(x2, y2))
            .into_styled(ray)
            .draw(fb).ok();
    }
}

/// Bar chart icon: 3 vertical bars of varying height.
fn draw_bars(fb: &mut Framebuffer, cx: i32, cy: i32) {
    let style = PrimitiveStyle::with_fill(BAR_COLOR);
    let bar_w = 9i32;
    let gap   = 5i32;
    let base  = cy + 18; // bottom of chart
    let heights = [28i32, 38, 22];
    let total_w = bar_w * 3 + gap * 2;
    let left = cx - total_w / 2;

    for (i, &h) in heights.iter().enumerate() {
        let bx = left + i as i32 * (bar_w + gap);
        Rectangle::new(Point::new(bx, base - h), Size::new(bar_w as u32, h as u32))
            .into_styled(style)
            .draw(fb).ok();
    }
}

/// Three horizontal lines icon (settings / menu).
fn draw_lines(fb: &mut Framebuffer, cx: i32, cy: i32) {
    let style = PrimitiveStyle::with_fill(GEAR_COLOR);
    let w = 36i32;
    let h = 4i32;
    let offsets = [-12i32, 0, 12];
    for dy in offsets {
        Rectangle::new(
            Point::new(cx - w / 2, cy + dy - h / 2),
            Size::new(w as u32, h as u32),
        )
        .into_styled(style)
        .draw(fb).ok();
    }
}

pub fn draw(fb: &mut Framebuffer, state: &AppState) {
    let (screen_w, screen_h) = screen_size(state.orientation);
    fb.clear_color(BG_ABOUT);
    draw_hline(fb, HEADER_LINE_Y, LINE_COLOR_1);

    let header_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_HEADER);
    Text::new("Go To", Point::new(14, 24), header_style).draw(fb).ok();

    let hint_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_BOTTOM);
    Text::with_alignment(
        "tap a group  |  swipe -> for home",
        Point::new(screen_w / 2, screen_h - 4),
        hint_style,
        Alignment::Center,
    ).draw(fb).ok();

    let rects = button_rects(state.orientation);
    // Landscape cards are 148px wide; PROFONT_12 is 7px/char → max ~21 chars.
    // "Indoor / HVAC / Pressure" (24 chars) overflows — shorten for landscape.
    let sensors_sub = if state.orientation.is_landscape() {
        "Indoor / HVAC"
    } else {
        "Indoor / HVAC / Pressure"
    };
    let labels   = [("Weather", "Now & Forecast"),
                    ("Sensors", sensors_sub),
                    ("System",  "Scan & About")];
    let fills    = [CARD_FILL_FORECAST, CARD_FILL_INDOOR, CARD_FILL_I2C];
    let borders  = [CARD_BORDER_FORECAST, CARD_BORDER_INDOOR, CARD_BORDER_I2C];

    let title_style = MonoTextStyle::new(&PROFONT_18_POINT, TEXT_PRIMARY);
    let sub_style   = MonoTextStyle::new(&PROFONT_12_POINT, TEXT_TERTIARY);

    for i in 0..3 {
        let (bx, by, bw, bh) = rects[i];
        draw_card(fb, bx, by, bw, bh, CARD_RADIUS as u32, fills[i], borders[i], 1);

        let cx  = bx + bw / 2;
        // Icon in upper third of button, text in lower half
        let icon_cy = by + bh / 3;
        let ty      = by + bh * 58 / 100;
        let sy      = ty + 22;

        match i {
            0 => draw_sun(fb, cx, icon_cy),
            1 => draw_bars(fb, cx, icon_cy),
            _ => draw_lines(fb, cx, icon_cy),
        }

        Text::with_alignment(labels[i].0, Point::new(cx, ty), title_style, Alignment::Center)
            .draw(fb).ok();
        Text::with_alignment(labels[i].1, Point::new(cx, sy), sub_style,   Alignment::Center)
            .draw(fb).ok();
    }
}
