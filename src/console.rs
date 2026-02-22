use anyhow::Result;
use esp_idf_svc::nvs::{EspNvs, NvsDefault};
use log::{info, warn};
use std::io::{self, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::config::Config;

pub fn spawn_console(
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
                            if let Err(e) = process_line(&line, &nvs, &config, &weather_refresh_flag)
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
        "help" | "?" => print_help(),
        "wifi" => handle_wifi(sub, rest, nvs, config)?,
        "api" => handle_api(sub, rest, nvs, config, weather_refresh_flag)?,
        "units" => handle_units(sub, nvs, config)?,
        "secrets" => handle_secrets(sub, nvs, config)?,
        "alerts" => handle_alerts(sub, rest, nvs, config)?,
        "flash" => handle_flash(sub, rest, nvs, config)?,
        "orientation" => handle_orientation(sub, rest, nvs, config)?,
        "i2c" => handle_i2c(sub),
        "imu" => handle_imu(sub),
        "debug" => handle_debug(sub),
        "version" => {
            info!("v{}", env!("CARGO_PKG_VERSION"));
        }
        "beep" => handle_beep(sub),
        "about" => {
            let cfg = config.lock().unwrap();
            info!("app: waveshare_s3_3p");
            info!("firmware: v{}", env!("CARGO_PKG_VERSION"));
            info!("author: David (DaQue)");
            info!("device: Waveshare ESP32-S3 3.5B");
            info!("units: {}", if cfg.use_celsius { "C" } else { "F" });
            info!("query: {}", cfg.weather_query);
            info!("orientation: {}", cfg.orientation_mode.as_str());
            info!("orientation flip: {}", if cfg.orientation_flip { "on" } else { "off" });
            let uptime_secs = unsafe { esp_idf_sys::esp_timer_get_time() } / 1_000_000;
            let hours = uptime_secs / 3600;
            let mins = (uptime_secs % 3600) / 60;
            info!("uptime: {}h {}m", hours, mins);
            let heap_kb = unsafe { esp_idf_sys::esp_get_free_heap_size() } / 1024;
            info!("free heap: {} KB", heap_kb);
            info!("hint: type 'help' for all commands");
        }
        "status" => {
            let cfg = config.lock().unwrap();
            info!("wifi: {}", if cfg.wifi_ssid.is_empty() { "not configured" } else { &cfg.wifi_ssid });
            info!("api key: {} chars", cfg.weather_api_key.len());
            info!("query: {}", cfg.weather_query);
            info!("flash time: {}", cfg.flash_time);
            info!("alerts enabled: {}", cfg.alerts_enabled);
            info!("alerts beep: {}", cfg.alerts_beep);
            info!("alerts auto-scope: {}", cfg.alerts_auto_scope);
            info!("alerts scope: {}", cfg.nws_scope);
            info!("alerts zone: {}", if cfg.nws_zone.is_empty() { "<unset>" } else { &cfg.nws_zone });
            info!("alerts ua: {}", cfg.nws_user_agent);
            info!("orientation: {}", cfg.orientation_mode.as_str());
            info!("orientation flip: {}", if cfg.orientation_flip { "on" } else { "off" });
            let heap_kb = unsafe { esp_idf_sys::esp_get_free_heap_size() } / 1024;
            info!("free heap: {} KB", heap_kb);
            info!("debug: {}", crate::debug_flags::status_line());
        }
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

fn print_help() {
    info!("commands:");
    info!("  wifi show                  - show Wi-Fi config");
    info!("  wifi set <ssid> <pass>     - set Wi-Fi credentials");
    info!("  wifi scan                  - scan for nearby networks");
    info!("  wifi clear                 - clear Wi-Fi override");
    info!("  i2c scan                   - rescan I2C bus");
    info!("  imu read                   - one-shot IMU reading");
    info!("  api show                   - show API config");
    info!("  api set-key <key>          - set OpenWeather API key");
    info!("  api set-query <query>      - set location query");
    info!("  api clear                  - clear API overrides");
    info!("  units f|c|show             - set/show temperature units");
    info!("  secrets show               - show local fallback availability");
    info!("  secrets seed-local         - save wifi.local.rs values into NVS");
    info!("  alerts show                - show alert settings");
    info!("  alerts on|off              - enable/disable NWS alerts");
    info!("  alerts beep on|off|show    - enable/disable alert beeps");
    info!("  alerts auto-scope on|off   - auto-discover NWS zone from Wi-Fi");
    info!("  alerts ua <user-agent>     - set NWS User-Agent");
    info!("  alerts scope <scope>       - set NWS scope (example: area=MO)");
    info!("  alerts zone show|clear     - show/clear cached zone");
    info!("  flash show                 - show flash metadata");
    info!("  flash set-time <text>      - set flash time metadata");
    info!("  orientation auto|landscape|portrait");
    info!("  orientation flip on|off|toggle|show");
    info!("  debug <module>             - toggle debug for module");
    info!("    modules: touch, bme280, wifi, weather, imu, all");
    info!("  debug show                 - show debug flag status");
    info!("  beep advisory|watch|warning|stop - play/stop speaker test tone");
    info!("  version                    - print firmware version");
    info!("  about                      - show firmware/device summary");
    info!("  status                     - show system status");
    info!("  reboot                     - reboot device");
}

fn handle_beep(sub: &str) {
    match sub {
        "advisory" => crate::debug_flags::request_beep_tone(0),
        "watch" => crate::debug_flags::request_beep_tone(1),
        "warning" => crate::debug_flags::request_beep_tone(2),
        "stop" => {
            crate::debug_flags::request_beep_stop();
            info!("beep stop requested");
            return;
        }
        "" | "show" => {
            info!("usage: beep advisory|watch|warning|stop");
            return;
        }
        _ => {
            info!("usage: beep advisory|watch|warning|stop");
            return;
        }
    }
    info!("beep requested: {}", sub);
}

fn handle_debug(sub: &str) {
    use crate::debug_flags::*;
    match sub {
        "show" | "" => {
            info!("debug: {}", status_line());
        }
        "touch" => {
            let on = toggle(&DEBUG_TOUCH);
            info!("debug touch: {}", if on { "ON" } else { "OFF" });
        }
        "bme280" | "bme" | "sensor" => {
            let on = toggle(&DEBUG_BME280);
            info!("debug bme280: {}", if on { "ON" } else { "OFF" });
        }
        "wifi" => {
            let on = toggle(&DEBUG_WIFI);
            info!("debug wifi: {}", if on { "ON" } else { "OFF" });
        }
        "weather" | "api" => {
            let on = toggle(&DEBUG_WEATHER);
            info!("debug weather: {}", if on { "ON" } else { "OFF" });
        }
        "imu" => {
            let on = toggle(&DEBUG_IMU);
            info!("debug imu: {}", if on { "ON" } else { "OFF" });
        }
        "all" => {
            // If any flag is off, turn all on; if all on, turn all off
            let any_off = !is_on(&DEBUG_TOUCH) || !is_on(&DEBUG_BME280)
                || !is_on(&DEBUG_WIFI) || !is_on(&DEBUG_WEATHER)
                || !is_on(&DEBUG_IMU);
            set(&DEBUG_TOUCH, any_off);
            set(&DEBUG_BME280, any_off);
            set(&DEBUG_WIFI, any_off);
            set(&DEBUG_WEATHER, any_off);
            set(&DEBUG_IMU, any_off);
            info!("debug all: {}", if any_off { "ON" } else { "OFF" });
        }
        _ => {
            info!("unknown module '{}'. options: touch, bme280, wifi, weather, imu, all", sub);
        }
    }
}

fn handle_i2c(sub: &str) {
    match sub {
        "scan" | "" => {
            info!("i2c: scan requested (will run on next tick)");
            crate::debug_flags::REQUEST_I2C_SCAN.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        _ => info!("usage: i2c scan"),
    }
}

fn handle_orientation(
    sub: &str,
    rest: &str,
    nvs: &Arc<Mutex<EspNvs<NvsDefault>>>,
    config: &Arc<Mutex<Config>>,
) -> Result<()> {
    if sub.is_empty() || sub == "show" {
        let cfg = config.lock().unwrap();
        info!("orientation: {}", cfg.orientation_mode.as_str());
        info!("orientation flip: {}", if cfg.orientation_flip { "on" } else { "off" });
        return Ok(());
    }

    if sub == "flip" {
        let mut cfg = config.lock().unwrap();
        if cfg.orientation_mode == crate::config::OrientationMode::Auto {
            info!("orientation flip is unavailable in auto mode");
            info!("pick landscape or portrait first");
            return Ok(());
        }
        let flip = match rest {
            "" | "toggle" => !cfg.orientation_flip,
            "show" => {
                info!("orientation flip: {}", if cfg.orientation_flip { "on" } else { "off" });
                return Ok(());
            }
            "on" | "1" | "true" => true,
            "off" | "0" | "false" => false,
            _ => {
                info!("usage: orientation flip on|off|toggle|show");
                return Ok(());
            }
        };
        {
            let mut nvs = nvs.lock().unwrap();
            Config::save_orientation_flip(&mut nvs, flip)?;
        }
        cfg.orientation_flip = flip;
        crate::debug_flags::request_orientation_flip(flip);
        info!("orientation flip: {}", if flip { "on" } else { "off" });
        return Ok(());
    }

    let Some(mode) = crate::config::OrientationMode::parse(sub) else {
        info!("usage: orientation auto|landscape|portrait");
        return Ok(());
    };

    {
        let mut nvs = nvs.lock().unwrap();
        Config::save_orientation_mode(&mut nvs, mode)?;
    }
    config.lock().unwrap().orientation_mode = mode;
    crate::debug_flags::request_orientation_mode(mode);
    info!("orientation set to {}", mode.as_str());
    Ok(())
}

fn handle_units(
    sub: &str,
    nvs: &Arc<Mutex<EspNvs<NvsDefault>>>,
    config: &Arc<Mutex<Config>>,
) -> Result<()> {
    match sub {
        "" | "show" => {
            let cfg = config.lock().unwrap();
            info!("units: {}", if cfg.use_celsius { "C" } else { "F" });
        }
        "f" | "fahrenheit" => {
            let mut nvs = nvs.lock().unwrap();
            Config::save_use_celsius(&mut nvs, false)?;
            config.lock().unwrap().use_celsius = false;
            info!("units set to F");
        }
        "c" | "celsius" => {
            let mut nvs = nvs.lock().unwrap();
            Config::save_use_celsius(&mut nvs, true)?;
            config.lock().unwrap().use_celsius = true;
            info!("units set to C");
        }
        _ => info!("usage: units f|c|show"),
    }
    Ok(())
}

fn handle_imu(sub: &str) {
    match sub {
        "read" | "" => {
            info!("imu: read requested (will run on next tick)");
            crate::debug_flags::REQUEST_IMU_READ.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        _ => info!("usage: imu read"),
    }
}

fn handle_wifi(
    sub: &str,
    rest: &str,
    nvs: &Arc<Mutex<EspNvs<NvsDefault>>>,
    config: &Arc<Mutex<Config>>,
) -> Result<()> {
    match sub {
        "show" => {
            let cfg = config.lock().unwrap();
            info!("wifi ssid: {}", cfg.wifi_ssid);
            let pass_len = cfg.wifi_pass.len();
            info!(
                "wifi pass: {} ({} chars)",
                if pass_len == 0 { "<empty>" } else { "********" },
                pass_len
            );
        }
        "set" => {
            let (ssid, pass) = rest
                .split_once(char::is_whitespace)
                .unwrap_or((rest, ""));
            let ssid = ssid.trim_matches('"').trim_matches('\'');
            let pass = pass.trim().trim_matches('"').trim_matches('\'');
            if ssid.is_empty() {
                warn!("usage: wifi set <ssid> <password>");
                return Ok(());
            }
            let mut nvs = nvs.lock().unwrap();
            Config::save_wifi(&mut nvs, ssid, pass)?;
            config.lock().unwrap().wifi_ssid = ssid.to_string();
            config.lock().unwrap().wifi_pass = pass.to_string();
            info!("saved: SSID='{}' pass=******** ({} chars)", ssid, pass.len());
            info!("type 'reboot' to apply");
        }
        "scan" => {
            info!("wifi: scanning...");
            unsafe {
                let scan_cfg = esp_idf_sys::wifi_scan_config_t {
                    ssid: core::ptr::null_mut(),
                    bssid: core::ptr::null_mut(),
                    channel: 0,
                    show_hidden: false,
                    scan_type: esp_idf_sys::wifi_scan_type_t_WIFI_SCAN_TYPE_ACTIVE,
                    scan_time: esp_idf_sys::wifi_scan_time_t {
                        active: esp_idf_sys::wifi_active_scan_time_t {
                            min: 100,
                            max: 300,
                        },
                        passive: 0,
                    },
                    home_chan_dwell_time: 0,
                };
                let rc = esp_idf_sys::esp_wifi_scan_start(&scan_cfg, true);
                if rc != esp_idf_sys::ESP_OK {
                    warn!("wifi scan start failed (err {})", rc);
                    return Ok(());
                }
                let mut count: u16 = 0;
                esp_idf_sys::esp_wifi_scan_get_ap_num(&mut count);
                if count == 0 {
                    info!("wifi: no networks found");
                    return Ok(());
                }
                let max = count.min(20);
                let mut records = vec![core::mem::zeroed::<esp_idf_sys::wifi_ap_record_t>(); max as usize];
                let mut actual = max;
                esp_idf_sys::esp_wifi_scan_get_ap_records(&mut actual, records.as_mut_ptr());
                info!("wifi: found {} networks", actual);
                for ap in &records[..actual as usize] {
                    let ssid = core::str::from_utf8(&ap.ssid)
                        .unwrap_or("?")
                        .trim_end_matches('\0');
                    info!("  {:>4} dBm  ch{:<2}  {}", ap.rssi, ap.primary, ssid);
                }
            }
        }
        "clear" => {
            let mut nvs = nvs.lock().unwrap();
            Config::save_wifi(&mut nvs, "", "")?;
            let mut cfg = config.lock().unwrap();
            cfg.wifi_ssid.clear();
            cfg.wifi_pass.clear();
            info!("Wi-Fi override cleared");
        }
        _ => print_help(),
    }
    Ok(())
}

fn handle_api(
    sub: &str,
    rest: &str,
    nvs: &Arc<Mutex<EspNvs<NvsDefault>>>,
    config: &Arc<Mutex<Config>>,
    weather_refresh_flag: &Arc<AtomicBool>,
) -> Result<()> {
    match sub {
        "show" => {
            let cfg = config.lock().unwrap();
            let key = &cfg.weather_api_key;
            let key_display = if key.len() <= 4 {
                key.clone()
            } else {
                format!("****{}", &key[key.len() - 4..])
            };
            info!("api key: {} ({} chars)", key_display, key.len());
            info!("api query: {}", cfg.weather_query);
        }
        "set-key" => {
            let key = rest.trim().trim_matches('"').trim_matches('\'');
            if key.is_empty() {
                warn!("usage: api set-key <openweather_api_key>");
                return Ok(());
            }
            let mut nvs = nvs.lock().unwrap();
            Config::save_weather_api_key(&mut nvs, key)?;
            config.lock().unwrap().weather_api_key = key.to_string();
            let display = if key.len() <= 4 {
                key.to_string()
            } else {
                format!("****{}", &key[key.len() - 4..])
            };
            info!("saved: api key='{}' ({} chars)", display, key.len());
            weather_refresh_flag.store(true, Ordering::Relaxed);
            info!("weather refresh requested");
        }
        "set-query" => {
            let query = rest.trim().trim_matches('"').trim_matches('\'');
            if query.is_empty() {
                warn!("usage: api set-query <query_string>");
                return Ok(());
            }
            let mut nvs = nvs.lock().unwrap();
            Config::save_weather_query(&mut nvs, query)?;
            config.lock().unwrap().weather_query = query.to_string();
            info!("saved: api query='{}'", query);
            weather_refresh_flag.store(true, Ordering::Relaxed);
            info!("weather refresh requested");
        }
        "clear" => {
            let mut nvs = nvs.lock().unwrap();
            Config::save_weather_api_key(&mut nvs, "")?;
            Config::save_weather_query(&mut nvs, "")?;
            let mut cfg = config.lock().unwrap();
            cfg.weather_api_key.clear();
            cfg.weather_query = "q=New York,US".to_string();
            info!("API overrides cleared (defaults restored)");
            weather_refresh_flag.store(true, Ordering::Relaxed);
            info!("weather refresh requested");
        }
        _ => print_help(),
    }
    Ok(())
}

fn handle_secrets(
    sub: &str,
    nvs: &Arc<Mutex<EspNvs<NvsDefault>>>,
    config: &Arc<Mutex<Config>>,
) -> Result<()> {
    let (local_ssid, local_pass, local_api_key) = crate::config::local_secret_fallbacks();
    match sub {
        "" | "show" => {
            info!(
                "local wifi ssid: {}",
                local_ssid.as_deref().unwrap_or("<unset>")
            );
            info!(
                "local wifi pass: {}",
                local_pass
                    .as_ref()
                    .map(|v| format!("<{} chars>", v.len()))
                    .unwrap_or_else(|| "<unset>".to_string())
            );
            info!(
                "local api key: {}",
                local_api_key
                    .as_ref()
                    .map(|v| format!("<{} chars>", v.len()))
                    .unwrap_or_else(|| "<unset>".to_string())
            );
        }
        "seed-local" => {
            let Some(ssid) = local_ssid else {
                info!("secrets: LOCAL_WIFI_SSID is unset");
                return Ok(());
            };
            let Some(pass) = local_pass else {
                info!("secrets: LOCAL_WIFI_PASS is unset");
                return Ok(());
            };
            let Some(api_key) = local_api_key else {
                info!("secrets: LOCAL_OPENWEATHER_API_KEY is unset");
                return Ok(());
            };

            {
                let mut n = nvs.lock().unwrap();
                Config::save_wifi(&mut n, &ssid, &pass)?;
                Config::save_weather_api_key(&mut n, &api_key)?;
            }
            let mut cfg = config.lock().unwrap();
            cfg.wifi_ssid = ssid;
            cfg.wifi_pass = pass;
            cfg.weather_api_key = api_key;
            info!("secrets: local values saved to NVS");
        }
        _ => info!("usage: secrets show|seed-local"),
    }
    Ok(())
}

fn handle_alerts(
    sub: &str,
    rest: &str,
    nvs: &Arc<Mutex<EspNvs<NvsDefault>>>,
    config: &Arc<Mutex<Config>>,
) -> Result<()> {
    match sub {
        "" | "show" => {
            let cfg = config.lock().unwrap();
            info!("alerts enabled: {}", cfg.alerts_enabled);
            info!("alerts beep: {}", cfg.alerts_beep);
            info!("alerts auto-scope: {}", cfg.alerts_auto_scope);
            info!("alerts scope: {}", cfg.nws_scope);
            info!("alerts zone: {}", if cfg.nws_zone.is_empty() { "<unset>" } else { &cfg.nws_zone });
            info!("alerts ua: {}", cfg.nws_user_agent);
        }
        "on" | "enable" | "enabled" => {
            let mut nvs = nvs.lock().unwrap();
            Config::save_alerts_enabled(&mut nvs, true)?;
            config.lock().unwrap().alerts_enabled = true;
            info!("alerts enabled");
        }
        "off" | "disable" | "disabled" => {
            let mut nvs = nvs.lock().unwrap();
            Config::save_alerts_enabled(&mut nvs, false)?;
            config.lock().unwrap().alerts_enabled = false;
            info!("alerts disabled");
        }
        "ua" => {
            let ua = rest.trim().trim_matches('"').trim_matches('\'');
            if ua.is_empty() {
                info!("usage: alerts ua <user-agent>");
                return Ok(());
            }
            let mut nvs = nvs.lock().unwrap();
            Config::save_nws_user_agent(&mut nvs, ua)?;
            config.lock().unwrap().nws_user_agent = ua.to_string();
            info!("alerts ua saved");
        }
        "scope" => {
            let scope = rest.trim().trim_matches('"').trim_matches('\'');
            if scope.is_empty() {
                info!("usage: alerts scope <scope>");
                return Ok(());
            }
            let mut nvs = nvs.lock().unwrap();
            Config::save_nws_scope(&mut nvs, scope)?;
            config.lock().unwrap().nws_scope = scope.to_string();
            info!("alerts scope saved: {}", scope);
        }
        "auto-scope" => {
            let val = rest.trim().to_ascii_lowercase();
            let enabled = match val.as_str() {
                "on" | "1" | "true" | "enable" | "enabled" => true,
                "off" | "0" | "false" | "disable" | "disabled" => false,
                "" | "show" => {
                    let cfg = config.lock().unwrap();
                    info!("alerts auto-scope: {}", cfg.alerts_auto_scope);
                    return Ok(());
                }
                _ => {
                    info!("usage: alerts auto-scope on|off");
                    return Ok(());
                }
            };
            let mut nvs = nvs.lock().unwrap();
            Config::save_alerts_auto_scope(&mut nvs, enabled)?;
            config.lock().unwrap().alerts_auto_scope = enabled;
            info!("alerts auto-scope: {}", enabled);
        }
        "beep" => {
            let val = rest.trim().to_ascii_lowercase();
            let enabled = match val.as_str() {
                "on" | "1" | "true" | "enable" | "enabled" => true,
                "off" | "0" | "false" | "disable" | "disabled" => false,
                "" | "show" => {
                    let cfg = config.lock().unwrap();
                    info!("alerts beep: {}", cfg.alerts_beep);
                    return Ok(());
                }
                _ => {
                    info!("usage: alerts beep on|off|show");
                    return Ok(());
                }
            };
            let mut nvs = nvs.lock().unwrap();
            Config::save_alerts_beep(&mut nvs, enabled)?;
            config.lock().unwrap().alerts_beep = enabled;
            info!("alerts beep: {}", enabled);
        }
        "zone" => {
            let op = rest.trim().to_ascii_lowercase();
            match op.as_str() {
                "" | "show" => {
                    let cfg = config.lock().unwrap();
                    info!("alerts zone: {}", if cfg.nws_zone.is_empty() { "<unset>" } else { &cfg.nws_zone });
                }
                "clear" => {
                    let mut nvs = nvs.lock().unwrap();
                    Config::save_nws_zone(&mut nvs, "")?;
                    config.lock().unwrap().nws_zone.clear();
                    info!("alerts zone cleared");
                }
                _ => info!("usage: alerts zone show|clear"),
            }
        }
        _ => info!("usage: alerts show|on|off|beep on|off|show|auto-scope on|off|ua <user-agent>|scope <scope>|zone show|clear"),
    }
    Ok(())
}

fn handle_flash(
    sub: &str,
    rest: &str,
    nvs: &Arc<Mutex<EspNvs<NvsDefault>>>,
    config: &Arc<Mutex<Config>>,
) -> Result<()> {
    match sub {
        "" | "show" => {
            let cfg = config.lock().unwrap();
            info!("flash time: {}", cfg.flash_time);
        }
        "set-time" => {
            let flash_time = rest.trim().trim_matches('"').trim_matches('\'');
            if flash_time.is_empty() {
                info!("usage: flash set-time <text>");
                return Ok(());
            }
            let mut nvs = nvs.lock().unwrap();
            Config::save_flash_time(&mut nvs, flash_time)?;
            config.lock().unwrap().flash_time = flash_time.to_string();
            info!("flash time saved: {}", flash_time);
        }
        _ => info!("usage: flash show|set-time <text>"),
    }
    Ok(())
}
