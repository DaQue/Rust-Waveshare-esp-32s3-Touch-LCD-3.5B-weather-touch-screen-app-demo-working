mod geo;
mod parse;

use anyhow::Result;
use log::info;

use crate::speaker;
use crate::weather_icons::WeatherIcon;

// ── Public data types ────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct CurrentWeather {
    pub temp_f: f32,
    pub feels_f: f32,
    pub wind_mph: f32,
    pub humidity: i32,
    pub pressure_hpa: i32,
    pub icon: WeatherIcon,
    pub city: String,
    pub country: String,
    pub condition: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct ForecastRow {
    pub temp_f: i32,
    pub low_f: i32,
    pub wind_mph: i32,
    pub icon: WeatherIcon,
    pub title: String,
    pub detail: String,
    pub temp_text: String,
    pub condition: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct Forecast {
    pub rows: Vec<ForecastRow>,
    pub preview_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertKind {
    Warning,
    Watch,
    Advisory,
    Other,
}

impl AlertKind {
    pub fn as_str(self) -> &'static str {
        match self {
            AlertKind::Warning => "Warning",
            AlertKind::Watch => "Watch",
            AlertKind::Advisory => "Advisory",
            AlertKind::Other => "Alert",
        }
    }
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct WeatherAlert {
    pub id: String,
    pub event: String,
    pub headline: String,
    pub expires: String,
    pub description: String,
    pub instruction: String,
    pub severity: String,
    pub certainty: String,
    pub urgency: String,
}

impl WeatherAlert {
    pub fn kind(&self) -> AlertKind {
        parse::classify_alert_kind(&self.event)
    }
}

// ── Fetch functions ──────────────────────────────────────────────────

/// Fetch current weather + forecast from OpenWeatherMap.
pub(crate) fn fetch_weather(query: &str, api_key: &str) -> Result<(CurrentWeather, Forecast)> {
    let weather_url = format!(
        "https://api.openweathermap.org/data/2.5/weather?{}&units=imperial&appid={}",
        query, api_key
    );
    let forecast_url = format!(
        "https://api.openweathermap.org/data/2.5/forecast?{}&units=imperial&cnt=32&appid={}",
        query, api_key
    );

    info!("Fetching current weather...");
    let current =
        crate::http_client::https_get_json(&weather_url, &[], parse::parse_current_weather)?;

    // 1s gap lets mbedTLS fully release heap before the next TLS handshake.
    std::thread::sleep(std::time::Duration::from_secs(1));

    info!("Fetching forecast...");
    let forecast = crate::http_client::https_get_json(&forecast_url, &[], parse::parse_forecast)?;

    Ok((current, forecast))
}

pub(crate) fn fetch_nws_alerts(scope: &str, user_agent: &str) -> Result<Vec<WeatherAlert>> {
    let mut scope = scope.trim().to_string();
    if let Some(rest) = scope.strip_prefix("state=") {
        scope = format!("area={}", rest);
    }
    let url = if scope.is_empty() {
        "https://api.weather.gov/alerts/active".to_string()
    } else {
        format!("https://api.weather.gov/alerts/active?{}", scope)
    };
    let headers = [
        ("User-Agent", user_agent),
        ("Accept", "application/geo+json"),
    ];
    info!("Fetching NWS alerts...");
    crate::http_client::https_get_json(&url, &headers, parse::parse_nws_alerts)
}

// ── Geo discovery (delegates to geo submodule) ───────────────────────

pub(crate) fn discover_openweather_query(user_agent: &str) -> Result<String> {
    geo::discover_openweather_query(user_agent)
}

pub(crate) fn discover_nws_zone(user_agent: &str) -> Result<String> {
    geo::discover_nws_zone(user_agent)
}

// ── Utility helpers ──────────────────────────────────────────────────

/// Convert an ISO-8601 timestamp like "2026-02-27T08:30:00-06:00" to "Feb 27 8:30 AM".
pub(crate) fn format_alert_expiry(iso: &str) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let t = match iso.find('T') {
        Some(i) => i,
        None => return iso.chars().take(20).collect(),
    };
    let date = &iso[..t];
    let rest = &iso[t + 1..];
    if date.len() < 10 || rest.len() < 5 {
        return iso.chars().take(20).collect();
    }
    let month: usize = date[5..7].parse().unwrap_or(0);
    let day: u32 = date[8..10].parse().unwrap_or(0);
    let hour: u32 = rest[..2].parse().unwrap_or(0);
    let min: u32 = rest[3..5].parse().unwrap_or(0);
    let month_str = if (1..=12).contains(&month) {
        MONTHS[month - 1]
    } else {
        "???"
    };
    let (h12, ampm) = if hour == 0 {
        (12u32, "AM")
    } else if hour < 12 {
        (hour, "AM")
    } else if hour == 12 {
        (12, "PM")
    } else {
        (hour - 12, "PM")
    };
    format!("{} {} {}:{:02} {}", month_str, day, h12, min, ampm)
}

/// Dump a single alert's full text to the serial console at WARN level.
pub(crate) fn log_alert_to_console(alert: &WeatherAlert) {
    log::warn!("=== NWS ALERT: {} ===", alert.event);
    log::warn!(
        "  Severity: {}  Urgency: {}  Certainty: {}",
        alert.severity,
        alert.urgency,
        alert.certainty
    );
    log::warn!("  Headline: {}", alert.headline);
    log::warn!("  Expires:  {}", alert.expires);
    if !alert.description.is_empty() {
        log::warn!("  --- Description ---");
        for line in alert.description.lines() {
            let line = line.trim();
            if !line.is_empty() {
                log::warn!("  {}", line);
            }
        }
    }
    if !alert.instruction.is_empty() {
        log::warn!("  --- Instruction ---");
        for line in alert.instruction.lines() {
            let line = line.trim();
            if !line.is_empty() {
                log::warn!("  {}", line);
            }
        }
    }
    log::warn!("=== END ALERT ===");
}

// ── Alert helpers (used by main loop) ────────────────────────────────

/// Stable fingerprint of the current alert set — changes when alerts change.
pub fn alert_fingerprint(alerts: &[WeatherAlert]) -> String {
    let mut parts: Vec<String> = alerts
        .iter()
        .map(|a| format!("{}|{}|{}|{}", a.id, a.event, a.expires, a.severity))
        .collect();
    parts.sort_unstable();
    parts.join("||")
}

/// Choose the appropriate beep tone based on the highest-severity alert.
pub fn alert_tone_for(alerts: &[WeatherAlert]) -> speaker::AlertTone {
    let mut rank = 0u8;
    for alert in alerts {
        let severity = alert.severity.to_ascii_lowercase();
        let local_rank = if severity.contains("extreme")
            || severity.contains("severe")
            || alert.kind() == AlertKind::Warning
        {
            3
        } else if severity.contains("moderate") || alert.kind() == AlertKind::Watch {
            2
        } else {
            1
        };
        if local_rank > rank {
            rank = local_rank;
        }
    }
    match rank {
        3 => speaker::AlertTone::Warning,
        2 => speaker::AlertTone::Watch,
        _ => speaker::AlertTone::Advisory,
    }
}
