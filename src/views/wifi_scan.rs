use embedded_graphics::{
    mono_font::MonoTextStyle,
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{PrimitiveStyleBuilder, Rectangle},
    text::Text,
};
use profont::{PROFONT_14_POINT, PROFONT_12_POINT, PROFONT_10_POINT};
use crate::framebuffer::Framebuffer;
use crate::layout::*;
use crate::views::AppState;

pub fn draw(fb: &mut Framebuffer, state: &AppState) {
    let (screen_w, screen_h) = screen_size(state.orientation);
    // Fill background
    let bg_style = PrimitiveStyleBuilder::new().fill_color(BG_WIFI).build();
    Rectangle::new(Point::zero(), Size::new(screen_w as u32, screen_h as u32))
        .into_styled(bg_style)
        .draw(fb)
        .ok();

    // Header line
    draw_hline(fb, HEADER_LINE_Y, LINE_COLOR_1);

    // Header text
    let header_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_HEADER);
    let count = state.wifi_networks.len();
    let title = format!("WiFi  ({} network{})", count, if count == 1 { "" } else { "s" });
    Text::new(&title, Point::new(14, 26), header_style)
        .draw(fb)
        .ok();

    // Card
    let card_w = screen_w - 20;
    let card_h = screen_h - 56 - INFO_CARD_Y;
    draw_card(
        fb,
        CARD_MARGIN, INFO_CARD_Y,
        card_w, card_h,
        CARD_RADIUS as u32,
        CARD_FILL_WIFI, CARD_BORDER_WIFI, 1,
    );

    if state.wifi_networks.is_empty() {
        let placeholder_style = MonoTextStyle::new(&PROFONT_12_POINT, TEXT_DETAIL);
        let msg = if state.wifi_scan_pending { "Scanning..." } else { "No networks found" };
        Text::new(msg, Point::new(24, 80), placeholder_style)
            .draw(fb)
            .ok();
    } else {
        // Sort by signal strength (strongest first)
        let mut sorted: Vec<_> = state.wifi_networks.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));

        let connected_color = Rgb565::new(8, 48, 8); // green
        let normal_style = MonoTextStyle::new(&PROFONT_12_POINT, TEXT_SECONDARY);
        let connected_style = MonoTextStyle::new(&PROFONT_12_POINT, connected_color);
        let rssi_normal = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_DETAIL);
        let rssi_connected = MonoTextStyle::new(&PROFONT_10_POINT, connected_color);

        for (i, (ssid, rssi)) in sorted.iter().take(10).enumerate() {
            let y = 76 + (i as i32) * 22;
            if y > screen_h - 60 {
                break;
            }

            let is_connected = !state.wifi_ssid.is_empty() && *ssid == state.wifi_ssid;
            let style = if is_connected { connected_style } else { normal_style };
            let r_style = if is_connected { rssi_connected } else { rssi_normal };

            // Truncate long SSIDs
            let display_ssid = if ssid.len() > 22 {
                format!("{}...", &ssid[..19])
            } else {
                ssid.to_string()
            };

            Text::new(&display_ssid, Point::new(24, y), style)
                .draw(fb)
                .ok();

            let rssi_text = format!("{} dBm", rssi);
            Text::new(&rssi_text, Point::new(screen_w - 110, y), r_style)
                .draw(fb)
                .ok();
        }
    }

    // Bottom text
    let bottom_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_BOTTOM);
    Text::new(&state.bottom_text, Point::new(12, screen_h - 12), bottom_style)
        .draw(fb)
        .ok();
}
