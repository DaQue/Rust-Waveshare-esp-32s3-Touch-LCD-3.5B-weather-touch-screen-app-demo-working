use std::sync::atomic::{AtomicBool, Ordering};
/// BME280 polling and NWS alert processing helpers for the main event loop.
use std::sync::{Arc, Mutex};

use esp_idf_hal::i2c::I2cDriver;
use log::info;

use crate::{bme280_sensor, config, debug_flags, speaker, views, weather};

// ── BME280 poll ───────────────────────────────────────────────────────────────

#[derive(Default)]
pub(crate) struct BmePollState {
    pub last_ms: u32,
    pub last_init_ms: u32,
    pub reject_streak: u16,
    pub sample_tick: u32,
}

pub(crate) fn poll_bme280(
    state: &mut views::AppState,
    bme: &mut Option<bme280_sensor::Bme280>,
    i2c: &mut I2cDriver<'_>,
    t: u32,
    ps: &mut BmePollState,
) {
    if crate::SRAM_BME_RESET.swap(false, Ordering::Relaxed) {
        log::warn!("BME280 reset due to low SRAM — will re-init on next interval");
        *bme = None;
        ps.reject_streak = 0;
    }

    if t.wrapping_sub(ps.last_ms) < crate::BME280_INTERVAL_MS {
        return;
    }
    ps.last_ms = t;

    if bme.is_none() && t.wrapping_sub(ps.last_init_ms) >= 30_000 {
        ps.last_init_ms = t;
        *bme = bme280_sensor::Bme280::init(i2c);
        if bme.is_some() {
            info!("BME280 found on retry — sensor now active");
            ps.reject_streak = 0;
        }
    }

    if let Some(ref sensor) = *bme {
        match sensor.read(i2c) {
            Some(reading) => {
                let plausible = bme280_sensor::reading_is_plausible(state, &reading);
                if !plausible {
                    ps.reject_streak = ps.reject_streak.saturating_add(1);
                }
                // Accept if plausible, or force-accept after 12 rejects (~60s) to recover.
                if !plausible && ps.reject_streak < 12 {
                    if debug_flags::is_on(&debug_flags::DEBUG_BME280) {
                        log::warn!(
                            "BME280 outlier dropped ({}): {:.1}°F {:.1}%RH {:.0}hPa",
                            ps.reject_streak,
                            reading.temperature_f,
                            reading.humidity,
                            reading.pressure_hpa
                        );
                    }
                } else {
                    if !plausible && debug_flags::is_on(&debug_flags::DEBUG_BME280) {
                        log::warn!(
                            "BME280 re-baselining: {:.1}°F {:.1}%RH {:.0}hPa",
                            reading.temperature_f,
                            reading.humidity,
                            reading.pressure_hpa
                        );
                    }
                    ps.reject_streak = 0;
                    ps.sample_tick = ps.sample_tick.wrapping_add(1);
                    if debug_flags::is_on(&debug_flags::DEBUG_BME280) {
                        info!(
                            "BME280: {:.1}°F  {:.1}%RH  {:.0}hPa",
                            reading.temperature_f, reading.humidity, reading.pressure_hpa
                        );
                    }
                    state.indoor_temp = Some(reading.temperature_f);
                    state.indoor_humidity = Some(reading.humidity);
                    state.indoor_pressure = Some(reading.pressure_hpa);

                    if ps.sample_tick.is_multiple_of(3) {
                        state.indoor_temp_history.push_back(reading.temperature_f);
                        state.indoor_hum_history.push_back(reading.humidity);
                        let bme_hpa = state.indoor_pressure;
                        let owm_hpa = state.current_weather.as_ref().and_then(|cw| {
                            if cw.pressure_hpa > 0 {
                                Some(cw.pressure_hpa as f32)
                            } else {
                                None
                            }
                        });
                        state.pressure_history.push_short(bme_hpa, owm_hpa);
                    }
                    if ps.sample_tick.is_multiple_of(36) {
                        state.indoor_temp_hist_long.push_back(reading.temperature_f);
                        state.indoor_hum_hist_long.push_back(reading.humidity);
                    }
                    state.dirty = true;
                }
            }
            None => log::warn!("BME280 read returned None (I2C failed)"),
        }
    }
}

// ── Alert state ───────────────────────────────────────────────────────────────

#[derive(Default)]
pub(crate) struct AlertPollState {
    pub fingerprint: String,
    pub snapshot_seen: bool,
    pub last_beep_ms: Option<u32>,
    pub watch_beeps_remaining: u8,
}

pub(crate) fn process_alert_data(
    state: &mut views::AppState,
    alerts: Vec<weather::WeatherAlert>,
    als: &mut AlertPollState,
    speaker_ready: &AtomicBool,
    cfg: &Arc<Mutex<config::Config>>,
) {
    let fp = weather::alert_fingerprint(&alerts);
    let changed = !als.snapshot_seen || fp != als.fingerprint;
    let beep_enabled = cfg.lock().unwrap().alerts_beep;

    if changed && !alerts.is_empty() {
        for alert in &alerts {
            weather::log_alert_to_console(alert);
        }
    }

    if changed && !alerts.is_empty() && beep_enabled {
        let tone = weather::alert_tone_for(&alerts);
        let highest_kind = alerts
            .first()
            .map(|a| a.kind())
            .unwrap_or(weather::AlertKind::Other);

        match highest_kind {
            weather::AlertKind::Warning => {
                let already_silenced = fp == state.warning_silenced_fingerprint;
                if !already_silenced {
                    state.warning_active = true;
                    state.warning_scroll = 0;
                    state.current_view = views::View::Warning;
                    if speaker_ready.load(Ordering::Relaxed) {
                        debug_flags::request_beep_tone(tone.request_code());
                        als.last_beep_ms = Some(crate::now_ms());
                        info!(
                            "WARNING takeover: {} ({} active)",
                            tone.as_str(),
                            alerts.len()
                        );
                    }
                }
            }
            weather::AlertKind::Watch => {
                als.watch_beeps_remaining = 3;
                if speaker_ready.load(Ordering::Relaxed) {
                    debug_flags::request_beep_tone(tone.request_code());
                    als.watch_beeps_remaining -= 1;
                    als.last_beep_ms = Some(crate::now_ms());
                    info!("watch beep 1/3 queued ({} active)", alerts.len());
                }
            }
            _ => {
                let now = crate::now_ms();
                let can_beep = als
                    .last_beep_ms
                    .map(|ts| {
                        now.wrapping_sub(ts) >= (crate::ALERT_BEEP_COOLDOWN_SECS as u32 * 1000)
                    })
                    .unwrap_or(true);
                if can_beep && speaker_ready.load(Ordering::Relaxed) {
                    debug_flags::request_beep_tone(tone.request_code());
                    als.last_beep_ms = Some(now);
                    info!("advisory beep queued ({} active)", alerts.len());
                }
            }
        }
    }

    state.weather_alerts = alerts;
    if state.weather_alerts.is_empty() {
        state.now_alerts_open = false;
        if state.warning_active {
            state.warning_active = false;
            if state.current_view == views::View::Warning {
                state.current_view = views::View::Now;
            }
            info!("warning cleared: no active alerts");
        }
    }
    als.fingerprint = fp;
    als.snapshot_seen = true;
    state.dirty = true;
}

pub(crate) fn tick_alert_beeps(
    state: &mut views::AppState,
    als: &mut AlertPollState,
    speaker_ready: &AtomicBool,
) {
    // Repeating warning beep (every 20s while warning_active)
    if state.warning_active {
        if let Some(last) = als.last_beep_ms {
            if crate::now_ms().wrapping_sub(last) >= crate::WARNING_BEEP_INTERVAL_MS
                && speaker_ready.load(Ordering::Relaxed)
            {
                let tone = weather::alert_tone_for(&state.weather_alerts);
                debug_flags::request_beep_tone(tone.request_code());
                als.last_beep_ms = Some(crate::now_ms());
            }
        }
    }

    // Watch beep repeat (every 10s, up to 3 total)
    if als.watch_beeps_remaining > 0 {
        if let Some(last) = als.last_beep_ms {
            if crate::now_ms().wrapping_sub(last) >= crate::WATCH_BEEP_INTERVAL_MS
                && speaker_ready.load(Ordering::Relaxed)
            {
                let tone = weather::alert_tone_for(&state.weather_alerts);
                debug_flags::request_beep_tone(tone.request_code());
                als.watch_beeps_remaining -= 1;
                als.last_beep_ms = Some(crate::now_ms());
                info!(
                    "watch beep repeat ({} remaining)",
                    als.watch_beeps_remaining
                );
            }
        }
    }

    // Console-requested warning silence
    if debug_flags::REQUEST_SILENCE_WARNING.swap(false, Ordering::Relaxed) && state.warning_active {
        state.warning_active = false;
        state.warning_silenced_fingerprint = als.fingerprint.clone();
        debug_flags::request_beep_stop();
        info!("warning silenced via console");
        state.dirty = true;
    }

    // Test warning injection from console
    if debug_flags::REQUEST_TEST_WARNING.swap(false, Ordering::Relaxed) {
        let fake = weather::WeatherAlert {
            id: format!("test-{}", crate::now_ms()),
            event: "Tornado Warning".to_string(),
            headline: "The National Weather Service has issued a Tornado Warning for your area.".to_string(),
            description: "At 3:15 AM CDT, a severe thunderstorm capable of producing a tornado was located near Springfield, moving northeast at 45 mph.\n\nHAZARD: Tornado and quarter size hail.\n\nSOURCE: Radar indicated rotation.\n\nIMPACT: Flying debris will be dangerous to those caught without shelter. Mobile homes will be damaged or destroyed. Damage to roofs, windows, and vehicles will occur. Tree damage is likely.\n\nThis dangerous storm will be near Springfield by 3:30 AM CDT.".to_string(),
            instruction: "TAKE SHELTER NOW! Move to a basement or an interior room on the lowest floor of a sturdy building. Avoid windows. If you are outdoors, in a mobile home, or in a vehicle, move to the closest substantial shelter and protect yourself from flying debris.".to_string(),
            severity: "Extreme".to_string(),
            certainty: "Observed".to_string(),
            urgency: "Immediate".to_string(),
            expires: "2026-02-22T05:00:00-06:00".to_string(),
        };
        info!("TEST WARNING injected");
        state.weather_alerts = vec![fake];
        state.warning_active = true;
        state.warning_scroll = 0;
        state.warning_silenced_fingerprint.clear();
        state.current_view = views::View::Warning;
        als.fingerprint = "test-warning".to_string();
        if speaker_ready.load(Ordering::Relaxed) {
            debug_flags::request_beep_tone(speaker::AlertTone::Warning.request_code());
            als.last_beep_ms = Some(crate::now_ms());
        }
        state.dirty = true;
    }
}
