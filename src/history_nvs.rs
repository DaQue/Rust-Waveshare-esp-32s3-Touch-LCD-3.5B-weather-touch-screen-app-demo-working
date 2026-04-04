/// NVS persistence for sensor history and dashboard history rings.
///
/// ## Ping-pong two-slot scheme
///
/// Each save writes to the *inactive* slot, then bumps a generation counter.
/// The previous complete save is untouched until the new one finishes, so a
/// flash/reboot mid-write always leaves one valid copy to restore from.
///
/// `KEY_HIST_GEN` (u32, 4 bytes):
///   gen == 0  → no ping-pong save yet; restore tries slot-A keys (legacy)
///   gen  > 0  → last complete write was to slot (gen-1) % 2  (0=A, 1=B)
///
/// All serialisation buffers are allocated from PSRAM to avoid SRAM pressure.
use std::sync::{Arc, Mutex};

use esp_idf_svc::nvs::EspNvs;

use crate::{config, history_ring, pressure_history, psbox::PsramRing, views};

// ── helpers ──────────────────────────────────────────────────────────────────

fn nvs_read_gen(nvs: &Arc<Mutex<EspNvs<esp_idf_svc::nvs::NvsDefault>>>) -> u32 {
    let mut buf = [0u8; 4];
    if let Ok(guard) = nvs.lock() {
        if matches!(guard.get_raw(config::KEY_HIST_GEN, &mut buf), Ok(Some(_))) {
            return u32::from_le_bytes(buf);
        }
    }
    0
}

fn nvs_write_gen(nvs: &Arc<Mutex<EspNvs<esp_idf_svc::nvs::NvsDefault>>>, gen: u32) {
    let buf = gen.to_le_bytes();
    if let Ok(mut guard) = nvs.lock() {
        if let Err(e) = guard.set_raw(config::KEY_HIST_GEN, &buf) {
            log::warn!("NVS gen write failed: {:?}", e);
        }
    }
}

/// Returns the (indoor_key, press_key, dash_key) for the given slot (0=A, 1=B).
fn slot_keys(slot: u32) -> (&'static str, &'static str, &'static str) {
    if slot == 0 {
        (
            config::KEY_HIST_INDOOR,
            config::KEY_HIST_PRESS,
            config::KEY_HIST_DASHBOARD,
        )
    } else {
        (
            config::KEY_HIST_INDOOR_B,
            config::KEY_HIST_PRESS_B,
            config::KEY_HIST_DASH_B,
        )
    }
}

// ── indoor / pressure ────────────────────────────────────────────────────────

/// Serialise all sensor history rings and pressure rings to NVS blobs (ping-pong).
/// Returns `true` if both blobs were written successfully.
/// Does NOT bump the generation counter — callers should use [`history_nvs_save_all`].
pub fn history_nvs_save(
    state: &views::AppState,
    nvs: &Arc<Mutex<EspNvs<esp_idf_svc::nvs::NvsDefault>>>,
) -> bool {
    let gen = nvs_read_gen(nvs);
    let write_slot = gen % 2; // 0=A on first write, then alternates
    let (indoor_key, press_key, _) = slot_keys(write_slot);

    const F: usize = core::mem::size_of::<f32>();
    const N: usize = 480;
    // 4 arrays × (8 bytes length + N × 4 bytes f32)
    let total = 4 * (8 + N * F);
    let buf_ptr =
        unsafe { esp_idf_sys::heap_caps_malloc(total, esp_idf_sys::MALLOC_CAP_SPIRAM) as *mut u8 };
    if buf_ptr.is_null() {
        log::warn!(
            "history_nvs_save: PSRAM alloc failed ({}B), skipping",
            total
        );
        return false;
    }
    let buf = unsafe {
        core::ptr::write_bytes(buf_ptr, 0, total);
        core::slice::from_raw_parts_mut(buf_ptr, total)
    };

    fn write_deque(buf: &mut [u8], off: &mut usize, dq: &PsramRing, max: usize) {
        let len = dq.len().min(max);
        buf[*off..*off + 8].copy_from_slice(&(len as u64).to_le_bytes());
        *off += 8;
        let (s0, s1) = dq.as_slices();
        for &v in s0.iter().chain(s1.iter()).take(len) {
            buf[*off..*off + 4].copy_from_slice(&v.to_le_bytes());
            *off += 4;
        }
        for _ in len..max {
            buf[*off..*off + 4].copy_from_slice(&0f32.to_le_bytes());
            *off += 4;
        }
    }

    let mut off = 0usize;
    write_deque(buf, &mut off, &state.indoor_temp_history, N);
    write_deque(buf, &mut off, &state.indoor_hum_history, N);
    write_deque(buf, &mut off, &state.indoor_temp_hist_long, N);
    write_deque(buf, &mut off, &state.indoor_hum_hist_long, N);

    let indoor_ok = if let Ok(mut nvs_guard) = nvs.lock() {
        match nvs_guard.set_raw(indoor_key, buf) {
            Ok(_) => true,
            Err(e) => {
                log::warn!("NVS hist_indoor save failed: {:?}", e);
                false
            }
        }
    } else {
        false
    };

    unsafe {
        esp_idf_sys::heap_caps_free(buf_ptr as *mut core::ffi::c_void);
    }

    // Pressure history
    let press_total = pressure_history::PressureHistory::serialised_size();
    let pbuf_ptr = unsafe {
        esp_idf_sys::heap_caps_malloc(press_total, esp_idf_sys::MALLOC_CAP_SPIRAM) as *mut u8
    };
    let press_ok = if pbuf_ptr.is_null() {
        log::warn!(
            "history_nvs_save: PSRAM alloc failed for pressure ({}B), skipping",
            press_total
        );
        false
    } else {
        let pbuf = unsafe {
            core::ptr::write_bytes(pbuf_ptr, 0, press_total);
            core::slice::from_raw_parts_mut(pbuf_ptr, press_total)
        };
        state.pressure_history.write_bytes(pbuf);
        let ok = if let Ok(mut nvs_guard) = nvs.lock() {
            match nvs_guard.set_raw(press_key, pbuf) {
                Ok(_) => true,
                Err(e) => {
                    log::warn!("NVS hist_press save failed: {:?}", e);
                    false
                }
            }
        } else {
            false
        };
        unsafe {
            esp_idf_sys::heap_caps_free(pbuf_ptr as *mut core::ffi::c_void);
        }
        ok
    };

    if indoor_ok && press_ok {
        log::info!(
            "History NVS indoor+pressure written to slot {} (gen={}) ({} + {} bytes)",
            write_slot,
            gen,
            total,
            press_total
        );
    } else {
        log::warn!(
            "History NVS save partial (slot {}); gen NOT bumped",
            write_slot
        );
    }
    indoor_ok && press_ok
}

/// Restore sensor history from the most recent complete NVS slot.
pub fn history_nvs_restore(
    state: &mut views::AppState,
    nvs: &Arc<Mutex<EspNvs<esp_idf_svc::nvs::NvsDefault>>>,
) {
    let gen = nvs_read_gen(nvs);
    // gen==0 → no ping-pong save; try slot A (legacy single-slot keys still work)
    let read_slot = if gen == 0 { 0 } else { (gen - 1) % 2 };
    let (indoor_key, press_key, _) = slot_keys(read_slot);
    log::info!(
        "history_nvs_restore: gen={} → reading slot {}",
        gen,
        read_slot
    );

    const F: usize = core::mem::size_of::<f32>();
    const N: usize = 480;
    let total = 4 * (8 + N * F);
    let buf_ptr =
        unsafe { esp_idf_sys::heap_caps_malloc(total, esp_idf_sys::MALLOC_CAP_SPIRAM) as *mut u8 };
    if buf_ptr.is_null() {
        log::warn!(
            "history_nvs_restore: PSRAM alloc failed ({}B), skipping",
            total
        );
        return;
    }
    unsafe {
        core::ptr::write_bytes(buf_ptr, 0, total);
    }
    let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr, total) };

    let loaded = if let Ok(nvs_guard) = nvs.lock() {
        matches!(nvs_guard.get_raw(indoor_key, buf), Ok(Some(_)))
    } else {
        false
    };

    if !loaded {
        log::info!("No saved indoor history found in NVS (slot {})", read_slot);
        unsafe {
            esp_idf_sys::heap_caps_free(buf_ptr as *mut core::ffi::c_void);
        }
        return;
    }

    fn read_deque(buf: &[u8], off: &mut usize, dq: &mut crate::psbox::PsramRing, max: usize) {
        let len = (u64::from_le_bytes(buf[*off..*off + 8].try_into().unwrap()) as usize).min(max);
        *off += 8;
        for _ in 0..len {
            let v = f32::from_le_bytes(buf[*off..*off + 4].try_into().unwrap());
            *off += 4;
            dq.push_back(v);
        }
        *off += (max - len) * 4;
    }

    let mut off = 0usize;
    read_deque(buf, &mut off, &mut state.indoor_temp_history, N);
    read_deque(buf, &mut off, &mut state.indoor_hum_history, N);
    read_deque(buf, &mut off, &mut state.indoor_temp_hist_long, N);
    read_deque(buf, &mut off, &mut state.indoor_hum_hist_long, N);
    log::info!(
        "Restored indoor history (slot {}): {} short, {} long temp samples",
        read_slot,
        state.indoor_temp_history.len(),
        state.indoor_temp_hist_long.len()
    );
    unsafe {
        esp_idf_sys::heap_caps_free(buf_ptr as *mut core::ffi::c_void);
    }

    // Restore pressure history
    let press_total = pressure_history::PressureHistory::serialised_size();
    let pbuf_ptr = unsafe {
        esp_idf_sys::heap_caps_malloc(press_total, esp_idf_sys::MALLOC_CAP_SPIRAM) as *mut u8
    };
    if pbuf_ptr.is_null() {
        log::warn!(
            "history_nvs_restore: PSRAM alloc failed for pressure buf ({}B), skipping",
            press_total
        );
        return;
    }
    unsafe {
        core::ptr::write_bytes(pbuf_ptr, 0, press_total);
    }
    let pbuf = unsafe { core::slice::from_raw_parts_mut(pbuf_ptr, press_total) };
    let press_loaded = if let Ok(nvs_guard) = nvs.lock() {
        matches!(nvs_guard.get_raw(press_key, pbuf), Ok(Some(_)))
    } else {
        false
    };
    if press_loaded {
        state.pressure_history.load_from_bytes(pbuf);
        log::info!("Restored pressure history from NVS (slot {})", read_slot);
    }
    unsafe {
        esp_idf_sys::heap_caps_free(pbuf_ptr as *mut core::ffi::c_void);
    }
}

/// Coordinated ping-pong save: writes indoor, pressure, and dashboard history
/// to the same inactive slot, then bumps the generation counter once.
/// This is the function callers should use instead of the individual save functions.
pub fn history_nvs_save_all(
    state: &views::AppState,
    history: &Arc<Mutex<history_ring::HistoryRing>>,
    nvs: &Arc<Mutex<EspNvs<esp_idf_svc::nvs::NvsDefault>>>,
) {
    let gen = nvs_read_gen(nvs);
    let write_slot = gen % 2;
    let indoor_ok = history_nvs_save(state, nvs);
    let dash_ok = dashboard_history_nvs_save_slot(history, nvs, write_slot);
    if indoor_ok && dash_ok {
        nvs_write_gen(nvs, gen + 1);
        log::info!(
            "NVS ping-pong save complete: slot {} → gen {}",
            write_slot,
            gen + 1
        );
    } else {
        log::warn!(
            "NVS ping-pong save incomplete (indoor={} dash={}); gen NOT bumped",
            indoor_ok,
            dash_ok
        );
    }
}

// ── dashboard ─────────────────────────────────────────────────────────────────

/// Serialise the most recent 24h of dashboard history to NVS into `write_slot`.
/// Returns `true` on success.  Callers should use [`history_nvs_save_all`] which
/// coordinates the slot and gen bump.
fn dashboard_history_nvs_save_slot(
    history: &Arc<Mutex<history_ring::HistoryRing>>,
    nvs: &Arc<Mutex<EspNvs<esp_idf_svc::nvs::NvsDefault>>>,
    write_slot: u32,
) -> bool {
    let (_, _, dash_key) = slot_keys(write_slot);

    const SAMPLE_SIZE: usize = core::mem::size_of::<history_ring::HistorySample>();
    const N: usize = 1440;
    let total = 8 + N * SAMPLE_SIZE;

    let buf_ptr =
        unsafe { esp_idf_sys::heap_caps_malloc(total, esp_idf_sys::MALLOC_CAP_SPIRAM) as *mut u8 };
    if buf_ptr.is_null() {
        log::warn!(
            "dashboard_history_nvs_save: PSRAM alloc failed ({}B), skipping",
            total
        );
        return false;
    }
    let buf = unsafe {
        core::ptr::write_bytes(buf_ptr, 0, total);
        core::slice::from_raw_parts_mut(buf_ptr, total)
    };

    let count = if let Ok(ring) = history.lock() {
        let mut cnt = 0usize;
        let mut off = 8usize;
        for s in ring.iter_recent(24) {
            if off + SAMPLE_SIZE <= total {
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        s as *const history_ring::HistorySample as *const u8,
                        buf.as_mut_ptr().add(off),
                        SAMPLE_SIZE,
                    );
                }
                off += SAMPLE_SIZE;
                cnt += 1;
            }
        }
        buf[0..8].copy_from_slice(&(cnt as u64).to_le_bytes());
        cnt
    } else {
        0
    };

    let ok = if count > 0 {
        if let Ok(mut nvs_guard) = nvs.lock() {
            match nvs_guard.set_raw(dash_key, &buf[..8 + count * SAMPLE_SIZE]) {
                Ok(_) => {
                    log::info!(
                        "NVS hist_dash saved {} samples ({} B) to slot {}",
                        count,
                        8 + count * SAMPLE_SIZE,
                        write_slot
                    );
                    true
                }
                Err(e) => {
                    log::warn!("NVS hist_dash save failed: {:?}", e);
                    false
                }
            }
        } else {
            false
        }
    } else {
        true
    }; // nothing to write is not a failure
    unsafe {
        esp_idf_sys::heap_caps_free(buf_ptr as *mut core::ffi::c_void);
    }
    ok
}

/// Restore dashboard history from the most recent complete NVS slot.
pub fn dashboard_history_nvs_restore(
    history: &Arc<Mutex<history_ring::HistoryRing>>,
    nvs: &Arc<Mutex<EspNvs<esp_idf_svc::nvs::NvsDefault>>>,
) {
    let gen = nvs_read_gen(nvs);
    let read_slot = if gen == 0 { 0 } else { (gen - 1) % 2 };
    let (_, _, dash_key) = slot_keys(read_slot);

    const SAMPLE_SIZE: usize = core::mem::size_of::<history_ring::HistorySample>();
    const N: usize = 1440;
    let total = 8 + N * SAMPLE_SIZE;

    let buf_ptr =
        unsafe { esp_idf_sys::heap_caps_malloc(total, esp_idf_sys::MALLOC_CAP_SPIRAM) as *mut u8 };
    if buf_ptr.is_null() {
        log::warn!(
            "dashboard_history_nvs_restore: PSRAM alloc failed ({}B), skipping",
            total
        );
        return;
    }
    unsafe {
        core::ptr::write_bytes(buf_ptr, 0, total);
    }
    let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr, total) };

    let loaded = if let Ok(nvs_guard) = nvs.lock() {
        matches!(nvs_guard.get_raw(dash_key, buf), Ok(Some(_)))
    } else {
        false
    };

    if !loaded {
        log::info!(
            "No saved dashboard history found in NVS (slot {})",
            read_slot
        );
        unsafe {
            esp_idf_sys::heap_caps_free(buf_ptr as *mut core::ffi::c_void);
        }
        return;
    }

    let cnt = (u64::from_le_bytes(buf[0..8].try_into().unwrap()) as usize).min(N);
    if let Ok(mut ring) = history.lock() {
        let mut off = 8usize;
        for _ in 0..cnt {
            if off + SAMPLE_SIZE <= total {
                let mut sample = history_ring::HistorySample::default();
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        buf.as_ptr().add(off),
                        &mut sample as *mut history_ring::HistorySample as *mut u8,
                        SAMPLE_SIZE,
                    );
                }
                ring.push(sample);
                off += SAMPLE_SIZE;
            }
        }
        log::info!(
            "Restored {} dashboard history samples from NVS (slot {})",
            cnt,
            read_slot
        );
    }
    unsafe {
        esp_idf_sys::heap_caps_free(buf_ptr as *mut core::ffi::c_void);
    }
}
