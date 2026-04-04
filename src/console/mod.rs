mod cmd_alerts;
mod cmd_device;
mod cmd_wifi;

use anyhow::Result;
use esp_idf_svc::nvs::{EspNvs, NvsDefault};
use log::{info, warn};
use std::io::{self, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::config::Config;

static DEV_HELP: AtomicBool = AtomicBool::new(false);

pub(crate) fn spawn_console(
    nvs: Arc<Mutex<EspNvs<NvsDefault>>>,
    config: Arc<Mutex<Config>>,
    weather_refresh_flag: Arc<AtomicBool>,
) {
    std::thread::Builder::new()
        .name("console".into())
        .stack_size(16384)
        .spawn(move || {
            info!("console: ready (type 'help') — use minicom Ctrl+A E for local echo");
            let stdin = io::stdin();
            let mut reader = stdin.lock();
            let mut line = String::new();
            let mut buf = [0u8; 1];
            let mut in_escape = false;
            loop {
                match reader.read(&mut buf) {
                    Ok(1) => {
                        let ch = buf[0];
                        if in_escape {
                            if (ch as char).is_ascii_alphabetic() || ch == b'~' {
                                in_escape = false;
                            }
                            continue;
                        }
                        if ch == 0x1b {
                            in_escape = true;
                            continue;
                        }
                        if ch == b'\n' || ch == b'\r' {
                            if line.is_empty() {
                                continue;
                            }
                            info!("> {}", line);
                            if let Err(e) =
                                process_line(&line, &nvs, &config, &weather_refresh_flag)
                            {
                                warn!("console: error: {}", e);
                            }
                            line.clear();
                        } else if ch == 0x7f || ch == 0x08 {
                            line.pop();
                        } else if ch >= 0x20 {
                            line.push(ch as char);
                        }
                    }
                    Ok(_) => {
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                    Err(_) => {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                }
            }
        })
        .expect("failed to spawn console thread");
}

fn process_line(
    line: &str,
    nvs: &Arc<Mutex<EspNvs<NvsDefault>>>,
    config: &Arc<Mutex<Config>>,
    weather_refresh_flag: &Arc<AtomicBool>,
) -> Result<()> {
    let clean = line.trim().trim_end_matches('\\');
    if clean.is_empty() {
        return Ok(());
    }
    let mut parts = clean.splitn(3, char::is_whitespace);
    let cmd = parts.next().unwrap_or("");
    let sub = parts.next().unwrap_or("");
    let rest = parts.next().unwrap_or("").trim();

    match cmd {
        "help" | "?" => {
            let dev = sub == "developer" || sub == "dev";
            if dev {
                match rest {
                    "on" => {
                        DEV_HELP.store(true, Ordering::Relaxed);
                        info!("developer help: ON  (type 'help' to see all commands)");
                        return Ok(());
                    }
                    "off" => {
                        DEV_HELP.store(false, Ordering::Relaxed);
                        info!("developer help: OFF");
                        return Ok(());
                    }
                    _ => {}
                }
            }
            if DEV_HELP.load(Ordering::Relaxed) || dev {
                print_help_full();
            } else {
                print_help_user();
            }
        }
        "wifi" => cmd_wifi::handle_wifi(sub, rest, nvs, config)?,
        "api" => cmd_wifi::handle_api(sub, rest, nvs, config, weather_refresh_flag)?,
        "units" => cmd_device::handle_units(sub, nvs, config)?,
        "secrets" => cmd_wifi::handle_secrets(sub, nvs, config)?,
        "alerts" => cmd_alerts::handle_alerts(sub, rest, nvs, config)?,
        "flash" => cmd_device::handle_flash(sub, rest, nvs, config)?,
        "orientation" => cmd_device::handle_orientation(sub, rest, nvs, config)?,
        "i2c" => cmd_device::handle_i2c(sub),
        "imu" => cmd_device::handle_imu(sub),
        "debug" => cmd_device::handle_debug(sub),
        "beep" => cmd_device::handle_beep(sub),
        "version" => {
            info!("v{}", env!("CARGO_PKG_VERSION"));
        }
        "about" => {
            let cfg = config.lock().unwrap();
            info!("app: waveshare_esp32-s3-touch-lcd-3p5b");
            info!("firmware: v{}", env!("CARGO_PKG_VERSION"));
            info!("author: David (DaQue)");
            info!("device: Waveshare ESP32-S3 3.5B");
            info!("units: {}", if cfg.use_celsius { "C" } else { "F" });
            info!("query: {}", cfg.weather_query);
            info!("orientation: {}", cfg.orientation_mode.as_str());
            info!(
                "orientation flip: {}",
                if cfg.orientation_flip { "on" } else { "off" }
            );
            let uptime_secs = unsafe { esp_idf_sys::esp_timer_get_time() } / 1_000_000;
            let hours = uptime_secs / 3600;
            let mins = (uptime_secs % 3600) / 60;
            info!("uptime: {}h {}m", hours, mins);
            let heap_kb = unsafe { esp_idf_sys::esp_get_free_heap_size() } / 1024;
            info!("free heap: {} KB", heap_kb);
            let sram_kb = unsafe {
                esp_idf_sys::heap_caps_get_largest_free_block(esp_idf_sys::MALLOC_CAP_INTERNAL)
            } / 1024;
            info!("SRAM block: {} KB", sram_kb);
            info!("hint: type 'help' for all commands");
        }
        "status" => {
            let cfg = config.lock().unwrap();
            info!(
                "wifi: {}",
                if cfg.wifi_ssid.is_empty() {
                    "not configured"
                } else {
                    &cfg.wifi_ssid
                }
            );
            info!("api key: {} chars", cfg.weather_api_key.len());
            info!("query: {}", cfg.weather_query);
            info!("flash time: {}", cfg.flash_time);
            info!("alerts enabled: {}", cfg.alerts_enabled);
            info!("alerts beep: {}", cfg.alerts_beep);
            info!("alerts auto-scope: {}", cfg.alerts_auto_scope);
            info!("alerts scope: {}", cfg.nws_scope);
            info!(
                "alerts zone: {}",
                if cfg.nws_zone.is_empty() {
                    "<unset>"
                } else {
                    &cfg.nws_zone
                }
            );
            info!("alerts ua: {}", cfg.nws_user_agent);
            info!("orientation: {}", cfg.orientation_mode.as_str());
            info!(
                "orientation flip: {}",
                if cfg.orientation_flip { "on" } else { "off" }
            );
            let heap_kb = unsafe { esp_idf_sys::esp_get_free_heap_size() } / 1024;
            info!("free heap: {} KB", heap_kb);
            info!("debug: {}", crate::debug_flags::status_line());
        }
        "history" => {
            if sub == "save" || sub.is_empty() {
                crate::debug_flags::REQUEST_HISTORY_SAVE.store(true, Ordering::Relaxed);
                info!("history save requested (will run on next tick)");
            } else {
                info!("usage: history save");
            }
        }
        "log" => match sub {
            "quiet" | "q" => {
                unsafe {
                    esp_idf_sys::esp_log_level_set(
                        c"*".as_ptr(),
                        esp_idf_sys::esp_log_level_t_ESP_LOG_WARN,
                    );
                }
                warn!("Log level -> WARN (quiet). Use 'log verbose' to restore.");
            }
            "verbose" | "v" | "info" => {
                unsafe {
                    esp_idf_sys::esp_log_level_set(
                        c"*".as_ptr(),
                        esp_idf_sys::esp_log_level_t_ESP_LOG_INFO,
                    );
                }
                info!("Log level -> INFO (verbose).");
            }
            _ => info!("usage: log quiet | log verbose"),
        },
        "reboot" => {
            info!("console: rebooting now");
            std::thread::sleep(std::time::Duration::from_millis(100));
            unsafe { esp_idf_sys::esp_restart() };
        }
        _ => {
            warn!("console: unknown command '{}' (type 'help')", cmd);
        }
    }
    Ok(())
}

pub(super) fn print_help_user() {
    info!("commands:");
    info!("  [Setup]");
    info!("  wifi set <ssid> <pass>     - set Wi-Fi credentials");
    info!("  wifi show                  - show Wi-Fi config");
    info!("  wifi scan                  - scan for nearby networks");
    info!("  api set-key <key>          - set OpenWeather API key");
    info!("  api set-query <query>      - set location query");
    info!("  api show                   - show API config");
    info!("  units f|c|show             - set/show temperature units");
    info!("  [Display]");
    info!("  orientation auto|landscape|portrait");
    info!("  orientation flip on|off|toggle|show");
    info!("  [Alerts]");
    info!("  alerts show                - show alert settings");
    info!("  alerts on|off              - enable/disable NWS alerts");
    info!("  alerts beep on|off|show    - enable/disable alert beeps");
    info!("  alerts silence             - stop beeping now");
    info!("  alerts auto-scope on|off   - auto-discover NWS zone from Wi-Fi");
    info!("  alerts test warning        - inject fake warning for testing");
    info!("  [System]");
    info!("  status                     - show system status");
    info!("  about                      - show firmware/device summary");
    info!("  reboot                     - reboot device");
    info!("  history save               - save sensor history to NVS now");
    info!("  help                       - show this help");
    info!("  (type 'help developer on' to show developer commands)");
}

fn print_help_full() {
    print_help_user();
    info!("  [Developer / Diagnostics]");
    info!("  version                    - print firmware version");
    info!("  log quiet | log verbose    - set log level (quiet=WARN, verbose=INFO)");
    info!("  wifi clear                 - clear Wi-Fi override");
    info!("  api clear                  - clear API overrides");
    info!("  secrets show               - show local fallback availability");
    info!("  secrets seed-local         - save wifi.local.rs values into NVS");
    info!("  alerts ua <user-agent>     - set NWS User-Agent (author contact)");
    info!("  alerts scope <scope>       - set NWS scope (example: area=MO)");
    info!("  alerts zone show|clear     - show/clear cached NWS zone");
    info!("  imu read                   - one-shot IMU reading");
    info!("  i2c scan                   - rescan I2C bus");
    info!("  debug <module>|on|off|show - toggle per-module debug logging");
    info!("    modules: touch, bme280, wifi, weather, imu, all");
    info!("  beep advisory|watch|warning|stop - speaker test tone");
    info!("  flash show                 - show flash metadata");
    info!("  flash set-time <text>      - set flash time metadata");
    info!("  help developer off         - hide developer commands again");
}
