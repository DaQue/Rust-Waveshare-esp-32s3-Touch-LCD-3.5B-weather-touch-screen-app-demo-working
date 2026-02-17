use anyhow::{bail, Result};
use esp_idf_svc::http::client::{Configuration, EspHttpConnection};
use log::info;

const TIMEOUT_MS: u64 = 15_000;

/// Perform an HTTPS GET request and return the response body as a String.
pub fn https_get(url: &str) -> Result<String> {
    let config = Configuration {
        timeout: Some(std::time::Duration::from_millis(TIMEOUT_MS)),
        use_global_ca_store: true,
        crt_bundle_attach: Some(esp_idf_sys::esp_crt_bundle_attach),
        ..Default::default()
    };

    let connection = EspHttpConnection::new(&config)?;

    use embedded_svc::http::client::Client;
    let mut client = Client::wrap(connection);

    let request = client.get(url)?.submit()?;

    let status = request.status();
    info!("HTTP GET {} -> status {}", url.chars().take(80).collect::<String>(), status);

    if status == 429 {
        bail!("API rate limited (HTTP 429)");
    }
    if status != 200 {
        bail!("HTTP error: status {}", status);
    }

    let mut body = Vec::with_capacity(8192);
    let mut buf = [0u8; 1024];
    let mut reader = request;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&buf[..n]);
        if body.len() > 32768 {
            bail!("Response too large (>32KB)");
        }
    }

    let text = String::from_utf8(body)?;
    if !text.trim_start().starts_with('{') {
        bail!("Response is not JSON");
    }

    Ok(text)
}
