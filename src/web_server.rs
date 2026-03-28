use std::sync::{Arc, Mutex};

use embedded_svc::http::Method;
use esp_idf_svc::http::server::{Configuration as HttpConfig, EspHttpServer};
use serde::Serialize;

use crate::history_ring::HistoryRing;
use crate::SRAM_ADMIT_MIN_BLOCK;

fn parse_hours_from_uri(uri: &str) -> u32 {
    uri.find("hours=")
        .and_then(|i| uri[i + 6..].split('&').next())
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(24)
}

/// Snapshot of current sensor + weather data served by /api/current.
/// Updated by the main loop every 5s; read by HTTP handler under a short lock.
#[derive(Serialize, Clone, Default)]
pub struct WebSnapshot {
    // Outdoor weather (None = not yet received)
    pub temp_f:       Option<f32>,
    pub feels_f:      Option<f32>,
    pub wind_mph:     Option<f32>,
    pub humidity:     Option<i32>,
    pub condition:    Option<String>,
    pub city:         Option<String>,
    // Indoor sensor
    pub indoor_temp_f:       Option<f32>,
    pub indoor_humidity_pct: Option<f32>,
    pub indoor_pressure_hpa: Option<f32>,
    // System
    pub uptime_s:   u32,
    pub firmware:   String,
    pub ip_address: String,
    // HVAC summary
    pub hvac_heat_mins:   u32,
    pub hvac_cool_mins:   u32,
    pub hvac_heat_cycles: u32,
    pub hvac_cool_cycles: u32,
    pub hvac_state:       u8,   // 0=Idle 1=Heating 2=Cooling
    // System memory
    pub free_heap_kb:     u32,  // PSRAM free (KB)
    pub sram_block_kb:    u32,  // Largest free internal SRAM block (KB)
    // Active weather alerts (first alert only)
    pub warning_active: bool,
    pub alert_count:    u32,
    pub alert_event:    String,
    pub alert_severity: String,
    pub alert_headline: String,
    pub alert_expires:  String,
}

#[derive(Serialize)]
struct AlertsSnap {
    count:          u32,
    warning_active: bool,
    event:          String,
    severity:       String,
    headline:       String,
    expires:        String,
}

const CORS_HEADERS: &[(&str, &str)] = &[
    ("Access-Control-Allow-Origin", "*"),
    ("Content-Type", "application/json"),
];

// History responses are streamed — tell proxies (Tailscale) to wait for
// connection close before completing delivery, preventing truncated JSON.
const HISTORY_HEADERS: &[(&str, &str)] = &[
    ("Access-Control-Allow-Origin", "*"),
    ("Content-Type", "application/json"),
    ("Connection", "close"),
];

/// Start the HTTP server on port 80. Returns the server handle — must be kept
/// alive for the lifetime of the program. Call only after WiFi is connected.
pub fn start(
    snapshot: Arc<Mutex<WebSnapshot>>,
    history: Arc<Mutex<HistoryRing>>,
) -> anyhow::Result<EspHttpServer<'static>> {
    let config = HttpConfig {
        stack_size: 16384,
        ..Default::default()
    };

    let mut server = EspHttpServer::new(&config)?;

    // GET / — embedded dashboard UI
    server.fn_handler("/", Method::Get, |req| -> Result<(), anyhow::Error> {
        const DASHBOARD: &[u8] = include_str!("assets/dashboard.html").as_bytes();
        const HTML_HEADERS: &[(&str, &str)] = &[
            ("Content-Type", "text/html; charset=utf-8"),
            ("Access-Control-Allow-Origin", "*"),
        ];

        let largest = unsafe {
            esp_idf_sys::heap_caps_get_largest_free_block(esp_idf_sys::MALLOC_CAP_INTERNAL)
        } as u32;

        if largest < SRAM_ADMIT_MIN_BLOCK {
            req.into_response(503, Some("Service Unavailable"), &[])?
                .write(b"503 Low memory\n")?;
            return Ok(());
        }

        let mut resp = req.into_response(200, Some("OK"), HTML_HEADERS)?;
        // Write in 4KB chunks with yields so the HTTP task doesn't hold
        // CPU 0 for the full file duration, keeping IDLE0 fed.
        for chunk in DASHBOARD.chunks(4096) {
            resp.write(chunk)?;
            unsafe { esp_idf_sys::vTaskDelay(1) };
        }
        Ok(())
    })?;

    // GET /favicon.ico — return 204 No Content to stop browser 404 spam
    server.fn_handler("/favicon.ico", Method::Get, |req| -> Result<(), anyhow::Error> {
        req.into_response(204, Some("No Content"), &[])?.write(b"")?;
        Ok(())
    })?;

    let snapshot_alerts = snapshot.clone();

    // GET /api/current — current sensor + weather snapshot as JSON
    server.fn_handler("/api/current", Method::Get, move |req| -> Result<(), anyhow::Error> {
        let largest = unsafe {
            esp_idf_sys::heap_caps_get_largest_free_block(esp_idf_sys::MALLOC_CAP_INTERNAL)
        } as u32;

        if largest < SRAM_ADMIT_MIN_BLOCK {
            log::warn!("HTTP /api/current: admission control — largest block {} KB < {} KB",
                largest / 1024, SRAM_ADMIT_MIN_BLOCK / 1024);
            req.into_response(503, Some("Service Unavailable"), &[])?
                .write(b"{\"error\":\"low memory\"}\n")?;
            return Ok(());
        }

        let data = snapshot.lock().unwrap().clone();
        let json = serde_json::to_string(&data)?;

        req.into_response(200, Some("OK"), CORS_HEADERS)?
            .write(json.as_bytes())?;
        Ok(())
    })?;

    // GET /api/history?hours=N — dashboard history ring as JSON array.
    // Streams samples oldest-first using a per-sample heapless stack buffer
    // so no large SRAM allocation occurs regardless of sample count.
    server.fn_handler("/api/history", Method::Get, move |req| -> Result<(), anyhow::Error> {
        let largest = unsafe {
            esp_idf_sys::heap_caps_get_largest_free_block(esp_idf_sys::MALLOC_CAP_INTERNAL)
        } as u32;

        if largest < SRAM_ADMIT_MIN_BLOCK {
            log::warn!("HTTP /api/history: admission control — largest block {} KB",
                largest / 1024);
            req.into_response(503, Some("Service Unavailable"), &[])?
                .write(b"{\"error\":\"low memory\"}\n")?;
            return Ok(());
        }

        let hours = parse_hours_from_uri(req.uri());
        let ring = history.lock().unwrap();
        let mut resp = req.into_response(200, Some("OK"), HISTORY_HEADERS)?;

        resp.write(b"[")?;
        let mut first = true;
        let mut count = 0u32;
        for s in ring.iter_recent(hours) {
            if !first { resp.write(b",")?; }
            first = false;
            let mut buf = heapless::String::<160>::new();
            {
                use core::fmt::Write as FmtWrite;
                let _ = write!(
                    buf,
                    "{{\"ts\":{},\"tf\":{:.1},\"h\":{:.1},\"p\":{:.2},\"hv\":{},\"otu\":{},\"op\":{}}}",
                    s.timestamp, s.temp_f, s.humidity_pct, s.pressure_hpa, s.hvac_state,
                    s.outdoor_temp_u8, s.outdoor_press_u16
                );
            }
            resp.write(buf.as_bytes())?;
            count += 1;
            // Yield every 5 samples — TCP send window is ~5760 bytes; at ~60
            // bytes/sample the buffer fills fast. Frequent yields let lwIP
            // drain the window between writes, preventing EAGAIN drops.
            if count.is_multiple_of(5) {
                unsafe { esp_idf_sys::vTaskDelay(1) };
            }
        }
        resp.write(b"]")?;
        Ok(())
    })?;

    // GET /api/alerts — active weather alert summary (count + first alert fields)
    server.fn_handler("/api/alerts", Method::Get, move |req| -> Result<(), anyhow::Error> {
        let largest = unsafe {
            esp_idf_sys::heap_caps_get_largest_free_block(esp_idf_sys::MALLOC_CAP_INTERNAL)
        } as u32;
        if largest < SRAM_ADMIT_MIN_BLOCK {
            req.into_response(503, Some("Service Unavailable"), &[])?
                .write(b"{\"error\":\"low memory\"}\n")?;
            return Ok(());
        }
        let snap = snapshot_alerts.lock().unwrap();
        let resp = AlertsSnap {
            count:          snap.alert_count,
            warning_active: snap.warning_active,
            event:          snap.alert_event.clone(),
            severity:       snap.alert_severity.clone(),
            headline:       snap.alert_headline.clone(),
            expires:        snap.alert_expires.clone(),
        };
        drop(snap);
        let json = serde_json::to_string(&resp)?;
        req.into_response(200, Some("OK"), CORS_HEADERS)?
            .write(json.as_bytes())?;
        Ok(())
    })?;

    // GET /api/silence — silence the active alert beep on the device
    server.fn_handler("/api/silence", Method::Get, |req| -> Result<(), anyhow::Error> {
        crate::debug_flags::request_beep_stop();
        crate::debug_flags::REQUEST_SILENCE_WARNING.store(
            true, std::sync::atomic::Ordering::Relaxed,
        );
        log::info!("HTTP /api/silence: alert silenced via web");
        req.into_response(200, Some("OK"), CORS_HEADERS)?
            .write(b"{\"ok\":true}")?;
        Ok(())
    })?;

    log::info!("HTTP server started (port 80, task stack 16 KB)");
    Ok(server)
}
