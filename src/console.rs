use anyhow::Result;
use esp_idf_svc::nvs::{EspNvs, NvsDefault};
use log::{info, warn};
use std::io::{self, Read};
use std::sync::{Arc, Mutex};

use crate::config::Config;

pub fn spawn_console(nvs: Arc<Mutex<EspNvs<NvsDefault>>>, config: Arc<Mutex<Config>>) {
    std::thread::Builder::new()
        .name("console".into())
        .stack_size(8192)
        .spawn(move || {
            info!("console: ready (type 'help')");
            let stdin = io::stdin();
            let mut reader = stdin.lock();
            let mut line = String::new();
            let mut buf = [0u8; 1];
            loop {
                match reader.read(&mut buf) {
                    Ok(1) => {
                        let ch = buf[0];
                        if ch == b'\n' || ch == b'\r' {
                            if line.is_empty() {
                                continue;
                            }
                            info!("> {}", line);
                            if let Err(e) = process_line(&line, &nvs, &config) {
                                warn!("console: error: {}", e);
                            }
                            line.clear();
                        } else if ch == 0x7f || ch == 0x08 {
                            // Backspace
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
) -> Result<()> {
    let mut parts = line.splitn(3, char::is_whitespace);
    let cmd = parts.next().unwrap_or("");
    let sub = parts.next().unwrap_or("");
    let rest = parts.next().unwrap_or("").trim();

    match cmd {
        "help" | "?" => print_help(),
        "wifi" => handle_wifi(sub, rest, nvs, config)?,
        "api" => handle_api(sub, rest, nvs, config)?,
        "i2c" => handle_i2c(sub),
        "debug" => handle_debug(sub),
        "status" => {
            let cfg = config.lock().unwrap();
            info!("wifi: {}", if cfg.wifi_ssid.is_empty() { "not configured" } else { &cfg.wifi_ssid });
            info!("api key: {} chars", cfg.weather_api_key.len());
            info!("query: {}", cfg.weather_query);
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
    info!("  api show                   - show API config");
    info!("  api set-key <key>          - set OpenWeather API key");
    info!("  api set-query <query>      - set location query");
    info!("  api clear                  - clear API overrides");
    info!("  debug <module>             - toggle debug for module");
    info!("    modules: touch, bme280, wifi, weather, all");
    info!("  debug show                 - show debug flag status");
    info!("  status                     - show system status");
    info!("  reboot                     - reboot device");
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
        "all" => {
            // If any flag is off, turn all on; if all on, turn all off
            let any_off = !is_on(&DEBUG_TOUCH) || !is_on(&DEBUG_BME280)
                || !is_on(&DEBUG_WIFI) || !is_on(&DEBUG_WEATHER);
            set(&DEBUG_TOUCH, any_off);
            set(&DEBUG_BME280, any_off);
            set(&DEBUG_WIFI, any_off);
            set(&DEBUG_WEATHER, any_off);
            info!("debug all: {}", if any_off { "ON" } else { "OFF" });
        }
        _ => {
            info!("unknown module '{}'. options: touch, bme280, wifi, weather, all", sub);
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
                let max = count.min(20) as u16;
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
            info!("type 'reboot' to apply");
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
            info!("type 'reboot' to apply");
        }
        "clear" => {
            let mut nvs = nvs.lock().unwrap();
            Config::save_weather_api_key(&mut nvs, "")?;
            Config::save_weather_query(&mut nvs, "")?;
            let mut cfg = config.lock().unwrap();
            cfg.weather_api_key.clear();
            cfg.weather_query = "q=New York,US".to_string();
            info!("API overrides cleared (defaults restored)");
        }
        _ => print_help(),
    }
    Ok(())
}
