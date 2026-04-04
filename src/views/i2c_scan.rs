use crate::framebuffer::Framebuffer;
use crate::layout::*;
use crate::views::AppState;
use embedded_graphics::{
    mono_font::MonoTextStyle,
    prelude::*,
    primitives::{PrimitiveStyleBuilder, Rectangle},
    text::Text,
};
use profont::{PROFONT_10_POINT, PROFONT_12_POINT, PROFONT_14_POINT};

/// Map known I2C addresses to device names.
fn device_name(addr: u8) -> &'static str {
    match addr {
        0x18 => "QMI8658 IMU",
        0x20 => "PCF8574 GPIO Exp",
        0x34 => "AXP2101 PMIC",
        0x3B => "FT6x36 Touch",
        0x51 => "EEPROM",
        0x6B => "QMI8658 IMU",
        0x76 | 0x77 => "BME280 Sensor",
        _ => "",
    }
}

pub fn draw(fb: &mut Framebuffer, state: &AppState) {
    let (screen_w, screen_h) = screen_size(state.orientation);
    // Fill background
    let bg_style = PrimitiveStyleBuilder::new().fill_color(BG_I2C).build();
    Rectangle::new(Point::zero(), Size::new(screen_w as u32, screen_h as u32))
        .into_styled(bg_style)
        .draw(fb)
        .ok();

    // Header line
    draw_hline(fb, HEADER_LINE_Y, LINE_COLOR_1);

    // Header text
    let header_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_HEADER);
    let count = state.i2c_devices.len();
    let title = format!(
        "I2C Bus  ({} device{})",
        count,
        if count == 1 { "" } else { "s" }
    );
    Text::new(&title, Point::new(14, 26), header_style)
        .draw(fb)
        .ok();

    // Card
    let card_w = screen_w - 20;
    let card_h = screen_h - 56 - INFO_CARD_Y;
    draw_card(
        fb,
        CARD_MARGIN,
        INFO_CARD_Y,
        card_w,
        card_h,
        CARD_RADIUS as u32,
        CARD_FILL_I2C,
        CARD_BORDER_I2C,
        1,
    );

    if state.i2c_devices.is_empty() {
        let placeholder_style = MonoTextStyle::new(&PROFONT_12_POINT, TEXT_DETAIL);
        Text::new("No devices found", Point::new(24, 80), placeholder_style)
            .draw(fb)
            .ok();
    } else {
        let addr_style = MonoTextStyle::new(&PROFONT_12_POINT, TEXT_SECONDARY);
        let name_style = MonoTextStyle::new(&PROFONT_12_POINT, TEXT_DETAIL);

        for (i, addr) in state.i2c_devices.iter().enumerate() {
            let y = 76 + (i as i32) * 24;
            if y > screen_h - 60 {
                break;
            }

            let addr_text = format!("0x{:02X}", addr);
            Text::new(&addr_text, Point::new(24, y), addr_style)
                .draw(fb)
                .ok();

            let name = device_name(*addr);
            if !name.is_empty() {
                Text::new(name, Point::new(90, y), name_style).draw(fb).ok();
            }
        }
    }

    // Bottom nav hint
    let bottom_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_BOTTOM);
    Text::new(
        "--> WiFi Scan  |  hold = menu",
        Point::new(12, screen_h - 12),
        bottom_style,
    )
    .draw(fb)
    .ok();
}
