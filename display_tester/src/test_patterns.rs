use crate::display_config::{ByteOrder, ColorFormat, DisplayConfig};

pub fn fill_red_chunk(buf: &mut [u8], width: usize, height: usize, cfg: &DisplayConfig) {
    match cfg.format {
        ColorFormat::RGB565 | ColorFormat::BGR565 => {
            let pixel = red_rgb565(cfg.format);
            fill_16bpp(buf, width * height, pixel, cfg.byte_order);
        }
        ColorFormat::RGB666 | ColorFormat::BGR666 => {
            let pixel = red_rgb666(cfg.format);
            fill_24bpp(buf, width * height, pixel);
        }
        ColorFormat::RGB888 | ColorFormat::BGR888 => {
            let pixel = red_rgb888(cfg.format);
            fill_24bpp(buf, width * height, pixel);
        }
    }
}

fn red_rgb565(format: ColorFormat) -> u16 {
    match format {
        ColorFormat::BGR565 => 0x001F,
        _ => 0xF800,
    }
}

fn red_rgb666(format: ColorFormat) -> [u8; 3] {
    if format.is_bgr() {
        [0x00, 0x00, 0xFC]
    } else {
        [0xFC, 0x00, 0x00]
    }
}

fn red_rgb888(format: ColorFormat) -> [u8; 3] {
    if format.is_bgr() {
        [0x00, 0x00, 0xFF]
    } else {
        [0xFF, 0x00, 0x00]
    }
}

fn fill_16bpp(buf: &mut [u8], pixels: usize, value: u16, order: ByteOrder) {
    let hi = (value >> 8) as u8;
    let lo = (value & 0xFF) as u8;
    let (b0, b1) = match order {
        ByteOrder::BigEndian => (hi, lo),
        ByteOrder::LittleEndian => (lo, hi),
    };
    for i in 0..pixels {
        let idx = i * 2;
        buf[idx] = b0;
        buf[idx + 1] = b1;
    }
}

fn fill_24bpp(buf: &mut [u8], pixels: usize, value: [u8; 3]) {
    for i in 0..pixels {
        let idx = i * 3;
        buf[idx] = value[0];
        buf[idx + 1] = value[1];
        buf[idx + 2] = value[2];
    }
}
