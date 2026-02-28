use anyhow::Result;
use esp_idf_hal::modem::Modem;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::wifi::{
    AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi,
};
use log::info;

/// WiFi connection result.
pub struct WifiResult {
    pub wifi: Box<EspWifi<'static>>,
    pub connected: bool,
    pub ip_address: Option<String>,
}

/// Returns the connected IP address, or None if reconnect failed.
pub type ReconnectOutcome = Option<String>;

/// Log WiFi/AP state from ESP-IDF internals.
fn log_wifi_diag(label: &str) {
    unsafe {
        // Current WiFi mode
        let mut mode: esp_idf_sys::wifi_mode_t = 0;
        let mode_str = if esp_idf_sys::esp_wifi_get_mode(&mut mode) == esp_idf_sys::ESP_OK {
            match mode {
                x if x == esp_idf_sys::wifi_mode_t_WIFI_MODE_STA => "STA",
                x if x == esp_idf_sys::wifi_mode_t_WIFI_MODE_AP => "AP",
                x if x == esp_idf_sys::wifi_mode_t_WIFI_MODE_APSTA => "AP+STA",
                _ => "?",
            }
        } else {
            "err"
        };

        // Try to get AP info (RSSI, channel) if associated
        let mut ap_info: esp_idf_sys::wifi_ap_record_t = core::mem::zeroed();
        let ap_rc = esp_idf_sys::esp_wifi_sta_get_ap_info(&mut ap_info);
        if ap_rc == esp_idf_sys::ESP_OK {
            let ssid = core::str::from_utf8(&ap_info.ssid)
                .unwrap_or("?")
                .trim_end_matches('\0');
            info!(
                "WiFi [{}]: mode={} assoc=YES rssi={} ch={} ssid={}",
                label, mode_str, ap_info.rssi, ap_info.primary, ssid
            );
        } else {
            info!(
                "WiFi [{}]: mode={} assoc=NO (ap_info err={})",
                label, mode_str, ap_rc
            );
        }
    }
}

pub fn connect_wifi(
    modem: Modem,
    sysloop: EspSystemEventLoop,
    ssid: &str,
    password: &str,
) -> Result<WifiResult> {
    let mut esp_wifi = EspWifi::new(modem, sysloop.clone(), None)?;

    let auth = if password.is_empty() {
        AuthMethod::None
    } else {
        AuthMethod::WPA2Personal
    };

    let mut wifi_ssid = heapless::String::<32>::new();
    let mut wifi_pass = heapless::String::<64>::new();
    wifi_ssid.push_str(ssid).ok();
    wifi_pass.push_str(password).ok();

    esp_wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: wifi_ssid,
        password: wifi_pass,
        auth_method: auth,
        ..Default::default()
    }))?;

    let mut blocking_wifi = BlockingWifi::wrap(&mut esp_wifi, sysloop)?;

    blocking_wifi.start()?;
    info!("WiFi connecting to '{}'...", ssid);

    // Retry connect up to 5 times (matching C factory)
    let mut connected = false;
    for attempt in 1..=5 {
        let t0 = unsafe { esp_idf_sys::esp_timer_get_time() };
        match blocking_wifi.connect() {
            Ok(_) => {
                let elapsed_ms = (unsafe { esp_idf_sys::esp_timer_get_time() } - t0) / 1000;
                info!("WiFi connect OK on attempt {} ({}ms)", attempt, elapsed_ms);
                log_wifi_diag(&format!("attempt {} OK", attempt));
                connected = true;
                break;
            }
            Err(e) => {
                let elapsed_ms = (unsafe { esp_idf_sys::esp_timer_get_time() } - t0) / 1000;
                log::warn!(
                    "WiFi connect attempt {}/5 failed after {}ms: {}",
                    attempt, elapsed_ms, e
                );
                log_wifi_diag(&format!("attempt {} FAIL", attempt));

                if attempt < 5 {
                    // Full stop/start cycle to reset radio state
                    let _ = blocking_wifi.disconnect();
                    blocking_wifi.stop().ok();
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    blocking_wifi.start().ok();
                    std::thread::sleep(std::time::Duration::from_millis(300));
                }
            }
        }
    }
    let mut ip_address: Option<String> = None;
    if connected {
        info!("WiFi associated, waiting for IP address...");
        blocking_wifi.wait_netif_up()?;

        let ip_info = blocking_wifi.wifi().sta_netif().get_ip_info()?;
        info!("WiFi connected — IP: {}", ip_info.ip);
        ip_address = Some(ip_info.ip.to_string());
    } else {
        log::warn!("WiFi failed after 5 attempts; will retry later");
    }

    // Drop the BlockingWifi wrapper; the underlying EspWifi remains usable.
    drop(blocking_wifi);

    Ok(WifiResult {
        wifi: Box::new(esp_wifi),
        connected,
        ip_address,
    })
}

pub fn reconnect_existing(
    wifi: &mut EspWifi<'static>,
    sysloop: EspSystemEventLoop,
) -> Result<ReconnectOutcome> {
    let mut blocking_wifi = BlockingWifi::wrap(wifi, sysloop)?;
    let _ = blocking_wifi.start();

    let mut connected = false;
    for attempt in 1..=5 {
        let t0 = unsafe { esp_idf_sys::esp_timer_get_time() };
        match blocking_wifi.connect() {
            Ok(_) => {
                let elapsed_ms = (unsafe { esp_idf_sys::esp_timer_get_time() } - t0) / 1000;
                info!("WiFi reconnect OK on attempt {} ({}ms)", attempt, elapsed_ms);
                log_wifi_diag(&format!("reconnect {} OK", attempt));
                connected = true;
                break;
            }
            Err(e) => {
                let elapsed_ms = (unsafe { esp_idf_sys::esp_timer_get_time() } - t0) / 1000;
                log::warn!(
                    "WiFi reconnect attempt {}/5 failed after {}ms: {}",
                    attempt, elapsed_ms, e
                );
                log_wifi_diag(&format!("reconnect {} FAIL", attempt));
                if attempt < 5 {
                    let _ = blocking_wifi.disconnect();
                    blocking_wifi.stop().ok();
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    blocking_wifi.start().ok();
                    std::thread::sleep(std::time::Duration::from_millis(300));
                }
            }
        }
    }

    if !connected {
        return Ok(None);
    }

    blocking_wifi.wait_netif_up()?;
    let ip_info = blocking_wifi.wifi().sta_netif().get_ip_info()?;
    info!("WiFi reconnected — IP: {}", ip_info.ip);

    Ok(Some(ip_info.ip.to_string()))
}

/// Scan for nearby networks on an already-connected WiFi interface.
/// Returns a list of (ssid, rssi) pairs sorted by signal strength.
pub fn scan_wifi(
    wifi: &mut EspWifi<'static>,
    sysloop: EspSystemEventLoop,
) -> Vec<(String, i8)> {
    let mut blocking_wifi = match BlockingWifi::wrap(wifi, sysloop) {
        Ok(bw) => bw,
        Err(e) => {
            log::warn!("WiFi scan: failed to wrap: {}", e);
            return Vec::new();
        }
    };
    info!("WiFi scanning for nearby networks...");
    match blocking_wifi.scan() {
        Ok(aps) => {
            info!("WiFi scan found {} networks", aps.len());
            let mut results: Vec<(String, i8)> = aps.iter()
                .map(|ap| (ap.ssid.to_string(), ap.signal_strength))
                .collect();
            results.sort_by(|a, b| b.1.cmp(&a.1));
            results
        }
        Err(e) => {
            log::warn!("WiFi scan failed: {}", e);
            Vec::new()
        }
    }
}
