use anyhow::Result;
use core::ffi::c_void;
use std::time::Duration;

use crate::display_config::{BitOrder, DisplayConfig, QspiMode};

pub const LCD_WIDTH: i32 = 320;
pub const LCD_HEIGHT: i32 = 480;

const PIN_LCD_CS: i32 = 45;
const PIN_LCD_SCLK: i32 = 47;
const PIN_LCD_D0: i32 = 21;
const PIN_LCD_D1: i32 = 48;
const PIN_LCD_D2: i32 = 40;
const PIN_LCD_D3: i32 = 39;
const PIN_LCD_DC: i32 = 8;
const PIN_LCD_BL: i32 = 1;
const PIN_LCD_RST: i32 = -1;

pub struct LcdIo {
    host: esp_idf_sys::spi_host_device_t,
    pub io: esp_idf_sys::esp_lcd_panel_io_handle_t,
}

impl LcdIo {
    pub fn cleanup(&mut self) {
        unsafe {
            esp_idf_sys::esp_lcd_panel_io_del(self.io);
            esp_idf_sys::spi_bus_free(self.host);
        }
    }
}

pub fn init_bus(config: &DisplayConfig, max_transfer: i32) -> Result<LcdIo> {
    let mut bus_cfg = esp_idf_sys::spi_bus_config_t::default();
    let (miso, quadwp, quadhd) = match config.qspi_mode {
        QspiMode::Quad => (PIN_LCD_D1, PIN_LCD_D2, PIN_LCD_D3),
        QspiMode::Dual => (PIN_LCD_D1, -1, -1),
        QspiMode::Single => (-1, -1, -1),
    };

    bus_cfg.__bindgen_anon_1.mosi_io_num = PIN_LCD_D0;
    bus_cfg.__bindgen_anon_2.miso_io_num = miso;
    bus_cfg.__bindgen_anon_3.quadwp_io_num = quadwp;
    bus_cfg.__bindgen_anon_4.quadhd_io_num = quadhd;
    bus_cfg.sclk_io_num = PIN_LCD_SCLK;
    bus_cfg.max_transfer_sz = max_transfer;
    bus_cfg.flags = 0;
    bus_cfg.intr_flags = 0;

    let host = esp_idf_sys::spi_host_device_t_SPI2_HOST;
    let bus_res = unsafe {
        esp_idf_sys::spi_bus_initialize(
            host,
            &bus_cfg,
            esp_idf_sys::spi_common_dma_t_SPI_DMA_CH_AUTO,
        )
    };
    if bus_res != esp_idf_sys::ESP_OK {
        return Err(anyhow::anyhow!("spi_bus_initialize failed {}", bus_res));
    }

    let mut io: esp_idf_sys::esp_lcd_panel_io_handle_t = core::ptr::null_mut();
    let quad = matches!(config.qspi_mode, QspiMode::Quad);
    let sio = matches!(config.qspi_mode, QspiMode::Single);
    let lsb = matches!(config.bit_order, BitOrder::LSBFirst);

    let io_cfg = esp_idf_sys::esp_lcd_panel_io_spi_config_t {
        cs_gpio_num: PIN_LCD_CS,
        dc_gpio_num: PIN_LCD_DC,
        spi_mode: 0,
        pclk_hz: 5_000_000,
        trans_queue_depth: 1,
        on_color_trans_done: None,
        user_ctx: core::ptr::null_mut(),
        lcd_cmd_bits: 8,
        lcd_param_bits: 8,
        flags: esp_idf_sys::esp_lcd_panel_io_spi_config_t__bindgen_ty_1 {
            _bitfield_align_1: [],
            _bitfield_1: esp_idf_sys::esp_lcd_panel_io_spi_config_t__bindgen_ty_1::new_bitfield_1(
                0,
                0,
                0,
                0,
                if quad { 1 } else { 0 },
                if sio { 1 } else { 0 },
                if lsb { 1 } else { 0 },
                0,
            ),
            __bindgen_padding_0: [0; 3],
        },
    };

    let io_res = unsafe {
        esp_idf_sys::esp_lcd_new_panel_io_spi(
            host as esp_idf_sys::esp_lcd_spi_bus_handle_t,
            &io_cfg,
            &mut io,
        )
    };
    if io_res != esp_idf_sys::ESP_OK {
        unsafe { esp_idf_sys::spi_bus_free(host) };
        return Err(anyhow::anyhow!("esp_lcd_new_panel_io_spi failed {}", io_res));
    }

    Ok(LcdIo { host, io })
}

pub fn set_backlight(level: bool) {
    unsafe {
        let io_conf = esp_idf_sys::gpio_config_t {
            pin_bit_mask: 1u64 << (PIN_LCD_BL as u64),
            mode: esp_idf_sys::gpio_mode_t_GPIO_MODE_OUTPUT,
            pull_up_en: esp_idf_sys::gpio_pullup_t_GPIO_PULLUP_DISABLE,
            pull_down_en: esp_idf_sys::gpio_pulldown_t_GPIO_PULLDOWN_DISABLE,
            intr_type: esp_idf_sys::gpio_int_type_t_GPIO_INTR_DISABLE,
        };
        esp_idf_sys::gpio_config(&io_conf);
        esp_idf_sys::gpio_set_level(PIN_LCD_BL, if level { 1 } else { 0 });
    }
}

pub fn set_reset(level: bool) {
    if PIN_LCD_RST < 0 {
        return;
    }
    unsafe {
        let io_conf = esp_idf_sys::gpio_config_t {
            pin_bit_mask: 1u64 << (PIN_LCD_RST as u64),
            mode: esp_idf_sys::gpio_mode_t_GPIO_MODE_OUTPUT,
            pull_up_en: esp_idf_sys::gpio_pullup_t_GPIO_PULLUP_DISABLE,
            pull_down_en: esp_idf_sys::gpio_pulldown_t_GPIO_PULLDOWN_DISABLE,
            intr_type: esp_idf_sys::gpio_int_type_t_GPIO_INTR_DISABLE,
        };
        esp_idf_sys::gpio_config(&io_conf);
        esp_idf_sys::gpio_set_level(PIN_LCD_RST, if level { 1 } else { 0 });
    }
}

pub fn delay_ms(ms: u32) {
    std::thread::sleep(Duration::from_millis(ms as u64));
}

pub fn tx_param(io: esp_idf_sys::esp_lcd_panel_io_handle_t, cmd: i32, data: Option<&[u8]>) -> Result<()> {
    let (ptr, len) = match data {
        Some(bytes) => (bytes.as_ptr() as *const c_void, bytes.len()),
        None => (core::ptr::null(), 0),
    };
    let res = unsafe { esp_idf_sys::esp_lcd_panel_io_tx_param(io, cmd, ptr, len) };
    if res != esp_idf_sys::ESP_OK {
        return Err(anyhow::anyhow!("tx_param cmd {:#x} failed {}", cmd, res));
    }
    Ok(())
}

pub fn tx_color(io: esp_idf_sys::esp_lcd_panel_io_handle_t, cmd: i32, buf: &[u8]) -> Result<()> {
    let res = unsafe {
        esp_idf_sys::esp_lcd_panel_io_tx_color(io, cmd, buf.as_ptr() as *const c_void, buf.len())
    };
    if res != esp_idf_sys::ESP_OK {
        return Err(anyhow::anyhow!("tx_color cmd {:#x} failed {}", cmd, res));
    }
    Ok(())
}

pub fn flush_io(io: esp_idf_sys::esp_lcd_panel_io_handle_t) -> Result<()> {
    let res = unsafe { esp_idf_sys::esp_lcd_panel_io_tx_param(io, -1, core::ptr::null(), 0) };
    if res != esp_idf_sys::ESP_OK {
        return Err(anyhow::anyhow!("flush_io failed {}", res));
    }
    Ok(())
}
