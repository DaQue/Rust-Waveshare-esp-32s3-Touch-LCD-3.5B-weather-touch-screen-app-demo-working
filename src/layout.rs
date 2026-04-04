pub use ws_s3_3p5_bsp::layout::*;

use crate::{config, qmi8658};

// ── Orientation IMU thresholds ────────────────────────────────────────
pub(crate) const ORIENTATION_SWITCH_MARGIN_G: f32 = 0.25;
pub(crate) const ORIENTATION_MAX_Z_G: f32 = 0.90;
pub(crate) const ORIENTATION_MIN_AXIS_G: f32 = 0.75;
pub(crate) const ORIENTATION_SIGN_THRESHOLD_G: f32 = 0.20;
pub(crate) const ORIENTATION_CONFIRM_SAMPLES: u8 = 25;
pub(crate) const ORIENTATION_CHANGE_COOLDOWN_MS: u32 = 10_000;
pub(crate) const ORIENTATION_POLL_TICKS: u32 = 2;

pub(crate) fn framebuffer_dims(orientation: Orientation) -> (u32, u32) {
    if orientation.is_landscape() {
        (480, 320)
    } else {
        (320, 480)
    }
}

pub(crate) fn locked_orientation(mode: config::OrientationMode, flip: bool) -> Orientation {
    match mode {
        config::OrientationMode::Landscape => {
            if flip {
                Orientation::LandscapeFlipped
            } else {
                Orientation::Landscape
            }
        }
        config::OrientationMode::Portrait => {
            if flip {
                Orientation::PortraitFlipped
            } else {
                Orientation::Portrait
            }
        }
        config::OrientationMode::Auto => Orientation::Landscape,
    }
}

pub(crate) fn detect_orientation_from_imu(r: &qmi8658::ImuReading) -> Option<Orientation> {
    if r.accel_z.abs() > ORIENTATION_MAX_Z_G {
        return None;
    }
    let ax = r.accel_x.abs();
    let ay = r.accel_y.abs();
    let dominant = ax.max(ay);
    if dominant < ORIENTATION_MIN_AXIS_G {
        return None;
    }
    if ax > ay + ORIENTATION_SWITCH_MARGIN_G {
        // For this board mounting, +X tilt corresponds to upside-down portrait.
        if r.accel_x < -ORIENTATION_SIGN_THRESHOLD_G {
            Some(Orientation::Portrait)
        } else if r.accel_x > ORIENTATION_SIGN_THRESHOLD_G {
            Some(Orientation::PortraitFlipped)
        } else {
            None
        }
    } else if ay > ax + ORIENTATION_SWITCH_MARGIN_G {
        if r.accel_y < -ORIENTATION_SIGN_THRESHOLD_G {
            Some(Orientation::Landscape)
        } else if r.accel_y > ORIENTATION_SIGN_THRESHOLD_G {
            Some(Orientation::LandscapeFlipped)
        } else {
            None
        }
    } else {
        None
    }
}
