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
    let bg_style = PrimitiveStyleBuilder::new().fill_color(BG_WIFI).build();
    Rectangle::new(Point::zero(), Size::new(SCREEN_W as u32, SCREEN_H as u32))
        .into_styled(bg_style)
        .draw(fb)
        .ok();

    // Header line
    draw_hline(fb, HEADER_LINE_Y, LINE_COLOR_1);

    // Header text
    let header_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_HEADER);
    Text::new("WiFi Scan", Point::new(14, 26), header_style)
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
        CARD_FILL_WIFI, CARD_BORDER_WIFI, 1,
    );

    if state.wifi_networks.is_empty() {
        let placeholder_style = MonoTextStyle::new(&PROFONT_12_POINT, TEXT_DETAIL);
        Text::new("Scanning...", Point::new(24, 80), placeholder_style)
            .draw(fb)
            .ok();
    } else {
        let ssid_style = MonoTextStyle::new(&PROFONT_12_POINT, TEXT_SECONDARY);
        let rssi_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_DETAIL);

        for (i, (ssid, rssi)) in state.wifi_networks.iter().take(8).enumerate() {
            let y = 80 + (i as i32) * 20;

            // SSID
            Text::new(ssid, Point::new(24, y), ssid_style)
                .draw(fb)
                .ok();

            // Signal bars
            let bars = if *rssi >= -50 {
                "||||"
            } else if *rssi >= -60 {
                "|||."
            } else if *rssi >= -70 {
                "||.."
            } else {
                "|..."
            };
            Text::new(bars, Point::new(SCREEN_W - 80, y), rssi_style)
                .draw(fb)
                .ok();
        }
    }

    // Bottom text
    let bottom_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_BOTTOM);
    Text::new(&state.bottom_text, Point::new(BOTTOM_X, BOTTOM_Y), bottom_style)
        .draw(fb)
        .ok();
}
