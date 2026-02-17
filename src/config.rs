use anyhow::Result;
use esp_idf_svc::nvs::{EspNvs, NvsDefault};
use log::info;

pub const NS: &str = "app_cfg";

const KEY_WIFI_SSID: &str = "wifi_ssid";
const KEY_WIFI_PASS: &str = "wifi_pass";
const KEY_WX_API_KEY: &str = "wx_api_key";
const KEY_WX_QUERY: &str = "wx_query";
const KEY_TIMEZONE: &str = "timezone";

const DEFAULT_WIFI_SSID: &str = "YOUR_WIFI_SSID";
const DEFAULT_WIFI_PASS: &str = "A7MZLB2nCuvUqIrpBQB";
const DEFAULT_WX_API_KEY: &str = "YOUR_API_KEY_HERE";
const DEFAULT_WEATHER_QUERY: &str = "zip=00000,US";
const DEFAULT_TIMEZONE: &str = "CST6CDT,M3.2.0,M11.1.0";

pub struct Config {
    pub wifi_ssid: String,
    pub wifi_pass: String,
    pub weather_api_key: String,
    pub weather_query: String,
    pub timezone: String,
}

/// Read a string from NVS, returning None if the key is absent or on error.
fn nvs_get_str(nvs: &EspNvs<NvsDefault>, key: &str) -> Option<String> {
    // First call with None to get the required buffer length.
    let len = match nvs.str_len(key) {
        Ok(Some(len)) => len,
        _ => return None,
    };

    let mut buf = vec![0u8; len];
    match nvs.get_str(key, &mut buf) {
        Ok(Some(val)) => {
            let s = val.trim_end_matches('\0').to_string();
            if s.is_empty() { None } else { Some(s) }
        }
        _ => None,
    }
}

impl Config {
    /// Load configuration from NVS, falling back to defaults for any missing
    /// keys.
    pub fn load(nvs: &EspNvs<NvsDefault>) -> Config {
        let wifi_ssid = nvs_get_str(nvs, KEY_WIFI_SSID)
            .unwrap_or_else(|| DEFAULT_WIFI_SSID.to_string());
        info!("NVS wifi_ssid = {:?}", wifi_ssid);

        let wifi_pass = nvs_get_str(nvs, KEY_WIFI_PASS)
            .unwrap_or_else(|| DEFAULT_WIFI_PASS.to_string());
        info!("NVS wifi_pass = <{} chars>", wifi_pass.len());

        let weather_api_key = nvs_get_str(nvs, KEY_WX_API_KEY)
            .unwrap_or_else(|| DEFAULT_WX_API_KEY.to_string());
        info!("NVS wx_api_key = <{} chars>", weather_api_key.len());

        let weather_query = nvs_get_str(nvs, KEY_WX_QUERY)
            .unwrap_or_else(|| DEFAULT_WEATHER_QUERY.to_string());
        info!("NVS wx_query = {:?}", weather_query);

        let timezone = nvs_get_str(nvs, KEY_TIMEZONE)
            .unwrap_or_else(|| DEFAULT_TIMEZONE.to_string());
        info!("NVS timezone = {:?}", timezone);

        Config {
            wifi_ssid,
            wifi_pass,
            weather_api_key,
            weather_query,
            timezone,
        }
    }

    pub fn save_wifi(nvs: &mut EspNvs<NvsDefault>, ssid: &str, pass: &str) -> Result<()> {
        nvs.set_str(KEY_WIFI_SSID, ssid)?;
        nvs.set_str(KEY_WIFI_PASS, pass)?;
        info!("NVS saved wifi_ssid={:?}", ssid);
        Ok(())
    }

    pub fn save_weather_api_key(nvs: &mut EspNvs<NvsDefault>, key: &str) -> Result<()> {
        nvs.set_str(KEY_WX_API_KEY, key)?;
        info!("NVS saved wx_api_key=<{} chars>", key.len());
        Ok(())
    }

    pub fn save_weather_query(nvs: &mut EspNvs<NvsDefault>, query: &str) -> Result<()> {
        nvs.set_str(KEY_WX_QUERY, query)?;
        info!("NVS saved wx_query={:?}", query);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn save_timezone(nvs: &mut EspNvs<NvsDefault>, tz: &str) -> Result<()> {
        nvs.set_str(KEY_TIMEZONE, tz)?;
        info!("NVS saved timezone={:?}", tz);
        Ok(())
    }
}
