mod bme280_sensor;
mod config;
mod console;
mod framebuffer;
mod http_client;
mod layout;
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

// ── I2C ──────────────────────────────────────────────────────────────
const I2C_FREQ_HZ: u32 = 100_000;

// ── Timing ──────────────────────────────────────────────────────────
const WEATHER_INTERVAL_SECS: u64 = 600;
const WEATHER_RETRY_SECS: u64 = 30;
const BME280_INTERVAL_MS: u32 = 5_000;
const TICK_MS: u64 = 100;
const TIME_UPDATE_TICKS: u32 = 10; // every second

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

fn now_ms() -> u32 {
    unsafe { (esp_idf_sys::esp_timer_get_time() / 1000) as u32 }
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
    use profont::{PROFONT_18_POINT, PROFONT_12_POINT};

    let bg = layout::rgb(20, 24, 32);
    fb.clear_color(bg);

    let title_style = MonoTextStyle::new(&PROFONT_18_POINT, Rgb565::new(28, 56, 31));
    Text::with_alignment(
        "Weather Station",
        Point::new(240, 120),
        title_style,
        Alignment::Center,
    )
    .draw(fb)
    .ok();

    let sub_style = MonoTextStyle::new(&PROFONT_12_POINT, Rgb565::new(18, 36, 20));
    Text::with_alignment(
        "Waveshare ESP32-S3 3.5B",
        Point::new(240, 148),
        sub_style,
        Alignment::Center,
    )
    .draw(fb)
    .ok();

    let status_style = MonoTextStyle::new(&PROFONT_12_POINT, Rgb565::new(12, 28, 14));
    Text::with_alignment(
        status,
        Point::new(240, 200),
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

    info!("BOOT — waveshare_s3_3p weather dashboard");

    // ── 1. Board power (TCA9554 IO expander + AXP2101 PMIC + LCD reset) ──
    esp_check(unsafe { board_power_init() }, "board_power_init")?;
    info!("Power + LCD reset OK");

    // ── 2. Display init + immediate splash screen ──
    let ctx = init_display()?;

    // Create framebuffer early so we can show a boot screen immediately
    let mut fb = framebuffer::Framebuffer::new(
        framebuffer::FB_WIDTH,
        framebuffer::FB_HEIGHT,
    );

    // Show splash before backlight so first frame is ready
    draw_splash(&mut fb, "Starting...");
    fb.flush_to_panel(ctx.io, ctx.panel);
    enable_backlight();

    // ── 3. Peripherals ──
    let peripherals = unsafe { Peripherals::new() };
    let sysloop = EspSystemEventLoop::take()?;
    let nvs_partition = EspDefaultNvsPartition::take()?;

    // ── 4. NVS config ──
    let nvs = EspNvs::new(nvs_partition.clone(), config::NS, true)?;
    let cfg = config::Config::load(&nvs);
    let wifi_ssid = cfg.wifi_ssid.clone();
    let wifi_pass = cfg.wifi_pass.clone();
    let api_key = cfg.weather_api_key.clone();
    let weather_query = cfg.weather_query.clone();
    let timezone = cfg.timezone.clone();

    let nvs = Arc::new(Mutex::new(nvs));
    let cfg = Arc::new(Mutex::new(cfg));

    // ── 5. Console (serial interactive) ──
    console::spawn_console(nvs.clone(), cfg.clone());

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

    // ── 7. BME280 sensor ──
    let bme280 = bme280_sensor::Bme280::init(&mut i2c);
    if bme280.is_some() {
        info!("BME280 sensor ready");
    }

    // ── 8. Touch controller ──
    touch::probe(&mut i2c);
    let mut touch_state = touch::TouchState::new();

    // ── 9. WiFi ──
    let mut ip_address = String::new();
    let mut wifi_networks: Vec<(String, i8)> = Vec::new();
    let wifi_ok;
    let _wifi = if !wifi_ssid.is_empty() {
        draw_splash(&mut fb, &format!("Connecting to '{}'...", wifi_ssid));
        fb.flush_to_panel(ctx.io, ctx.panel);
        info!("Connecting to WiFi '{}'...", wifi_ssid);
        match wifi::connect_wifi(peripherals.modem, sysloop.clone(), &wifi_ssid, &wifi_pass) {
            Ok(result) => {
                if let Ok(ip_info) = result.wifi.sta_netif().get_ip_info() {
                    ip_address = format!("{}", ip_info.ip);
                }
                wifi_networks = result.networks;
                wifi_ok = true;
                Some(result.wifi)
            }
            Err(e) => {
                log::warn!("WiFi failed: {}", e);
                wifi_ok = false;
                None
            }
        }
    } else {
        log::warn!("No WiFi SSID configured (use console: wifi set <ssid> <pass>)");
        wifi_ok = false;
        None
    };

    // ── 10. NTP time sync ──
    let _sntp = if wifi_ok {
        draw_splash(&mut fb, "Syncing time...");
        fb.flush_to_panel(ctx.io, ctx.panel);
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

    // ── 11. App state ──
    let mut state = views::AppState::new();
    state.i2c_devices = i2c_devices;
    state.wifi_networks = wifi_networks;
    state.ip_address = ip_address.clone();
    if wifi_ok {
        state.status_text = format!("WiFi: {} | {}", wifi_ssid, ip_address);
    } else if wifi_ssid.is_empty() {
        state.status_text = "No WiFi configured".to_string();
    } else {
        state.status_text = format!("WiFi '{}' failed", wifi_ssid);
    }

    // ── 12. Weather fetch thread ──
    let weather_data: Arc<Mutex<Option<(weather::CurrentWeather, weather::Forecast)>>> =
        Arc::new(Mutex::new(None));

    if wifi_ok && !api_key.is_empty() {
        let wd = weather_data.clone();
        let query = weather_query.clone();
        let key = api_key.clone();
        std::thread::Builder::new()
            .name("weather".into())
            .stack_size(16384)
            .spawn(move || {
                loop {
                    info!("Weather fetch starting...");
                    match weather::fetch_weather(&query, &key) {
                        Ok((current, forecast)) => {
                            info!(
                                "Weather: {}°F {} in {}",
                                current.temp_f as i32, current.condition, current.city
                            );
                            *wd.lock().unwrap() = Some((current, forecast));
                        }
                        Err(e) => {
                            log::warn!("Weather fetch failed: {}", e);
                            std::thread::sleep(Duration::from_secs(WEATHER_RETRY_SECS));
                            continue;
                        }
                    }
                    std::thread::sleep(Duration::from_secs(WEATHER_INTERVAL_SECS));
                }
            })
            .expect("failed to spawn weather thread");
    } else if api_key.is_empty() {
        log::warn!("No weather API key (use console: api set-key <key>)");
    }

    // ── 13. Main event loop ──
    info!("Entering main loop");
    let mut last_bme_ms: u32 = 0;
    let mut tick_count: u32 = 0;

    // Initial draw
    views::draw_current_view(&mut fb, &state);
    fb.flush_to_panel(ctx.io, ctx.panel);
    state.dirty = false;

    loop {
        let t = now_ms();

        // Poll touch
        let gesture = touch_state.poll(&mut i2c, t);
        if gesture != touch::Gesture::None
            && state.handle_gesture(gesture) {
                info!("Gesture {:?} -> view {:?}", gesture, state.current_view);
            }

        // BME280 read
        if t.wrapping_sub(last_bme_ms) >= BME280_INTERVAL_MS {
            last_bme_ms = t;
            if let Some(ref sensor) = bme280 {
                if let Some(reading) = sensor.read(&mut i2c) {
                    state.indoor_temp = Some(reading.temperature_f);
                    state.indoor_humidity = Some(reading.humidity);
                    state.indoor_pressure = Some(reading.pressure_hpa);
                    state.dirty = true;
                }
            }
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
                state.dirty = true;
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
            fb.flush_to_panel(ctx.io, ctx.panel);
            state.dirty = false;
        }

        tick_count = tick_count.wrapping_add(1);
        std::thread::sleep(Duration::from_millis(TICK_MS));
    }
}
