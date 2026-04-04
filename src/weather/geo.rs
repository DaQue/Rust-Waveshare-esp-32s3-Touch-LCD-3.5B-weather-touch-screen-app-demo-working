/// IP-based geolocation and NWS zone discovery.
use anyhow::Result;
use serde::Deserialize;

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

fn discover_geo_coords(user_agent: &str) -> Result<(f64, f64)> {
    let geo_headers = [("User-Agent", user_agent), ("Accept", "application/json")];
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

pub(super) fn discover_openweather_query(user_agent: &str) -> Result<String> {
    let (lat, lon) = discover_geo_coords(user_agent)?;
    Ok(format!("lat={:.4}&lon={:.4}", lat, lon))
}

pub(super) fn discover_nws_zone(user_agent: &str) -> Result<String> {
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
