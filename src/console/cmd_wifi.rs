use anyhow::Result;
use esp_idf_svc::nvs::{EspNvs, NvsDefault};
use log::{info, warn};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::config::Config;

pub(super) fn handle_wifi(
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
            let (ssid, pass) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
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
            info!(
                "saved: SSID='{}' pass=******** ({} chars)",
                ssid,
                pass.len()
            );
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
                        active: esp_idf_sys::wifi_active_scan_time_t { min: 100, max: 300 },
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
                let mut records =
                    vec![core::mem::zeroed::<esp_idf_sys::wifi_ap_record_t>(); max as usize];
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
        _ => super::print_help_user(),
    }
    Ok(())
}

pub(super) fn handle_api(
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
        _ => super::print_help_user(),
    }
    Ok(())
}

pub(super) fn handle_secrets(
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
