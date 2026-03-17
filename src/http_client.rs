use anyhow::{bail, Result};
use esp_idf_svc::http::client::{Configuration, EspHttpConnection};
use embedded_svc::http::Method;
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

// Per-host persistent HTTP/TLS sessions.
// Reusing these avoids a full TLS handshake + DNS lookup on each weather fetch,
// which is the primary source of lwIP TCP/DNS allocations fragmenting internal
// SRAM over time.  On success each call stores the connection back; on error
// it is dropped and the next call opens a fresh one.
//
// NOTE: we call EspHttpConnection methods directly (initiate_request /
// initiate_response / read) rather than going through embedded_svc::Client<C>.
// Client::wrap() panics if is_response_initiated() is true, but
// EspHttpConnection::initiate_request() handles the Response state correctly
// (calls flush_response + esp_http_client_set_url).  Bypassing Client<C>
// avoids the spurious panic while still reusing the live TLS socket.

/// Newtype wrapper so EspHttpConnection can be stored in a static Mutex.
///
/// SAFETY: EspHttpConnection wraps an esp_http_client_handle_t (an opaque
/// ESP-IDF handle).  It is NOT safe to use from multiple threads concurrently,
/// but it IS safe to *transfer* between threads — which is all Send requires.
/// Access is serialised by the Mutex below; we never touch the handle from two
/// threads at the same time.
struct HttpConn(EspHttpConnection);
unsafe impl Send for HttpConn {}

static SESSION_OWM:  OnceLock<Mutex<Option<HttpConn>>> = OnceLock::new();
static SESSION_NWS:  OnceLock<Mutex<Option<HttpConn>>> = OnceLock::new();
static SESSION_MISC: OnceLock<Mutex<Option<HttpConn>>> = OnceLock::new();

fn get_session(url: &str) -> &'static Mutex<Option<HttpConn>> {
    if url.contains("openweathermap.org") {
        SESSION_OWM.get_or_init(|| Mutex::new(None))
    } else if url.contains("weather.gov") {
        SESSION_NWS.get_or_init(|| Mutex::new(None))
    } else {
        SESSION_MISC.get_or_init(|| Mutex::new(None))
    }
}

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

/// Inner fetch: drives the request/response cycle on a raw EspHttpConnection.
///
/// Calls initiate_request → initiate_response → read loop directly, bypassing
/// embedded_svc::Client<C>::wrap() which panics on reused (Response-state)
/// connections.  EspHttpConnection::initiate_request() handles the Response
/// state correctly by flushing the prior response before re-opening the socket.
///
/// On return the connection is in Response state and ready to be stored back
/// for the next reuse.  On error the connection should be dropped.
fn do_fetch(
    conn: &mut EspHttpConnection,
    url: &str,
    headers: &[(&str, &str)],
    buf: &mut PsramBuf,
) -> Result<()> {
    conn.initiate_request(Method::Get, url, headers)?;
    conn.initiate_response()?;

    let status = conn.status();
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
    loop {
        let n = conn.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        if !buf.extend_from_slice(&chunk[..n]) {
            bail!("Response too large (>{}B)", MAX_RESPONSE_SIZE);
        }
    }
    Ok(())
} // connection is in Response state here — initiate_request() will handle on next reuse

/// Do the actual HTTP GET, reusing a persistent TLS session when available.
///
/// On success the connection is stored back for the next call.
/// On failure the connection is dropped; the next call opens a fresh one.
///
/// Separated from the public API so the large stack frame
/// (EspHttpConnection, chunk[1024]) is fully popped before the
/// caller's parse callback runs.
fn http_fetch_into(url: &str, headers: &[(&str, &str)], buf: &mut PsramBuf) -> Result<()> {
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

    let session = get_session(url);
    let mut session_guard = session
        .lock()
        .map_err(|_| anyhow::anyhow!("HTTP session lock poisoned"))?;

    let is_new_connection = session_guard.is_none();

    // Hard bail on SRAM fragmentation only when we need a new TLS handshake.
    // If a connection is already open we can reuse it without touching SRAM.
    if is_new_connection && largest_block < 4_000 {
        bail!(
            "SRAM too fragmented for TLS ({} KB largest block)",
            largest_block / 1024
        );
    }

    let mut connection = match session_guard.take() {
        Some(HttpConn(c)) => {
            info!("HTTP: reusing persistent connection");
            c
        }
        None => {
            info!("HTTP: opening new TLS connection");
            EspHttpConnection::new(&make_config())?
        }
    };

    let fetch_result = do_fetch(&mut connection, url, headers, buf);

    if fetch_result.is_ok() {
        *session_guard = Some(HttpConn(connection));
        info!("HTTP: connection preserved for reuse");
    } else {
        info!("HTTP: connection dropped after error (will reconnect next call)");
        // connection is dropped here → esp_http_client_cleanup()
    }

    fetch_result
}

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
