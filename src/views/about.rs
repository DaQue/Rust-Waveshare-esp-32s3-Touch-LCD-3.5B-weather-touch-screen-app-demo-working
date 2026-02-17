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
    let bg_style = PrimitiveStyleBuilder::new().fill_color(BG_ABOUT).build();
    Rectangle::new(Point::zero(), Size::new(SCREEN_W as u32, SCREEN_H as u32))
        .into_styled(bg_style)
        .draw(fb)
        .ok();

    // Header line
    draw_hline(fb, HEADER_LINE_Y, LINE_COLOR_1);

    // Header text
    let header_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_HEADER);
    Text::new("About", Point::new(14, 26), header_style)
        .draw(fb)
        .ok();

    // Card
    let card_w = SCREEN_W - 20;
    let card_h = SCREEN_H - 56 - INFO_CARD_Y;
    draw_card(
        fb,
        CARD_MARGIN, INFO_CARD_Y,
        card_w, card_h,
        16,
        CARD_FILL_INDOOR, CARD_BORDER_INDOOR, 1,
    );

    let text_style = MonoTextStyle::new(&PROFONT_12_POINT, TEXT_SECONDARY);

    // Device name
    Text::new("Waveshare ESP32-S3 3.5B", Point::new(24, 70), text_style)
        .draw(fb)
        .ok();

    // Firmware version
    Text::new("Firmware: waveshare_s3_3p v0.1", Point::new(24, 95), text_style)
        .draw(fb)
        .ok();

    // IP address
    let ip_text = if state.ip_address.is_empty() {
        "IP: not connected".to_string()
    } else {
        format!("IP: {}", state.ip_address)
    };
    Text::new(&ip_text, Point::new(24, 120), text_style)
        .draw(fb)
        .ok();

    // Free heap
    let heap_kb = unsafe { esp_idf_sys::esp_get_free_heap_size() } / 1024;
    let heap_text = format!("Free heap: {} KB", heap_kb);
    Text::new(&heap_text, Point::new(24, 145), text_style)
        .draw(fb)
        .ok();

    // Uptime
    let uptime_min = unsafe { esp_idf_sys::esp_timer_get_time() } / 60_000_000;
    let uptime_text = format!("Uptime: {} min", uptime_min);
    Text::new(&uptime_text, Point::new(24, 170), text_style)
        .draw(fb)
        .ok();

    // Author
    let small_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_DETAIL);
    Text::new("By David + Claude Code", Point::new(24, 200), small_style)
        .draw(fb)
        .ok();
    Text::new("github.com/david", Point::new(24, 216), small_style)
        .draw(fb)
        .ok();

    // Bottom text
    let bottom_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_BOTTOM);
    Text::new(&state.bottom_text, Point::new(BOTTOM_X, BOTTOM_Y), bottom_style)
        .draw(fb)
        .ok();
}
