mod bme280_sensor;
mod config;
mod console;
mod debug_flags;
mod framebuffer;
mod http_client;
mod hvac;
mod layout;
mod pressure_history;
mod qmi8658;
mod speaker;
mod touch;
mod time_sync;
mod views;
mod weather;
mod weather_icons;
mod wifi;

use anyhow::Result;
use core::ffi::c_void;
use esp_idf_hal::i2c::{I2cConfig, I2cDriver};
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::units::Hertz;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::nvs::{EspDefaultNvsPartition, EspNvs};
use log::info;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

// ── Display geometry (physical panel is 320x480 portrait) ───────────
const PANEL_WIDTH: i32 = 320;
const CHUNK_LINES: i32 = 20;

// ── SPI / QSPI ─────────────────────────────────────────────────────
const PCLK_HZ: u32 = 40_000_000;

// ── Pins (match C factory exactly) ─────────────────────────────────
const PIN_LCD_SCLK: i32 = 5;
const PIN_LCD_D0: i32 = 1;
const PIN_LCD_D1: i32 = 2;
const PIN_LCD_D2: i32 = 3;
const PIN_LCD_D3: i32 = 4;
const PIN_LCD_CS: i32 = 12;
const PIN_LCD_BL: i32 = 6;
const PIN_I2S_BCLK: i32 = 13;
const PIN_I2S_DOUT: i32 = 16;
const PIN_I2S_WS: i32 = 15;
const PIN_I2S_MCLK: i32 = 44;

// ── I2C ──────────────────────────────────────────────────────────────
const I2C_FREQ_HZ: u32 = 100_000;

// ── Timing ──────────────────────────────────────────────────────────
const WEATHER_INTERVAL_SECS: u64 = 600;
const WEATHER_RETRY_SECS: u64 = 30;
const WEATHER_STALE_AFTER_SECS: u64 = WEATHER_INTERVAL_SECS + 120;
const ALERTS_INTERVAL_SECS: u64 = 180;
const ALERTS_START_DELAY_SECS: u64 = 20;
const ALERT_BEEP_COOLDOWN_SECS: u64 = 600;
const WARNING_BEEP_INTERVAL_MS: u32 = 20_000;
const WATCH_BEEP_INTERVAL_MS: u32 = 10_000;
const BME280_INTERVAL_MS: u32 = 5_000;
const HVAC_DETECT_INTERVAL_MS: u32 = 5_000;
const HVAC_RECORD_INTERVAL_MS: u32 = 30_000;
const PRESSURE_SAMPLE_INTERVAL_MS: u32 = pressure_history::SAMPLE_PERIOD_SECS * 1000;
const BME_TEMP_MIN_F: f32 = -40.0;
const BME_TEMP_MAX_F: f32 = 185.0;
const BME_PRESSURE_MIN_HPA: f32 = 300.0;
const BME_PRESSURE_MAX_HPA: f32 = 1200.0;
const BME_TEMP_MAX_STEP_F: f32 = 8.0;
const BME_HUM_MAX_STEP: f32 = 20.0;
const BME_PRESSURE_MAX_STEP_HPA: f32 = 12.0;
const TICK_MS: u64 = 100;
const TIME_UPDATE_TICKS: u32 = 10; // every second
const WIFI_DEBUG_TICKS: u32 = 100; // every 10 seconds
const ORIENTATION_POLL_TICKS: u32 = 2; // every 200ms
const ORIENTATION_SWITCH_MARGIN_G: f32 = 0.22;
const ORIENTATION_MAX_Z_G: f32 = 0.85;
const ORIENTATION_MIN_AXIS_G: f32 = 0.62;
const ORIENTATION_CONFIRM_SAMPLES: u8 = 4;
const ORIENTATION_CHANGE_COOLDOWN_MS: u32 = 1_200;
const WIFI_RETRY_INTERVAL_MS: u32 = 300_000;
const FAILURE_WARN_EVERY: u32 = 10;

// ── FFI structs matching the C AXS15231B driver ────────────────────

#[repr(C)]
struct Axs15231bLcdInitCmd {
    cmd: i32,
    data: *const c_void,
    data_bytes: usize,
    delay_ms: u32,
}

unsafe impl Sync for Axs15231bLcdInitCmd {}

#[repr(C)]
struct Axs15231bVendorFlags {
    use_qspi_interface: u32,
}

#[repr(C)]
struct Axs15231bVendorConfig {
    init_cmds: *const Axs15231bLcdInitCmd,
    init_cmds_size: u16,
    flags: Axs15231bVendorFlags,
}

extern "C" {
    fn esp_lcd_new_panel_axs15231b(
        io: esp_idf_sys::esp_lcd_panel_io_handle_t,
        panel_dev_config: *const esp_idf_sys::esp_lcd_panel_dev_config_t,
        ret_panel: *mut esp_idf_sys::esp_lcd_panel_handle_t,
    ) -> esp_idf_sys::esp_err_t;
    fn board_power_init() -> esp_idf_sys::esp_err_t;
}

// ── LCD init command data (byte-for-byte identical to C factory) ───

static LCD_INIT_CMD_00: [u8; 8] = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x5A, 0xA5];
static LCD_INIT_CMD_01: [u8; 17] = [
    0xC0, 0x10, 0x00, 0x02, 0x00, 0x00, 0x04, 0x3F, 0x20, 0x05, 0x3F, 0x3F, 0x00, 0x00,
    0x00, 0x00, 0x00,
];
static LCD_INIT_CMD_02: [u8; 31] = [
    0x30, 0x3C, 0x24, 0x14, 0xD0, 0x20, 0xFF, 0xE0, 0x40, 0x19, 0x80, 0x80, 0x80, 0x20,
    0xF9, 0x10, 0x02, 0xFF, 0xFF, 0xF0, 0x90, 0x01, 0x32, 0xA0, 0x91, 0xE0, 0x20, 0x7F,
    0xFF, 0x00, 0x5A,
];
static LCD_INIT_CMD_03: [u8; 30] = [
    0xE0, 0x40, 0x51, 0x24, 0x08, 0x05, 0x10, 0x01, 0x20, 0x15, 0x42, 0xC2, 0x22, 0x22,
    0xAA, 0x03, 0x10, 0x12, 0x60, 0x14, 0x1E, 0x51, 0x15, 0x00, 0x8A, 0x20, 0x00, 0x03,
    0x3A, 0x12,
];
static LCD_INIT_CMD_04: [u8; 22] = [
    0xA0, 0x06, 0xAA, 0x00, 0x08, 0x02, 0x0A, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04,
    0x04, 0x04, 0x04, 0x04, 0x04, 0x00, 0x55, 0x55,
];
static LCD_INIT_CMD_05: [u8; 30] = [
    0x31, 0x04, 0x02, 0x02, 0x71, 0x05, 0x24, 0x55, 0x02, 0x00, 0x41, 0x00, 0x53, 0xFF,
    0xFF, 0xFF, 0x4F, 0x52, 0x00, 0x4F, 0x52, 0x00, 0x45, 0x3B, 0x0B, 0x02, 0x0D, 0x00,
    0xFF, 0x40,
];
static LCD_INIT_CMD_06: [u8; 11] = [
    0x00, 0x00, 0x00, 0x50, 0x03, 0x00, 0x00, 0x00, 0x01, 0x80, 0x01,
];
static LCD_INIT_CMD_07: [u8; 29] = [
    0x00, 0x24, 0x33, 0x80, 0x00, 0xEA, 0x64, 0x32, 0xC8, 0x64, 0xC8, 0x32, 0x90, 0x90,
    0x11, 0x06, 0xDC, 0xFA, 0x00, 0x00, 0x80, 0xFE, 0x10, 0x10, 0x00, 0x0A, 0x0A, 0x44,
    0x50,
];
static LCD_INIT_CMD_08: [u8; 23] = [
    0x18, 0x00, 0x00, 0x03, 0xFE, 0x3A, 0x4A, 0x20, 0x30, 0x10, 0x88, 0xDE, 0x0D, 0x08,
    0x0F, 0x0F, 0x01, 0x3A, 0x4A, 0x20, 0x10, 0x10, 0x00,
];
static LCD_INIT_CMD_09: [u8; 20] = [
    0x05, 0x0A, 0x05, 0x0A, 0x00, 0xE0, 0x2E, 0x0B, 0x12, 0x22, 0x12, 0x22, 0x01, 0x03,
    0x00, 0x3F, 0x6A, 0x18, 0xC8, 0x22,
];
static LCD_INIT_CMD_10: [u8; 20] = [
    0x50, 0x32, 0x28, 0x00, 0xA2, 0x80, 0x8F, 0x00, 0x80, 0xFF, 0x07, 0x11, 0x9C, 0x67,
    0xFF, 0x24, 0x0C, 0x0D, 0x0E, 0x0F,
];
static LCD_INIT_CMD_11: [u8; 4] = [0x33, 0x44, 0x44, 0x01];
static LCD_INIT_CMD_12: [u8; 27] = [
    0x2C, 0x1E, 0x88, 0x58, 0x13, 0x18, 0x56, 0x18, 0x1E, 0x68, 0x88, 0x00, 0x65, 0x09,
    0x22, 0xC4, 0x0C, 0x77, 0x22, 0x44, 0xAA, 0x55, 0x08, 0x08, 0x12, 0xA0, 0x08,
];
static LCD_INIT_CMD_13: [u8; 30] = [
    0x40, 0x8E, 0x8D, 0x01, 0x35, 0x04, 0x92, 0x74, 0x04, 0x92, 0x74, 0x04, 0x08, 0x6A,
    0x04, 0x46, 0x03, 0x03, 0x03, 0x03, 0x82, 0x01, 0x03, 0x00, 0xE0, 0x51, 0xA1, 0x00,
    0x00, 0x00,
];
static LCD_INIT_CMD_14: [u8; 30] = [
    0x10, 0x32, 0x54, 0x76, 0x98, 0xBA, 0xDC, 0xFE, 0x93, 0x00, 0x01, 0x83, 0x07, 0x07,
    0x00, 0x07, 0x07, 0x00, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x00, 0x84, 0x00, 0x20,
    0x01, 0x00,
];
static LCD_INIT_CMD_15: [u8; 19] = [
    0x03, 0x01, 0x0B, 0x09, 0x0F, 0x0D, 0x1E, 0x1F, 0x18, 0x1D, 0x1F, 0x19, 0x40, 0x8E,
    0x04, 0x00, 0x20, 0xA0, 0x1F,
];
static LCD_INIT_CMD_16: [u8; 12] = [
    0x02, 0x00, 0x0A, 0x08, 0x0E, 0x0C, 0x1E, 0x1F, 0x18, 0x1D, 0x1F, 0x19,
];
static LCD_INIT_CMD_17: [u8; 12] = [
    0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F,
];
static LCD_INIT_CMD_18: [u8; 12] = [
    0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F,
];
static LCD_INIT_CMD_19: [u8; 8] = [0x44, 0x73, 0x4B, 0x69, 0x00, 0x0A, 0x02, 0x90];
static LCD_INIT_CMD_20: [u8; 17] = [
    0x3B, 0x28, 0x10, 0x16, 0x0C, 0x06, 0x11, 0x28, 0x5C, 0x21, 0x0D, 0x35, 0x13, 0x2C,
    0x33, 0x28, 0x0D,
];
static LCD_INIT_CMD_21: [u8; 17] = [
    0x37, 0x28, 0x10, 0x16, 0x0B, 0x06, 0x11, 0x28, 0x5C, 0x21, 0x0D, 0x35, 0x14, 0x2C,
    0x33, 0x28, 0x0F,
];
static LCD_INIT_CMD_22: [u8; 17] = [
    0x3B, 0x07, 0x12, 0x18, 0x0E, 0x0D, 0x17, 0x35, 0x44, 0x32, 0x0C, 0x14, 0x14, 0x36,
    0x3A, 0x2F, 0x0D,
];
static LCD_INIT_CMD_23: [u8; 17] = [
    0x37, 0x07, 0x12, 0x18, 0x0E, 0x0D, 0x17, 0x35, 0x44, 0x32, 0x0C, 0x14, 0x14, 0x36,
    0x32, 0x2F, 0x0F,
];
static LCD_INIT_CMD_24: [u8; 17] = [
    0x3B, 0x07, 0x12, 0x18, 0x0E, 0x0D, 0x17, 0x39, 0x44, 0x2E, 0x0C, 0x14, 0x14, 0x36,
    0x3A, 0x2F, 0x0D,
];
static LCD_INIT_CMD_25: [u8; 17] = [
    0x37, 0x07, 0x12, 0x18, 0x0E, 0x0D, 0x17, 0x39, 0x44, 0x2E, 0x0C, 0x14, 0x14, 0x36,
    0x3A, 0x2F, 0x0F,
];
static LCD_INIT_CMD_26: [u8; 16] = [
    0x85, 0x85, 0x95, 0x82, 0xAF, 0xAA, 0xAA, 0x80, 0x10, 0x30, 0x40, 0x40, 0x20, 0xFF,
    0x60, 0x30,
];
static LCD_INIT_CMD_27: [u8; 4] = [0x85, 0x85, 0x95, 0x85];
static LCD_INIT_CMD_28: [u8; 8] = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
static LCD_INIT_CMD_29: [u8; 4] = [0x00, 0x00, 0x00, 0x00];

static LCD_INIT_CMDS: [Axs15231bLcdInitCmd; 32] = [
    Axs15231bLcdInitCmd { cmd: 0xBB, data: LCD_INIT_CMD_00.as_ptr().cast(), data_bytes: LCD_INIT_CMD_00.len(), delay_ms: 0 },
    Axs15231bLcdInitCmd { cmd: 0xA0, data: LCD_INIT_CMD_01.as_ptr().cast(), data_bytes: LCD_INIT_CMD_01.len(), delay_ms: 0 },
    Axs15231bLcdInitCmd { cmd: 0xA2, data: LCD_INIT_CMD_02.as_ptr().cast(), data_bytes: LCD_INIT_CMD_02.len(), delay_ms: 0 },
    Axs15231bLcdInitCmd { cmd: 0xD0, data: LCD_INIT_CMD_03.as_ptr().cast(), data_bytes: LCD_INIT_CMD_03.len(), delay_ms: 0 },
    Axs15231bLcdInitCmd { cmd: 0xA3, data: LCD_INIT_CMD_04.as_ptr().cast(), data_bytes: LCD_INIT_CMD_04.len(), delay_ms: 0 },
    Axs15231bLcdInitCmd { cmd: 0xC1, data: LCD_INIT_CMD_05.as_ptr().cast(), data_bytes: LCD_INIT_CMD_05.len(), delay_ms: 0 },
    Axs15231bLcdInitCmd { cmd: 0xC3, data: LCD_INIT_CMD_06.as_ptr().cast(), data_bytes: LCD_INIT_CMD_06.len(), delay_ms: 0 },
    Axs15231bLcdInitCmd { cmd: 0xC4, data: LCD_INIT_CMD_07.as_ptr().cast(), data_bytes: LCD_INIT_CMD_07.len(), delay_ms: 0 },
    Axs15231bLcdInitCmd { cmd: 0xC5, data: LCD_INIT_CMD_08.as_ptr().cast(), data_bytes: LCD_INIT_CMD_08.len(), delay_ms: 0 },
    Axs15231bLcdInitCmd { cmd: 0xC6, data: LCD_INIT_CMD_09.as_ptr().cast(), data_bytes: LCD_INIT_CMD_09.len(), delay_ms: 0 },
    Axs15231bLcdInitCmd { cmd: 0xC7, data: LCD_INIT_CMD_10.as_ptr().cast(), data_bytes: LCD_INIT_CMD_10.len(), delay_ms: 0 },
    Axs15231bLcdInitCmd { cmd: 0xC9, data: LCD_INIT_CMD_11.as_ptr().cast(), data_bytes: LCD_INIT_CMD_11.len(), delay_ms: 0 },
    Axs15231bLcdInitCmd { cmd: 0xCF, data: LCD_INIT_CMD_12.as_ptr().cast(), data_bytes: LCD_INIT_CMD_12.len(), delay_ms: 0 },
    Axs15231bLcdInitCmd { cmd: 0xD5, data: LCD_INIT_CMD_13.as_ptr().cast(), data_bytes: LCD_INIT_CMD_13.len(), delay_ms: 0 },
    Axs15231bLcdInitCmd { cmd: 0xD6, data: LCD_INIT_CMD_14.as_ptr().cast(), data_bytes: LCD_INIT_CMD_14.len(), delay_ms: 0 },
    Axs15231bLcdInitCmd { cmd: 0xD7, data: LCD_INIT_CMD_15.as_ptr().cast(), data_bytes: LCD_INIT_CMD_15.len(), delay_ms: 0 },
    Axs15231bLcdInitCmd { cmd: 0xD8, data: LCD_INIT_CMD_16.as_ptr().cast(), data_bytes: LCD_INIT_CMD_16.len(), delay_ms: 0 },
    Axs15231bLcdInitCmd { cmd: 0xD9, data: LCD_INIT_CMD_17.as_ptr().cast(), data_bytes: LCD_INIT_CMD_17.len(), delay_ms: 0 },
    Axs15231bLcdInitCmd { cmd: 0xDD, data: LCD_INIT_CMD_18.as_ptr().cast(), data_bytes: LCD_INIT_CMD_18.len(), delay_ms: 0 },
    Axs15231bLcdInitCmd { cmd: 0xDF, data: LCD_INIT_CMD_19.as_ptr().cast(), data_bytes: LCD_INIT_CMD_19.len(), delay_ms: 0 },
    Axs15231bLcdInitCmd { cmd: 0xE0, data: LCD_INIT_CMD_20.as_ptr().cast(), data_bytes: LCD_INIT_CMD_20.len(), delay_ms: 0 },
    Axs15231bLcdInitCmd { cmd: 0xE1, data: LCD_INIT_CMD_21.as_ptr().cast(), data_bytes: LCD_INIT_CMD_21.len(), delay_ms: 0 },
    Axs15231bLcdInitCmd { cmd: 0xE2, data: LCD_INIT_CMD_22.as_ptr().cast(), data_bytes: LCD_INIT_CMD_22.len(), delay_ms: 0 },
    Axs15231bLcdInitCmd { cmd: 0xE3, data: LCD_INIT_CMD_23.as_ptr().cast(), data_bytes: LCD_INIT_CMD_23.len(), delay_ms: 0 },
    Axs15231bLcdInitCmd { cmd: 0xE4, data: LCD_INIT_CMD_24.as_ptr().cast(), data_bytes: LCD_INIT_CMD_24.len(), delay_ms: 0 },
    Axs15231bLcdInitCmd { cmd: 0xE5, data: LCD_INIT_CMD_25.as_ptr().cast(), data_bytes: LCD_INIT_CMD_25.len(), delay_ms: 0 },
    Axs15231bLcdInitCmd { cmd: 0xA4, data: LCD_INIT_CMD_26.as_ptr().cast(), data_bytes: LCD_INIT_CMD_26.len(), delay_ms: 0 },
    Axs15231bLcdInitCmd { cmd: 0xA4, data: LCD_INIT_CMD_27.as_ptr().cast(), data_bytes: LCD_INIT_CMD_27.len(), delay_ms: 0 },
    Axs15231bLcdInitCmd { cmd: 0xBB, data: LCD_INIT_CMD_28.as_ptr().cast(), data_bytes: LCD_INIT_CMD_28.len(), delay_ms: 0 },
    Axs15231bLcdInitCmd { cmd: 0x13, data: core::ptr::null(), data_bytes: 0, delay_ms: 0 },
    Axs15231bLcdInitCmd { cmd: 0x11, data: core::ptr::null(), data_bytes: 0, delay_ms: 120 },
    Axs15231bLcdInitCmd { cmd: 0x2C, data: LCD_INIT_CMD_29.as_ptr().cast(), data_bytes: LCD_INIT_CMD_29.len(), delay_ms: 0 },
];

// ── Helpers ─────────────────────────────────────────────────────────

fn esp_check(res: esp_idf_sys::esp_err_t, msg: &str) -> Result<()> {
    if res != esp_idf_sys::ESP_OK {
        Err(anyhow::anyhow!("{} (err {})", msg, res))
    } else {
        Ok(())
    }
}

pub fn now_ms() -> u32 {
    unsafe { (esp_idf_sys::esp_timer_get_time() / 1000) as u32 }
}

fn alert_fingerprint(alerts: &[weather::WeatherAlert]) -> String {
    let mut parts: Vec<String> = alerts
        .iter()
        .map(|a| format!("{}|{}|{}|{}", a.id, a.event, a.expires, a.severity))
        .collect();
    parts.sort_unstable();
    parts.join("||")
}

fn alert_tone_for(alerts: &[weather::WeatherAlert]) -> speaker::AlertTone {
    let mut rank = 0u8;
    for alert in alerts {
        let severity = alert.severity.to_ascii_lowercase();
        let local_rank = if severity.contains("extreme")
            || severity.contains("severe")
            || alert.kind() == weather::AlertKind::Warning
        {
            3
        } else if severity.contains("moderate") || alert.kind() == weather::AlertKind::Watch {
            2
        } else {
            1
        };
        if local_rank > rank {
            rank = local_rank;
        }
    }
    match rank {
        3 => speaker::AlertTone::Warning,
        2 => speaker::AlertTone::Watch,
        _ => speaker::AlertTone::Advisory,
    }
}

fn locked_orientation(mode: config::OrientationMode, flip: bool) -> layout::Orientation {
    match mode {
        config::OrientationMode::Landscape => {
            if flip {
                layout::Orientation::LandscapeFlipped
            } else {
                layout::Orientation::Landscape
            }
        }
        config::OrientationMode::Portrait => {
            if flip {
                layout::Orientation::PortraitFlipped
            } else {
                layout::Orientation::Portrait
            }
        }
        config::OrientationMode::Auto => layout::Orientation::Landscape,
    }
}

fn framebuffer_dims(orientation: layout::Orientation) -> (u32, u32) {
    if orientation.is_landscape() { (480, 320) } else { (320, 480) }
}

fn detect_orientation_from_imu(r: &qmi8658::ImuReading) -> Option<layout::Orientation> {
    if r.accel_z.abs() > ORIENTATION_MAX_Z_G {
        return None;
    }
    let ax = r.accel_x.abs();
    let ay = r.accel_y.abs();
    let dominant = ax.max(ay);
    if dominant < ORIENTATION_MIN_AXIS_G {
        return None;
    }
    if ax > ay + ORIENTATION_SWITCH_MARGIN_G {
        // For this board mounting, +X tilt corresponds to upside-down portrait.
        if r.accel_x >= 0.0 {
            Some(layout::Orientation::PortraitFlipped)
        } else {
            Some(layout::Orientation::Portrait)
        }
    } else if ay > ax + ORIENTATION_SWITCH_MARGIN_G {
        if r.accel_y < 0.0 {
            Some(layout::Orientation::Landscape)
        } else {
            Some(layout::Orientation::LandscapeFlipped)
        }
    } else {
        None
    }
}

fn apply_orientation(
    state: &mut views::AppState,
    fb: &mut framebuffer::Framebuffer,
    next: layout::Orientation,
) {
    if state.orientation == next {
        return;
    }
    let (old_w, old_h) = framebuffer_dims(state.orientation);
    let (new_w, new_h) = framebuffer_dims(next);
    state.orientation = next;
    if old_w != new_w || old_h != new_h {
        *fb = framebuffer::Framebuffer::new(new_w, new_h);
    }
    state.dirty = true;
}

fn bme_reading_is_plausible(state: &views::AppState, reading: &bme280_sensor::Bme280Reading) -> bool {
    if !reading.temperature_f.is_finite()
        || !reading.humidity.is_finite()
        || !reading.pressure_hpa.is_finite()
    {
        return false;
    }
    if !(BME_TEMP_MIN_F..=BME_TEMP_MAX_F).contains(&reading.temperature_f) {
        return false;
    }
    if !(0.0..=100.0).contains(&reading.humidity) {
        return false;
    }
    if !(BME_PRESSURE_MIN_HPA..=BME_PRESSURE_MAX_HPA).contains(&reading.pressure_hpa) {
        return false;
    }

    // Block abrupt spikes from bad reads to keep now-view values and graph stable.
    if let Some(prev) = state.indoor_temp {
        if (reading.temperature_f - prev).abs() > BME_TEMP_MAX_STEP_F {
            return false;
        }
    }
    if let Some(prev) = state.indoor_humidity {
        if (reading.humidity - prev).abs() > BME_HUM_MAX_STEP {
            return false;
        }
    }
    if let Some(prev) = state.indoor_pressure {
        if (reading.pressure_hpa - prev).abs() > BME_PRESSURE_MAX_STEP_HPA {
            return false;
        }
    }
    true
}

// ── Display init (mirrors C factory bsp_display_init exactly) ──────

struct LcdContext {
    io: esp_idf_sys::esp_lcd_panel_io_handle_t,
    panel: esp_idf_sys::esp_lcd_panel_handle_t,
    _vendor_config: Box<Axs15231bVendorConfig>,
}

fn init_display() -> Result<LcdContext> {
    let mut bus_cfg = esp_idf_sys::spi_bus_config_t::default();
    bus_cfg.__bindgen_anon_1.mosi_io_num = PIN_LCD_D0;
    bus_cfg.__bindgen_anon_2.miso_io_num = PIN_LCD_D1;
    bus_cfg.__bindgen_anon_3.quadwp_io_num = PIN_LCD_D2;
    bus_cfg.__bindgen_anon_4.quadhd_io_num = PIN_LCD_D3;
    bus_cfg.sclk_io_num = PIN_LCD_SCLK;
    bus_cfg.max_transfer_sz = PANEL_WIDTH * CHUNK_LINES * 2;

    let host = esp_idf_sys::spi_host_device_t_SPI2_HOST;
    esp_check(
        unsafe { esp_idf_sys::spi_bus_initialize(host, &bus_cfg, esp_idf_sys::spi_common_dma_t_SPI_DMA_CH_AUTO) },
        "spi_bus_initialize",
    )?;

    let mut io: esp_idf_sys::esp_lcd_panel_io_handle_t = std::ptr::null_mut();
    let io_cfg = esp_idf_sys::esp_lcd_panel_io_spi_config_t {
        cs_gpio_num: PIN_LCD_CS,
        dc_gpio_num: -1,
        spi_mode: 3,
        pclk_hz: PCLK_HZ,
        trans_queue_depth: 10,
        on_color_trans_done: None,
        user_ctx: std::ptr::null_mut(),
        lcd_cmd_bits: 32,
        lcd_param_bits: 8,
        flags: esp_idf_sys::esp_lcd_panel_io_spi_config_t__bindgen_ty_1 {
            _bitfield_align_1: [],
            _bitfield_1:
                esp_idf_sys::esp_lcd_panel_io_spi_config_t__bindgen_ty_1::new_bitfield_1(
                    0, 0, 0, 0, 1, 0, 0, 0,
                ),
            __bindgen_padding_0: [0; 3],
        },
    };
    esp_check(
        unsafe { esp_idf_sys::esp_lcd_new_panel_io_spi(host as esp_idf_sys::esp_lcd_spi_bus_handle_t, &io_cfg, &mut io) },
        "esp_lcd_new_panel_io_spi",
    )?;

    let mut panel: esp_idf_sys::esp_lcd_panel_handle_t = std::ptr::null_mut();
    let vendor_config = Box::new(Axs15231bVendorConfig {
        init_cmds: LCD_INIT_CMDS.as_ptr(),
        init_cmds_size: LCD_INIT_CMDS.len() as u16,
        flags: Axs15231bVendorFlags { use_qspi_interface: 1 },
    });

    let panel_cfg = esp_idf_sys::esp_lcd_panel_dev_config_t {
        reset_gpio_num: -1,
        __bindgen_anon_1: esp_idf_sys::esp_lcd_panel_dev_config_t__bindgen_ty_1 {
            rgb_ele_order: esp_idf_sys::lcd_rgb_element_order_t_LCD_RGB_ELEMENT_ORDER_RGB,
        },
        data_endian: esp_idf_sys::lcd_rgb_data_endian_t_LCD_RGB_DATA_ENDIAN_BIG,
        bits_per_pixel: 16,
        flags: esp_idf_sys::esp_lcd_panel_dev_config_t__bindgen_ty_2 {
            _bitfield_align_1: [],
            _bitfield_1: esp_idf_sys::esp_lcd_panel_dev_config_t__bindgen_ty_2::new_bitfield_1(0),
            __bindgen_padding_0: [0; 3],
        },
        vendor_config: (&*vendor_config) as *const Axs15231bVendorConfig as *mut c_void,
    };

    esp_check(
        unsafe { esp_lcd_new_panel_axs15231b(io, &panel_cfg, &mut panel) },
        "esp_lcd_new_panel_axs15231b",
    )?;

    esp_check(unsafe { esp_idf_sys::esp_lcd_panel_reset(panel) }, "panel_reset")?;
    esp_check(unsafe { esp_idf_sys::esp_lcd_panel_init(panel) }, "panel_init")?;
    esp_check(unsafe { esp_idf_sys::esp_lcd_panel_disp_on_off(panel, false) }, "disp_on")?;

    info!("Display initialized OK");
    Ok(LcdContext { io, panel, _vendor_config: vendor_config })
}

fn enable_backlight() {
    unsafe {
        let io_conf = esp_idf_sys::gpio_config_t {
            pin_bit_mask: 1u64 << (PIN_LCD_BL as u64),
            mode: esp_idf_sys::gpio_mode_t_GPIO_MODE_OUTPUT,
            pull_up_en: esp_idf_sys::gpio_pullup_t_GPIO_PULLUP_DISABLE,
            pull_down_en: esp_idf_sys::gpio_pulldown_t_GPIO_PULLDOWN_DISABLE,
            intr_type: esp_idf_sys::gpio_int_type_t_GPIO_INTR_DISABLE,
        };
        esp_idf_sys::gpio_config(&io_conf);
        esp_idf_sys::gpio_set_level(PIN_LCD_BL, 1);
    }
    info!("Backlight ON");
}

// ── I2C bus scan ────────────────────────────────────────────────────

fn scan_i2c(i2c: &mut I2cDriver<'_>) -> Vec<u8> {
    let mut found = Vec::new();
    for addr in 1..=127u8 {
        if i2c.write(addr, &[0], 50).is_ok() {
            found.push(addr);
        }
    }
    if found.is_empty() {
        info!("I2C scan: no devices found");
    } else {
        info!("I2C scan: found {} device(s): {:02X?}", found.len(), found);
    }
    found
}

// ── Boot splash screen ──────────────────────────────────────────────

fn draw_splash(fb: &mut framebuffer::Framebuffer, status: &str) {
    use embedded_graphics::{
        mono_font::MonoTextStyle,
        pixelcolor::Rgb565,
        prelude::*,
        text::{Alignment, Text},
    };
    use profont::{PROFONT_24_POINT, PROFONT_18_POINT, PROFONT_14_POINT};

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

// ── Entry point ─────────────────────────────────────────────────────

fn main() -> Result<()> {
    esp_idf_sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    info!(
        "BOOT — waveshare_s3_3p weather dashboard v{}",
        env!("CARGO_PKG_VERSION")
    );

    // ── 1. Board power (TCA9554 IO expander + AXP2101 PMIC + LCD reset) ──
    esp_check(unsafe { board_power_init() }, "board_power_init")?;
    info!("Power + LCD reset OK");

    // ── 2. Display init + immediate splash screen ──
    let ctx = init_display()?;

    // Create framebuffer early so we can show a boot screen immediately
    let mut fb = framebuffer::Framebuffer::new(framebuffer::FB_WIDTH, framebuffer::FB_HEIGHT);

    // Show splash before backlight so first frame is ready
    draw_splash(&mut fb, "Starting...");
    fb.flush_to_panel(ctx.io, ctx.panel, layout::Orientation::Landscape);
    enable_backlight();

    // ── 3. Peripherals ──
    let peripherals = unsafe { Peripherals::new() };
    let sysloop = EspSystemEventLoop::take()?;
    let nvs_partition = EspDefaultNvsPartition::take()?;

    // ── 4. NVS config ──
    let mut nvs = EspNvs::new(nvs_partition.clone(), config::NS, true)?;
    let mut cfg = config::Config::load(&nvs);
    let legacy_nws_ua = "waveshare_s3_3p/0.1 (contact: unset)";
    let default_nws_ua = format!(
        "waveshare_s3_3p/{} (contact: unset)",
        env!("CARGO_PKG_VERSION")
    );
    if cfg.nws_user_agent == legacy_nws_ua {
        match config::Config::save_nws_user_agent(&mut nvs, &default_nws_ua) {
            Ok(()) => {
                cfg.nws_user_agent = default_nws_ua.clone();
                info!("Migrated NWS User-Agent default to {}", cfg.nws_user_agent);
            }
            Err(e) => log::warn!("Failed to migrate NWS User-Agent default: {}", e),
        }
    }
    let wifi_ssid = cfg.wifi_ssid.clone();
    let wifi_pass = cfg.wifi_pass.clone();
    let timezone = cfg.timezone.clone();

    let nvs = Arc::new(Mutex::new(nvs));
    let cfg = Arc::new(Mutex::new(cfg));

    let weather_refresh_flag: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));

    // ── 5. Console (serial interactive) ──
    console::spawn_console(nvs.clone(), cfg.clone(), weather_refresh_flag.clone());

    // ── 6. I2C bus ──
    // board_power_init() already installed an I2C driver on port 0 for the PMIC.
    // We must delete it before creating our own Rust I2cDriver on the same port.
    unsafe { esp_idf_sys::i2c_driver_delete(0); }

    let i2c_config = I2cConfig::new().baudrate(Hertz(I2C_FREQ_HZ));
    let mut i2c = I2cDriver::new(
        peripherals.i2c0,
        peripherals.pins.gpio8,
        peripherals.pins.gpio7,
        &i2c_config,
    )?;

    let i2c_devices = scan_i2c(&mut i2c);

    if let Err(e) = speaker::init_audio_path(&mut i2c) {
        log::warn!("Audio path init failed (PA + ES8311): {}", e);
    } else {
        info!("Audio path initialized (PA enabled + ES8311 ready)");
    }

    // ── 7. BME280 sensor ──
    let bme280 = bme280_sensor::Bme280::init(&mut i2c);
    if bme280.is_some() {
        info!("BME280 sensor ready");
    } else {
        log::warn!("BME280 init failed — sensor unavailable");
    }

    // ── 8. IMU (QMI8658) ──
    let imu_ok = qmi8658::init(&mut i2c);

    // ── 9. Touch controller ──
    touch::probe(&mut i2c);
    let mut touch_state = touch::TouchState::new();

    // ── 9a. Speaker / tone output (I2S -> ES8311 path) ──
    let speaker_ready = Arc::new(AtomicBool::new(false));
    {
        let speaker_ready = speaker_ready.clone();
        let i2s0 = peripherals.i2s0;
        let pin_bclk = peripherals.pins.gpio13;
        let pin_dout = peripherals.pins.gpio16;
        let pin_ws = peripherals.pins.gpio15;
        let pin_mclk = peripherals.pins.gpio44;
        std::thread::Builder::new()
            .name("beep".into())
            .stack_size(8192)
            .spawn(move || {
                let mut speaker = match speaker::Speaker::new(
                    i2s0,
                    pin_bclk,
                    pin_dout,
                    pin_ws,
                    Some(pin_mclk),
                ) {
                    Ok(s) => {
                        speaker_ready.store(true, Ordering::Relaxed);
                        info!(
                            "Speaker I2S initialized (BCLK={} DOUT={} WS={} MCLK={})",
                            PIN_I2S_BCLK, PIN_I2S_DOUT, PIN_I2S_WS, PIN_I2S_MCLK
                        );
                        Some(s)
                    }
                    Err(e) => {
                        log::warn!("Speaker I2S init failed: {}", e);
                        None
                    }
                };

                loop {
                    if let Some(code) = debug_flags::take_beep_tone_request() {
                        if let Some(tone) = speaker::AlertTone::from_request(code) {
                            if let Some(spk) = speaker.as_mut() {
                                match spk.play(tone, crate::debug_flags::take_beep_stop_request) {
                                    Ok(()) => info!("beep: {}", tone.as_str()),
                                    Err(e) => log::warn!("speaker beep failed: {}", e),
                                }
                            } else {
                                log::warn!("speaker: not initialized");
                            }
                        }
                    } else {
                        // Keep latency low for console-triggered tones while avoiding busy wait.
                        std::thread::sleep(Duration::from_millis(20));
                    }
                }
            })
            .expect("failed to spawn beep thread");
    }

    // ── 9. WiFi ──
    let mut ip_address = String::new();
    let mut wifi_networks: Vec<(String, i8)> = Vec::new();
    let mut wifi_ok = false;
    let mut wifi_handle = if !wifi_ssid.is_empty() {
        draw_splash(&mut fb, &format!("Connecting to '{}'...", wifi_ssid));
        fb.flush_to_panel(ctx.io, ctx.panel, layout::Orientation::Landscape);
        info!("Connecting to WiFi '{}'...", wifi_ssid);
        match wifi::connect_wifi(peripherals.modem, sysloop.clone(), &wifi_ssid, &wifi_pass) {
            Ok(result) => {
                if let Some(ip) = result.ip_address {
                    ip_address = ip;
                }
                wifi_networks = result.networks;
                wifi_ok = result.connected;
                Some(result.wifi)
            }
            Err(e) => {
                log::warn!("WiFi failed: {}", e);
                None
            }
        }
    } else {
        log::warn!("No WiFi SSID configured (use console: wifi set <ssid> <pass>)");
        None
    };

    // ── 10. NTP time sync ──
    let mut sntp = if wifi_ok {
        draw_splash(&mut fb, "Syncing time...");
        fb.flush_to_panel(ctx.io, ctx.panel, layout::Orientation::Landscape);
        match time_sync::sync_time(&timezone) {
            Ok(sntp) => Some(sntp),
            Err(e) => {
                log::warn!("NTP sync failed: {}", e);
                None
            }
        }
    } else {
        None
    };

    if wifi_ok {
        let (needs_auto_query, user_agent, existing_query) = {
            let c = cfg.lock().unwrap();
            (
                config::weather_query_needs_autodiscovery(&c.weather_query),
                c.nws_user_agent.clone(),
                c.weather_query.clone(),
            )
        };
        if needs_auto_query {
            info!("Weather query is unset/placeholder; attempting auto-discovery...");
            match weather::discover_openweather_query(&user_agent) {
                Ok(query) => {
                    if let Err(e) = config::Config::save_weather_query(&mut nvs.lock().unwrap(), &query)
                    {
                        log::warn!("Failed to save auto-discovered weather query: {}", e);
                    } else {
                        cfg.lock().unwrap().weather_query = query.clone();
                        info!("Auto-discovered weather query: {}", query);
                        weather_refresh_flag.store(true, Ordering::Relaxed);
                    }
                }
                Err(e) => log::warn!("Weather auto-discovery failed: {}", e),
            }
        } else {
            info!(
                "Weather query already set; skipping auto-discovery (wx_query={})",
                existing_query
            );
        }
    }

    // ── 11. App state ──
    let mut state = views::AppState::new();
    state.use_celsius = cfg.lock().unwrap().use_celsius;
    {
        let cfg_guard = cfg.lock().unwrap();
        state.orientation_mode = cfg_guard.orientation_mode;
        state.orientation_flip = cfg_guard.orientation_flip;
    }
    state.orientation = if state.orientation_mode == config::OrientationMode::Auto {
        layout::Orientation::Landscape
    } else {
        locked_orientation(state.orientation_mode, state.orientation_flip)
    };
    state.i2c_devices = i2c_devices;
    state.wifi_networks = wifi_networks;
    state.wifi_ssid = wifi_ssid.clone();
    state.ip_address = ip_address.clone();
    if wifi_ok {
        state.status_text = ip_address.clone();
    } else if wifi_ssid.is_empty() {
        state.status_text = "No WiFi".to_string();
    } else {
        state.status_text = "WiFi failed".to_string();
    }

    if state.orientation_mode == config::OrientationMode::Auto && imu_ok {
        if let Some(r) = qmi8658::read(&mut i2c) {
            if let Some(orientation) = detect_orientation_from_imu(&r) {
                state.orientation = orientation;
            }
        }
    }

    if state.orientation.is_portrait() {
        let (fb_w, fb_h) = framebuffer_dims(state.orientation);
        fb = framebuffer::Framebuffer::new(fb_w, fb_h);
    }

    // ── 12. Weather fetch thread ──
    let weather_data: Arc<Mutex<Option<(weather::CurrentWeather, weather::Forecast)>>> =
        Arc::new(Mutex::new(None));
    {
        let wd = weather_data.clone();
        let refresh = weather_refresh_flag.clone();
        let cfg_weather = cfg.clone();
        std::thread::Builder::new()
            .name("weather".into())
            .stack_size(16384)
            .spawn(move || {
                let mut first = true;
                let mut warned_missing_key = false;
                let mut consecutive_failures: u32 = 0;
                loop {
                    let (query, key) = {
                        let c = cfg_weather.lock().unwrap();
                        (c.weather_query.clone(), c.weather_api_key.clone())
                    };
                    if key.is_empty() {
                        if !warned_missing_key {
                            log::warn!("No weather API key (use console: api set-key <key>)");
                            warned_missing_key = true;
                        }
                        std::thread::sleep(Duration::from_secs(WEATHER_RETRY_SECS));
                        continue;
                    }
                    warned_missing_key = false;

                    let verbose =
                        first || crate::debug_flags::is_on(&crate::debug_flags::DEBUG_WEATHER);
                    if verbose {
                        info!("Weather fetch starting...");
                    }
                    match weather::fetch_weather(&query, &key) {
                        Ok((current, forecast)) => {
                            if verbose {
                                info!(
                                    "Weather: {}°F {} in {} ({} forecast days)",
                                    current.temp_f as i32,
                                    current.condition,
                                    current.city,
                                    forecast.rows.len()
                                );
                            }
                            first = false;
                            consecutive_failures = 0;
                            *wd.lock().unwrap() = Some((current, forecast));
                        }
                        Err(e) => {
                            consecutive_failures = consecutive_failures.saturating_add(1);
                            if consecutive_failures == 1
                                || consecutive_failures.is_multiple_of(FAILURE_WARN_EVERY)
                            {
                                log::warn!(
                                    "Weather fetch failed ({} consecutive): {}",
                                    consecutive_failures,
                                    e
                                );
                            } else {
                                info!("Weather fetch failed ({} consecutive)", consecutive_failures);
                            }
                            // Retry sleep that can be interrupted by a manual refresh request.
                            for _ in 0..WEATHER_RETRY_SECS {
                                if refresh.swap(false, Ordering::Relaxed) {
                                    info!("Weather refresh requested");
                                    break;
                                }
                                std::thread::sleep(Duration::from_secs(1));
                            }
                            continue;
                        }
                    }
                    // Sleep but wake early on refresh request
                    for _ in 0..WEATHER_INTERVAL_SECS {
                        if refresh.swap(false, Ordering::Relaxed) {
                            info!("Weather refresh requested");
                            break;
                        }
                        std::thread::sleep(Duration::from_secs(1));
                    }
                }
            })
            .expect("failed to spawn weather thread");
    }

    // ── 12b. NWS alerts fetch thread ──
    let alert_data: Arc<Mutex<Option<Vec<weather::WeatherAlert>>>> = Arc::new(Mutex::new(None));
    {
        let ad = alert_data.clone();
        let cfg_alerts = cfg.clone();
        let nvs_alerts = nvs.clone();
        std::thread::Builder::new()
            .name("alerts".into())
            .stack_size(32768)
            .spawn(move || {
                info!(
                    "NWS alerts thread startup delay: {}s",
                    ALERTS_START_DELAY_SECS
                );
                std::thread::sleep(Duration::from_secs(ALERTS_START_DELAY_SECS));
                let mut consecutive_failures: u32 = 0;
                loop {
                    let (enabled, auto_scope, scope, cached_zone, ua) = {
                        let c = cfg_alerts.lock().unwrap();
                        (
                            c.alerts_enabled,
                            c.alerts_auto_scope,
                            c.nws_scope.clone(),
                            c.nws_zone.clone(),
                            c.nws_user_agent.clone(),
                        )
                    };
                    if !enabled {
                        std::thread::sleep(Duration::from_secs(5));
                        continue;
                    }

                    let effective_scope = if auto_scope {
                        if cached_zone.is_empty() {
                            match weather::discover_nws_zone(&ua) {
                                Ok(zone) => {
                                    info!("NWS auto-scope discovered zone={}", zone);
                                    consecutive_failures = 0;
                                    if let Ok(mut c) = cfg_alerts.lock() {
                                        c.nws_zone = zone.clone();
                                    }
                                    if let Ok(mut nvs) = nvs_alerts.lock() {
                                        let _ = config::Config::save_nws_zone(&mut nvs, &zone);
                                    }
                                    format!("zone={}", zone)
                                }
                                Err(e) => {
                                    consecutive_failures = consecutive_failures.saturating_add(1);
                                    if consecutive_failures == 1
                                        || consecutive_failures.is_multiple_of(FAILURE_WARN_EVERY)
                                    {
                                        log::warn!(
                                            "NWS auto-scope discovery failed ({} consecutive): {}",
                                            consecutive_failures,
                                            e
                                        );
                                    } else {
                                        info!(
                                            "NWS auto-scope discovery failed ({} consecutive)",
                                            consecutive_failures
                                        );
                                    }
                                    std::thread::sleep(Duration::from_secs(60));
                                    continue;
                                }
                            }
                        } else {
                            format!("zone={}", cached_zone)
                        }
                    } else {
                        scope
                    };

                    match weather::fetch_nws_alerts(&effective_scope, &ua) {
                        Ok(alerts) => {
                            let count = alerts.len();
                            consecutive_failures = 0;
                            *ad.lock().unwrap() = Some(alerts);
                            info!("NWS alerts: {} active", count);
                            std::thread::sleep(Duration::from_secs(ALERTS_INTERVAL_SECS));
                        }
                        Err(e) => {
                            consecutive_failures = consecutive_failures.saturating_add(1);
                            if consecutive_failures == 1
                                || consecutive_failures.is_multiple_of(FAILURE_WARN_EVERY)
                            {
                                log::warn!(
                                    "NWS alerts fetch failed ({} consecutive): {}",
                                    consecutive_failures,
                                    e
                                );
                            } else {
                                info!("NWS alerts fetch failed ({} consecutive)", consecutive_failures);
                            }
                            std::thread::sleep(Duration::from_secs(WEATHER_RETRY_SECS));
                        }
                    }
                }
            })
            .expect("failed to spawn alerts thread");
    }

    // ── 13. Main event loop ──
    info!("Entering main loop");
    let mut last_bme_ms: u32 = 0;
    let mut bme_reject_streak: u16 = 0;
    let mut last_hvac_detect_ms: u32 = 0;
    let mut last_hvac_record_ms: u32 = 0;
    let mut last_pressure_sample_ms: u32 = 0;
    let mut tick_count: u32 = 0;
    let mut orientation_candidate = state.orientation;
    let mut orientation_candidate_count: u8 = 0;
    let mut last_orientation_change_ms: u32 = now_ms();
    let mut last_wifi_retry_ms: u32 = now_ms();
    let mut last_weather_success_ms: Option<u32> = None;
    let mut alerts_snapshot_seen = false;
    let mut last_alert_fingerprint = String::new();
    let mut last_alert_beep_ms: Option<u32> = None;
    let mut watch_beeps_remaining: u8 = 0;

    // Initial draw
    views::draw_current_view(&mut fb, &state);
    fb.flush_to_panel(ctx.io, ctx.panel, state.orientation);
    state.dirty = false;

    loop {
        let t = now_ms();

        // Poll touch
        let gesture = touch_state.poll(&mut i2c, t, state.orientation);
        if gesture != touch::Gesture::None
            && state.handle_gesture(gesture) {
                info!("Gesture {:?} -> view {:?}", gesture, state.current_view);
            }

        // BME280 read
        if t.wrapping_sub(last_bme_ms) >= BME280_INTERVAL_MS {
            last_bme_ms = t;
            if let Some(ref sensor) = bme280 {
                match sensor.read(&mut i2c) {
                  Some(reading) => {
                    let plausible = bme_reading_is_plausible(&state, &reading);
                    if !plausible {
                        bme_reject_streak = bme_reject_streak.saturating_add(1);
                    }
                    // Accept if plausible, or force-accept after 12 rejects (~60s) to recover.
                    if !plausible && bme_reject_streak < 12 {
                        if debug_flags::is_on(&debug_flags::DEBUG_BME280) {
                            log::warn!(
                                "BME280 outlier dropped ({}): {:.1}°F {:.1}%RH {:.0}hPa",
                                bme_reject_streak,
                                reading.temperature_f,
                                reading.humidity,
                                reading.pressure_hpa
                            );
                        }
                        // Skip this reading but do NOT continue — let rendering proceed.
                    } else {
                        if !plausible && debug_flags::is_on(&debug_flags::DEBUG_BME280) {
                            log::warn!(
                                "BME280 re-baselining: {:.1}°F {:.1}%RH {:.0}hPa",
                                reading.temperature_f, reading.humidity, reading.pressure_hpa
                            );
                        }
                        bme_reject_streak = 0;
                        if debug_flags::is_on(&debug_flags::DEBUG_BME280) {
                            info!(
                                "BME280: {:.1}°F  {:.1}%RH  {:.0}hPa",
                                reading.temperature_f, reading.humidity, reading.pressure_hpa
                            );
                        }
                        state.indoor_temp = Some(reading.temperature_f);
                        state.indoor_humidity = Some(reading.humidity);
                        state.indoor_pressure = Some(reading.pressure_hpa);
                        // Push to history ring buffer (VecDeque: O(1) pop_front)
                        if state.indoor_temp_history.len() >= views::INDOOR_HISTORY_MAX {
                            state.indoor_temp_history.pop_front();
                        }
                        state.indoor_temp_history.push_back(reading.temperature_f);
                        if state.indoor_hum_history.len() >= views::INDOOR_HISTORY_MAX {
                            state.indoor_hum_history.pop_front();
                        }
                        state.indoor_hum_history.push_back(reading.humidity);
                        state.dirty = true;
                    } // end accept block
                  }
                  None => {
                    log::warn!("BME280 read returned None (I2C failed)");
                  }
                }
            }
        }

        // HVAC fast detection (every 5s)
        if t.wrapping_sub(last_hvac_detect_ms) >= HVAC_DETECT_INTERVAL_MS {
            if let Some(temp_f) = state.indoor_temp {
                let temp_c = (temp_f - 32.0) * 5.0 / 9.0;
                state.hvac.detect(temp_c, t);
                last_hvac_detect_ms = t;
                state.dirty = true;
            }
        }

        // HVAC history record (every 30s)
        if t.wrapping_sub(last_hvac_record_ms) >= HVAC_RECORD_INTERVAL_MS {
            state.hvac.record();
            last_hvac_record_ms = t;
        }

        // Pressure history sample (every 5 minutes)
        if t.wrapping_sub(last_pressure_sample_ms) >= PRESSURE_SAMPLE_INTERVAL_MS {
            let bme = state.indoor_pressure;
            let owm = state
                .current_weather
                .as_ref()
                .and_then(|cw| {
                    if cw.pressure_hpa > 0 {
                        Some(cw.pressure_hpa as f32)
                    } else {
                        None
                    }
                });
            state.pressure_history.push(bme, owm);
            last_pressure_sample_ms = t;
            state.dirty = true;
        }

        // Check for weather data from background thread
        if let Ok(mut wd) = weather_data.try_lock() {
            if let Some((current, forecast)) = wd.take() {
                state.bottom_text = format!(
                    "{}, {} | {}",
                    current.city, current.country, current.condition
                );
                state.current_weather = Some(current);
                state.forecast = Some(forecast);
                state.status_text = ip_address.clone();
                state.weather_stale = false;
                last_weather_success_ms = Some(t);
                state.dirty = true;
            }
        }

        // Flag stale weather data when the last successful fetch is too old.
        let stale = last_weather_success_ms
            .map(|ts| t.wrapping_sub(ts) > (WEATHER_STALE_AFTER_SECS as u32 * 1000))
            .unwrap_or(false);
        if stale != state.weather_stale {
            state.weather_stale = stale;
            state.dirty = true;
        }

        // Check for alert data from background thread
        if let Ok(mut ad) = alert_data.try_lock() {
            if let Some(alerts) = ad.take() {
                let fp = alert_fingerprint(&alerts);
                let changed = !alerts_snapshot_seen || fp != last_alert_fingerprint;
                let beep_enabled = cfg.lock().unwrap().alerts_beep;

                if changed && !alerts.is_empty() {
                    for alert in &alerts {
                        weather::log_alert_to_console(alert);
                    }
                }

                if changed && alerts_snapshot_seen && !alerts.is_empty() && beep_enabled {
                    let tone = alert_tone_for(&alerts);
                    let highest_kind = alerts.first().map(|a| a.kind()).unwrap_or(weather::AlertKind::Other);

                    match highest_kind {
                        weather::AlertKind::Warning => {
                            // Warning: takeover screen + repeating beep until silenced
                            let already_silenced = fp == state.warning_silenced_fingerprint;
                            if !already_silenced {
                                state.warning_active = true;
                                state.warning_scroll = 0;
                                state.current_view = views::View::Warning;
                                if speaker_ready.load(Ordering::Relaxed) {
                                    crate::debug_flags::request_beep_tone(tone.request_code());
                                    last_alert_beep_ms = Some(now_ms());
                                    info!("WARNING takeover: {} ({} active)", tone.as_str(), alerts.len());
                                }
                            }
                        }
                        weather::AlertKind::Watch => {
                            // Watch: play 3x over ~30s, no screen takeover
                            watch_beeps_remaining = 3;
                            if speaker_ready.load(Ordering::Relaxed) {
                                crate::debug_flags::request_beep_tone(tone.request_code());
                                watch_beeps_remaining -= 1;
                                last_alert_beep_ms = Some(now_ms());
                                info!("watch beep 1/3 queued ({} active)", alerts.len());
                            }
                        }
                        _ => {
                            // Advisory/Other: single beep with cooldown
                            let now = now_ms();
                            let can_beep = last_alert_beep_ms
                                .map(|ts| now.wrapping_sub(ts) >= (ALERT_BEEP_COOLDOWN_SECS as u32 * 1000))
                                .unwrap_or(true);
                            if can_beep && speaker_ready.load(Ordering::Relaxed) {
                                crate::debug_flags::request_beep_tone(tone.request_code());
                                last_alert_beep_ms = Some(now);
                                info!("advisory beep queued ({} active)", alerts.len());
                            }
                        }
                    }
                }

                state.weather_alerts = alerts;
                if state.weather_alerts.is_empty() {
                    state.now_alerts_open = false;
                    if state.warning_active {
                        state.warning_active = false;
                        if state.current_view == views::View::Warning {
                            state.current_view = views::View::Now;
                        }
                        info!("warning cleared: no active alerts");
                    }
                }
                last_alert_fingerprint = fp;
                alerts_snapshot_seen = true;
                state.dirty = true;
            }
        }

        // Repeating warning beep (every 20s while warning_active)
        if state.warning_active {
            if let Some(last) = last_alert_beep_ms {
                if now_ms().wrapping_sub(last) >= WARNING_BEEP_INTERVAL_MS
                    && speaker_ready.load(Ordering::Relaxed)
                {
                    let tone = alert_tone_for(&state.weather_alerts);
                    crate::debug_flags::request_beep_tone(tone.request_code());
                    last_alert_beep_ms = Some(now_ms());
                }
            }
        }

        // Watch beep repeat (every 10s, up to 3 total)
        if watch_beeps_remaining > 0 {
            if let Some(last) = last_alert_beep_ms {
                if now_ms().wrapping_sub(last) >= WATCH_BEEP_INTERVAL_MS
                    && speaker_ready.load(Ordering::Relaxed)
                {
                    let tone = alert_tone_for(&state.weather_alerts);
                    crate::debug_flags::request_beep_tone(tone.request_code());
                    watch_beeps_remaining -= 1;
                    last_alert_beep_ms = Some(now_ms());
                    info!("watch beep repeat ({} remaining)", watch_beeps_remaining);
                }
            }
        }

        // Console-requested warning silence
        if debug_flags::REQUEST_SILENCE_WARNING.swap(false, Ordering::Relaxed)
            && state.warning_active
        {
            state.warning_active = false;
            state.warning_silenced_fingerprint = last_alert_fingerprint.clone();
            crate::debug_flags::request_beep_stop();
            info!("warning silenced via console");
            state.dirty = true;
        }

        // Test warning injection from console
        if debug_flags::REQUEST_TEST_WARNING.swap(false, Ordering::Relaxed) {
            let fake = weather::WeatherAlert {
                id: format!("test-{}", now_ms()),
                event: "Tornado Warning".to_string(),
                headline: "The National Weather Service has issued a Tornado Warning for your area.".to_string(),
                description: "At 3:15 AM CDT, a severe thunderstorm capable of producing a tornado was located near Springfield, moving northeast at 45 mph.\n\nHAZARD: Tornado and quarter size hail.\n\nSOURCE: Radar indicated rotation.\n\nIMPACT: Flying debris will be dangerous to those caught without shelter. Mobile homes will be damaged or destroyed. Damage to roofs, windows, and vehicles will occur. Tree damage is likely.\n\nThis dangerous storm will be near Springfield by 3:30 AM CDT.".to_string(),
                instruction: "TAKE SHELTER NOW! Move to a basement or an interior room on the lowest floor of a sturdy building. Avoid windows. If you are outdoors, in a mobile home, or in a vehicle, move to the closest substantial shelter and protect yourself from flying debris.".to_string(),
                severity: "Extreme".to_string(),
                certainty: "Observed".to_string(),
                urgency: "Immediate".to_string(),
                expires: "2026-02-22T05:00:00-06:00".to_string(),
            };
            info!("TEST WARNING injected");
            state.weather_alerts = vec![fake];
            state.warning_active = true;
            state.warning_scroll = 0;
            state.warning_silenced_fingerprint.clear();
            state.current_view = views::View::Warning;
            last_alert_fingerprint = "test-warning".to_string();
            if speaker_ready.load(Ordering::Relaxed) {
                crate::debug_flags::request_beep_tone(speaker::AlertTone::Warning.request_code());
                last_alert_beep_ms = Some(now_ms());
            }
            state.dirty = true;
        }

        // IMU one-shot read requested from console
        if debug_flags::REQUEST_IMU_READ.swap(false, Ordering::Relaxed) {
            if imu_ok {
                if let Some(r) = qmi8658::read(&mut i2c) {
                    info!(
                        "IMU accel: x={:+.3}g y={:+.3}g z={:+.3}g  gyro: x={:+.1} y={:+.1} z={:+.1} dps  temp={:.1}C",
                        r.accel_x, r.accel_y, r.accel_z,
                        r.gyro_x, r.gyro_y, r.gyro_z,
                        r.temp_c
                    );
                } else {
                    info!("IMU: read failed");
                }
            } else {
                info!("IMU: not initialized");
            }
        }

        // IMU continuous debug logging (every 1s when debug imu is on)
        if imu_ok && debug_flags::is_on(&debug_flags::DEBUG_IMU)
            && tick_count.is_multiple_of(TIME_UPDATE_TICKS)
        {
            if let Some(r) = qmi8658::read(&mut i2c) {
                info!(
                    "IMU: ax={:+.2} ay={:+.2} az={:+.2}  gx={:+.1} gy={:+.1} gz={:+.1}",
                    r.accel_x, r.accel_y, r.accel_z,
                    r.gyro_x, r.gyro_y, r.gyro_z,
                );
            }
        }

        // Orientation mode updates requested from console
        if let Some(mode) = debug_flags::take_orientation_mode_request() {
            state.orientation_mode = mode;
            if mode != config::OrientationMode::Auto {
                let target = locked_orientation(mode, state.orientation_flip);
                if state.orientation != target {
                    apply_orientation(&mut state, &mut fb, target);
                    last_orientation_change_ms = now_ms();
                    info!("Orientation: {:?}", state.orientation);
                }
            }
        }

        // Orientation flip updates requested from console
        if let Some(flip) = debug_flags::take_orientation_flip_request() {
            state.orientation_flip = flip;
            if state.orientation_mode != config::OrientationMode::Auto {
                let target = locked_orientation(state.orientation_mode, state.orientation_flip);
                if state.orientation != target {
                    apply_orientation(&mut state, &mut fb, target);
                    last_orientation_change_ms = now_ms();
                    info!("Orientation: {:?}", state.orientation);
                }
            } else {
                info!("orientation flip ignored in auto mode");
            }
        }

        // IMU auto-orientation with hysteresis
        if imu_ok
            && state.orientation_mode == config::OrientationMode::Auto
            && tick_count.is_multiple_of(ORIENTATION_POLL_TICKS)
            && now_ms().wrapping_sub(last_orientation_change_ms) >= ORIENTATION_CHANGE_COOLDOWN_MS
        {
            if let Some(r) = qmi8658::read(&mut i2c) {
                if let Some(next) = detect_orientation_from_imu(&r) {
                    if next != state.orientation {
                        if orientation_candidate == next {
                            orientation_candidate_count =
                                orientation_candidate_count.saturating_add(1);
                        } else {
                            orientation_candidate = next;
                            orientation_candidate_count = 1;
                        }
                        if orientation_candidate_count >= ORIENTATION_CONFIRM_SAMPLES {
                            apply_orientation(&mut state, &mut fb, next);
                            orientation_candidate_count = 0;
                            last_orientation_change_ms = now_ms();
                            info!("Auto-rotation -> {:?}", state.orientation);
                        }
                    } else {
                        orientation_candidate_count = 0;
                    }
                }
            }
        }

        // IMU auto-orientation hysteresis state reset when not in auto mode
        if state.orientation_mode != config::OrientationMode::Auto {
            orientation_candidate_count = 0;
        }

        // I2C rescan requested from console
        if debug_flags::REQUEST_I2C_SCAN.swap(false, Ordering::Relaxed) {
            info!("I2C rescan...");
            let devices = scan_i2c(&mut i2c);
            state.i2c_devices = devices;
            state.dirty = true;
        }

        // Save C/F preference to NVS on toggle
        if state.save_celsius_pref {
            state.save_celsius_pref = false;
            if let Ok(mut nvs) = nvs.try_lock() {
                let _ = config::Config::save_use_celsius(&mut nvs, state.use_celsius);
            }
        }

        // Check for force weather refresh from tap
        if state.force_weather_refresh {
            state.force_weather_refresh = false;
            weather_refresh_flag.store(true, Ordering::Relaxed);
            state.status_text = "Refreshing...".to_string();
            state.dirty = true;
        }

        // Retry WiFi association every 5 minutes while disconnected.
        if !wifi_ok
            && !wifi_ssid.is_empty()
            && t.wrapping_sub(last_wifi_retry_ms) >= WIFI_RETRY_INTERVAL_MS
        {
            last_wifi_retry_ms = t;
            if let Some(wifi) = wifi_handle.as_mut() {
                info!("WiFi retry window reached; attempting reconnect...");
                match wifi::reconnect_existing(wifi.as_mut(), sysloop.clone()) {
                    Ok(Some((ip, networks))) => {
                        wifi_ok = true;
                        ip_address = ip;
                        state.ip_address = ip_address.clone();
                        state.status_text = ip_address.clone();
                        state.wifi_networks = networks;
                        if sntp.is_none() {
                            match time_sync::sync_time(&timezone) {
                                Ok(new_sntp) => sntp = Some(new_sntp),
                                Err(e) => log::warn!("NTP sync failed after WiFi reconnect: {}", e),
                            }
                        }
                        state.dirty = true;
                    }
                    Ok(None) => {
                        info!("WiFi reconnect did not succeed; retrying in 5 minutes");
                    }
                    Err(e) => {
                        log::warn!("WiFi reconnect error: {}", e);
                    }
                }
            }
        }

        // WiFi debug logging (RSSI etc)
        if tick_count.is_multiple_of(WIFI_DEBUG_TICKS)
            && debug_flags::is_on(&debug_flags::DEBUG_WIFI)
        {
            unsafe {
                let mut ap_info: esp_idf_sys::wifi_ap_record_t = core::mem::zeroed();
                if esp_idf_sys::esp_wifi_sta_get_ap_info(&mut ap_info) == esp_idf_sys::ESP_OK {
                    info!(
                        "WiFi: RSSI={} ch={} SSID={}",
                        ap_info.rssi,
                        ap_info.primary,
                        core::str::from_utf8(&ap_info.ssid)
                            .unwrap_or("?")
                            .trim_end_matches('\0')
                    );
                } else {
                    info!("WiFi: not connected");
                }
            }
        }

        // Update time display
        if tick_count.is_multiple_of(TIME_UPDATE_TICKS) {
            if let Some(t) = time_sync::format_local_time() {
                if t != state.time_text {
                    state.time_text = t;
                    state.dirty = true;
                }
            }
        }

        // Redraw if needed
        if state.dirty {
            views::draw_current_view(&mut fb, &state);
            fb.flush_to_panel(ctx.io, ctx.panel, state.orientation);
            state.dirty = false;
        }

        tick_count = tick_count.wrapping_add(1);
        std::thread::sleep(Duration::from_millis(TICK_MS));
    }
}
