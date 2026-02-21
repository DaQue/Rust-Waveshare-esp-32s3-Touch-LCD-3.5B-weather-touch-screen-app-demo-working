use embedded_graphics::{
    mono_font::MonoTextStyle,
    prelude::*,
    text::{Alignment, Text},
};
use profont::{PROFONT_14_POINT, PROFONT_12_POINT, PROFONT_10_POINT};
use crate::framebuffer::Framebuffer;
use crate::layout::*;
use crate::views::AppState;

const FW_LABEL: &str = concat!("waveshare_s3_3p v", env!("CARGO_PKG_VERSION"));

pub fn draw(fb: &mut Framebuffer, state: &AppState) {
    let (screen_w, screen_h) = screen_size(state.orientation);
    fb.clear_color(BG_ABOUT);
    draw_hline(fb, HEADER_LINE_Y, LINE_COLOR_1);

    let header_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_HEADER);
    Text::new("About", Point::new(14, 24), header_style)
        .draw(fb)
        .ok();

    // Card
    let card_y = 38;
    let card_h = screen_h - card_y - 20;
    draw_card(
        fb,
        CARD_MARGIN, card_y,
        screen_w - 2 * CARD_MARGIN, card_h,
        CARD_RADIUS as u32,
        CARD_FILL_INDOOR, CARD_BORDER_INDOOR, 1,
    );

    let label_style = MonoTextStyle::new(&PROFONT_12_POINT, TEXT_SECONDARY);
    let value_style = MonoTextStyle::new(&PROFONT_12_POINT, TEXT_TERTIARY);
    let lx = CARD_MARGIN + 16;
    let vx = if state.orientation.is_portrait() { 132 } else { 200 };
    let mut y = card_y + 24;
    let line_h = 24;

    // Device
    Text::new("Device", Point::new(lx, y), label_style).draw(fb).ok();
    Text::new("Waveshare ESP32-S3 3.5B", Point::new(vx, y), value_style).draw(fb).ok();
    y += line_h;

    // Firmware
    Text::new("Firmware", Point::new(lx, y), label_style).draw(fb).ok();
    Text::new(FW_LABEL, Point::new(vx, y), value_style).draw(fb).ok();
    y += line_h;

    // IP
    Text::new("IP", Point::new(lx, y), label_style).draw(fb).ok();
    let ip = if state.ip_address.is_empty() { "not connected" } else { &state.ip_address };
    Text::new(ip, Point::new(vx, y), value_style).draw(fb).ok();
    y += line_h;

    // WiFi
    Text::new("WiFi", Point::new(lx, y), label_style).draw(fb).ok();
    let wifi_info = if state.wifi_networks.is_empty() {
        "no scan data".to_string()
    } else {
        format!("{} networks found", state.wifi_networks.len())
    };
    Text::new(&wifi_info, Point::new(vx, y), value_style).draw(fb).ok();
    y += line_h;

    // BME280
    Text::new("BME280", Point::new(lx, y), label_style).draw(fb).ok();
    let bme_info = if state.indoor_temp.is_some() {
        "connected".to_string()
    } else {
        "not found".to_string()
    };
    Text::new(&bme_info, Point::new(vx, y), value_style).draw(fb).ok();
    y += line_h;

    // Free heap
    let heap_kb = unsafe { esp_idf_sys::esp_get_free_heap_size() } / 1024;
    Text::new("Free heap", Point::new(lx, y), label_style).draw(fb).ok();
    let heap_text = format!("{} KB", heap_kb);
    Text::new(&heap_text, Point::new(vx, y), value_style).draw(fb).ok();
    y += line_h;

    // Uptime
    let uptime_secs = unsafe { esp_idf_sys::esp_timer_get_time() } / 1_000_000;
    let hours = uptime_secs / 3600;
    let mins = (uptime_secs % 3600) / 60;
    Text::new("Uptime", Point::new(lx, y), label_style).draw(fb).ok();
    let uptime_text = format!("{}h {}m", hours, mins);
    Text::new(&uptime_text, Point::new(vx, y), value_style).draw(fb).ok();
    y += line_h;

    // I2C devices
    Text::new("I2C devices", Point::new(lx, y), label_style).draw(fb).ok();
    let i2c_text = if state.i2c_devices.is_empty() {
        "none".to_string()
    } else {
        state.i2c_devices.iter().map(|a| format!("0x{:02X}", a)).collect::<Vec<_>>().join(" ")
    };
    Text::new(&i2c_text, Point::new(vx, y), value_style).draw(fb).ok();
    y += line_h + 4;

    // Author
    let small_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_DETAIL);
    Text::new("By David + Claude Code", Point::new(lx, y), small_style).draw(fb).ok();

    // Bottom hint
    let hint_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_BOTTOM);
    Text::with_alignment(
        "(swipe <-/-> or tap header to switch pages)",
        Point::new(screen_w / 2, screen_h - 4),
        hint_style,
        Alignment::Center,
    )
    .draw(fb)
    .ok();
}
