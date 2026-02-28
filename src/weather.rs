use anyhow::Result;
use log::info;
use serde::Deserialize;

use crate::weather_icons::WeatherIcon;

const WEEKDAY_SHORT: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const FORECAST_MAX_DAYS: usize = 8;
const FORECAST_HOURLY_MAX: usize = 12;

// ── Data types ──────────────────────────────────────────────────────

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
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct HourlyEntry {
    pub temp_f: i32,
    pub feels_f: i32,
    pub wind_mph: i32,
    pub icon: WeatherIcon,
    pub time_text: String,
    pub detail: String,
    pub temp_text: String,
}

#[derive(Debug, Clone, Default)]
pub struct ForecastDay {
    pub entries: Vec<HourlyEntry>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct Forecast {
    pub rows: Vec<ForecastRow>,
    pub days: Vec<ForecastDay>,
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
        classify_alert_kind(&self.event)
    }
}

// ── OWM JSON structures ─────────────────────────────────────────────

#[derive(Deserialize)]
struct OwmCurrentRoot {
    main: Option<OwmMain>,
    weather: Option<Vec<OwmWeather>>,
    wind: Option<OwmWind>,
    name: Option<String>,
    sys: Option<OwmSys>,
}

#[derive(Deserialize)]
struct OwmMain {
    temp: Option<f64>,
    feels_like: Option<f64>,
    humidity: Option<i32>,
    pressure: Option<i32>,
}

#[derive(Deserialize)]
struct OwmWeather {
    id: Option<i32>,
    icon: Option<String>,
    description: Option<String>,
}

#[derive(Deserialize)]
struct OwmWind {
    speed: Option<f64>,
}

#[derive(Deserialize)]
struct OwmSys {
    country: Option<String>,
}

#[derive(Deserialize)]
struct OwmForecastRoot {
    list: Option<Vec<OwmForecastEntry>>,
    city: Option<OwmCity>,
}

#[derive(Deserialize)]
struct OwmForecastEntry {
    dt: Option<i64>,
    main: Option<OwmMain>,
    weather: Option<Vec<OwmWeather>>,
    wind: Option<OwmWind>,
}

#[derive(Deserialize)]
struct OwmCity {
    timezone: Option<i32>,
}

#[derive(Deserialize)]
struct NwsAlertsRoot {
    features: Option<Vec<NwsFeature>>,
}

#[derive(Deserialize)]
struct NwsFeature {
    id: Option<String>,
    properties: Option<NwsProperties>,
}

#[derive(Deserialize)]
struct NwsProperties {
    event: Option<String>,
    headline: Option<String>,
    expires: Option<String>,
    ends: Option<String>,
    description: Option<String>,
    instruction: Option<String>,
    severity: Option<String>,
    certainty: Option<String>,
    urgency: Option<String>,
}

#[derive(Deserialize)]
struct IpApiCo {
    latitude: Option<f64>,
    longitude: Option<f64>,
}

#[derive(Deserialize)]
struct NwsPointsRoot {
    properties: Option<NwsPointsProps>,
}

#[derive(Deserialize)]
struct NwsPointsProps {
    #[serde(rename = "forecastZone")]
    forecast_zone: Option<String>,
}

// ── Icon mapping (matches C factory exactly) ────────────────────────

pub fn map_condition_to_icon(weather_id: i32, _icon_code: &str) -> WeatherIcon {
    match weather_id {
        200..=299 => WeatherIcon::Thunderstorm,
        300..=399 => WeatherIcon::Drizzle,
        500..=504 => WeatherIcon::Rain,
        511 => WeatherIcon::Snow,
        520..=599 => WeatherIcon::ShowerRain,
        600..=699 => WeatherIcon::Snow,
        701 => WeatherIcon::Mist,
        711..=762 => WeatherIcon::Atmosphere,
        771..=799 => WeatherIcon::Fog,
        800 => WeatherIcon::Clear,
        801 => WeatherIcon::FewClouds,
        802 => WeatherIcon::ScatteredClouds,
        803 => WeatherIcon::BrokenClouds,
        804 => WeatherIcon::Overcast,
        _ => WeatherIcon::ScatteredClouds,
    }
}

fn condition_short(weather_id: i32) -> &'static str {
    match weather_id {
        200..=299 => "Storm",
        300..=399 => "Drizzle",
        500..=599 if weather_id == 511 => "Sleet",
        500..=599 => "Rain",
        600..=699 => "Snow",
        700..=799 if weather_id == 741 => "Fog",
        700..=799 => "Mist",
        800 => "Clear",
        801 => "Partly Cloudy",
        802 => "Cloudy",
        803..=804 => "Overcast",
        _ => "Cloudy",
    }
}

fn format_hour_label(hour24: i32) -> String {
    let hour12 = if hour24 % 12 == 0 { 12 } else { hour24 % 12 };
    let ampm = if hour24 >= 12 { "PM" } else { "AM" };
    format!("{}{}", hour12, ampm)
}

fn classify_alert_kind(event: &str) -> AlertKind {
    let e = event.to_ascii_lowercase();
    if e.contains("warning") {
        AlertKind::Warning
    } else if e.contains("watch") {
        AlertKind::Watch
    } else if e.contains("advisory") {
        AlertKind::Advisory
    } else {
        AlertKind::Other
    }
}

// ── Parsing ─────────────────────────────────────────────────────────

pub fn parse_current_weather(json: &str) -> Result<CurrentWeather> {
    let root: OwmCurrentRoot = serde_json::from_str(json)?;

    let main = root.main.unwrap_or(OwmMain {
        temp: None,
        feels_like: None,
        humidity: None,
        pressure: None,
    });
    let temp = main.temp.unwrap_or(0.0) as f32;
    let feels = main.feels_like.unwrap_or(temp as f64) as f32;
    let humidity = main.humidity.unwrap_or(-1);
    let pressure = main.pressure.unwrap_or(-1);
    let wind = root
        .wind
        .and_then(|w| w.speed)
        .unwrap_or(0.0) as f32;

    let (weather_id, icon_code, description) = root
        .weather
        .and_then(|arr| arr.into_iter().next())
        .map(|w| {
            (
                w.id.unwrap_or(0),
                w.icon.unwrap_or_default(),
                w.description.unwrap_or_else(|| "(unknown)".to_string()),
            )
        })
        .unwrap_or((0, String::new(), "(unknown)".to_string()));

    let icon = map_condition_to_icon(weather_id, &icon_code);
    let city = root.name.unwrap_or_else(|| "?".to_string());
    let country = root
        .sys
        .and_then(|s| s.country)
        .unwrap_or_default();

    info!(
        "weather: id={} icon={} desc={} mapped={:?}",
        weather_id, icon_code, description, icon
    );

    Ok(CurrentWeather {
        temp_f: temp,
        feels_f: feels,
        wind_mph: wind,
        humidity,
        pressure_hpa: pressure,
        icon,
        city,
        country,
        condition: description,
    })
}

pub fn parse_forecast(json: &str) -> Result<Forecast> {
    let root: OwmForecastRoot = serde_json::from_str(json)?;
    let list = root.list.unwrap_or_default();
    let tz_offset = root.city.and_then(|c| c.timezone).unwrap_or(0);

    struct DaySummary {
        yday: i32,
        year: i32,
        wday: i32,
        high_f: f32,
        low_f: f32,
        wind_peak: f32,
        icon: WeatherIcon,
        condition: String,
        icon_score: i32,
        hourly: Vec<HourlyEntry>,
    }

    let mut days: Vec<DaySummary> = Vec::new();
    let mut first_hour: Option<i32> = None;

    for entry in &list {
        let dt = match entry.dt {
            Some(dt) => dt,
            None => continue,
        };
        let main = match &entry.main {
            Some(m) => m,
            None => continue,
        };
        let temp = main.temp.unwrap_or(0.0) as f32;
        let feels = main.feels_like.unwrap_or(temp as f64) as f32;
        let wind_speed = entry.wind.as_ref().and_then(|w| w.speed).unwrap_or(0.0) as f32;

        let local_epoch = dt + tz_offset as i64;
        let mut tm: libc::tm = unsafe { core::mem::zeroed() };
        unsafe {
            libc::gmtime_r(&local_epoch as *const i64 as *const libc::time_t, &mut tm);
        }

        if first_hour.is_none() {
            first_hour = Some(tm.tm_hour);
        }

        let idx = days
            .iter()
            .position(|d| d.year == tm.tm_year && d.yday == tm.tm_yday);

        let idx = match idx {
            Some(i) => i,
            None => {
                if days.len() >= FORECAST_MAX_DAYS {
                    continue;
                }
                days.push(DaySummary {
                    yday: tm.tm_yday,
                    year: tm.tm_year,
                    wday: tm.tm_wday,
                    high_f: temp,
                    low_f: temp,
                    wind_peak: wind_speed,
                    icon: WeatherIcon::ScatteredClouds,
                    condition: "Cloudy".to_string(),
                    icon_score: -1,
                    hourly: Vec::new(),
                });
                days.len() - 1
            }
        };

        let day = &mut days[idx];
        if temp > day.high_f {
            day.high_f = temp;
        }
        if temp < day.low_f {
            day.low_f = temp;
        }
        if wind_speed > day.wind_peak {
            day.wind_peak = wind_speed;
        }

        let (weather_id, icon_code) = entry
            .weather
            .as_ref()
            .and_then(|arr| arr.first())
            .map(|w| (w.id.unwrap_or(0), w.icon.clone().unwrap_or_default()))
            .unwrap_or((0, String::new()));

        let mapped = map_condition_to_icon(weather_id, &icon_code);

        let score = match tm.tm_hour {
            12 => 3,
            9 | 15 => 2,
            _ => 1,
        };
        if score > day.icon_score {
            day.icon = mapped;
            day.condition = condition_short(weather_id).to_string();
            day.icon_score = score;
        }

        if day.hourly.len() < FORECAST_HOURLY_MAX {
            let temp_i = temp.round() as i32;
            let feels_i = feels.round() as i32;
            let wind_i = wind_speed.round() as i32;
            day.hourly.push(HourlyEntry {
                temp_f: temp_i,
                feels_f: feels_i,
                wind_mph: wind_i,
                icon: mapped,
                time_text: format_hour_label(tm.tm_hour),
                detail: format!("Feels {}° Wind {}", feels_i, wind_i),
                temp_text: format!("{}°", temp_i),
            });
        }
    }

    // Skip first partial day if forecast doesn't start at midnight
    let start_day = if days.len() > 1 && first_hour.unwrap_or(0) > 0 {
        1
    } else {
        0
    };

    let available = &days[start_day..];
    let row_count = available.len().min(4);

    let mut rows = Vec::with_capacity(row_count);
    let mut forecast_days = Vec::with_capacity(row_count);

    for day in &available[..row_count] {
        let high_i = day.high_f.round() as i32;
        let low_i = day.low_f.round() as i32;
        let wind_i = day.wind_peak.round() as i32;
        let wday_name = WEEKDAY_SHORT[day.wday as usize % 7];

        rows.push(ForecastRow {
            temp_f: high_i,
            low_f: low_i,
            wind_mph: wind_i,
            icon: day.icon,
            title: wday_name.to_string(),
            detail: format!("{} Low {}° Wind {}", day.condition, low_i, wind_i),
            temp_text: format!("{}°", high_i),
        });

        forecast_days.push(ForecastDay {
            entries: day.hourly.clone(),
        });
    }

    // Build preview text
    let preview_count = row_count.min(3);
    let preview_text = available[..preview_count]
        .iter()
        .map(|d| {
            format!(
                "{} {}°",
                WEEKDAY_SHORT[d.wday as usize % 7],
                d.high_f.round() as i32
            )
        })
        .collect::<Vec<_>>()
        .join("   ");

    Ok(Forecast {
        rows,
        days: forecast_days,
        preview_text,
    })
}

/// Fetch current weather + forecast from OpenWeatherMap.
pub fn fetch_weather(
    query: &str,
    api_key: &str,
) -> Result<(CurrentWeather, Forecast)> {
    let weather_url = format!(
        "https://api.openweathermap.org/data/2.5/weather?{}&units=imperial&appid={}",
        query, api_key
    );
    let forecast_url = format!(
        "https://api.openweathermap.org/data/2.5/forecast?{}&units=imperial&cnt=32&appid={}",
        query, api_key
    );

    info!("Fetching current weather...");
    let weather_json = crate::http_client::https_get(&weather_url)?;
    let current = parse_current_weather(&weather_json)?;

    info!("Fetching forecast...");
    let forecast_json = crate::http_client::https_get(&forecast_url)?;
    let forecast = parse_forecast(&forecast_json)?;

    Ok((current, forecast))
}

/// Convert an ISO-8601 timestamp like "2026-02-27T08:30:00-06:00" to "Feb 27 8:30 AM".
pub fn format_alert_expiry(iso: &str) -> String {
    const MONTHS: [&str; 12] = ["Jan","Feb","Mar","Apr","May","Jun",
                                 "Jul","Aug","Sep","Oct","Nov","Dec"];
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
    let month_str = if month >= 1 && month <= 12 { MONTHS[month - 1] } else { "???" };
    let (h12, ampm) = if hour == 0 { (12u32, "AM") }
        else if hour < 12 { (hour, "AM") }
        else if hour == 12 { (12, "PM") }
        else { (hour - 12, "PM") };
    format!("{} {} {}:{:02} {}", month_str, day, h12, min, ampm)
}

/// Dump a single alert's full text to the serial console at WARN level.
pub fn log_alert_to_console(alert: &WeatherAlert) {
    log::warn!("=== NWS ALERT: {} ===", alert.event);
    log::warn!("  Severity: {}  Urgency: {}  Certainty: {}", alert.severity, alert.urgency, alert.certainty);
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

pub fn parse_nws_alerts(json: &str) -> Result<Vec<WeatherAlert>> {
    let root: NwsAlertsRoot = serde_json::from_str(json)?;
    let mut out = Vec::new();
    for feature in root.features.unwrap_or_default() {
        let props = match feature.properties {
            Some(p) => p,
            None => continue,
        };
        let event = props.event.unwrap_or_else(|| "Unknown Alert".to_string());
        let headline = props.headline.unwrap_or_else(|| event.clone());
        let expires = props.expires.or(props.ends).unwrap_or_else(|| "unknown".to_string());
        out.push(WeatherAlert {
            id: feature.id.unwrap_or_default(),
            event,
            headline,
            expires,
            description: props.description.unwrap_or_default(),
            instruction: props.instruction.unwrap_or_default(),
            severity: props.severity.unwrap_or_default(),
            certainty: props.certainty.unwrap_or_default(),
            urgency: props.urgency.unwrap_or_default(),
        });
    }

    // Put warnings first, then watches, then advisories.
    out.sort_by_key(|a| match a.kind() {
        AlertKind::Warning => 0,
        AlertKind::Watch => 1,
        AlertKind::Advisory => 2,
        AlertKind::Other => 3,
    });
    Ok(out)
}

pub fn fetch_nws_alerts(scope: &str, user_agent: &str) -> Result<Vec<WeatherAlert>> {
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
    let json = crate::http_client::https_get_with_headers(&url, &headers)?;
    parse_nws_alerts(&json)
}

fn discover_geo_coords(user_agent: &str) -> Result<(f64, f64)> {
    let geo_headers = [
        ("User-Agent", user_agent),
        ("Accept", "application/json"),
    ];
    let geo_json =
        crate::http_client::https_get_with_headers("https://ipapi.co/json/", &geo_headers)?;
    let geo: IpApiCo = serde_json::from_str(&geo_json)?;
    let lat = geo
        .latitude
        .ok_or_else(|| anyhow::anyhow!("geolocation missing latitude"))?;
    let lon = geo
        .longitude
        .ok_or_else(|| anyhow::anyhow!("geolocation missing longitude"))?;
    Ok((lat, lon))
}

pub fn discover_openweather_query(user_agent: &str) -> Result<String> {
    let (lat, lon) = discover_geo_coords(user_agent)?;
    Ok(format!("lat={:.4}&lon={:.4}", lat, lon))
}

pub fn discover_nws_zone(user_agent: &str) -> Result<String> {
    let (lat, lon) = discover_geo_coords(user_agent)?;

    let points_url = format!("https://api.weather.gov/points/{:.4},{:.4}", lat, lon);
    let nws_headers = [
        ("User-Agent", user_agent),
        ("Accept", "application/geo+json"),
    ];
    let points_json = crate::http_client::https_get_with_headers(&points_url, &nws_headers)?;
    let points: NwsPointsRoot = serde_json::from_str(&points_json)?;
    let forecast_zone = points
        .properties
        .and_then(|p| p.forecast_zone)
        .ok_or_else(|| anyhow::anyhow!("points response missing forecastZone"))?;

    let zone = forecast_zone
        .rsplit('/')
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    if zone.is_empty() {
        anyhow::bail!("unable to extract forecast zone");
    }
    Ok(zone)
}
