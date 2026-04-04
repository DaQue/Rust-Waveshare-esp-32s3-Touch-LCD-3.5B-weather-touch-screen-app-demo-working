pub use ws_s3_3p5_bsp::bme280::*;

// ── Plausibility filter constants ─────────────────────────────────────
const BME_TEMP_MIN_F: f32 = 35.0; // indoor — below freezing is sensor glitch
const BME_TEMP_MAX_F: f32 = 115.0; // indoor — above 115°F is sensor glitch
const BME_PRESSURE_MIN_HPA: f32 = 950.0;
const BME_PRESSURE_MAX_HPA: f32 = 1100.0;
const BME_TEMP_MAX_STEP_F: f32 = 8.0;
const BME_HUM_MAX_STEP: f32 = 20.0;
const BME_PRESSURE_MAX_STEP_HPA: f32 = 12.0;

/// Returns false if `reading` is physically implausible for an indoor sensor
/// or represents a spike relative to the previous accepted values in `state`.
pub fn reading_is_plausible(state: &crate::views::AppState, reading: &Bme280Reading) -> bool {
    if !reading.temperature_f.is_finite()
        || !reading.humidity.is_finite()
        || !reading.pressure_hpa.is_finite()
    {
        return false;
    }
    if !(BME_TEMP_MIN_F..=BME_TEMP_MAX_F).contains(&reading.temperature_f) {
        log::warn!("BME280 temp out of range: {:.1}°F", reading.temperature_f);
        return false;
    }
    if !(0.0..=100.0).contains(&reading.humidity) {
        log::warn!("BME280 humidity out of range: {:.1}%", reading.humidity);
        return false;
    }
    if !(BME_PRESSURE_MIN_HPA..=BME_PRESSURE_MAX_HPA).contains(&reading.pressure_hpa) {
        log::warn!(
            "BME280 pressure out of range: {:.0} hPa",
            reading.pressure_hpa
        );
        return false;
    }
    if let Some(prev) = state.indoor_temp {
        if (reading.temperature_f - prev).abs() > BME_TEMP_MAX_STEP_F {
            return false;
        }
    }
    if let Some(prev) = state.indoor_humidity {
        if (reading.humidity - prev).abs() > BME_HUM_MAX_STEP {
            return false;
        }
    }
    if let Some(prev) = state.indoor_pressure {
        if (reading.pressure_hpa - prev).abs() > BME_PRESSURE_MAX_STEP_HPA {
            return false;
        }
    }
    true
}
