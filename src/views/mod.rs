pub mod now;
pub mod indoor;
pub mod hvac;
pub mod pressure_hvac;
pub mod forecast;
pub mod i2c_scan;
pub mod wifi_scan;
pub mod about;
pub mod warning;

use std::collections::VecDeque;
use crate::config::OrientationMode;
use crate::framebuffer::Framebuffer;
use crate::layout::{self, Orientation};
use crate::touch::Gesture;

/// Views in navigation order (matches C factory cycle).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Indoor,
    Hvac,
    PressureHvac,
    Now,
    Forecast,
    I2cScan,
    WifiScan,
    About,
    Warning,
}

impl View {
    pub fn next(self) -> View {
        match self {
            View::Indoor => View::Hvac,
            View::Hvac => View::PressureHvac,
            View::PressureHvac => View::Now,
            View::Now => View::Forecast,
            View::Forecast => View::I2cScan,
            View::I2cScan => View::WifiScan,
            View::WifiScan => View::About,
            View::About => View::Indoor,
            View::Warning => View::Now,
        }
    }

    pub fn prev(self) -> View {
        match self {
            View::Indoor => View::About,
            View::Hvac => View::Indoor,
            View::PressureHvac => View::Hvac,
            View::Now => View::PressureHvac,
            View::Forecast => View::Now,
            View::I2cScan => View::Forecast,
            View::WifiScan => View::I2cScan,
            View::About => View::WifiScan,
            View::Warning => View::Now,
        }
    }
}

/// Ring buffer for indoor sensor history (temp & humidity).
pub const INDOOR_HISTORY_MAX: usize = 720; // 720 samples @ 5s = 1 hour

/// Central app state shared across views.
pub struct AppState {
    pub current_view: View,
    pub current_weather: Option<crate::weather::CurrentWeather>,
    pub forecast: Option<crate::weather::Forecast>,
    pub indoor_temp: Option<f32>,
    pub indoor_humidity: Option<f32>,
    pub indoor_pressure: Option<f32>,
    pub indoor_temp_history: VecDeque<f32>,
    pub indoor_hum_history: VecDeque<f32>,
    pub time_text: String,
    pub status_text: String,
    pub bottom_text: String,
    pub i2c_devices: Vec<u8>,
    pub wifi_networks: Vec<(String, i8)>, // (ssid, rssi)
    pub wifi_scan_pending: bool,
    pub wifi_ssid: String,
    pub ip_address: String,
    pub forecast_hourly_open: bool,
    pub forecast_hourly_day: usize,
    pub forecast_hourly_scroll: usize,
    pub weather_alerts: Vec<crate::weather::WeatherAlert>,
    pub now_alerts_open: bool,
    pub use_celsius: bool,
    pub weather_stale: bool,
    pub save_celsius_pref: bool,
    pub force_weather_refresh: bool,
    pub warning_active: bool,
    pub warning_silenced_fingerprint: String,
    pub warning_scroll: usize,
    pub hvac: crate::hvac::HvacDetector,
    pub pressure_history: crate::pressure_history::PressureHistory,
    pub orientation: Orientation,
    pub orientation_mode: OrientationMode,
    pub orientation_flip: bool,
    pub dirty: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            current_view: View::Now,
            current_weather: None,
            forecast: None,
            indoor_temp: None,
            indoor_humidity: None,
            indoor_pressure: None,
            indoor_temp_history: VecDeque::new(),
            indoor_hum_history: VecDeque::new(),
            time_text: String::new(),
            status_text: "Starting...".to_string(),
            bottom_text: String::new(),
            i2c_devices: Vec::new(),
            wifi_networks: Vec::new(),
            wifi_scan_pending: false,
            wifi_ssid: String::new(),
            ip_address: String::new(),
            forecast_hourly_open: false,
            forecast_hourly_day: 0,
            forecast_hourly_scroll: 0,
            weather_alerts: Vec::new(),
            now_alerts_open: false,
            use_celsius: false,
            weather_stale: false,
            save_celsius_pref: false,
            force_weather_refresh: false,
            warning_active: false,
            warning_silenced_fingerprint: String::new(),
            warning_scroll: 0,
            hvac: crate::hvac::HvacDetector::new(5.0, 30.0),
            pressure_history: crate::pressure_history::PressureHistory::new(),
            orientation: Orientation::Landscape,
            orientation_mode: OrientationMode::Auto,
            orientation_flip: false,
            dirty: true,
        }
    }

    pub fn screen_w(&self) -> i32 {
        layout::screen_w(self.orientation)
    }

    pub fn screen_h(&self) -> i32 {
        layout::screen_h(self.orientation)
    }

    /// Handle a touch gesture, returning true if the display needs a redraw.
    pub fn handle_gesture(&mut self, gesture: Gesture) -> bool {
        // Warning view intercepts all gestures
        if self.current_view == View::Warning {
            return self.handle_warning_gesture(gesture);
        }

        match gesture {
            Gesture::None => false,
            Gesture::SwipeLeft => {
                self.current_view = self.current_view.next();
                self.forecast_hourly_open = false;
                self.dirty = true;
                true
            }
            Gesture::SwipeRight => {
                self.current_view = self.current_view.prev();
                self.forecast_hourly_open = false;
                self.dirty = true;
                true
            }
            Gesture::SwipeUp => {
                if self.current_view == View::Forecast && self.forecast_hourly_open {
                    self.forecast_hourly_scroll = self.forecast_hourly_scroll.saturating_add(4);
                    self.dirty = true;
                    true
                } else if self.current_view == View::Warning {
                    self.warning_scroll = self.warning_scroll.saturating_add(3);
                    self.dirty = true;
                    true
                } else {
                    false
                }
            }
            Gesture::SwipeDown => {
                if self.current_view == View::Forecast && self.forecast_hourly_open {
                    self.forecast_hourly_scroll = self.forecast_hourly_scroll.saturating_sub(4);
                    self.dirty = true;
                    true
                } else if self.current_view == View::Warning {
                    self.warning_scroll = self.warning_scroll.saturating_sub(3);
                    self.dirty = true;
                    true
                } else {
                    false
                }
            }
            Gesture::Tap { x, y } => {
                self.handle_tap(x, y)
            }
        }
    }

    fn handle_warning_gesture(&mut self, gesture: Gesture) -> bool {
        let screen_h = self.screen_h() as i16;
        let button_y = screen_h - screen_h / 3;

        match gesture {
            Gesture::Tap { y, .. } => {
                if self.warning_active && y >= button_y {
                    // Silence: stop beeping, record fingerprint, stay on page
                    self.warning_active = false;
                    self.warning_silenced_fingerprint = self.alert_fingerprint();
                    crate::debug_flags::request_beep_stop();
                    self.dirty = true;
                    true
                } else {
                    false
                }
            }
            Gesture::SwipeLeft | Gesture::SwipeRight => {
                if !self.warning_active {
                    // Already silenced — allow exit
                    self.current_view = View::Now;
                    self.warning_scroll = 0;
                    self.dirty = true;
                    true
                } else {
                    false // Block navigation while alarm is active
                }
            }
            Gesture::SwipeUp => {
                self.warning_scroll = self.warning_scroll.saturating_add(3);
                self.dirty = true;
                true
            }
            Gesture::SwipeDown => {
                self.warning_scroll = self.warning_scroll.saturating_sub(3);
                self.dirty = true;
                true
            }
            Gesture::None => false,
        }
    }

    /// Compute a fingerprint of the current alert set for silence tracking.
    fn alert_fingerprint(&self) -> String {
        let mut parts: Vec<String> = self
            .weather_alerts
            .iter()
            .map(|a| format!("{}|{}|{}", a.id, a.event, a.severity))
            .collect();
        parts.sort_unstable();
        parts.join("||")
    }

    fn handle_tap(&mut self, x: i16, y: i16) -> bool {
        let screen_w = self.screen_w() as i16;
        let screen_h = self.screen_h() as i16;

        // ── Header tap: "Main >" on forecast, or right side of header navigates forward ──
        if y < 30 {
            if x >= screen_w - 120 {
                // Right header tap: navigate to next view (or back to Now from Forecast)
                if self.current_view == View::Forecast {
                    self.current_view = View::Now;
                } else {
                    self.current_view = self.current_view.next();
                }
                self.forecast_hourly_open = false;
                self.dirty = true;
                return true;
            }
            if x < 120 {
                // Left header tap: navigate to prev view
                self.current_view = self.current_view.prev();
                self.forecast_hourly_open = false;
                self.dirty = true;
                return true;
            }
        }

        // ── NOW view taps ──
        if self.current_view == View::Now {
            let (temp_x0, temp_x1, temp_y0, temp_y1) = match self.orientation {
                o if o.is_landscape() => (100, 280, 36, 110),
                _ => (100, 310, 36, 126),
            };
            if (temp_x0..=temp_x1).contains(&x) && (temp_y0..=temp_y1).contains(&y) {
                self.use_celsius = !self.use_celsius;
                self.save_celsius_pref = true;
                self.dirty = true;
                return true;
            }

            let (icon_x0, icon_x1, icon_y0, icon_y1) = match self.orientation {
                o if o.is_landscape() => (10, 105, 36, 130),
                _ => (10, 120, 36, 150),
            };
            if (icon_x0..=icon_x1).contains(&x) && (icon_y0..=icon_y1).contains(&y) {
                if self.weather_alerts.is_empty() {
                    self.force_weather_refresh = true;
                } else {
                    self.now_alerts_open = !self.now_alerts_open;
                }
                self.dirty = true;
                return true;
            }

            // Tap on forecast card at bottom → navigate to Forecast view
            let forecast_tap_top = match self.orientation {
                o if o.is_landscape() => 208,
                _ => 250,
            };
            if y >= forecast_tap_top && y < screen_h {
                self.current_view = View::Forecast;
                self.dirty = true;
                return true;
            }
        }

        // ── Forecast view taps ──
        if self.current_view == View::Forecast {
            // Tap on daily row → open hourly drill-down
            if !self.forecast_hourly_open {
                let row_top = 38i16;
                let row_stride = 66i16;
                if y >= row_top && y < row_top + 4 * row_stride {
                    let row = ((y - row_top) / row_stride) as usize;
                    if let Some(fc) = &self.forecast {
                        if row < fc.days.len() && !fc.days[row].entries.is_empty() {
                            self.forecast_hourly_open = true;
                            self.forecast_hourly_day = row;
                            self.forecast_hourly_scroll = 0;
                            self.dirty = true;
                            return true;
                        }
                    }
                }
            }
        }

        false
    }
}

/// Draw the current view into the framebuffer.
pub fn draw_current_view(fb: &mut Framebuffer, state: &AppState) {
    match state.current_view {
        View::Now => now::draw(fb, state),
        View::Indoor => indoor::draw(fb, state),
        View::Hvac => hvac::draw(fb, state),
        View::PressureHvac => pressure_hvac::draw(fb, state),
        View::Forecast => forecast::draw(fb, state),
        View::I2cScan => i2c_scan::draw(fb, state),
        View::WifiScan => wifi_scan::draw(fb, state),
        View::About => about::draw(fb, state),
        View::Warning => warning::draw(fb, state),
    }
}
