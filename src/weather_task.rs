/// Background threads for weather and NWS alert fetching.
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use esp_idf_svc::nvs::{EspNvs, NvsDefault};
use log::info;

use crate::{config, debug_flags, weather};

pub(crate) fn spawn_weather_thread(
    weather_data: Arc<Mutex<Option<(weather::CurrentWeather, weather::Forecast)>>>,
    weather_refresh_flag: Arc<AtomicBool>,
    cfg: Arc<Mutex<config::Config>>,
    http_lock: Arc<Mutex<()>>,
) {
    std::thread::Builder::new()
        .name("weather".into())
        .stack_size(16384) // inner-scope fix in https_get_json drops HTTP vars before parse callback
        .spawn(move || {
            let mut first = true;
            let mut warned_missing_key = false;
            let mut consecutive_failures: u32 = 0;
            // Brief pause before first fetch — SNTP may still be active on
            // the lwIP thread immediately after sync completes, and starting
            // a DNS lookup at the same moment causes a spinlock deadlock.
            std::thread::sleep(Duration::from_secs(2));
            loop {
                let (query, key) = {
                    let c = cfg.lock().unwrap();
                    (c.weather_query.clone(), c.weather_api_key.clone())
                };
                if key.is_empty() {
                    if !warned_missing_key {
                        log::warn!("No weather API key (use console: api set-key <key>)");
                        warned_missing_key = true;
                    }
                    std::thread::sleep(Duration::from_secs(crate::WEATHER_RETRY_SECS));
                    continue;
                }
                warned_missing_key = false;
                let verbose = first || debug_flags::is_on(&debug_flags::DEBUG_WEATHER);
                if verbose { info!("Weather fetch starting..."); }
                let fetch_result = {
                    let _http = http_lock.lock().unwrap();
                    weather::fetch_weather(&query, &key)
                };
                match fetch_result {
                    Ok((current, forecast)) => {
                        info!(
                            "Weather: {}°F {} in {} ({} forecast days)",
                            current.temp_f as i32, current.condition, current.city, forecast.rows.len()
                        );
                        first = false;
                        consecutive_failures = 0;
                        *weather_data.lock().unwrap() = Some((current, forecast));
                    }
                    Err(e) => {
                        consecutive_failures = consecutive_failures.saturating_add(1);
                        if consecutive_failures == 1
                            || consecutive_failures.is_multiple_of(crate::FAILURE_WARN_EVERY)
                        {
                            log::warn!("Weather fetch failed ({} consecutive): {}", consecutive_failures, e);
                        } else {
                            info!("Weather fetch failed ({} consecutive)", consecutive_failures);
                        }
                        // After 60 consecutive failures (~30min), reboot to recover
                        // from mbedTLS heap fragmentation that cannot self-heal.
                        if consecutive_failures >= 60 {
                            log::warn!(
                                "Weather: {} consecutive failures — rebooting to recover heap",
                                consecutive_failures
                            );
                            std::thread::sleep(Duration::from_secs(3));
                            unsafe { esp_idf_sys::esp_restart(); }
                        }
                        let retry_secs = if consecutive_failures >= 30 { 120u64 }
                            else if consecutive_failures >= 10 { 60 }
                            else { crate::WEATHER_RETRY_SECS };
                        for _ in 0..retry_secs {
                            if weather_refresh_flag.swap(false, Ordering::Relaxed) {
                                info!("Weather refresh requested");
                                break;
                            }
                            std::thread::sleep(Duration::from_secs(1));
                        }
                        continue;
                    }
                }
                // Sleep but wake early on refresh request
                for _ in 0..crate::WEATHER_INTERVAL_SECS {
                    if weather_refresh_flag.swap(false, Ordering::Relaxed) {
                        info!("Weather refresh requested");
                        break;
                    }
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
        })
        .expect("failed to spawn weather thread");
}

pub(crate) fn spawn_alerts_thread(
    alert_data: Arc<Mutex<Option<Vec<weather::WeatherAlert>>>>,
    cfg: Arc<Mutex<config::Config>>,
    nvs: Arc<Mutex<EspNvs<NvsDefault>>>,
    http_lock: Arc<Mutex<()>>,
) {
    std::thread::Builder::new()
        .name("alerts".into())
        .stack_size(32768)
        .spawn(move || {
            info!("NWS alerts thread startup delay: {}s", crate::ALERTS_START_DELAY_SECS);
            std::thread::sleep(Duration::from_secs(crate::ALERTS_START_DELAY_SECS));
            let mut consecutive_failures: u32 = 0;
            loop {
                let (enabled, auto_scope, scope, cached_zone, ua) = {
                    let c = cfg.lock().unwrap();
                    (c.alerts_enabled, c.alerts_auto_scope, c.nws_scope.clone(),
                     c.nws_zone.clone(), c.nws_user_agent.clone())
                };
                if !enabled {
                    std::thread::sleep(Duration::from_secs(5));
                    continue;
                }
                let effective_scope = if auto_scope {
                    if cached_zone.is_empty() {
                        let _http = http_lock.lock().unwrap();
                        match weather::discover_nws_zone(&ua) {
                            Ok(zone) => {
                                info!("NWS auto-scope discovered zone={}", zone);
                                consecutive_failures = 0;
                                if let Ok(mut c) = cfg.lock() { c.nws_zone = zone.clone(); }
                                if let Ok(mut nvs) = nvs.lock() {
                                    let _ = config::Config::save_nws_zone(&mut nvs, &zone);
                                }
                                format!("zone={}", zone)
                            }
                            Err(e) => {
                                consecutive_failures = consecutive_failures.saturating_add(1);
                                if consecutive_failures == 1
                                    || consecutive_failures.is_multiple_of(crate::FAILURE_WARN_EVERY)
                                {
                                    log::warn!(
                                        "NWS auto-scope discovery failed ({} consecutive): {}",
                                        consecutive_failures, e
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
                let alerts_result = {
                    let _http = http_lock.lock().unwrap();
                    weather::fetch_nws_alerts(&effective_scope, &ua)
                };
                match alerts_result {
                    Ok(alerts) => {
                        let count = alerts.len();
                        consecutive_failures = 0;
                        if count == 0 {
                            info!("NWS alerts: 0 active");
                        } else {
                            let names = alerts.iter()
                                .map(|a| a.event.as_str())
                                .collect::<Vec<_>>()
                                .join(", ");
                            info!("NWS alerts: {} active — {}", count, names);
                        }
                        *alert_data.lock().unwrap() = Some(alerts);
                        let interval = if count > 0 {
                            crate::ALERTS_ACTIVE_INTERVAL_SECS
                        } else {
                            crate::ALERTS_INTERVAL_SECS
                        };
                        std::thread::sleep(Duration::from_secs(interval));
                    }
                    Err(e) => {
                        consecutive_failures = consecutive_failures.saturating_add(1);
                        if consecutive_failures == 1
                            || consecutive_failures.is_multiple_of(crate::FAILURE_WARN_EVERY)
                        {
                            log::warn!(
                                "NWS alerts fetch failed ({} consecutive): {}",
                                consecutive_failures, e
                            );
                        } else {
                            info!("NWS alerts fetch failed ({} consecutive)", consecutive_failures);
                        }
                        let retry_secs = if consecutive_failures >= 30 { 120u64 }
                            else if consecutive_failures >= 10 { 60 }
                            else { crate::WEATHER_RETRY_SECS };
                        std::thread::sleep(Duration::from_secs(retry_secs));
                    }
                }
            }
        })
        .expect("failed to spawn alerts thread");
}
