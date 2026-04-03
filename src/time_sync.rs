use anyhow::Result;
use esp_idf_svc::sntp::{EspSntp, OperatingMode, SntpConf, SyncMode, SyncStatus};
// TODO: upgrade ESP-IDF from v5.2.3 to v5.3.x+ to fix lwIP tcpip_thread_handle_msg
//       crash on periodic SNTP re-sync (EXCVADDR 0x1, ~22h uptime)
use log::info;
use std::thread;
use std::time::Duration;

const SNTP_SERVER: &str = "pool.ntp.org";
const SYNC_TIMEOUT_MS: u32 = 20_000;
const POLL_INTERVAL_MS: u32 = 250;

/// Start SNTP time synchronization with the given POSIX timezone string.
///
/// Sets the TZ environment variable, then creates an SNTP client that polls
/// pool.ntp.org. Waits up to 20 seconds for an initial sync before returning.
/// The returned EspSntp must be kept alive to maintain periodic re-sync.
pub(crate) fn sync_time(tz: &str) -> Result<EspSntp<'static>> {
    info!("Setting timezone: {}", tz);
    // Safety: single-threaded at this point during init
    unsafe {
        std::env::set_var("TZ", tz);
    }

    let conf = SntpConf {
        servers: [SNTP_SERVER, "time.nist.gov"],
        sync_mode: SyncMode::Immediate,
        operating_mode: OperatingMode::Poll,
    };

    info!("Starting SNTP sync with {}", SNTP_SERVER);
    let sntp = EspSntp::new(&conf)?;

    let mut elapsed_ms = 0u32;
    while elapsed_ms < SYNC_TIMEOUT_MS {
        if sntp.get_sync_status() == SyncStatus::Completed {
            info!("SNTP time synchronized after {}ms", elapsed_ms);
            if let Some(t) = format_local_time() {
                info!("Current local time: {}", t);
            }
            return Ok(sntp);
        }
        thread::sleep(Duration::from_millis(POLL_INTERVAL_MS as u64));
        elapsed_ms += POLL_INTERVAL_MS;
    }

    log::warn!(
        "SNTP sync not completed within {}s, continuing anyway (will sync in background)",
        SYNC_TIMEOUT_MS / 1000
    );
    Ok(sntp)
}

/// Format the current local time as "HH:MM", or None if the clock is not set.
pub(crate) fn format_local_time() -> Option<String> {
    let mut now: libc::time_t = 0;
    unsafe {
        libc::time(&mut now);
    }
    // If time is near epoch, clock probably hasn't been set yet
    if now < 1_000_000_000 {
        return None;
    }
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe {
        libc::localtime_r(&now, &mut tm);
    }
    let hour24 = tm.tm_hour;
    let (hour12, ampm) = if hour24 == 0 {
        (12, "AM")
    } else if hour24 < 12 {
        (hour24, "AM")
    } else if hour24 == 12 {
        (12, "PM")
    } else {
        (hour24 - 12, "PM")
    };
    Some(format!("{}:{:02} {}", hour12, tm.tm_min, ampm))
}
