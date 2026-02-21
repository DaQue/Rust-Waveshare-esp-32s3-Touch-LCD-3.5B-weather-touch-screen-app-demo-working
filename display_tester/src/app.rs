use anyhow::Result;
use std::time::{Duration, Instant};

use crate::display_config::DisplayConfig;
use crate::esp_display::{
    delay_ms, flush_io, init_bus, set_backlight, tx_color, tx_param, LCD_HEIGHT, LCD_WIDTH,
};
use crate::test_generator::{build_tests, InitVariant, TestCase};
use crate::test_patterns::fill_red_chunk;
use esp_idf_svc::nvs::{EspDefaultNvsPartition, EspNvs};

const CHUNK_LINES: i32 = 20;
const RESULTS_FILE: &str = "test_results.log";
const WORKING_FILE: &str = "working_configs.json";

struct ProgressState {
    last_test_number: usize,
    total_tests: usize,
    phase: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct WorkingEntry {
    format: String,
    qspi_mode: String,
    byte_order: String,
    bit_order: String,
    timing: crate::timing::TimingConfig,
    result: String,
    user_notes: String,
    timestamp: String,
}

pub fn run() -> Result<()> {
    esp_idf_sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    init_usb_serial_jtag();
    log::info!("RSTN is hardware-controlled (shared). Using software reset 0x01 when requested.");
    log::info!("Waiting 300ms after boot for power/reset stabilization.");
    delay_ms(300);

    let tests = build_tests();
    let default_nvs = EspDefaultNvsPartition::take()?;
    let mut nvs = EspNvs::new(default_nvs, "disp_test", true)?;
    log::info!("Progress resume enabled via NVS (namespace: disp_test).");
    let mut progress = load_progress(&mut nvs, tests.len());

    let mut start_idx = progress.last_test_number;
    if start_idx > 0 {
        start_idx -= 1;
    }

    for (idx, test) in tests.iter().enumerate().skip(start_idx) {
        run_test(idx + 1, tests.len(), test)?;
        progress.last_test_number = idx + 1;
        progress.phase = test.phase.to_string();
        save_progress(&mut nvs, &progress);
    }

    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}

fn run_test(index: usize, total: usize, test: &TestCase) -> Result<()> {
    log::info!("==============================================================");
    log::info!(
        "TEST #{} of {} ({})",
        index,
        total,
        test.phase
    );
    log::info!("==============================================================");

    print_config(test);

    let max_transfer = (LCD_WIDTH * CHUNK_LINES * test.config.format.bytes_per_pixel() as i32) as i32;
    let mut io = init_bus(&test.config, max_transfer)?;

    log_sequence_start(&test.config);
    run_init_sequence(&test.config, test.variant, io.io)?;
    set_backlight(true);
    delay_ms(test.config.timing.backlight_delay_ms);

    log::info!("Drawing RED test pattern...");
    draw_red_pattern(&test.config, io.io)?;
    log::info!("Test pattern complete");

    log::info!("What do you see? (w/gfb/nfb/b/off/n/pn/wc/s/q): ");
    let result = read_result_code();
    append_result(test, &result);

    if result.trim() == "w" {
        save_working_config(test, "w", "User marked as working")?;
    }

    io.cleanup();
    Ok(())
}

fn run_init_sequence(config: &DisplayConfig, variant: InitVariant, io: esp_idf_sys::esp_lcd_panel_io_handle_t) -> Result<()> {
    // Power rails assumed controlled externally; we just delay to simulate sequencing.
    delay_ms(config.timing.power_rail_delay_ms);
    delay_ms(config.timing.power_rail_delay_ms);

    if matches!(variant, InitVariant::ResetHeldDuringPower) {
        log::info!("Variant: software reset before init");
        tx_param(io, 0x01, None)?;
        delay_ms(config.timing.post_reset_wait_ms);
    }

    if matches!(variant, InitVariant::DoubleReset) {
        log::info!("Variant: double software reset");
        tx_param(io, 0x01, None)?;
        delay_ms(config.timing.post_reset_wait_ms);
        tx_param(io, 0x01, None)?;
        delay_ms(config.timing.post_reset_wait_ms);
    }

    if matches!(variant, InitVariant::AltCommandOrder) {
        tx_param(io, 0x3A, Some(&[config.format.pixel_format_param()]))?;
        delay_ms(config.timing.inter_command_delay_ms);
    }

    tx_param(io, 0x11, None)?;
    delay_ms(config.timing.sleep_out_wait_ms);

    tx_param(io, 0x3A, Some(&[config.format.pixel_format_param()]))?;
    delay_ms(config.timing.inter_command_delay_ms);

    tx_param(io, 0x36, Some(&[0x00]))?;
    delay_ms(config.timing.inter_command_delay_ms);

    if !matches!(variant, InitVariant::MinimalInit) {
        tx_param(io, 0x29, None)?;
        delay_ms(config.timing.display_on_wait_ms);
    }

    Ok(())
}

fn draw_red_pattern(config: &DisplayConfig, io: esp_idf_sys::esp_lcd_panel_io_handle_t) -> Result<()> {
    let mut buf = vec![0u8; (LCD_WIDTH * CHUNK_LINES) as usize * config.format.bytes_per_pixel()];

    let col = [0x00, 0x00, 0x01, 0x3F];
    let row = [0x00, 0x00, 0x01, 0xDF];
    tx_param(io, 0x2A, Some(&col))?;
    tx_param(io, 0x2B, Some(&row))?;

    let mut y = 0;
    while y < LCD_HEIGHT {
        let lines = (LCD_HEIGHT - y).min(CHUNK_LINES) as usize;
        let pixels = LCD_WIDTH as usize * lines;
        let bytes = pixels * config.format.bytes_per_pixel();
        let slice = &mut buf[..bytes];
        fill_red_chunk(slice, LCD_WIDTH as usize, lines, config);
        tx_color(io, 0x2C, slice)?;
        flush_io(io)?;
        y += CHUNK_LINES;
    }

    Ok(())
}

fn print_config(test: &TestCase) {
    let cfg = &test.config;
    log::info!("Configuration:");
    log::info!("  Color Format:     {}", cfg.format.name());
    log::info!("  QSPI Mode:        {}", cfg.qspi_mode.name());
    log::info!("  Byte Order:       {}", cfg.byte_order.name());
    log::info!("  Bit Order:        {}", cfg.bit_order.name());
    log::info!("  Command 3Ah:      0x{:02X}", cfg.format.pixel_format_param());
    log::info!("");
    log::info!("Timing Configuration:");
    log::info!("  Reset Hold:       {}ms", cfg.timing.reset_hold_ms);
    log::info!("  Post-Reset Wait:  {}ms", cfg.timing.post_reset_wait_ms);
    log::info!("  Sleep Out Wait:   {}ms", cfg.timing.sleep_out_wait_ms);
    log::info!("  Display On Wait:  {}ms", cfg.timing.display_on_wait_ms);
    log::info!("  Power Rail Delay: {}ms", cfg.timing.power_rail_delay_ms);
    log::info!("  Backlight Delay:  {}ms", cfg.timing.backlight_delay_ms);
    log::info!("  Inter Cmd Delay:  {}ms", cfg.timing.inter_command_delay_ms);
    log::info!("");
    log::info!("Pin Configuration:");
    log::info!("  CS:   GPIO 45");
    log::info!("  CLK:  GPIO 47");
    log::info!("  D0:   GPIO 21");
    log::info!("  D1:   GPIO 48");
    log::info!("  D2:   GPIO 40");
    log::info!("  D3:   GPIO 39");
    log::info!("  DC:   GPIO 8");
    log::info!("  BL:   GPIO 1");
    log::info!("  RST:  -1 (shared)");
    log::info!("");
}

fn log_sequence_start(config: &DisplayConfig) {
    log::info!("Initialization Sequence:");
    log::info!("[00.000] Enabling VCI...");
    log::info!("[00.100] Enabling VDDI...");
    log::info!("[00.200] RSTN hardware-controlled (shared)...");
    log::info!("[00.700] Waiting {}ms post-reset...", config.timing.post_reset_wait_ms);
}

fn append_result(test: &TestCase, result: &str) {
    let line = format!(
        "[{}] TEST #{} START\nConfig: {}, {}, {}, Timing: {}/{}/{}/{}ms\nResult: {}\n----------------------------------------\n",
        now_iso(),
        test.id,
        test.config.format.name(),
        test.config.qspi_mode.name(),
        test.config.byte_order.name(),
        test.config.timing.reset_hold_ms,
        test.config.timing.post_reset_wait_ms,
        test.config.timing.sleep_out_wait_ms,
        test.config.timing.display_on_wait_ms,
        result.trim()
    );
    append_to_file(RESULTS_FILE, &line);
}

fn save_working_config(test: &TestCase, result: &str, note: &str) -> Result<()> {
    let entry = WorkingEntry {
        format: test.config.format.name().to_string(),
        qspi_mode: test.config.qspi_mode.name().to_string(),
        byte_order: test.config.byte_order.name().to_string(),
        bit_order: test.config.bit_order.name().to_string(),
        timing: test.config.timing,
        result: result.to_string(),
        user_notes: note.to_string(),
        timestamp: now_iso(),
    };
    let key = format!("test_{}", test.id);

    let mut map = load_json_map::<WorkingEntry>(WORKING_FILE);
    map.insert(key, entry);
    let json = serde_json::to_string_pretty(&map)?;
    std::fs::write(WORKING_FILE, json)?;
    Ok(())
}

fn load_progress(nvs: &mut EspNvs<EspDefaultNvsPartition>, total_tests: usize) -> ProgressState {
    let last = nvs.get_u32("last_test").ok().flatten().unwrap_or(0) as usize;
    let mut phase_buf = [0u8; 64];
    let phase = nvs
        .get_str("phase", &mut phase_buf)
        .ok()
        .flatten()
        .unwrap_or("Phase 1: Timing Validation")
        .to_string();

    ProgressState {
        last_test_number: last.min(total_tests),
        total_tests,
        phase,
    }
}

fn save_progress(nvs: &mut EspNvs<EspDefaultNvsPartition>, progress: &ProgressState) {
    let _ = nvs.set_u32("last_test", progress.last_test_number as u32);
    let _ = nvs.set_str("phase", &progress.phase);
}

fn append_to_file(path: &str, contents: &str) {
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(contents.as_bytes());
    }
}

fn load_json_map<T>(path: &str) -> std::collections::BTreeMap<String, T>
where
    T: serde::de::DeserializeOwned,
{
    if let Ok(contents) = std::fs::read_to_string(path) {
        if let Ok(map) = serde_json::from_str(&contents) {
            return map;
        }
    }
    std::collections::BTreeMap::new()
}

fn now_iso() -> String {
    let now = esp_idf_sys::esp_timer_get_time();
    let secs = now / 1_000_000;
    format!("{}s", secs)
}

fn init_usb_serial_jtag() {
    unsafe {
        let mut cfg = esp_idf_sys::usb_serial_jtag_driver_config_t {
            tx_buffer_size: 1024,
            rx_buffer_size: 1024,
        };
        esp_idf_sys::usb_serial_jtag_driver_install(&mut cfg);
    }
}

fn read_result_code() -> String {
    let tick_hz = esp_idf_sys::configTICK_RATE_HZ as u32;
    let ticks = ((10 * tick_hz) / 1000).max(1);
    let mut buf = [0u8; 1];
    let mut out = String::new();
    let start = Instant::now();
    loop {
        let n = unsafe {
            esp_idf_sys::usb_serial_jtag_read_bytes(buf.as_mut_ptr().cast(), 1, ticks)
        };
        if n > 0 {
            let b = buf[0];
            if b == b'\r' || b == b'\n' {
                if !out.is_empty() {
                    return out;
                }
            } else if b == 0x08 || b == 0x7f {
                out.pop();
            } else if let Some(ch) = char::from_u32(b as u32) {
                if !ch.is_control() {
                    out.push(ch);
                }
            }
        }
        if start.elapsed() > Duration::from_secs(120) && !out.is_empty() {
            return out;
        }
    }
}
