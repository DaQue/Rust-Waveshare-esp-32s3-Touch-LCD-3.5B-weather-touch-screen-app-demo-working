pub mod now;
pub mod indoor;
pub mod forecast;
pub mod i2c_scan;
pub mod wifi_scan;
pub mod about;

use crate::framebuffer::Framebuffer;
use crate::touch::Gesture;

/// Views in navigation order (matches C factory cycle).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Indoor,
    Now,
    Forecast,
    I2cScan,
    WifiScan,
    About,
}

impl View {
    pub fn next(self) -> View {
        match self {
            View::Indoor => View::Now,
            View::Now => View::Forecast,
            View::Forecast => View::I2cScan,
            View::I2cScan => View::WifiScan,
            View::WifiScan => View::About,
            View::About => View::Indoor,
        }
    }

    pub fn prev(self) -> View {
        match self {
            View::Indoor => View::About,
            View::Now => View::Indoor,
            View::Forecast => View::Now,
            View::I2cScan => View::Forecast,
            View::WifiScan => View::I2cScan,
            View::About => View::WifiScan,
        }
    }
}

/// Central app state shared across views.
pub struct AppState {
    pub current_view: View,
    pub current_weather: Option<crate::weather::CurrentWeather>,
    pub forecast: Option<crate::weather::Forecast>,
    pub indoor_temp: Option<f32>,
    pub indoor_humidity: Option<f32>,
    pub indoor_pressure: Option<f32>,
    pub time_text: String,
    pub status_text: String,
    pub bottom_text: String,
    pub i2c_devices: Vec<u8>,
    pub wifi_networks: Vec<(String, i8)>, // (ssid, rssi)
    pub ip_address: String,
    pub forecast_hourly_open: bool,
    pub forecast_hourly_day: usize,
    pub forecast_hourly_scroll: usize,
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
            time_text: String::new(),
            status_text: "Starting...".to_string(),
            bottom_text: String::new(),
            i2c_devices: Vec::new(),
            wifi_networks: Vec::new(),
            ip_address: String::new(),
            forecast_hourly_open: false,
            forecast_hourly_day: 0,
            forecast_hourly_scroll: 0,
            dirty: true,
        }
    }

    /// Handle a touch gesture, returning true if the display needs a redraw.
    pub fn handle_gesture(&mut self, gesture: Gesture) -> bool {
        match gesture {
            Gesture::None => false,
            Gesture::SwipeLeft => {
                self.current_view = self.current_view.next();
                self.dirty = true;
                true
            }
            Gesture::SwipeRight => {
                self.current_view = self.current_view.prev();
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
            Gesture::Tap { x, y } => {
                self.handle_tap(x, y)
            }
        }
    }

    fn handle_tap(&mut self, x: i16, y: i16) -> bool {
        let screen_w = crate::layout::SCREEN_W as i16;
        let screen_h = crate::layout::SCREEN_H as i16;

        // Bottom edge tap navigation (matches C factory)
        if y >= screen_h - 60 {
            if x <= 60 {
                self.current_view = self.current_view.prev();
                self.dirty = true;
                return true;
            } else if x >= screen_w - 60 {
                self.current_view = self.current_view.next();
                self.dirty = true;
                return true;
            }
        }

        // Forecast row tap to open hourly
        if self.current_view == View::Forecast && !self.forecast_hourly_open {
            let row_top = 52i16;
            let row_stride = 64i16;
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

        // Close hourly on header tap
        if self.current_view == View::Forecast && self.forecast_hourly_open
            && y < 42 && x >= screen_w - 144 {
                self.forecast_hourly_open = false;
                self.dirty = true;
                return true;
            }

        false
    }
}

/// Draw the current view into the framebuffer.
pub fn draw_current_view(fb: &mut Framebuffer, state: &AppState) {
    match state.current_view {
        View::Now => now::draw(fb, state),
        View::Indoor => indoor::draw(fb, state),
        View::Forecast => forecast::draw(fb, state),
        View::I2cScan => i2c_scan::draw(fb, state),
        View::WifiScan => wifi_scan::draw(fb, state),
        View::About => about::draw(fb, state),
    }
}
