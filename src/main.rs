mod bme280_sensor;
mod config;
mod console;
mod debug_flags;
mod framebuffer;
mod history_nvs;
mod history_ring;
mod http_client;
mod hvac;
mod layout;
mod pressure_history;
mod psbox;
mod qmi8658;
mod render;
mod sensor_poll;
mod speaker;
mod time_sync;
mod touch;
mod views;
mod weather;
mod weather_icons;
mod weather_task;
mod web_server;
mod wifi;

use anyhow::Result;
use esp_idf_hal::i2c::{I2cConfig, I2cDriver};
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::units::Hertz;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::nvs::{EspDefaultNvsPartition, EspNvs};
use log::info;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// ── SRAM watch atomics (shared with http_client.rs) ─────────────────
/// Set by http_fetch_into when SRAM largest block drops below 12 KB for the
/// first time; cleared by the main loop which then resets the BME280 driver
/// to free any leaked I2C state.
pub static SRAM_BME_RESET: AtomicBool = AtomicBool::new(false);
/// Counts consecutive HTTP fetches where the largest SRAM block was < 12 KB.
/// Resets to 0 whenever a fetch completes with sufficient headroom.
pub static SRAM_LOW_STREAK: AtomicU32 = AtomicU32::new(0);
/// Set by http_fetch_into after 3 consecutive low-SRAM fetches; the main loop
/// saves history to NVS then calls esp_restart().
pub static SRAM_DO_REBOOT: AtomicBool = AtomicBool::new(false);

// ── HTTP admission control thresholds (v0.6.0) ───────────────────────
/// Minimum SRAM largest-free-block (bytes) to serve heavy endpoints (/api/history).
/// Below this, return HTTP 503 for history requests; /api/current still served.
pub const SRAM_ADMIT_MIN_BLOCK: u32 = 20_000;
/// Minimum SRAM largest-free-block (bytes) before all non-essential HTTP returns 503.
pub const SRAM_CRITICAL_BLOCK: u32 = 12_000;
/// Minimum SRAM largest-free-block (bytes); if sustained for REBOOT_STREAK_COUNT
/// consecutive main-loop checks, triggers graceful reboot.
pub const REBOOT_THRESHOLD: u32 = 7_000;
/// Consecutive main-loop ticks below REBOOT_THRESHOLD before graceful reboot fires.
/// At ~5s check interval this equals ~25s of sustained critical memory pressure.
pub const REBOOT_STREAK_COUNT: u32 = 5;

// ── Pins ─────────────────────────────────────────────────────────────
const PIN_I2S_BCLK: i32 = 13;
const PIN_I2S_DOUT: i32 = 16;
const PIN_I2S_WS: i32 = 15;
const PIN_I2S_MCLK: i32 = 44;

// ── I2C ──────────────────────────────────────────────────────────────
const I2C_FREQ_HZ: u32 = 100_000;

// ── Timing ──────────────────────────────────────────────────────────
pub(crate) const WEATHER_INTERVAL_SECS: u64 = 300;
pub(crate) const WEATHER_RETRY_SECS: u64 = 30;
const WEATHER_STALE_AFTER_SECS: u64 = WEATHER_INTERVAL_SECS + 120;
pub(crate) const ALERTS_INTERVAL_SECS: u64 = 180; // 3 min — scanning for new alerts
pub(crate) const ALERTS_ACTIVE_INTERVAL_SECS: u64 = 900; // 15 min — alert known, no need to hammer
pub(crate) const ALERTS_START_DELAY_SECS: u64 = 20;
pub(crate) const ALERT_BEEP_COOLDOWN_SECS: u64 = 600;
pub(crate) const WARNING_BEEP_INTERVAL_MS: u32 = 20_000;
pub(crate) const WATCH_BEEP_INTERVAL_MS: u32 = 10_000;
pub(crate) const BME280_INTERVAL_MS: u32 = 5_000;
const HVAC_DETECT_INTERVAL_MS: u32 = 5_000;
const HVAC_RECORD_INTERVAL_MS: u32 = 30_000;
const PRESSURE_SAMPLE_INTERVAL_MS: u32 = pressure_history::LONG_PERIOD_SECS * 1000;
const TICK_MS: u64 = 20;
const TIME_UPDATE_TICKS: u32 = 10; // every second
const WIFI_DEBUG_TICKS: u32 = 100; // every 10 seconds
const WIFI_RETRY_INTERVAL_MS: u32 = 300_000;
pub(crate) const FAILURE_WARN_EVERY: u32 = 10;

// ── Helpers ─────────────────────────────────────────────────────────

pub(crate) fn esp_check(res: esp_idf_sys::esp_err_t, msg: &str) -> Result<()> {
    if res != esp_idf_sys::ESP_OK {
        Err(anyhow::anyhow!("{} (err {})", msg, res))
    } else {
        Ok(())
    }
}

pub fn now_ms() -> u32 {
    unsafe { (esp_idf_sys::esp_timer_get_time() / 1000) as u32 }
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

// ── Entry point ─────────────────────────────────────────────────────

fn main() -> Result<()> {
    esp_idf_sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    info!(
        "BOOT — waveshare_esp32-s3-touch-lcd-3p5b weather dashboard v{}",
        env!("CARGO_PKG_VERSION")
    );

    // ── PSRAM availability check ──────────────────────────────────────
    let psram_total =
        unsafe { esp_idf_sys::heap_caps_get_total_size(esp_idf_sys::MALLOC_CAP_SPIRAM) };
    if psram_total == 0 {
        log::error!("PSRAM not available — dashboard history storage requires PSRAM");
    } else {
        info!("PSRAM: {} KB available", psram_total / 1024);
    }

    // ── History ring (PSRAM, ~236 KB, 7-day @ 1-min) ─────────────────
    let history: std::sync::Arc<std::sync::Mutex<history_ring::HistoryRing>> =
        std::sync::Arc::new(std::sync::Mutex::new(history_ring::HistoryRing::new()));
    info!(
        "History ring allocated ({} samples × 24 B = {} KB)",
        history_ring::HISTORY_CAP,
        history_ring::HISTORY_CAP * 24 / 1024
    );

    // ── 1. Board power (TCA9554 IO expander + AXP2101 PMIC + LCD reset) ──
    esp_check(
        unsafe { framebuffer::board_power_init() },
        "board_power_init",
    )?;
    info!("Power + LCD reset OK");

    // ── 2. Display init + immediate splash screen ──
    let ctx = framebuffer::init_display()?;

    // Create framebuffer early so we can show a boot screen immediately
    let mut fb = framebuffer::Framebuffer::new(framebuffer::FB_WIDTH, framebuffer::FB_HEIGHT);

    // Show splash before backlight so first frame is ready
    views::draw_splash(&mut fb, "Starting...");
    ctx.flush_fb(&fb, layout::Orientation::Landscape);
    framebuffer::enable_backlight();

    // ── 3. Peripherals ──
    let peripherals = unsafe { Peripherals::new() };
    let sysloop = EspSystemEventLoop::take()?;
    let nvs_partition = EspDefaultNvsPartition::take()?;

    // ── 4. NVS config ──
    // If NVS is corrupted (e.g. after reflash with different encryption keys),
    // auto-erase and reinitialise rather than boot-looping.
    let mut nvs = match EspNvs::new(nvs_partition.clone(), config::NS, true) {
        Ok(nvs) => nvs,
        Err(e) => {
            log::error!(
                "NVS open failed ({}); erasing partition and reinitialising",
                e
            );
            views::draw_splash(&mut fb, "NVS error — resetting config...");
            ctx.flush_fb(&fb, layout::Orientation::Landscape);
            unsafe { esp_idf_sys::nvs_flash_erase() };
            EspNvs::new(nvs_partition.clone(), config::NS, true)?
        }
    };
    let mut cfg = config::Config::load(&nvs);
    let legacy_nws_ua = "waveshare_esp32-s3-touch-lcd-3p5b/0.1 (contact: unset)";
    let default_nws_ua = format!(
        "waveshare_esp32-s3-touch-lcd-3p5b/{} (contact: unset)",
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
    // Force Rust pthread mutex lazy-init now while the heap is clean.
    // Without this, the first lock (line ~819) can crash in heap_caps_alloc
    // if the heap has been disturbed by board/display init and WiFi never ran.
    drop(nvs.lock().unwrap());
    drop(cfg.lock().unwrap());

    let weather_refresh_flag: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));

    // ── 5. Console (serial interactive) ──
    console::spawn_console(nvs.clone(), cfg.clone(), weather_refresh_flag.clone());

    // ── 6. I2C bus ──
    // board_power_init() already installed an I2C driver on port 0 for the PMIC.
    // We must delete it before creating our own Rust I2cDriver on the same port.
    unsafe {
        esp_idf_sys::i2c_driver_delete(0);
    }

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
    let mut bme280 = bme280_sensor::Bme280::init(&mut i2c);
    if bme280.is_some() {
        info!("BME280 sensor ready");
    } else {
        log::warn!("BME280 init failed — will retry every 30s");
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
                let mut speaker =
                    match speaker::Speaker::new(i2s0, pin_bclk, pin_dout, pin_ws, Some(pin_mclk)) {
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
    let mut wifi_ok = false;
    let mut wifi_handle = if !wifi_ssid.is_empty() {
        views::draw_splash(&mut fb, &format!("Connecting to '{}'...", wifi_ssid));
        ctx.flush_fb(&fb, layout::Orientation::Landscape);
        info!("Connecting to WiFi '{}'...", wifi_ssid);
        match wifi::connect_wifi(peripherals.modem, sysloop.clone(), &wifi_ssid, &wifi_pass) {
            Ok(result) => {
                if let Some(ip) = result.ip_address {
                    ip_address = ip;
                }
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
        views::draw_splash(&mut fb, "Syncing time...");
        ctx.flush_fb(&fb, layout::Orientation::Landscape);
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

    // ── 10a. HTTP server (after WiFi confirmed) ──
    let web_snapshot: std::sync::Arc<std::sync::Mutex<web_server::WebSnapshot>> =
        std::sync::Arc::new(std::sync::Mutex::new(web_server::WebSnapshot {
            firmware: env!("CARGO_PKG_VERSION").to_string(),
            ip_address: ip_address.clone(),
            ..Default::default()
        }));
    let _http_server = if wifi_ok {
        match web_server::start(web_snapshot.clone(), history.clone()) {
            Ok(s) => {
                info!("Web server ready");
                Some(s)
            }
            Err(e) => {
                log::warn!("HTTP server failed to start: {}", e);
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
                    if let Err(e) =
                        config::Config::save_weather_query(&mut nvs.lock().unwrap(), &query)
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
        layout::locked_orientation(state.orientation_mode, state.orientation_flip)
    };
    // Restore sensor history from NVS (populated by proactive reboot path)
    history_nvs::history_nvs_restore(&mut state, &nvs);
    history_nvs::dashboard_history_nvs_restore(&history, &nvs);

    state.i2c_devices = i2c_devices;
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
            if let Some(orientation) = layout::detect_orientation_from_imu(&r) {
                state.orientation = orientation;
            }
        }
    }

    if state.orientation.is_portrait() {
        let (fb_w, fb_h) = layout::framebuffer_dims(state.orientation);
        fb = framebuffer::Framebuffer::new(fb_w, fb_h);
    }

    // ── 12. Weather fetch thread ──
    // Shared mutex: prevents weather and NWS alert threads from running
    // simultaneous TLS handshakes, which interleave SRAM allocations and
    // permanently fragment the heap. Held only for the duration of HTTP calls.
    let http_lock: Arc<Mutex<()>> = Arc::new(Mutex::new(()));

    let weather_data: Arc<Mutex<Option<(weather::CurrentWeather, weather::Forecast)>>> =
        Arc::new(Mutex::new(None));
    weather_task::spawn_weather_thread(
        weather_data.clone(),
        weather_refresh_flag.clone(),
        cfg.clone(),
        http_lock.clone(),
    );

    // ── 12b. NWS alerts fetch thread ──
    let alert_data: Arc<Mutex<Option<Vec<weather::WeatherAlert>>>> = Arc::new(Mutex::new(None));
    weather_task::spawn_alerts_thread(
        alert_data.clone(),
        cfg.clone(),
        nvs.clone(),
        http_lock.clone(),
    );

    // ── 13. Main event loop ──
    info!("Entering main loop");
    let mut bme_ps = sensor_poll::BmePollState::default();
    let mut alert_ps = sensor_poll::AlertPollState::default();
    let mut last_hvac_detect_ms: u32 = 0;
    let mut last_hvac_record_ms: u32 = 0;
    let mut last_pressure_sample_ms: u32 = 0;
    let mut tick_count: u32 = 0;
    let mut render_sram_low_streak: u8 = 0;
    let mut orientation_candidate = state.orientation;
    let mut orientation_candidate_count: u8 = 0;
    let mut last_orientation_change_ms: u32 = now_ms();
    let mut last_wifi_retry_ms: u32 = now_ms();
    let mut last_nvs_save_ms: u32 = now_ms();
    let mut last_mem_log_ms: u32 = now_ms();
    let mut last_snapshot_ms: u32 = 0;
    let mut last_history_ms: u32 = 0;
    let mut last_weather_success_ms: Option<u32> = None;

    // ── Render thread: owns fb + ctx, draws on demand ──
    // Arc<Mutex<Option<AppState>>> replaces sync_channel to avoid the MPMC
    // spin_light starvation bug: sync_channel's start_recv spins in a tight
    // CPU loop (no FreeRTOS yield) when the sender has claimed a slot but
    // not yet completed write(). The Mutex uses a proper FreeRTOS semaphore
    // so contention always blocks rather than spinning, preventing IDLE0
    // starvation and the resulting Task Watchdog trigger.
    let render_slot: Arc<Mutex<Option<views::AppState>>> = Arc::new(Mutex::new(None));
    let render_slot_thread = render_slot.clone();

    render::spawn_render_thread(render_slot_thread, fb, ctx);

    // Initial draw via render channel
    *render_slot.lock().unwrap() = Some(state.clone());
    state.dirty = false;

    let mut prev_view = state.current_view;
    let mut last_touch_ms = now_ms();

    loop {
        let t = now_ms();

        // Poll touch
        let gesture = touch_state.poll(&mut i2c, t, state.orientation);
        if gesture != touch::Gesture::None {
            last_touch_ms = t;
            if state.handle_gesture(gesture) {
                info!("Gesture {:?} -> view {:?}", gesture, state.current_view);
            }
        }

        // Idle timeout: return to Now after 10 min with no touch
        if state.current_view != views::View::Now && t.wrapping_sub(last_touch_ms) >= 600_000 {
            info!("Idle timeout — returning to Now");
            state.current_view = views::View::Now;
            state.dirty = true;
            last_touch_ms = t;
        }

        // Periodic memory stats log (every 30s)
        if t.wrapping_sub(last_mem_log_ms) >= 30_000 {
            last_mem_log_ms = t;
            let sram_free =
                unsafe { esp_idf_sys::heap_caps_get_free_size(esp_idf_sys::MALLOC_CAP_INTERNAL) };
            let sram_largest = unsafe {
                esp_idf_sys::heap_caps_get_largest_free_block(esp_idf_sys::MALLOC_CAP_INTERNAL)
            };
            let psram_free =
                unsafe { esp_idf_sys::heap_caps_get_free_size(esp_idf_sys::MALLOC_CAP_SPIRAM) };
            info!(
                "MEM: SRAM free={} KB largest={} KB  PSRAM free={} KB",
                sram_free / 1024,
                sram_largest / 1024,
                psram_free / 1024
            );
        }

        // Web snapshot update (every 5s)
        if t.wrapping_sub(last_snapshot_ms) >= 5_000 {
            last_snapshot_ms = t;
            let uptime_s = unsafe { esp_idf_sys::esp_timer_get_time() / 1_000_000 } as u32;
            let mut snap = web_snapshot.lock().unwrap();
            if let Some(ref cw) = state.current_weather {
                snap.temp_f = Some(cw.temp_f);
                snap.feels_f = Some(cw.feels_f);
                snap.wind_mph = Some(cw.wind_mph);
                snap.humidity = Some(cw.humidity);
                snap.condition = Some(cw.condition.clone());
                snap.city = Some(cw.city.clone());
            }
            snap.indoor_temp_f = state.indoor_temp;
            snap.indoor_humidity_pct = state.indoor_humidity;
            let press_correction = state.pressure_history.delta_owm_bme_stable().unwrap_or(0.0);
            snap.indoor_pressure_hpa = state.indoor_pressure.map(|p| p + press_correction);
            snap.uptime_s = uptime_s;
            snap.ip_address = state.ip_address.clone();
            let hvac_stats = state.hvac.stats();
            snap.hvac_state = state.hvac.state_u8();
            snap.hvac_heat_mins = hvac_stats.heat.total_minutes;
            snap.hvac_cool_mins = hvac_stats.cool.total_minutes;
            snap.hvac_heat_cycles = hvac_stats.heat.cycles;
            snap.hvac_cool_cycles = hvac_stats.cool.cycles;
            snap.free_heap_kb =
                unsafe { esp_idf_sys::heap_caps_get_free_size(esp_idf_sys::MALLOC_CAP_SPIRAM) }
                    as u32
                    / 1024;
            snap.sram_block_kb = unsafe {
                esp_idf_sys::heap_caps_get_largest_free_block(esp_idf_sys::MALLOC_CAP_INTERNAL)
            } as u32
                / 1024;
            snap.warning_active = state.warning_active;
            snap.alert_count = state.weather_alerts.len() as u32;
            if let Some(a) = state.weather_alerts.first() {
                snap.alert_event = a.event.clone();
                snap.alert_severity = a.severity.clone();
                snap.alert_headline = a.headline.clone();
                snap.alert_expires = crate::weather::format_alert_expiry(&a.expires);
            } else {
                snap.alert_event.clear();
                snap.alert_severity.clear();
                snap.alert_headline.clear();
                snap.alert_expires.clear();
            }
        }

        // History ring push (every 60s, indoor sensor only)
        if t.wrapping_sub(last_history_ms) >= 60_000 {
            if let (Some(temp_f), Some(hum), Some(pres)) = (
                state.indoor_temp,
                state.indoor_humidity,
                state.indoor_pressure,
            ) {
                last_history_ms = t;
                let unix_s = unsafe { libc::time(core::ptr::null_mut()) } as u32;
                let correction = state.pressure_history.delta_owm_bme_stable().unwrap_or(0.0);
                let outdoor_raw = state
                    .current_weather
                    .as_ref()
                    .map(|w| (w.temp_f + 41.0).clamp(1.0, 255.0) as u8)
                    .unwrap_or(0);
                let outdoor_press_raw = state
                    .current_weather
                    .as_ref()
                    .filter(|w| w.pressure_hpa > 0)
                    .map(|w| (w.pressure_hpa as f32 * 10.0).clamp(1.0, 65535.0) as u16)
                    .unwrap_or(0);
                let sample = history_ring::HistorySample {
                    timestamp: unix_s,
                    temp_f,
                    humidity_pct: hum,
                    pressure_hpa: pres + correction,
                    hvac_state: state.hvac.state_u8(),
                    outdoor_temp_u8: outdoor_raw,
                    outdoor_press_u16: outdoor_press_raw,
                    version: history_ring::SAMPLE_VERSION,
                    ..Default::default()
                };
                history.lock().unwrap().push(sample);
            }
        }

        // BME280 read (or retry init if not yet found)
        sensor_poll::poll_bme280(&mut state, &mut bme280, &mut i2c, t, &mut bme_ps);

        // Periodic NVS history save (every 30 min) so data survives unexpected resets.
        // Wait for render flush to complete first: NVS writes disable the flash cache
        // on CPU1 via IPC; if the render thread's DMA ISR is executing from flash at
        // that moment, the IPC pause times out and fires the Interrupt WDT.
        if t.wrapping_sub(last_nvs_save_ms) >= 30 * 60 * 1_000 {
            let mut waited = 0u32;
            while debug_flags::RENDER_FLUSH_ACTIVE.load(Ordering::Acquire) && waited < 500 {
                unsafe { esp_idf_sys::vTaskDelay(1) };
                waited += 1;
            }
            history_nvs::history_nvs_save_all(&state, &history, &nvs);
            last_nvs_save_ms = t;
        }

        // Manual history save requested from console (`history save`).
        if debug_flags::REQUEST_HISTORY_SAVE.swap(false, Ordering::Relaxed) {
            log::info!("console: manual history save requested");
            let mut waited = 0u32;
            while debug_flags::RENDER_FLUSH_ACTIVE.load(Ordering::Acquire) && waited < 500 {
                unsafe { esp_idf_sys::vTaskDelay(1) };
                waited += 1;
            }
            history_nvs::history_nvs_save_all(&state, &history, &nvs);
            last_nvs_save_ms = t; // reset periodic timer too
            log::info!("console: history save complete");
        }

        // Proactive reboot when SRAM largest block ≤ 7 KB: save history first.
        if SRAM_DO_REBOOT.load(Ordering::Relaxed) {
            log::warn!("SRAM_DO_REBOOT: saving history to NVS then rebooting...");
            history_nvs::history_nvs_save_all(&state, &history, &nvs);
            unsafe {
                esp_idf_sys::esp_restart();
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
            let owm = state.current_weather.as_ref().and_then(|cw| {
                if cw.pressure_hpa > 0 {
                    Some(cw.pressure_hpa as f32)
                } else {
                    None
                }
            });
            state.pressure_history.push_long(bme, owm);
            last_pressure_sample_ms = t;
            state.dirty = true;
        }

        // Check for weather data from background thread
        if let Ok(mut wd) = weather_data.try_lock() {
            if let Some((current, forecast)) = wd.take() {
                state.bottom_text.clear();
                state.bottom_text.push_str(&current.city);
                state.bottom_text.push_str(", ");
                state.bottom_text.push_str(&current.country);
                state.bottom_text.push_str(" | ");
                state.bottom_text.push_str(&current.condition);
                let temp_f = current.temp_f;
                state.current_weather = Some(current);
                state.forecast = Some(forecast);
                // Push outdoor temp for trend arrow (keep ~12 readings ≈ 2h of history)
                state.outdoor_temp_history.push_back(temp_f);
                state.status_text.clear();
                state.status_text.push_str(&ip_address);
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
                sensor_poll::process_alert_data(
                    &mut state,
                    alerts,
                    &mut alert_ps,
                    &speaker_ready,
                    &cfg,
                );
            }
        }
        sensor_poll::tick_alert_beeps(&mut state, &mut alert_ps, &speaker_ready);

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
        if imu_ok
            && debug_flags::is_on(&debug_flags::DEBUG_IMU)
            && tick_count.is_multiple_of(TIME_UPDATE_TICKS)
        {
            if let Some(r) = qmi8658::read(&mut i2c) {
                info!(
                    "IMU: ax={:+.2} ay={:+.2} az={:+.2}  gx={:+.1} gy={:+.1} gz={:+.1}",
                    r.accel_x, r.accel_y, r.accel_z, r.gyro_x, r.gyro_y, r.gyro_z,
                );
            }
        }

        // Orientation mode updates requested from console
        if let Some(mode) = debug_flags::take_orientation_mode_request() {
            state.orientation_mode = mode;
            if mode != config::OrientationMode::Auto {
                let target = layout::locked_orientation(mode, state.orientation_flip);
                if state.orientation != target {
                    state.apply_orientation(target);
                    last_orientation_change_ms = now_ms();
                    info!("Orientation: {:?}", state.orientation);
                }
            }
        }

        // Orientation flip updates requested from console
        if let Some(flip) = debug_flags::take_orientation_flip_request() {
            state.orientation_flip = flip;
            if state.orientation_mode != config::OrientationMode::Auto {
                let target =
                    layout::locked_orientation(state.orientation_mode, state.orientation_flip);
                if state.orientation != target {
                    state.apply_orientation(target);
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
            && tick_count.is_multiple_of(layout::ORIENTATION_POLL_TICKS)
            && now_ms().wrapping_sub(last_orientation_change_ms)
                >= layout::ORIENTATION_CHANGE_COOLDOWN_MS
        {
            if let Some(r) = qmi8658::read(&mut i2c) {
                if let Some(next) = layout::detect_orientation_from_imu(&r) {
                    if next != state.orientation {
                        if orientation_candidate == next {
                            orientation_candidate_count =
                                orientation_candidate_count.saturating_add(1);
                        } else {
                            orientation_candidate = next;
                            orientation_candidate_count = 1;
                        }
                        if orientation_candidate_count >= layout::ORIENTATION_CONFIRM_SAMPLES {
                            state.apply_orientation(next);
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

        // Trigger WiFi scan when user navigates to WifiScan view.
        // We only SET the flag here; execution is intentionally deferred to the
        // NEXT tick so the render thread can draw "Scanning..." before the
        // blocking scan freezes the main loop.
        let wifi_scan_triggered_this_tick =
            state.current_view == views::View::WifiScan && prev_view != views::View::WifiScan;
        if wifi_scan_triggered_this_tick {
            state.wifi_networks.clear();
            state.wifi_scan_pending = true;
            debug_flags::REQUEST_WIFI_SCAN.store(true, Ordering::Relaxed);
            state.dirty = true;
        }
        prev_view = state.current_view;

        // WiFi scan execution — skip if the flag was set THIS tick (defer one tick).
        // Console-triggered scans (wifi scan command) bypass the trigger path so
        // wifi_scan_triggered_this_tick is false and they run immediately as before.
        if !wifi_scan_triggered_this_tick
            && debug_flags::REQUEST_WIFI_SCAN.swap(false, Ordering::Relaxed)
        {
            if let Some(wifi) = wifi_handle.as_mut() {
                state.wifi_networks = wifi::scan_wifi(wifi.as_mut(), sysloop.clone());
            }
            state.wifi_scan_pending = false;
            state.dirty = true;
        }

        // Save C/F preference to NVS on toggle.
        // Only clear the flag after a successful save so that a failed
        // try_lock() (NVS held by another thread) is retried next tick.
        if state.save_celsius_pref {
            if let Ok(mut nvs) = nvs.try_lock() {
                let _ = config::Config::save_use_celsius(&mut nvs, state.use_celsius);
                state.save_celsius_pref = false;
            }
        }
        if state.save_orientation_pref {
            if let Ok(mut nvs) = nvs.try_lock() {
                let _ = config::Config::save_orientation_mode(&mut nvs, state.orientation_mode);
                state.save_orientation_pref = false;
                // Apply the new orientation immediately (Settings tap only saves pref;
                // without this the display would not rotate until reboot).
                if state.orientation_mode != config::OrientationMode::Auto {
                    let target =
                        layout::locked_orientation(state.orientation_mode, state.orientation_flip);
                    if state.orientation != target {
                        state.apply_orientation(target);
                        last_orientation_change_ms = now_ms();
                        info!("Orientation locked to {:?}", state.orientation);
                    }
                } else {
                    // Switching back to Auto: expire the cooldown so IMU kicks in
                    // immediately instead of blocking for 10s.
                    last_orientation_change_ms = 0;
                }
            }
        }

        // Check for force weather refresh from tap
        if state.force_weather_refresh {
            state.force_weather_refresh = false;
            weather_refresh_flag.store(true, Ordering::Relaxed);
            state.status_text.clear();
            state.status_text.push_str("Refreshing...");
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
                    Ok(Some(ip)) => {
                        wifi_ok = true;
                        ip_address = ip;
                        state.ip_address.clear();
                        state.ip_address.push_str(&ip_address);
                        state.status_text.clear();
                        state.status_text.push_str(&ip_address);
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

        // WiFi RSSI update (always) + debug logging
        if tick_count.is_multiple_of(WIFI_DEBUG_TICKS) {
            unsafe {
                let mut ap_info: esp_idf_sys::wifi_ap_record_t = core::mem::zeroed();
                if esp_idf_sys::esp_wifi_sta_get_ap_info(&mut ap_info) == esp_idf_sys::ESP_OK {
                    let new_rssi = Some(ap_info.rssi);
                    if state.wifi_rssi != new_rssi {
                        state.wifi_rssi = new_rssi;
                        state.dirty = true;
                    }
                    if debug_flags::is_on(&debug_flags::DEBUG_WIFI) {
                        info!(
                            "WiFi: RSSI={} ch={} SSID={}",
                            ap_info.rssi,
                            ap_info.primary,
                            core::str::from_utf8(&ap_info.ssid)
                                .unwrap_or("?")
                                .trim_end_matches('\0')
                        );
                    }
                } else {
                    if state.wifi_rssi.is_some() {
                        state.wifi_rssi = None;
                        state.dirty = true;
                    }
                    if wifi_ok {
                        wifi_ok = false;
                        // Trigger reconnect after 60s (not 5m) on first detection
                        last_wifi_retry_ms = t.wrapping_sub(WIFI_RETRY_INTERVAL_MS - 60_000);
                        log::warn!("WiFi: lost association — reconnect in 60s");
                    }
                    if debug_flags::is_on(&debug_flags::DEBUG_WIFI) {
                        info!("WiFi: not connected");
                    }
                }
            }
        }

        // Update time display
        if tick_count.is_multiple_of(TIME_UPDATE_TICKS) {
            if let Some(t) = time_sync::format_local_time() {
                if t != state.time_text {
                    state.time_text.clear();
                    state.time_text.push_str(&t);
                    state.dirty = true;
                }
            }
        }

        // Redraw if needed — send snapshot to render thread.
        // Guard: skip clone if SRAM is too fragmented (largest contiguous
        // block < 8 KB).  Cloning AppState allocates Strings/Vecs from SRAM;
        // doing so during an active TLS handshake on the weather thread can
        // exhaust the heap.  The render thread keeps the previous frame and
        // we retry next tick when things settle.
        // Also: if SRAM stays below 12 KB for 5+ consecutive ticks (~100ms)
        // AND we are past 5 minutes of uptime, trigger a proactive reboot.
        // The first NWS TLS handshake (~50s) transiently dips below 12 KB
        // for <100ms — the 5-min guard prevents false-positive reboots from
        // that dip.  Genuine heap fragmentation happens at 15+ min.
        if state.dirty {
            let largest_sram = unsafe {
                esp_idf_sys::heap_caps_get_largest_free_block(esp_idf_sys::MALLOC_CAP_INTERNAL)
            };
            let uptime_us = unsafe { esp_idf_sys::esp_timer_get_time() };
            if largest_sram < 12_000 && uptime_us > 300_000_000 {
                render_sram_low_streak = render_sram_low_streak.saturating_add(1);
                if render_sram_low_streak >= 5 {
                    SRAM_DO_REBOOT.store(true, Ordering::Relaxed);
                }
            } else {
                render_sram_low_streak = 0;
                *render_slot.lock().unwrap() = Some(state.clone());
                state.dirty = false;
            }
        }

        tick_count = tick_count.wrapping_add(1);
        std::thread::sleep(Duration::from_millis(TICK_MS));
    }
}
