use anyhow::{bail, Result};
use esp_idf_svc::http::client::{Configuration, EspHttpConnection};
use log::info;
use std::sync::{Mutex, OnceLock};

const TIMEOUT_MS: u64 = 15_000;
static HTTP_REQUEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Perform an HTTPS GET request and return the response body as a String.
pub fn https_get(url: &str) -> Result<String> {
    https_get_with_headers(url, &[])
}

/// Perform an HTTPS GET request with custom headers and return body as String.
pub fn https_get_with_headers(url: &str, headers: &[(&str, &str)]) -> Result<String> {
    // ESP32-S3 + TLS can hit transient resource pressure when multiple HTTPS
    // handshakes are attempted concurrently. Serialize requests across threads.
    let lock = HTTP_REQUEST_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock.lock().map_err(|_| anyhow::anyhow!("HTTP lock poisoned"))?;

    let config = Configuration {
        timeout: Some(std::time::Duration::from_millis(TIMEOUT_MS)),
        use_global_ca_store: true,
        crt_bundle_attach: Some(esp_idf_sys::esp_crt_bundle_attach),
        ..Default::default()
    };

    let connection = EspHttpConnection::new(&config)?;

    use embedded_svc::http::client::Client;
    use embedded_svc::http::Method;
    let mut client = Client::wrap(connection);

    let request = client.request(Method::Get, url, headers)?.submit()?;

    let status = request.status();
    info!("HTTP GET {} -> status {}", url.chars().take(80).collect::<String>(), status);

    if status == 429 {
        bail!("API rate limited (HTTP 429)");
    }
    if status != 200 {
        bail!("HTTP error: status {}", status);
    }

    let mut body: Vec<u8> = Vec::with_capacity(4096);
    let mut buf = [0u8; 1024];
    let mut reader = request;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        if body.len().saturating_add(n) > 32768 {
            bail!("Response too large (>32KB)");
        }
        body
            .try_reserve_exact(n)
            .map_err(|_| anyhow::anyhow!("Out of memory while reading HTTP response"))?;
        body.extend_from_slice(&buf[..n]);
    }

    let text = String::from_utf8(body)?;
    if !text.trim_start().starts_with('{') {
        bail!("Response is not JSON");
    }

    Ok(text)
}
