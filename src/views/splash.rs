use embedded_graphics::{
    mono_font::MonoTextStyle,
    pixelcolor::Rgb565,
    prelude::*,
    text::{Alignment, Text},
};
use profont::{PROFONT_14_POINT, PROFONT_18_POINT, PROFONT_24_POINT};

use crate::{framebuffer, layout};

pub(crate) fn draw_splash(fb: &mut framebuffer::Framebuffer, status: &str) {
    let bg = layout::rgb(20, 24, 32);
    fb.clear_color(bg);
    let cx = (fb.size().width as i32) / 2;
    let cy = (fb.size().height as i32) / 2;

    let title_style = MonoTextStyle::new(&PROFONT_24_POINT, Rgb565::new(28, 56, 31));
    Text::with_alignment(
        "Weather Station",
        Point::new(cx, cy - 50),
        title_style,
        Alignment::Center,
    )
    .draw(fb)
    .ok();

    let sub_style = MonoTextStyle::new(&PROFONT_18_POINT, Rgb565::new(18, 36, 20));
    Text::with_alignment(
        "Waveshare ESP32-S3 3.5B",
        Point::new(cx, cy - 15),
        sub_style,
        Alignment::Center,
    )
    .draw(fb)
    .ok();

    let version_style = MonoTextStyle::new(&PROFONT_14_POINT, Rgb565::new(12, 28, 14));
    let version_text = format!("v{}", env!("CARGO_PKG_VERSION"));
    Text::with_alignment(
        &version_text,
        Point::new(cx, cy + 10),
        version_style,
        Alignment::Center,
    )
    .draw(fb)
    .ok();

    let status_style = MonoTextStyle::new(&PROFONT_14_POINT, Rgb565::new(12, 28, 14));
    Text::with_alignment(
        status,
        Point::new(cx, cy + 40),
        status_style,
        Alignment::Center,
    )
    .draw(fb)
    .ok();
}
