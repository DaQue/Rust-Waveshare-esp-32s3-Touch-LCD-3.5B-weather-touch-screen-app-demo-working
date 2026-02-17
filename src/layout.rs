use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::*;

/// Convert 8-bit RGB to Rgb565.
pub const fn rgb(r: u8, g: u8, b: u8) -> Rgb565 {
    Rgb565::new(r >> 3, g >> 2, b >> 3)
}

// ── Background colors (from C factory drawing_screen_canvas.c) ──────

/// NOW view background
pub const BG_NOW: Rgb565 = rgb(27, 31, 39);
/// INDOOR view background
pub const BG_INDOOR: Rgb565 = rgb(22, 28, 38);
/// FORECAST view background (same as NOW)
pub const BG_FORECAST: Rgb565 = rgb(27, 31, 39);
/// I2C scan background
pub const BG_I2C: Rgb565 = rgb(27, 31, 39);
/// WiFi scan background
pub const BG_WIFI: Rgb565 = rgb(24, 30, 39);
/// About view background
pub const BG_ABOUT: Rgb565 = rgb(22, 28, 38);

// ── Line / separator colors ─────────────────────────────────────────

pub const LINE_COLOR_1: Rgb565 = rgb(56, 63, 76);
pub const LINE_COLOR_2: Rgb565 = rgb(45, 52, 64);
pub const LINE_COLOR_3: Rgb565 = rgb(58, 70, 84);

// ── Card colors ─────────────────────────────────────────────────────

pub const CARD_FILL_NOW: Rgb565 = rgb(20, 25, 35);
pub const CARD_BORDER_NOW: Rgb565 = rgb(63, 75, 95);
pub const CARD_FILL_FORECAST_PREVIEW: Rgb565 = rgb(23, 29, 40);
pub const CARD_BORDER_FORECAST_PREVIEW: Rgb565 = rgb(66, 86, 108);
pub const CARD_FILL_FORECAST: Rgb565 = rgb(24, 29, 39);
pub const CARD_BORDER_FORECAST: Rgb565 = rgb(63, 75, 95);
pub const CARD_FILL_INDOOR: Rgb565 = rgb(20, 29, 40);
pub const CARD_BORDER_INDOOR: Rgb565 = rgb(66, 86, 108);
pub const CARD_FILL_I2C: Rgb565 = rgb(22, 27, 37);
pub const CARD_BORDER_I2C: Rgb565 = rgb(63, 75, 95);
pub const CARD_FILL_WIFI: Rgb565 = rgb(20, 29, 40);
pub const CARD_BORDER_WIFI: Rgb565 = rgb(66, 86, 108);

// ── Text colors ─────────────────────────────────────────────────────

pub const TEXT_HEADER: Rgb565 = rgb(222, 225, 230);
pub const TEXT_STATUS: Rgb565 = rgb(182, 187, 196);
pub const TEXT_PRIMARY: Rgb565 = rgb(232, 235, 240);
pub const TEXT_SECONDARY: Rgb565 = rgb(225, 228, 233);
pub const TEXT_TERTIARY: Rgb565 = rgb(188, 196, 208);
pub const TEXT_DETAIL: Rgb565 = rgb(184, 189, 198);
pub const TEXT_CONDITION: Rgb565 = rgb(166, 208, 255);
pub const TEXT_WEATHER: Rgb565 = rgb(214, 218, 226);
pub const TEXT_BOTTOM: Rgb565 = rgb(140, 148, 160);

// ── Layout constants (landscape: 480w x 320h) ──────────────────────

pub const SCREEN_W: i32 = 480;
pub const SCREEN_H: i32 = 320;

pub const HEADER_LINE_Y: i32 = 30;
pub const HEADER_LINE2_Y: i32 = 34;

pub const CARD_MARGIN: i32 = 8;
pub const CARD_RADIUS: i32 = 12;

// NOW view layout (landscape: weather card + stats + preview)
pub const NOW_CARD_Y: i32 = 38;
pub const NOW_CARD_H: i32 = 140;
pub const NOW_ICON_X: i32 = 20;
pub const NOW_TEMP_X: i32 = 160;
pub const NOW_CONDITION_X: i32 = 160;
pub const NOW_CONDITION_Y: i32 = 120;
pub const NOW_WEATHER_X: i32 = 160;
pub const NOW_WEATHER_Y: i32 = 142;
pub const NOW_DIVIDER_Y: i32 = 184;
pub const NOW_STATS_X: i32 = 12;
pub const NOW_STATS_Y1: i32 = 198;
pub const NOW_STATS_Y2: i32 = 214;

// Preview cards at bottom of NOW view
pub const PREVIEW_Y: i32 = 234;
pub const PREVIEW_H: i32 = 50;
pub const PREVIEW_GAP: i32 = 6;

// FORECAST view layout
pub const FORECAST_ROW_Y_BASE: i32 = 40;
pub const FORECAST_ROW_STRIDE: i32 = 56;
pub const FORECAST_ROW_H: i32 = 50;
pub const FORECAST_ICON_X: i32 = 19;
pub const FORECAST_TITLE_X: i32 = 80;
pub const FORECAST_DETAIL_X: i32 = 80;
pub const FORECAST_ROWS: usize = 4;
pub const PREVIEW_DAYS: usize = 3;

// INDOOR view layout
pub const INDOOR_CARD_Y: i32 = 40;
pub const INDOOR_TEMP_X: i32 = 24;
pub const INDOOR_TEMP_Y: i32 = 70;
pub const INDOOR_HUM_X: i32 = 24;
pub const INDOOR_HUM_Y: i32 = 140;
pub const INDOOR_PRESS_X: i32 = 24;
pub const INDOOR_PRESS_Y: i32 = 210;

// I2C / WiFi / About card
pub const INFO_CARD_Y: i32 = 40;

pub const BOTTOM_Y: i32 = SCREEN_H - 12;
pub const BOTTOM_X: i32 = 12;

// ── Helpers ─────────────────────────────────────────────────────────

use crate::framebuffer::Framebuffer;
use embedded_graphics::primitives::{PrimitiveStyleBuilder, Rectangle, RoundedRectangle};

/// Fill a horizontal line across the full screen width.
pub fn draw_hline(fb: &mut Framebuffer, y: i32, color: Rgb565) {
    let style = PrimitiveStyleBuilder::new().fill_color(color).build();
    Rectangle::new(Point::new(0, y), Size::new(SCREEN_W as u32, 1))
        .into_styled(style)
        .draw(fb)
        .ok();
}

/// Draw a filled rounded rectangle with border (card style).
#[allow(clippy::too_many_arguments)]
pub fn draw_card(
    fb: &mut Framebuffer,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    radius: u32,
    fill: Rgb565,
    border: Rgb565,
    border_width: u32,
) {
    // Draw border rect first (slightly larger)
    if border_width > 0 {
        let style = PrimitiveStyleBuilder::new()
            .fill_color(border)
            .build();
        RoundedRectangle::with_equal_corners(
            Rectangle::new(Point::new(x, y), Size::new(w as u32, h as u32)),
            Size::new(radius, radius),
        )
        .into_styled(style)
        .draw(fb)
        .ok();

        // Draw fill inside
        let bw = border_width as i32;
        let inner_style = PrimitiveStyleBuilder::new().fill_color(fill).build();
        RoundedRectangle::with_equal_corners(
            Rectangle::new(
                Point::new(x + bw, y + bw),
                Size::new((w - 2 * bw) as u32, (h - 2 * bw) as u32),
            ),
            Size::new(radius.saturating_sub(border_width), radius.saturating_sub(border_width)),
        )
        .into_styled(inner_style)
        .draw(fb)
        .ok();
    } else {
        let style = PrimitiveStyleBuilder::new().fill_color(fill).build();
        RoundedRectangle::with_equal_corners(
            Rectangle::new(Point::new(x, y), Size::new(w as u32, h as u32)),
            Size::new(radius, radius),
        )
        .into_styled(style)
        .draw(fb)
        .ok();
    }
}
