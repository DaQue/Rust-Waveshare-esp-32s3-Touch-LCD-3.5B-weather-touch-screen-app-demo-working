pub mod now;
pub mod indoor;
pub mod hvac;
pub mod pressure_hvac;
pub mod forecast;
pub mod i2c_scan;
pub mod wifi_scan;
pub mod about;
pub mod warning;
pub mod nav_menu;

use std::collections::VecDeque;
use crate::config::OrientationMode;
use crate::framebuffer::Framebuffer;
use crate::layout::{self, Orientation};
use crate::touch::Gesture;

/// Views grouped by category.
///
/// Navigation groups:
///   Weather:  Now <-> Forecast
///   Sensors:  Indoor <-> Hvac <-> PressureHvac
///   System:   I2cScan <-> WifiScan <-> About
///
/// Swiping left/right moves within the current group.
/// Reaching a group boundary (or swiping left from Now) opens NavMenu.
/// Center-header tap on any view returns to Now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    // Weather group
    Now,
    Forecast,
    // Sensors group
    Indoor,
    Hvac,
    PressureHvac,
    // System group
    I2cScan,
    WifiScan,
    About,
    // Special
    NavMenu,
    Warning,
}

impl View {
    /// Next view within the current group, or None at the group boundary.
    /// None from Now means SwipeLeft → NavMenu (the jump page).
    /// Forecast is reached by tapping the preview card on Now, not by swiping.
    pub fn next(self) -> Option<View> {
        match self {
            View::Now          => None, // SwipeLeft on home → NavMenu
            View::Forecast     => None,
            View::Indoor       => Some(View::Hvac),
            View::Hvac         => Some(View::PressureHvac),
            View::PressureHvac => None,
            View::I2cScan      => Some(View::WifiScan),
            View::WifiScan     => Some(View::About),
            View::About        => None,
            _                  => None,
        }
    }

    /// Previous view within the current group, or None at the group boundary.
    pub fn prev(self) -> Option<View> {
        match self {
            View::Forecast     => Some(View::Now),
            View::Hvac         => Some(View::Indoor),
            View::PressureHvac => Some(View::Hvac),
            View::WifiScan     => Some(View::I2cScan),
            View::About        => Some(View::WifiScan),
            _                  => None,
        }
    }

    /// The entry view for this view's group (used by NavMenu jumps).
    pub fn group_entry(self) -> View {
        match self {
            View::Now | View::Forecast                          => View::Now,
            View::Indoor | View::Hvac | View::PressureHvac     => View::Indoor,
            View::I2cScan | View::WifiScan | View::About        => View::I2cScan,
            other                                               => other,
        }
    }
}

/// Ring buffer for indoor sensor history (temp & humidity).
pub const INDOOR_HISTORY_MAX: usize = 720; // 720 samples @ 5s = 1 hour

/// Central app state shared across views.
#[derive(Clone)]
pub struct AppState {
    pub current_view: View,
    pub current_weather: Option<crate::weather::CurrentWeather>,
    pub forecast: Option<crate::weather::Forecast>,
    pub indoor_temp: Option<f32>,
    pub indoor_humidity: Option<f32>,
    pub indoor_pressure: Option<f32>,
    pub indoor_temp_history: VecDeque<f32>,
    pub indoor_hum_history: VecDeque<f32>,
    pub outdoor_temp_history: VecDeque<f32>,
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
    pub hvac: crate::psbox::PsBox<crate::hvac::HvacDetector>,
    pub pressure_history: crate::psbox::PsBox<crate::pressure_history::PressureHistory>,
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
            outdoor_temp_history: VecDeque::new(),
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
            hvac: crate::psbox::PsBox::new(crate::hvac::HvacDetector::new(5.0, 30.0)),
            pressure_history: crate::psbox::PsBox::new(crate::pressure_history::PressureHistory::new()),
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
                // Close hourly if open before navigating
                if self.current_view == View::Forecast && self.forecast_hourly_open {
                    self.forecast_hourly_open = false;
                    self.dirty = true;
                    return true;
                }
                // NavMenu: swipe left does nothing (use buttons or swipe right for home)
                if self.current_view == View::NavMenu {
                    return false;
                }
                // Move to next view within group, or open NavMenu at boundary
                if let Some(next) = self.current_view.next() {
                    self.current_view = next;
                } else {
                    self.current_view = View::NavMenu;
                }
                self.dirty = true;
                true
            }

            Gesture::SwipeRight => {
                // NavMenu: swipe right → home
                if self.current_view == View::NavMenu {
                    self.current_view = View::Now;
                    self.dirty = true;
                    return true;
                }
                // Move to prev view within group, or open NavMenu at boundary
                if let Some(prev) = self.current_view.prev() {
                    self.current_view = prev;
                    self.forecast_hourly_open = false;
                } else if self.current_view != View::Now {
                    // At left edge of a non-home group → NavMenu
                    self.current_view = View::NavMenu;
                }
                // SwipeRight on Now does nothing (already home)
                self.dirty = true;
                true
            }

            Gesture::SwipeUp => {
                if self.current_view == View::Forecast && self.forecast_hourly_open {
                    self.forecast_hourly_scroll = self.forecast_hourly_scroll.saturating_add(4);
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
                } else {
                    false
                }
            }

            Gesture::Tap { x, y } => self.handle_tap(x, y),
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

        // ── NavMenu taps ──
        if self.current_view == View::NavMenu {
            if let Some(group_tap) = nav_menu::hit_test(x, y, self.orientation) {
                self.current_view = match group_tap {
                    nav_menu::NavTap::Weather => View::Now,
                    nav_menu::NavTap::Sensors => View::Indoor,
                    nav_menu::NavTap::System  => View::I2cScan,
                };
                self.forecast_hourly_open = false;
                self.dirty = true;
                return true;
            }
            return false;
        }

        // ── Header tap ──
        if y < 30 {
            // Center header tap (any view) → home
            if x >= 100 && x < screen_w - 100 {
                self.current_view = View::Now;
                self.forecast_hourly_open = false;
                self.dirty = true;
                return true;
            }
            // Right header tap: advance within group (or NavMenu at boundary)
            if x >= screen_w - 100 {
                self.forecast_hourly_open = false;
                if let Some(next) = self.current_view.next() {
                    self.current_view = next;
                } else {
                    self.current_view = View::NavMenu;
                }
                self.dirty = true;
                return true;
            }
            // Left header tap: go back within group, or NavMenu at any left boundary
            if x < 100 {
                self.forecast_hourly_open = false;
                if let Some(prev) = self.current_view.prev() {
                    self.current_view = prev;
                } else {
                    self.current_view = View::NavMenu;
                }
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
        View::Now        => now::draw(fb, state),
        View::Forecast   => forecast::draw(fb, state),
        View::Indoor     => indoor::draw(fb, state),
        View::Hvac       => hvac::draw(fb, state),
        View::PressureHvac => pressure_hvac::draw(fb, state),
        View::I2cScan    => i2c_scan::draw(fb, state),
        View::WifiScan   => wifi_scan::draw(fb, state),
        View::About      => about::draw(fb, state),
        View::NavMenu    => nav_menu::draw(fb, state),
        View::Warning    => warning::draw(fb, state),
    }
}
