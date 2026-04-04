use anyhow::Result;
use esp_idf_svc::nvs::{EspNvs, NvsDefault};
use log::info;
use std::sync::{Arc, Mutex};

use crate::config::Config;

pub(super) fn handle_beep(sub: &str) {
    match sub {
        "advisory" => crate::debug_flags::request_beep_tone(0),
        "watch" => crate::debug_flags::request_beep_tone(1),
        "warning" => crate::debug_flags::request_beep_tone(2),
        "stop" => {
            crate::debug_flags::request_beep_stop();
            crate::debug_flags::REQUEST_SILENCE_WARNING
                .store(true, std::sync::atomic::Ordering::Relaxed);
            info!("beep stop + warning silence requested");
            return;
        }
        _ => {
            info!("usage: beep advisory|watch|warning|stop");
            return;
        }
    }
    info!("beep requested: {}", sub);
}

pub(super) fn handle_debug(sub: &str) {
    use crate::debug_flags::*;
    match sub {
        "show" | "" => {
            info!("debug: {}", status_line());
        }
        "on" => {
            set(&DEBUG_TOUCH, true);
            set(&DEBUG_BME280, true);
            set(&DEBUG_WIFI, true);
            set(&DEBUG_WEATHER, true);
            set(&DEBUG_IMU, true);
            info!("debug all: ON");
        }
        "off" => {
            set(&DEBUG_TOUCH, false);
            set(&DEBUG_BME280, false);
            set(&DEBUG_WIFI, false);
            set(&DEBUG_WEATHER, false);
            set(&DEBUG_IMU, false);
            info!("debug all: OFF");
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
            let any_off = !is_on(&DEBUG_TOUCH)
                || !is_on(&DEBUG_BME280)
                || !is_on(&DEBUG_WIFI)
                || !is_on(&DEBUG_WEATHER)
                || !is_on(&DEBUG_IMU);
            set(&DEBUG_TOUCH, any_off);
            set(&DEBUG_BME280, any_off);
            set(&DEBUG_WIFI, any_off);
            set(&DEBUG_WEATHER, any_off);
            set(&DEBUG_IMU, any_off);
            info!("debug all: {}", if any_off { "ON" } else { "OFF" });
        }
        _ => {
            info!(
                "unknown module '{}'. options: touch, bme280, wifi, weather, imu, all",
                sub
            );
        }
    }
}

pub(super) fn handle_i2c(sub: &str) {
    match sub {
        "scan" | "" => {
            info!("i2c: scan requested (will run on next tick)");
            crate::debug_flags::REQUEST_I2C_SCAN.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        _ => info!("usage: i2c scan"),
    }
}

pub(super) fn handle_imu(sub: &str) {
    match sub {
        "read" | "" => {
            info!("imu: read requested (will run on next tick)");
            crate::debug_flags::REQUEST_IMU_READ.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        _ => info!("usage: imu read"),
    }
}

pub(super) fn handle_orientation(
    sub: &str,
    rest: &str,
    nvs: &Arc<Mutex<EspNvs<NvsDefault>>>,
    config: &Arc<Mutex<Config>>,
) -> Result<()> {
    if sub.is_empty() || sub == "show" {
        let cfg = config.lock().unwrap();
        info!("orientation: {}", cfg.orientation_mode.as_str());
        info!(
            "orientation flip: {}",
            if cfg.orientation_flip { "on" } else { "off" }
        );
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
                info!(
                    "orientation flip: {}",
                    if cfg.orientation_flip { "on" } else { "off" }
                );
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

pub(super) fn handle_units(
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

pub(super) fn handle_flash(
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
