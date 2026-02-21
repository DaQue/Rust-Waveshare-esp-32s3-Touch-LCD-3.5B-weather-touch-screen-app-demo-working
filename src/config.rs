use anyhow::Result;
use esp_idf_svc::nvs::{EspNvs, NvsDefault};
use log::info;

pub const NS: &str = "app_cfg";

const KEY_WIFI_SSID: &str = "wifi_ssid";
const KEY_WIFI_PASS: &str = "wifi_pass";
const KEY_WX_API_KEY: &str = "wx_api_key";
const KEY_WX_QUERY: &str = "wx_query";
const KEY_TIMEZONE: &str = "timezone";
const KEY_USE_CELSIUS: &str = "use_celsius";
const KEY_ORIENTATION: &str = "orientation";
const KEY_ORIENTATION_FLIP: &str = "ori_flip";
const KEY_FLASH_TIME: &str = "flash_time";
const KEY_ALERTS_ENABLED: &str = "alerts_en";
const KEY_ALERTS_AUTO_SCOPE: &str = "al_auto";
const KEY_NWS_USER_AGENT: &str = "nws_ua";
const KEY_NWS_SCOPE: &str = "nws_scope";
const KEY_NWS_ZONE: &str = "nws_zone";

const DEFAULT_WIFI_SSID: &str = "YOUR_WIFI_SSID";
const DEFAULT_WIFI_PASS: &str = "A7MZLB2nCuvUqIrpBQB";
const DEFAULT_WX_API_KEY: &str = "YOUR_API_KEY_HERE";
const DEFAULT_WEATHER_QUERY: &str = "zip=00000,US";
const DEFAULT_TIMEZONE: &str = "CST6CDT,M3.2.0,M11.1.0";
const DEFAULT_FLASH_TIME: &str = "unknown";
const DEFAULT_NWS_USER_AGENT: &str = "waveshare_s3_3p/0.1 (contact: unset)";
const DEFAULT_NWS_SCOPE: &str = "area=MO";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrientationMode {
    Auto,
    Landscape,
    Portrait,
}

impl OrientationMode {
    pub fn as_str(self) -> &'static str {
        match self {
            OrientationMode::Auto => "auto",
            OrientationMode::Landscape => "landscape",
            OrientationMode::Portrait => "portrait",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(OrientationMode::Auto),
            "landscape" => Some(OrientationMode::Landscape),
            "portrait" => Some(OrientationMode::Portrait),
            _ => None,
        }
    }
}

pub struct Config {
    pub wifi_ssid: String,
    pub wifi_pass: String,
    pub weather_api_key: String,
    pub weather_query: String,
    pub timezone: String,
    pub use_celsius: bool,
    pub orientation_mode: OrientationMode,
    pub orientation_flip: bool,
    pub flash_time: String,
    pub alerts_enabled: bool,
    pub alerts_auto_scope: bool,
    pub nws_user_agent: String,
    pub nws_scope: String,
    pub nws_zone: String,
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

        let use_celsius = nvs.get_u8(KEY_USE_CELSIUS).unwrap_or(None).unwrap_or(0) != 0;
        info!("NVS use_celsius = {}", use_celsius);
        let orientation_mode = nvs_get_str(nvs, KEY_ORIENTATION)
            .as_deref()
            .and_then(OrientationMode::parse)
            .unwrap_or(OrientationMode::Auto);
        info!("NVS orientation = {}", orientation_mode.as_str());
        let orientation_flip = nvs
            .get_u8(KEY_ORIENTATION_FLIP)
            .unwrap_or(None)
            .unwrap_or(0)
            != 0;
        info!("NVS orientation_flip = {}", orientation_flip);
        let flash_time = nvs_get_str(nvs, KEY_FLASH_TIME)
            .unwrap_or_else(|| DEFAULT_FLASH_TIME.to_string());
        info!("NVS flash_time = {:?}", flash_time);
        let alerts_enabled = nvs
            .get_u8(KEY_ALERTS_ENABLED)
            .unwrap_or(None)
            .unwrap_or(0)
            != 0;
        info!("NVS alerts_enabled = {}", alerts_enabled);
        let alerts_auto_scope = nvs
            .get_u8(KEY_ALERTS_AUTO_SCOPE)
            .unwrap_or(None)
            .unwrap_or(0)
            != 0;
        info!("NVS alerts_auto_scope = {}", alerts_auto_scope);
        let nws_user_agent = nvs_get_str(nvs, KEY_NWS_USER_AGENT)
            .unwrap_or_else(|| DEFAULT_NWS_USER_AGENT.to_string());
        info!("NVS nws_user_agent = {:?}", nws_user_agent);
        let nws_scope = nvs_get_str(nvs, KEY_NWS_SCOPE)
            .unwrap_or_else(|| DEFAULT_NWS_SCOPE.to_string());
        info!("NVS nws_scope = {:?}", nws_scope);
        let nws_zone = nvs_get_str(nvs, KEY_NWS_ZONE).unwrap_or_default();
        info!("NVS nws_zone = {:?}", nws_zone);

        Config {
            wifi_ssid,
            wifi_pass,
            weather_api_key,
            weather_query,
            timezone,
            use_celsius,
            orientation_mode,
            orientation_flip,
            flash_time,
            alerts_enabled,
            alerts_auto_scope,
            nws_user_agent,
            nws_scope,
            nws_zone,
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

    pub fn save_use_celsius(nvs: &mut EspNvs<NvsDefault>, celsius: bool) -> Result<()> {
        nvs.set_u8(KEY_USE_CELSIUS, if celsius { 1 } else { 0 })?;
        info!("NVS saved use_celsius={}", celsius);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn save_timezone(nvs: &mut EspNvs<NvsDefault>, tz: &str) -> Result<()> {
        nvs.set_str(KEY_TIMEZONE, tz)?;
        info!("NVS saved timezone={:?}", tz);
        Ok(())
    }

    pub fn save_orientation_mode(
        nvs: &mut EspNvs<NvsDefault>,
        mode: OrientationMode,
    ) -> Result<()> {
        nvs.set_str(KEY_ORIENTATION, mode.as_str())?;
        info!("NVS saved orientation={}", mode.as_str());
        Ok(())
    }

    pub fn save_orientation_flip(nvs: &mut EspNvs<NvsDefault>, flip: bool) -> Result<()> {
        nvs.set_u8(KEY_ORIENTATION_FLIP, if flip { 1 } else { 0 })?;
        info!("NVS saved orientation_flip={}", flip);
        Ok(())
    }

    pub fn save_flash_time(nvs: &mut EspNvs<NvsDefault>, flash_time: &str) -> Result<()> {
        nvs.set_str(KEY_FLASH_TIME, flash_time)?;
        info!("NVS saved flash_time={:?}", flash_time);
        Ok(())
    }

    pub fn save_alerts_enabled(nvs: &mut EspNvs<NvsDefault>, enabled: bool) -> Result<()> {
        nvs.set_u8(KEY_ALERTS_ENABLED, if enabled { 1 } else { 0 })?;
        info!("NVS saved alerts_enabled={}", enabled);
        Ok(())
    }

    pub fn save_alerts_auto_scope(nvs: &mut EspNvs<NvsDefault>, enabled: bool) -> Result<()> {
        nvs.set_u8(KEY_ALERTS_AUTO_SCOPE, if enabled { 1 } else { 0 })?;
        info!("NVS saved alerts_auto_scope={}", enabled);
        Ok(())
    }

    pub fn save_nws_user_agent(nvs: &mut EspNvs<NvsDefault>, user_agent: &str) -> Result<()> {
        nvs.set_str(KEY_NWS_USER_AGENT, user_agent)?;
        info!("NVS saved nws_user_agent={:?}", user_agent);
        Ok(())
    }

    pub fn save_nws_scope(nvs: &mut EspNvs<NvsDefault>, scope: &str) -> Result<()> {
        nvs.set_str(KEY_NWS_SCOPE, scope)?;
        info!("NVS saved nws_scope={:?}", scope);
        Ok(())
    }

    pub fn save_nws_zone(nvs: &mut EspNvs<NvsDefault>, zone: &str) -> Result<()> {
        nvs.set_str(KEY_NWS_ZONE, zone)?;
        info!("NVS saved nws_zone={:?}", zone);
        Ok(())
    }
}
