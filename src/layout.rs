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

// ── Warning view colors ─────────────────────────────────────────────

pub const BG_WARNING: Rgb565 = rgb(48, 8, 8);
pub const CARD_FILL_SILENCE: Rgb565 = rgb(180, 40, 40);
pub const CARD_BORDER_SILENCE: Rgb565 = rgb(255, 80, 80);
pub const CARD_FILL_SILENCED: Rgb565 = rgb(40, 60, 40);
pub const CARD_BORDER_SILENCED: Rgb565 = rgb(80, 140, 80);
pub const TEXT_WARNING_TITLE: Rgb565 = rgb(255, 100, 100);
pub const TEXT_WARNING_BODY: Rgb565 = rgb(240, 230, 220);
pub const TEXT_SILENCE_BUTTON: Rgb565 = rgb(255, 255, 255);

// ── Text colors ─────────────────────────────────────────────────────

pub const TEXT_HEADER: Rgb565 = rgb(222, 225, 230);
pub const TEXT_STATUS: Rgb565 = rgb(182, 187, 196);
pub const TEXT_PRIMARY: Rgb565 = rgb(232, 235, 240);
pub const TEXT_SECONDARY: Rgb565 = rgb(225, 228, 233);
pub const TEXT_TERTIARY: Rgb565 = rgb(188, 196, 208);
pub const TEXT_DETAIL: Rgb565 = rgb(184, 189, 198);
pub const TEXT_CONDITION: Rgb565 = rgb(166, 208, 255);
pub const TEXT_BOTTOM: Rgb565 = rgb(140, 148, 160);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Landscape,
    LandscapeFlipped,
    Portrait,
    PortraitFlipped,
}

impl Orientation {
    pub fn is_landscape(self) -> bool {
        matches!(self, Orientation::Landscape | Orientation::LandscapeFlipped)
    }

    pub fn is_portrait(self) -> bool {
        matches!(self, Orientation::Portrait | Orientation::PortraitFlipped)
    }
}

// ── Layout constants ────────────────────────────────────────────────

pub const SCREEN_W_LANDSCAPE: i32 = 480;
pub const SCREEN_H_LANDSCAPE: i32 = 320;
pub const SCREEN_W_PORTRAIT: i32 = 320;
pub const SCREEN_H_PORTRAIT: i32 = 480;

pub const HEADER_LINE_Y: i32 = 30;
pub const CARD_MARGIN: i32 = 8;
pub const CARD_RADIUS: i32 = 12;

// FORECAST view layout
pub const FORECAST_ROWS: usize = 4;

// I2C / WiFi / About card
pub const INFO_CARD_Y: i32 = 40;

pub fn screen_w(orientation: Orientation) -> i32 {
    if orientation.is_landscape() {
        SCREEN_W_LANDSCAPE
    } else {
        SCREEN_W_PORTRAIT
    }
}

pub fn screen_h(orientation: Orientation) -> i32 {
    if orientation.is_landscape() {
        SCREEN_H_LANDSCAPE
    } else {
        SCREEN_H_PORTRAIT
    }
}

pub fn screen_size(orientation: Orientation) -> (i32, i32) {
    (screen_w(orientation), screen_h(orientation))
}

// ── Helpers ─────────────────────────────────────────────────────────

use crate::framebuffer::Framebuffer;
use embedded_graphics::primitives::{PrimitiveStyleBuilder, Rectangle, RoundedRectangle};

/// Fill a horizontal line across the full screen width.
pub fn draw_hline(fb: &mut Framebuffer, y: i32, color: Rgb565) {
    let style = PrimitiveStyleBuilder::new().fill_color(color).build();
    Rectangle::new(Point::new(0, y), Size::new(fb.size().width, 1))
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
