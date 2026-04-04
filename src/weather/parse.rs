/// OWM + NWS JSON deserialization structs and all parse functions.
/// Private to the weather module — only called via fetch_weather / fetch_nws_alerts.
use anyhow::Result;
use log::info;
use serde::Deserialize;

use super::{AlertKind, CurrentWeather, Forecast, ForecastRow, WeatherAlert};
use crate::weather_icons::WeatherIcon;

const WEEKDAY_SHORT: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const FORECAST_MAX_DAYS: usize = 8;

// ── OWM JSON structures ──────────────────────────────────────────────

#[derive(Deserialize)]
pub(super) struct OwmCurrentRoot {
    pub main: Option<OwmMain>,
    pub weather: Option<Vec<OwmWeather>>,
    pub wind: Option<OwmWind>,
    pub name: Option<String>,
    pub sys: Option<OwmSys>,
}

#[derive(Deserialize)]
pub(super) struct OwmMain {
    pub temp: Option<f64>,
    pub feels_like: Option<f64>,
    pub humidity: Option<i32>,
    pub pressure: Option<i32>,
}

#[derive(Deserialize)]
pub(super) struct OwmWeather {
    pub id: Option<i32>,
    pub icon: Option<String>,
    pub description: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct OwmWind {
    pub speed: Option<f64>,
}

#[derive(Deserialize)]
pub(super) struct OwmSys {
    pub country: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct OwmForecastRoot {
    pub list: Option<Vec<OwmForecastEntry>>,
    pub city: Option<OwmCity>,
}

#[derive(Deserialize)]
pub(super) struct OwmForecastEntry {
    pub dt: Option<i64>,
    pub main: Option<OwmMain>,
    pub weather: Option<Vec<OwmWeather>>,
    pub wind: Option<OwmWind>,
}

#[derive(Deserialize)]
pub(super) struct OwmCity {
    pub timezone: Option<i32>,
}

// ── NWS alert JSON structures ────────────────────────────────────────

#[derive(Deserialize)]
pub(super) struct NwsAlertsRoot {
    pub features: Option<Vec<NwsFeature>>,
}

#[derive(Deserialize)]
pub(super) struct NwsFeature {
    pub id: Option<String>,
    pub properties: Option<NwsProperties>,
}

#[derive(Deserialize)]
pub(super) struct NwsProperties {
    pub event: Option<String>,
    pub headline: Option<String>,
    pub expires: Option<String>,
    pub ends: Option<String>,
    pub description: Option<String>,
    pub instruction: Option<String>,
    pub severity: Option<String>,
    pub certainty: Option<String>,
    pub urgency: Option<String>,
}

// ── Icon / condition helpers ─────────────────────────────────────────

pub(super) fn map_condition_to_icon(weather_id: i32, _icon_code: &str) -> WeatherIcon {
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

pub(super) fn condition_short(weather_id: i32) -> &'static str {
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

pub(super) fn classify_alert_kind(event: &str) -> AlertKind {
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

// ── Parse functions ──────────────────────────────────────────────────

pub(super) fn parse_current_weather(json: &str) -> Result<CurrentWeather> {
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
    let wind = root.wind.and_then(|w| w.speed).unwrap_or(0.0) as f32;

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
    let country = root.sys.and_then(|s| s.country).unwrap_or_default();

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

pub(super) fn parse_forecast(json: &str) -> Result<Forecast> {
    let root: OwmForecastRoot = serde_json::from_str(json)?;
    // serde_json on 32KB JSON is CPU-intensive; yield before the loop so IDLE1 feeds WDT.
    unsafe { esp_idf_sys::vTaskDelay(1) };
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
        entry_count: usize,
    }

    let mut days: Vec<DaySummary> = Vec::new();
    let mut first_hour: Option<i32> = None;

    for (i, entry) in list.iter().enumerate() {
        if i % 8 == 0 {
            unsafe { esp_idf_sys::vTaskDelay(1) };
        }
        let dt = match entry.dt {
            Some(dt) => dt,
            None => continue,
        };
        let main = match &entry.main {
            Some(m) => m,
            None => continue,
        };
        let temp = main.temp.unwrap_or(0.0) as f32;
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
                    entry_count: 0,
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
        day.entry_count += 1;
    }

    let _ = first_hour;
    let start_day = if days.len() > 1 && days[0].entry_count < 4 {
        1
    } else {
        0
    };
    let available = &days[start_day..];
    let row_count = available.len().min(4);
    let mut rows = Vec::with_capacity(row_count);

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
            condition: day.condition.clone(),
        });
    }

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

    Ok(Forecast { rows, preview_text })
}

pub(super) fn parse_nws_alerts(json: &str) -> Result<Vec<WeatherAlert>> {
    let root: NwsAlertsRoot = serde_json::from_str(json)?;
    let mut out = Vec::new();
    for feature in root.features.unwrap_or_default() {
        let props = match feature.properties {
            Some(p) => p,
            None => continue,
        };
        let event = props.event.unwrap_or_else(|| "Unknown Alert".to_string());
        let headline = props.headline.unwrap_or_else(|| event.clone());
        let expires = props
            .expires
            .or(props.ends)
            .unwrap_or_else(|| "unknown".to_string());
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
    out.sort_by_key(|a| match a.kind() {
        AlertKind::Warning => 0,
        AlertKind::Watch => 1,
        AlertKind::Advisory => 2,
        AlertKind::Other => 3,
    });
    Ok(out)
}
