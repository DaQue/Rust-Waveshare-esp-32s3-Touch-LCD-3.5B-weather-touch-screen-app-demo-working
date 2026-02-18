use std::sync::atomic::{AtomicBool, Ordering};

/// Global debug flags toggled via console.
/// When a flag is true, the module logs at info! level instead of being silent.
pub static DEBUG_TOUCH: AtomicBool = AtomicBool::new(false);
pub static DEBUG_BME280: AtomicBool = AtomicBool::new(false);
pub static DEBUG_WIFI: AtomicBool = AtomicBool::new(false);
pub static DEBUG_WEATHER: AtomicBool = AtomicBool::new(false);

pub fn is_on(flag: &AtomicBool) -> bool {
    flag.load(Ordering::Relaxed)
}

pub fn toggle(flag: &AtomicBool) -> bool {
    let old = flag.load(Ordering::Relaxed);
    flag.store(!old, Ordering::Relaxed);
    !old
}

pub fn status_line() -> String {
    format!(
        "touch={} bme280={} wifi={} weather={}",
        if is_on(&DEBUG_TOUCH) { "ON" } else { "off" },
        if is_on(&DEBUG_BME280) { "ON" } else { "off" },
        if is_on(&DEBUG_WIFI) { "ON" } else { "off" },
        if is_on(&DEBUG_WEATHER) { "ON" } else { "off" },
    )
}
