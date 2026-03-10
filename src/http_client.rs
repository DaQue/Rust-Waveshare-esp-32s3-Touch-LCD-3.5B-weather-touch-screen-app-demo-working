use anyhow::{bail, Result};
use esp_idf_svc::http::client::{Configuration, EspHttpConnection};
use log::info;
use std::sync::{Mutex, OnceLock};
use std::sync::atomic::Ordering;

const TIMEOUT_MS: u64 = 15_000;
const MAX_RESPONSE_SIZE: usize = 32_768;

// Body buffer is allocated once from PSRAM and reused across all HTTP calls.
// PSRAM is in CAPS_ALLOC mode on this board — regular malloc() never uses it,
// so we must call heap_caps_malloc(MALLOC_CAP_SPIRAM) explicitly.  This keeps
// ~16KB of internal SRAM free for mbedTLS TLS contexts (~40KB each), which
// would otherwise fail to allocate after the body buffer fills internal SRAM.
static BODY_BUF: OnceLock<Mutex<PsramBuf>> = OnceLock::new();

fn body_buf() -> &'static Mutex<PsramBuf> {
    BODY_BUF.get_or_init(|| Mutex::new(PsramBuf::new()))
}

/// Fixed-size body buffer backed by PSRAM.  Pre-allocated at first use,
/// never freed, cleared and reused on each request.
struct PsramBuf {
    ptr: *mut u8,
    len: usize,
}

// SAFETY: PsramBuf is only accessed through the Mutex in BODY_BUF.
unsafe impl Send for PsramBuf {}

impl PsramBuf {
    fn new() -> Self {
        let ptr = unsafe {
            esp_idf_sys::heap_caps_malloc(
                MAX_RESPONSE_SIZE,
                esp_idf_sys::MALLOC_CAP_SPIRAM,
            ) as *mut u8
        };
        assert!(
            !ptr.is_null(),
            "PsramBuf: failed to allocate {}B from PSRAM",
            MAX_RESPONSE_SIZE
        );
        info!(
            "PsramBuf: allocated {}B HTTP body buffer from PSRAM",
            MAX_RESPONSE_SIZE
        );
        PsramBuf { ptr, len: 0 }
    }

    fn clear(&mut self) {
        self.len = 0;
    }

    fn extend_from_slice(&mut self, data: &[u8]) -> bool {
        if self.len + data.len() > MAX_RESPONSE_SIZE {
            return false;
        }
        unsafe {
            core::ptr::copy_nonoverlapping(data.as_ptr(), self.ptr.add(self.len), data.len());
        }
        self.len += data.len();
        true
    }

    fn as_slice(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.ptr, self.len) }
    }
}

fn make_config() -> Configuration {
    Configuration {
        timeout: Some(std::time::Duration::from_millis(TIMEOUT_MS)),
        use_global_ca_store: true,
        crt_bundle_attach: Some(esp_idf_sys::esp_crt_bundle_attach),
        // OWM forecast response headers exceed 512B (the default).  Without
        // this the HTTP client tries to realloc mid-request when SRAM is
        // nearly exhausted by the TLS context, causing a 2928-byte malloc
        // failure and a hard abort.  Pre-allocating 2048B before the TLS
        // handshake gives enough headroom for forecast headers to fit.
        buffer_size: Some(2048),
        ..Default::default()
    }
}

/// Do the actual HTTP GET and stream the body into `buf`.
///
/// Separated into its own function so that the large stack frame
/// (EspHttpConnection, Client, Response, chunk[1024]) is fully popped off
/// the stack before the caller invokes the parse callback.  In debug builds
/// inner scopes do NOT shrink the frame — only a function return does.
fn http_fetch_into(url: &str, headers: &[(&str, &str)], buf: &mut PsramBuf) -> Result<()> {
    use embedded_svc::http::client::Client;
    use embedded_svc::http::Method;

    let free_internal = unsafe {
        esp_idf_sys::heap_caps_get_free_size(esp_idf_sys::MALLOC_CAP_INTERNAL)
    };
    let largest_block = unsafe {
        esp_idf_sys::heap_caps_get_largest_free_block(esp_idf_sys::MALLOC_CAP_INTERNAL)
    };
    let uptime_secs = unsafe { esp_idf_sys::esp_timer_get_time() } / 1_000_000;
    let h = uptime_secs / 3600;
    let m = (uptime_secs % 3600) / 60;
    let s = uptime_secs % 60;
    info!(
        "HTTP fetch: internal SRAM free = {} KB, largest block = {} KB, uptime = {}h {}m {}s",
        free_internal / 1024,
        largest_block / 1024,
        h, m, s
    );
    // SRAM watch: graduated response to consecutive fetches below 12 KB.
    //   1–2 hits : warn, track streak
    //   3+ hits  : BME280 reset (drop driver object, re-init next interval)
    //   < 8 KB   : BME280 reset + proactive reboot (save history first)
    if largest_block < 12_000 {
        let streak = crate::SRAM_LOW_STREAK.fetch_add(1, Ordering::Relaxed) + 1;
        log::warn!(
            "SRAM < 12 KB ({} KB) — low SRAM streak {}",
            largest_block / 1024, streak
        );
        if streak >= 3 {
            log::error!(
                "SRAM low for {} consecutive fetches — signalling BME280 reset",
                streak
            );
            crate::SRAM_BME_RESET.store(true, Ordering::Relaxed);
        }
    } else {
        crate::SRAM_LOW_STREAK.store(0, Ordering::Relaxed);
    }

    // Proactive reboot if largest block is ≤ 7 KB — BME reset + save history first.
    if largest_block < 8_000 {
        log::warn!(
            "SRAM largest block ≤ 7 KB ({} KB) — signalling BME280 reset + proactive reboot",
            largest_block / 1024
        );
        crate::SRAM_BME_RESET.store(true, Ordering::Relaxed);
        crate::SRAM_DO_REBOOT.store(true, Ordering::Relaxed);
    }

    // Hard bail if essentially nothing is left contiguous.
    if largest_block < 4_000 {
        bail!(
            "SRAM too fragmented for TLS ({} KB largest block)",
            largest_block / 1024
        );
    }

    let connection = EspHttpConnection::new(&make_config())?;
    let mut client = Client::wrap(connection);
    let request = client.request(Method::Get, url, headers)?.submit()?;

    let status = request.status();
    info!(
        "HTTP GET {} -> status {}",
        url.chars().take(80).collect::<String>(),
        status
    );

    if status == 429 {
        bail!("API rate limited (HTTP 429)");
    }
    if status != 200 {
        bail!("HTTP error: status {}", status);
    }

    let mut chunk = [0u8; 1024];
    let mut reader = request;
    loop {
        let n = reader.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        if !buf.extend_from_slice(&chunk[..n]) {
            bail!("Response too large (>{}B)", MAX_RESPONSE_SIZE);
        }
    }
    Ok(())
} // connection, client, reader, chunk[1024] all freed here

/// Perform an HTTPS GET, read the entire body into a persistent PSRAM buffer
/// (never freed — only cleared and reused), then call `f` with a `&str` view
/// of the response.
///
/// All requests are serialised by the body-buf mutex.  The HTTP fetch is
/// delegated to `http_fetch_into` so its large stack frame is fully released
/// before the parse callback runs.
pub fn https_get_json<T, F>(url: &str, headers: &[(&str, &str)], f: F) -> Result<T>
where
    F: FnOnce(&str) -> Result<T>,
{
    let mut body = body_buf()
        .lock()
        .map_err(|_| anyhow::anyhow!("HTTP body-buf lock poisoned"))?;
    body.clear();

    http_fetch_into(url, headers, &mut body)?;

    let text = std::str::from_utf8(body.as_slice())
        .map_err(|e| anyhow::anyhow!("HTTP response not UTF-8: {}", e))?;
    if !text.trim_start().starts_with('{') {
        bail!("Response is not JSON");
    }

    f(text)
}

/// Perform an HTTPS GET request with custom headers and return body as String.
/// For large JSON responses prefer [`https_get_json`] to avoid heap pressure.
pub fn https_get_with_headers(url: &str, headers: &[(&str, &str)]) -> Result<String> {
    https_get_json(url, headers, |s| Ok(s.to_string()))
}
