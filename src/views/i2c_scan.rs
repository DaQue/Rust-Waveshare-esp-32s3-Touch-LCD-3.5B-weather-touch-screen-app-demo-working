use embedded_graphics::{
    mono_font::MonoTextStyle,
    prelude::*,
    primitives::{PrimitiveStyleBuilder, Rectangle},
    text::Text,
};
use profont::{PROFONT_14_POINT, PROFONT_12_POINT, PROFONT_10_POINT};
use crate::framebuffer::Framebuffer;
use crate::layout::*;
use crate::views::AppState;

pub fn draw(fb: &mut Framebuffer, state: &AppState) {
    // Fill background
    let bg_style = PrimitiveStyleBuilder::new().fill_color(BG_I2C).build();
    Rectangle::new(Point::zero(), Size::new(SCREEN_W as u32, SCREEN_H as u32))
        .into_styled(bg_style)
        .draw(fb)
        .ok();

    // Header line
    draw_hline(fb, HEADER_LINE_Y, LINE_COLOR_1);

    // Header text
    let header_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_HEADER);
    Text::new("I2C Scan", Point::new(14, 26), header_style)
        .draw(fb)
        .ok();

    // Card
    let card_w = SCREEN_W - 20;
    let card_h = SCREEN_H - 56 - INFO_CARD_Y;
    draw_card(
        fb,
        CARD_MARGIN, INFO_CARD_Y,
        card_w, card_h,
        CARD_RADIUS as u32,
        CARD_FILL_I2C, CARD_BORDER_I2C, 1,
    );

    if state.i2c_devices.is_empty() {
        let placeholder_style = MonoTextStyle::new(&PROFONT_12_POINT, TEXT_DETAIL);
        Text::new("No devices found", Point::new(24, 80), placeholder_style)
            .draw(fb)
            .ok();
    } else {
        let text_style = MonoTextStyle::new(&PROFONT_12_POINT, TEXT_SECONDARY);
        let mut x = 24;
        let mut y = 80;
        for (idx, addr) in state.i2c_devices.iter().enumerate() {
            let label = format!("0x{:02X}", addr);
            Text::new(&label, Point::new(x, y), text_style)
                .draw(fb)
                .ok();

            if (idx + 1) % 4 == 0 {
                x = 24;
                y += 24;
            } else {
                x += 68;
            }
        }
    }

    // Bottom text
    let bottom_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_BOTTOM);
    Text::new(&state.bottom_text, Point::new(BOTTOM_X, BOTTOM_Y), bottom_style)
        .draw(fb)
        .ok();
}
