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
        "https://api.openweathermap.org/data/2.5/forecast?{}&units=imperial&appid={}",
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
