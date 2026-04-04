use anyhow::Result;
use esp_idf_svc::nvs::{EspNvs, NvsDefault};
use log::info;
use std::sync::{Arc, Mutex};

use crate::config::Config;

pub(super) fn handle_alerts(
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
            info!(
                "alerts zone: {}",
                if cfg.nws_zone.is_empty() {
                    "<unset>"
                } else {
                    &cfg.nws_zone
                }
            );
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
                    info!(
                        "alerts zone: {}",
                        if cfg.nws_zone.is_empty() {
                            "<unset>"
                        } else {
                            &cfg.nws_zone
                        }
                    );
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
        "test" => {
            let kind = rest.trim().to_ascii_lowercase();
            match kind.as_str() {
                "warning" | "" => {
                    crate::debug_flags::REQUEST_TEST_WARNING
                        .store(true, std::sync::atomic::Ordering::Relaxed);
                    info!("test warning alert injected (will trigger on next tick)");
                }
                _ => info!("usage: alerts test warning"),
            }
        }
        "silence" => {
            crate::debug_flags::request_beep_stop();
            crate::debug_flags::REQUEST_SILENCE_WARNING
                .store(true, std::sync::atomic::Ordering::Relaxed);
            info!("alert beeping silenced");
        }
        _ => info!("usage: alerts show|on|off|beep|silence|auto-scope|ua|scope|zone|test warning"),
    }
    Ok(())
}
