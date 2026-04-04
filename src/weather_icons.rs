use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
use tinybmp::Bmp;

use crate::framebuffer::Framebuffer;

/// Icon background color in RGB565 (matches gen_icons.py BG = 0x14,0x19,0x23)
const ICON_BG: Rgb565 = Rgb565::new(0x14 >> 3, 0x19 >> 2, 0x23 >> 3);

/// Draw a BMP treating ICON_BG pixels as transparent.
fn draw_bmp_transparent(fb: &mut Framebuffer, bmp: &Bmp<Rgb565>, x: i32, y: i32) {
    let pixels = bmp
        .pixels()
        .filter(|p| p.1 != ICON_BG)
        .map(|p| Pixel(Point::new(x + p.0.x, y + p.0.y), p.1));
    fb.draw_iter(pixels).ok();
}

/// Weather icon identifiers mapped to OpenWeatherMap condition codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum WeatherIcon {
    #[default]
    Clear = 0,
    FewClouds = 1,
    ScatteredClouds = 2,
    BrokenClouds = 3,
    Overcast = 4,
    ShowerRain = 5,
    Rain = 6,
    Drizzle = 7,
    Thunderstorm = 8,
    Snow = 9,
    Atmosphere = 10,
    Mist = 11,
    Fog = 12,
}

// ── 80x80 icons (NOW view) ──────────────────────────────────────────

static ICON_CLEAR_80: &[u8] = include_bytes!("icons/clear_80.bmp");
static ICON_FEW_CLOUDS_80: &[u8] = include_bytes!("icons/few_clouds_80.bmp");
static ICON_SCATTERED_CLOUDS_80: &[u8] = include_bytes!("icons/scattered_clouds_80.bmp");
static ICON_BROKEN_CLOUDS_80: &[u8] = include_bytes!("icons/broken_clouds_80.bmp");
static ICON_OVERCAST_80: &[u8] = include_bytes!("icons/overcast_80.bmp");
static ICON_SHOWER_RAIN_80: &[u8] = include_bytes!("icons/shower_rain_80.bmp");
static ICON_RAIN_80: &[u8] = include_bytes!("icons/rain_80.bmp");
static ICON_DRIZZLE_80: &[u8] = include_bytes!("icons/drizzle_80.bmp");
static ICON_THUNDERSTORM_80: &[u8] = include_bytes!("icons/thunderstorm_80.bmp");
static ICON_SNOW_80: &[u8] = include_bytes!("icons/snow_80.bmp");
static ICON_ATMOSPHERE_80: &[u8] = include_bytes!("icons/atmosphere_80.bmp");
static ICON_MIST_80: &[u8] = include_bytes!("icons/mist_80.bmp");
static ICON_FOG_80: &[u8] = include_bytes!("icons/fog_80.bmp");

// ── 48x48 icons (forecast strip) ────────────────────────────────────

static ICON_CLEAR_48: &[u8] = include_bytes!("icons/clear_48.bmp");
static ICON_FEW_CLOUDS_48: &[u8] = include_bytes!("icons/few_clouds_48.bmp");
static ICON_SCATTERED_CLOUDS_48: &[u8] = include_bytes!("icons/scattered_clouds_48.bmp");
static ICON_BROKEN_CLOUDS_48: &[u8] = include_bytes!("icons/broken_clouds_48.bmp");
static ICON_OVERCAST_48: &[u8] = include_bytes!("icons/overcast_48.bmp");
static ICON_SHOWER_RAIN_48: &[u8] = include_bytes!("icons/shower_rain_48.bmp");
static ICON_RAIN_48: &[u8] = include_bytes!("icons/rain_48.bmp");
static ICON_DRIZZLE_48: &[u8] = include_bytes!("icons/drizzle_48.bmp");
static ICON_THUNDERSTORM_48: &[u8] = include_bytes!("icons/thunderstorm_48.bmp");
static ICON_SNOW_48: &[u8] = include_bytes!("icons/snow_48.bmp");
static ICON_ATMOSPHERE_48: &[u8] = include_bytes!("icons/atmosphere_48.bmp");
static ICON_MIST_48: &[u8] = include_bytes!("icons/mist_48.bmp");
static ICON_FOG_48: &[u8] = include_bytes!("icons/fog_48.bmp");

// ── 36x36 icons (forecast rows) ─────────────────────────────────────

static ICON_CLEAR_36: &[u8] = include_bytes!("icons/clear_36.bmp");
static ICON_FEW_CLOUDS_36: &[u8] = include_bytes!("icons/few_clouds_36.bmp");
static ICON_SCATTERED_CLOUDS_36: &[u8] = include_bytes!("icons/scattered_clouds_36.bmp");
static ICON_BROKEN_CLOUDS_36: &[u8] = include_bytes!("icons/broken_clouds_36.bmp");
static ICON_OVERCAST_36: &[u8] = include_bytes!("icons/overcast_36.bmp");
static ICON_SHOWER_RAIN_36: &[u8] = include_bytes!("icons/shower_rain_36.bmp");
static ICON_RAIN_36: &[u8] = include_bytes!("icons/rain_36.bmp");
static ICON_DRIZZLE_36: &[u8] = include_bytes!("icons/drizzle_36.bmp");
static ICON_THUNDERSTORM_36: &[u8] = include_bytes!("icons/thunderstorm_36.bmp");
static ICON_SNOW_36: &[u8] = include_bytes!("icons/snow_36.bmp");
static ICON_ATMOSPHERE_36: &[u8] = include_bytes!("icons/atmosphere_36.bmp");
static ICON_MIST_36: &[u8] = include_bytes!("icons/mist_36.bmp");
static ICON_FOG_36: &[u8] = include_bytes!("icons/fog_36.bmp");

impl WeatherIcon {
    fn bmp_data_80(self) -> &'static [u8] {
        match self {
            Self::Clear => ICON_CLEAR_80,
            Self::FewClouds => ICON_FEW_CLOUDS_80,
            Self::ScatteredClouds => ICON_SCATTERED_CLOUDS_80,
            Self::BrokenClouds => ICON_BROKEN_CLOUDS_80,
            Self::Overcast => ICON_OVERCAST_80,
            Self::ShowerRain => ICON_SHOWER_RAIN_80,
            Self::Rain => ICON_RAIN_80,
            Self::Drizzle => ICON_DRIZZLE_80,
            Self::Thunderstorm => ICON_THUNDERSTORM_80,
            Self::Snow => ICON_SNOW_80,
            Self::Atmosphere => ICON_ATMOSPHERE_80,
            Self::Mist => ICON_MIST_80,
            Self::Fog => ICON_FOG_80,
        }
    }

    fn bmp_data_48(self) -> &'static [u8] {
        match self {
            Self::Clear => ICON_CLEAR_48,
            Self::FewClouds => ICON_FEW_CLOUDS_48,
            Self::ScatteredClouds => ICON_SCATTERED_CLOUDS_48,
            Self::BrokenClouds => ICON_BROKEN_CLOUDS_48,
            Self::Overcast => ICON_OVERCAST_48,
            Self::ShowerRain => ICON_SHOWER_RAIN_48,
            Self::Rain => ICON_RAIN_48,
            Self::Drizzle => ICON_DRIZZLE_48,
            Self::Thunderstorm => ICON_THUNDERSTORM_48,
            Self::Snow => ICON_SNOW_48,
            Self::Atmosphere => ICON_ATMOSPHERE_48,
            Self::Mist => ICON_MIST_48,
            Self::Fog => ICON_FOG_48,
        }
    }

    fn bmp_data_36(self) -> &'static [u8] {
        match self {
            Self::Clear => ICON_CLEAR_36,
            Self::FewClouds => ICON_FEW_CLOUDS_36,
            Self::ScatteredClouds => ICON_SCATTERED_CLOUDS_36,
            Self::BrokenClouds => ICON_BROKEN_CLOUDS_36,
            Self::Overcast => ICON_OVERCAST_36,
            Self::ShowerRain => ICON_SHOWER_RAIN_36,
            Self::Rain => ICON_RAIN_36,
            Self::Drizzle => ICON_DRIZZLE_36,
            Self::Thunderstorm => ICON_THUNDERSTORM_36,
            Self::Snow => ICON_SNOW_36,
            Self::Atmosphere => ICON_ATMOSPHERE_36,
            Self::Mist => ICON_MIST_36,
            Self::Fog => ICON_FOG_36,
        }
    }

    /// Draw the 80x80 icon at the given position, treating BG color as transparent.
    pub fn draw_80(self, fb: &mut Framebuffer, x: i32, y: i32) {
        if let Ok(bmp) = Bmp::<Rgb565>::from_slice(self.bmp_data_80()) {
            draw_bmp_transparent(fb, &bmp, x, y);
        }
    }

    /// Draw the 48x48 icon at the given position, treating BG color as transparent.
    pub fn draw_48(self, fb: &mut Framebuffer, x: i32, y: i32) {
        if let Ok(bmp) = Bmp::<Rgb565>::from_slice(self.bmp_data_48()) {
            draw_bmp_transparent(fb, &bmp, x, y);
        }
    }

    /// Draw the 36x36 icon at the given position, treating BG color as transparent.
    pub fn draw_36(self, fb: &mut Framebuffer, x: i32, y: i32) {
        if let Ok(bmp) = Bmp::<Rgb565>::from_slice(self.bmp_data_36()) {
            draw_bmp_transparent(fb, &bmp, x, y);
        }
    }
}
