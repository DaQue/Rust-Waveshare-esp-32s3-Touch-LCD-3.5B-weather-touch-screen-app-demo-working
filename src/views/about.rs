use crate::framebuffer::Framebuffer;
use crate::layout::*;
use crate::views::AppState;
use embedded_graphics::{
    mono_font::MonoTextStyle,
    prelude::*,
    text::{Alignment, Text},
};
use profont::{PROFONT_10_POINT, PROFONT_14_POINT};

const FW_VERSION: &str = concat!("v", env!("CARGO_PKG_VERSION"));

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
        CARD_MARGIN,
        card_y,
        screen_w - 2 * CARD_MARGIN,
        card_h,
        CARD_RADIUS as u32,
        CARD_FILL_INDOOR,
        CARD_BORDER_INDOOR,
        1,
    );

    let portrait = state.orientation.is_portrait();
    let label_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_SECONDARY);
    // Portrait: PROFONT_10 for values so "Waveshare ESP32-S3 3.5B" (23 chars)
    // fits right of the label column (vx=100: 100 + 23*6 = 238 < 320).
    let value_style = if portrait {
        MonoTextStyle::new(&PROFONT_10_POINT, TEXT_TERTIARY)
    } else {
        MonoTextStyle::new(&PROFONT_14_POINT, TEXT_TERTIARY)
    };
    let lx = CARD_MARGIN + 16;
    let vx = if portrait { 100 } else { 160 };
    let mut y = card_y + 26;
    let line_h = 26;

    // Device
    Text::new("Device", Point::new(lx, y), label_style)
        .draw(fb)
        .ok();
    Text::new("Waveshare ESP32-S3 3.5B", Point::new(vx, y), value_style)
        .draw(fb)
        .ok();
    y += line_h;

    // Firmware
    Text::new("Firmware", Point::new(lx, y), label_style)
        .draw(fb)
        .ok();
    Text::new(FW_VERSION, Point::new(vx, y), value_style)
        .draw(fb)
        .ok();
    y += line_h;

    // IP
    Text::new("IP", Point::new(lx, y), label_style)
        .draw(fb)
        .ok();
    let ip = if state.ip_address.is_empty() {
        "not connected"
    } else {
        &state.ip_address
    };
    Text::new(ip, Point::new(vx, y), value_style).draw(fb).ok();
    y += line_h;

    // WiFi
    Text::new("WiFi", Point::new(lx, y), label_style)
        .draw(fb)
        .ok();
    let wifi_info = if state.wifi_networks.is_empty() {
        "no scan data".to_string()
    } else {
        format!("{} networks found", state.wifi_networks.len())
    };
    Text::new(&wifi_info, Point::new(vx, y), value_style)
        .draw(fb)
        .ok();
    y += line_h;

    // Free heap
    let heap_kb = unsafe { esp_idf_sys::esp_get_free_heap_size() } / 1024;
    Text::new("Free heap", Point::new(lx, y), label_style)
        .draw(fb)
        .ok();
    let heap_text = format!("{} KB", heap_kb);
    Text::new(&heap_text, Point::new(vx, y), value_style)
        .draw(fb)
        .ok();
    y += line_h;

    // SRAM largest contiguous block — real TLS viability indicator
    let sram_block =
        unsafe { esp_idf_sys::heap_caps_get_largest_free_block(esp_idf_sys::MALLOC_CAP_INTERNAL) }
            / 1024;
    Text::new("SRAM block", Point::new(lx, y), label_style)
        .draw(fb)
        .ok();
    let sram_text = format!("{} KB", sram_block);
    Text::new(&sram_text, Point::new(vx, y), value_style)
        .draw(fb)
        .ok();
    y += line_h;

    // Uptime
    let uptime_secs = unsafe { esp_idf_sys::esp_timer_get_time() } / 1_000_000;
    let hours = uptime_secs / 3600;
    let mins = (uptime_secs % 3600) / 60;
    Text::new("Uptime", Point::new(lx, y), label_style)
        .draw(fb)
        .ok();
    let uptime_text = format!("{}h {}m", hours, mins);
    Text::new(&uptime_text, Point::new(vx, y), value_style)
        .draw(fb)
        .ok();
    y += line_h + 6;

    // Author
    let small_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_DETAIL);
    Text::new("By David + Claude Code", Point::new(lx, y), small_style)
        .draw(fb)
        .ok();

    // Bottom hint
    let hint_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_BOTTOM);
    Text::with_alignment(
        "--> Settings  <-- WiFi Scan  |  hold = menu",
        Point::new(screen_w / 2, screen_h - 4),
        hint_style,
        Alignment::Center,
    )
    .draw(fb)
    .ok();
}
