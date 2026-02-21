#[derive(Clone, Copy, Debug)]
pub enum ColorFormat {
    RGB565,
    RGB666,
    RGB888,
    BGR565,
    BGR666,
    BGR888,
}

impl ColorFormat {
    pub fn pixel_format_param(self) -> u8 {
        match self {
            ColorFormat::RGB565 | ColorFormat::BGR565 => 0x55,
            ColorFormat::RGB666 | ColorFormat::BGR666 => 0x66,
            ColorFormat::RGB888 | ColorFormat::BGR888 => 0x77,
        }
    }

    pub fn bytes_per_pixel(self) -> usize {
        match self {
            ColorFormat::RGB565 | ColorFormat::BGR565 => 2,
            ColorFormat::RGB666 | ColorFormat::BGR666 => 3,
            ColorFormat::RGB888 | ColorFormat::BGR888 => 3,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ColorFormat::RGB565 => "RGB565",
            ColorFormat::RGB666 => "RGB666",
            ColorFormat::RGB888 => "RGB888",
            ColorFormat::BGR565 => "BGR565",
            ColorFormat::BGR666 => "BGR666",
            ColorFormat::BGR888 => "BGR888",
        }
    }

    pub fn is_bgr(self) -> bool {
        matches!(self, ColorFormat::BGR565 | ColorFormat::BGR666 | ColorFormat::BGR888)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ByteOrder {
    LittleEndian,
    BigEndian,
}

impl ByteOrder {
    pub fn name(self) -> &'static str {
        match self {
            ByteOrder::LittleEndian => "Little-endian",
            ByteOrder::BigEndian => "Big-endian",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum BitOrder {
    MSBFirst,
    LSBFirst,
}

impl BitOrder {
    pub fn name(self) -> &'static str {
        match self {
            BitOrder::MSBFirst => "MSB First",
            BitOrder::LSBFirst => "LSB First",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum QspiMode {
    Quad,
    Dual,
    Single,
}

impl QspiMode {
    pub fn name(self) -> &'static str {
        match self {
            QspiMode::Quad => "Quad",
            QspiMode::Dual => "Dual",
            QspiMode::Single => "Single",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DisplayConfig {
    pub format: ColorFormat,
    pub byte_order: ByteOrder,
    pub bit_order: BitOrder,
    pub qspi_mode: QspiMode,
    pub timing: crate::timing::TimingConfig,
}
