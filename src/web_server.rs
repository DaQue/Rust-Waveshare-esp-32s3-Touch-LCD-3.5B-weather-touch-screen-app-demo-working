use embedded_svc::http::Method;
use esp_idf_svc::http::server::{Configuration as HttpConfig, EspHttpServer};
use crate::SRAM_ADMIT_MIN_BLOCK;

/// Start the HTTP server on port 80. Returns the server handle — must be kept
/// alive for the lifetime of the program. Call only after WiFi is connected.
pub fn start() -> anyhow::Result<EspHttpServer<'static>> {
    let config = HttpConfig {
        stack_size: 16384,
        ..Default::default()
    };

    let mut server = EspHttpServer::new(&config)?;

    server.fn_handler("/", Method::Get, |req| -> Result<(), anyhow::Error> {
        let largest = unsafe {
            esp_idf_sys::heap_caps_get_largest_free_block(esp_idf_sys::MALLOC_CAP_INTERNAL)
        } as u32;

        if largest < SRAM_ADMIT_MIN_BLOCK {
            log::warn!("HTTP /: admission control — largest block {} KB < {} KB threshold",
                largest / 1024, SRAM_ADMIT_MIN_BLOCK / 1024);
            req.into_response(503, Some("Service Unavailable"), &[])?
                .write(b"503 Low memory\n")?;
        } else {
            req.into_ok_response()?
                .write(b"ESP32 OK\n")?;
        }
        Ok(())
    })?;

    log::info!("HTTP server started (port 80, task stack 16 KB)");
    Ok(server)
}
