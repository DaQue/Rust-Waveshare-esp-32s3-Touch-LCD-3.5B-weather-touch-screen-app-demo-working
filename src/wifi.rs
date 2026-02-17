use anyhow::{bail, Result};
use esp_idf_hal::modem::Modem;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::wifi::{
    AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi,
};
use log::info;

/// WiFi connection result including optional scan results.
pub struct WifiResult {
    pub wifi: Box<EspWifi<'static>>,
    pub networks: Vec<(String, i8)>,
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

    // Scan for networks before connecting
    info!("WiFi scanning...");
    let networks = match blocking_wifi.scan() {
        Ok(aps) => {
            info!("WiFi scan found {} networks", aps.len());
            aps.iter()
                .map(|ap| (ap.ssid.to_string(), ap.signal_strength))
                .collect::<Vec<_>>()
        }
        Err(e) => {
            log::warn!("WiFi scan failed: {}", e);
            Vec::new()
        }
    };

    info!("WiFi connecting to '{}'...", ssid);

    // Retry connect up to 5 times (matching C factory)
    let mut connected = false;
    for attempt in 1..=5 {
        match blocking_wifi.connect() {
            Ok(_) => {
                connected = true;
                break;
            }
            Err(e) => {
                log::warn!("WiFi connect attempt {}/5 failed: {}", attempt, e);
                if attempt < 5 {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            }
        }
    }
    if !connected {
        bail!("WiFi failed after 5 attempts");
    }
    info!("WiFi associated, waiting for IP address...");

    blocking_wifi.wait_netif_up()?;

    let ip_info = blocking_wifi.wifi().sta_netif().get_ip_info()?;
    info!("WiFi connected — IP: {}", ip_info.ip);

    // Drop the BlockingWifi wrapper; the underlying EspWifi remains usable.
    drop(blocking_wifi);

    Ok(WifiResult {
        wifi: Box::new(esp_wifi),
        networks,
    })
}
