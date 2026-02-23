use std::sync::atomic::{AtomicBool, AtomicI8, Ordering};

/// Global debug flags toggled via console.
/// When a flag is true, the module logs at info! level instead of being silent.
pub static DEBUG_TOUCH: AtomicBool = AtomicBool::new(false);
pub static DEBUG_BME280: AtomicBool = AtomicBool::new(false);
pub static DEBUG_WIFI: AtomicBool = AtomicBool::new(false);
pub static DEBUG_WEATHER: AtomicBool = AtomicBool::new(false);
pub static DEBUG_IMU: AtomicBool = AtomicBool::new(false);

/// Request flags — console sets these, main loop acts on them.
pub static REQUEST_I2C_SCAN: AtomicBool = AtomicBool::new(false);
pub static REQUEST_IMU_READ: AtomicBool = AtomicBool::new(false);
pub static REQUEST_ORIENTATION_MODE: AtomicI8 = AtomicI8::new(-1);
pub static REQUEST_ORIENTATION_FLIP: AtomicI8 = AtomicI8::new(-1);
pub static REQUEST_BEEP_TONE: AtomicI8 = AtomicI8::new(-1);
pub static REQUEST_BEEP_STOP: AtomicBool = AtomicBool::new(false);
pub static REQUEST_SILENCE_WARNING: AtomicBool = AtomicBool::new(false);
pub static REQUEST_TEST_WARNING: AtomicBool = AtomicBool::new(false);

pub fn is_on(flag: &AtomicBool) -> bool {
    flag.load(Ordering::Relaxed)
}

pub fn set(flag: &AtomicBool, val: bool) {
    flag.store(val, Ordering::Relaxed);
}

pub fn toggle(flag: &AtomicBool) -> bool {
    let old = flag.load(Ordering::Relaxed);
    flag.store(!old, Ordering::Relaxed);
    !old
}

pub fn status_line() -> String {
    format!(
        "touch={} bme280={} wifi={} weather={} imu={}",
        if is_on(&DEBUG_TOUCH) { "ON" } else { "off" },
        if is_on(&DEBUG_BME280) { "ON" } else { "off" },
        if is_on(&DEBUG_WIFI) { "ON" } else { "off" },
        if is_on(&DEBUG_WEATHER) { "ON" } else { "off" },
        if is_on(&DEBUG_IMU) { "ON" } else { "off" },
    )
}

pub fn request_orientation_mode(mode: crate::config::OrientationMode) {
    let v = match mode {
        crate::config::OrientationMode::Auto => 0,
        crate::config::OrientationMode::Landscape => 1,
        crate::config::OrientationMode::Portrait => 2,
    };
    REQUEST_ORIENTATION_MODE.store(v, Ordering::Relaxed);
}

pub fn take_orientation_mode_request() -> Option<crate::config::OrientationMode> {
    match REQUEST_ORIENTATION_MODE.swap(-1, Ordering::Relaxed) {
        0 => Some(crate::config::OrientationMode::Auto),
        1 => Some(crate::config::OrientationMode::Landscape),
        2 => Some(crate::config::OrientationMode::Portrait),
        _ => None,
    }
}

pub fn request_orientation_flip(flip: bool) {
    REQUEST_ORIENTATION_FLIP.store(if flip { 1 } else { 0 }, Ordering::Relaxed);
}

pub fn take_orientation_flip_request() -> Option<bool> {
    match REQUEST_ORIENTATION_FLIP.swap(-1, Ordering::Relaxed) {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

pub fn request_beep_tone(tone: i8) {
    REQUEST_BEEP_STOP.store(false, Ordering::Relaxed);
    REQUEST_BEEP_TONE.store(tone, Ordering::Relaxed);
}

pub fn take_beep_tone_request() -> Option<i8> {
    match REQUEST_BEEP_TONE.swap(-1, Ordering::Relaxed) {
        v @ (0..=2) => Some(v),
        _ => None,
    }
}

pub fn request_beep_stop() {
    REQUEST_BEEP_STOP.store(true, Ordering::Relaxed);
}

pub fn take_beep_stop_request() -> bool {
    REQUEST_BEEP_STOP.swap(false, Ordering::Relaxed)
}
