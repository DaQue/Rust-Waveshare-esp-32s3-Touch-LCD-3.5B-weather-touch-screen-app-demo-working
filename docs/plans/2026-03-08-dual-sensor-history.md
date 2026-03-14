# Dual Sensor History + Proactive Reboot + NVS Persistence

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the existing 1-hour indoor history ring buffers with dual short (2h) and long (24h) ring buffers for temp, humidity, and pressure; add a tap-to-toggle UI on the Indoor and PressureHvac views; proactively reboot when SRAM is near exhaustion and dump history to NVS so it survives the reboot.

**Architecture:** Add a `bme_sample_tick` counter in the main BME loop; every 3rd accepted read pushes to the "short" (2h @ 15s) buffers, every 36th pushes to the "long" (24h @ 3min) buffers. All new buffers are `VecDeque<f32>` pre-allocated with `with_capacity()` so they live entirely in PSRAM after the one boot-time alloc. Pressure history gains matching short/long methods. A boolean `plot_range_short` in AppState drives which dataset the Indoor and PressureHvac views render, toggled by tapping the graph area. Before a proactive reboot the buffers are serialised to raw bytes and saved as NVS blobs; on next boot they are restored before the first render.

**Tech Stack:** Rust no-std/std ESP-IDF, `esp-idf-svc` NVS (`EspNvs::set_raw` / `get_raw`), existing `PsBox<T>`, existing `draw_line_graph()`, existing `bme_series()` / `owm_series()` patterns.

---

## Sizing reference

| Buffer | Samples | Interval | Window | Bytes |
|--------|---------|----------|--------|-------|
| Indoor temp/hum short | 480 | 15 s | 2 h | 480 × 4 × 2 = 3 840 |
| Indoor temp/hum long  | 480 | 3 min | 24 h | 480 × 4 × 2 = 3 840 |
| Pressure short (BME+OWM) | 480 | 15 s | 2 h | 480 × 4 × 2 = 3 840 |
| Pressure long  (BME+OWM) | 480 | 3 min | 24 h | 480 × 4 × 2 = 3 840 |
| **Total** | | | | **~15.4 KB PSRAM** |

NVS blob (all buffers + indices): same ~15.4 KB, split into two keys < 8 KB each.

---

## Task 1: Extend `PressureHistory` with short + long dual buffers

**Files:**
- Modify: `src/pressure_history.rs`

Currently `PressureHistory` has one ring of 288 × `PressureSample` @ 5 min = 24 h.
Replace it with two rings: `long` (480 × PressureSample @ 3 min = 24 h) and `short` (480 × PressureSample @ 15 s = 2 h).

**Step 1: Update constants**

```rust
// Old:
pub const SAMPLE_PERIOD_SECS: u32 = 300;
pub const WINDOW_SECS: u32 = 24 * 3600;
pub const CAPACITY: usize = (WINDOW_SECS / SAMPLE_PERIOD_SECS) as usize; // 288

// New:
pub const SHORT_PERIOD_SECS: u32 = 15;   // every 3rd BME read
pub const LONG_PERIOD_SECS:  u32 = 180;  // every 36th BME read
pub const SHORT_CAP: usize = 480;        // 2 h at 15 s
pub const LONG_CAP:  usize = 480;        // 24 h at 3 min
```

**Step 2: Replace `PressureHistory` struct**

```rust
#[derive(Clone)]
pub struct PressureHistory {
    short: [PressureSample; SHORT_CAP],
    short_idx: usize,
    short_count: usize,
    long: [PressureSample; LONG_CAP],
    long_idx: usize,
    long_count: usize,
}

impl PressureHistory {
    pub fn new() -> Self {
        Self {
            short: [PressureSample::default(); SHORT_CAP],
            short_idx: 0,
            short_count: 0,
            long: [PressureSample::default(); LONG_CAP],
            long_idx: 0,
            long_count: 0,
        }
    }

    pub fn push_short(&mut self, bme_hpa: Option<f32>, owm_hpa: Option<f32>) {
        self.short[self.short_idx] = PressureSample { bme_hpa, owm_hpa };
        self.short_idx = (self.short_idx + 1) % SHORT_CAP;
        if self.short_count < SHORT_CAP { self.short_count += 1; }
    }

    pub fn push_long(&mut self, bme_hpa: Option<f32>, owm_hpa: Option<f32>) {
        self.long[self.long_idx] = PressureSample { bme_hpa, owm_hpa };
        self.long_idx = (self.long_idx + 1) % LONG_CAP;
        if self.long_count < LONG_CAP { self.long_count += 1; }
    }
```

**Step 3: Add iterator helpers for both buffers**

Add private `iter_short()` and `iter_long()` mirrors of the existing `iter()`. Then add public methods:

```rust
pub fn short_bme_series(&self) -> Vec<(usize, f32)> { self.extract_series_short(|s| s.bme_hpa) }
pub fn short_owm_series(&self) -> Vec<(usize, f32)> { self.extract_series_short(|s| s.owm_hpa) }
pub fn long_bme_series(&self)  -> Vec<(usize, f32)> { self.extract_series_long(|s| s.bme_hpa) }
pub fn long_owm_series(&self)  -> Vec<(usize, f32)> { self.extract_series_long(|s| s.owm_hpa) }
pub fn short_bme_min_max(&self) -> Option<(f32, f32)> { self.min_max_short(|s| s.bme_hpa) }
pub fn short_owm_min_max(&self) -> Option<(f32, f32)> { self.min_max_short(|s| s.owm_hpa) }
pub fn long_bme_min_max(&self)  -> Option<(f32, f32)> { self.min_max_long(|s| s.bme_hpa) }
pub fn long_owm_min_max(&self)  -> Option<(f32, f32)> { self.min_max_long(|s| s.owm_hpa) }
pub fn long_len(&self) -> usize { self.long_count }
pub fn short_len(&self) -> usize { self.short_count }
```

Keep `latest_bme()` and `latest_owm()` pointing to the short buffer (most recent data).
Keep `delta_owm_bme_recent()` pointing to the short buffer.

**Step 4: Remove old `push()`, `bme_series()`, `owm_series()`, `bme_min_max()`, `owm_min_max()`, `len()`**

These are replaced by the new `push_short/long` and `short_*/long_*` variants. The compiler will
flag all call sites — fix them in later tasks.

**Step 5: Build to check for compile errors in pressure_history.rs only**

```bash
cargo +esp build -Zbuild-std=std,panic_abort 2>&1 | grep "pressure_history" | head -20
```

Expected: errors about `push`, `bme_series`, etc. being undefined at call sites — that's correct,
fix in Task 3 (main.rs) and Task 5 (pressure_hvac.rs).

---

## Task 2: Extend `AppState` in `views/mod.rs`

**Files:**
- Modify: `src/views/mod.rs`

**Step 1: Update constants**

```rust
// Old:
pub const INDOOR_HISTORY_MAX: usize = 720;

// New:
pub const INDOOR_SHORT_MAX: usize = 480;  // 2 h at 15 s intervals
pub const INDOOR_LONG_MAX:  usize = 480;  // 24 h at 3 min intervals
```

**Step 2: Add new fields to `AppState`**

In the struct definition, after `indoor_hum_history`:

```rust
// Add these four fields:
pub indoor_temp_hist_long: VecDeque<f32>,
pub indoor_hum_hist_long: VecDeque<f32>,
pub plot_range_short: bool,      // true = 2h, false = 24h (for Indoor + PressureHvac)
```

Keep existing `indoor_temp_history` and `indoor_hum_history` — they become the short (2h) buffers.
The rename from `INDOOR_HISTORY_MAX` to `INDOOR_SHORT_MAX` will break references in main.rs — fix there.

**Step 3: Initialise new fields in `AppState::new()`**

```rust
indoor_temp_hist_long: VecDeque::with_capacity(INDOOR_LONG_MAX),
indoor_hum_hist_long:  VecDeque::with_capacity(INDOOR_LONG_MAX),
plot_range_short: true,
```

Also change existing initialisers to pre-allocate:

```rust
indoor_temp_history: VecDeque::with_capacity(INDOOR_SHORT_MAX),
indoor_hum_history:  VecDeque::with_capacity(INDOOR_SHORT_MAX),
```

**Step 4: Build**

```bash
cargo +esp build -Zbuild-std=std,panic_abort 2>&1 | grep "INDOOR_HISTORY_MAX\|plot_range" | head -20
```

Expected: errors at references to old `INDOOR_HISTORY_MAX` in main.rs — fix next.

---

## Task 3: Update main BME loop in `main.rs`

**Files:**
- Modify: `src/main.rs`

**Step 1: Add counter near the other `last_*_ms` variables (around line 68)**

Find the block where `last_bme_ms`, `last_pressure_sample_ms`, etc. are declared and add:

```rust
let mut bme_sample_tick: u32 = 0u32;
```

**Step 2: Fix `INDOOR_HISTORY_MAX` references (lines ~1238–1244)**

```rust
// Old:
if state.indoor_temp_history.len() >= views::INDOOR_HISTORY_MAX {
// New:
if state.indoor_temp_history.len() >= views::INDOOR_SHORT_MAX {
```

(Same for `indoor_hum_history`.)

**Step 3: Change short push to every 3rd BME read**

Inside the `} else { // accept block` (currently at ~line 1220), after `bme_reject_streak = 0;`:

```rust
bme_sample_tick = bme_sample_tick.wrapping_add(1);

// Short buffer: every 3rd accepted read = 15 s
if bme_sample_tick % 3 == 0 {
    if state.indoor_temp_history.len() >= views::INDOOR_SHORT_MAX {
        state.indoor_temp_history.pop_front();
    }
    state.indoor_temp_history.push_back(reading.temperature_f);
    if state.indoor_hum_history.len() >= views::INDOOR_SHORT_MAX {
        state.indoor_hum_history.pop_front();
    }
    state.indoor_hum_history.push_back(reading.humidity);
}

// Long buffer: every 36th accepted read = 3 min
if bme_sample_tick % 36 == 0 {
    if state.indoor_temp_hist_long.len() >= views::INDOOR_LONG_MAX {
        state.indoor_temp_hist_long.pop_front();
    }
    state.indoor_temp_hist_long.push_back(reading.temperature_f);
    if state.indoor_hum_hist_long.len() >= views::INDOOR_LONG_MAX {
        state.indoor_hum_hist_long.pop_front();
    }
    state.indoor_hum_hist_long.push_back(reading.humidity);
}
```

Remove the old `if state.indoor_temp_history.len() >= views::INDOOR_HISTORY_MAX` block that was
unconditional (pushed every BME read).

**Step 4: Fix pressure_history push sites**

Find the pressure push near line 1285:

```rust
// Old:
state.pressure_history.push(bme, owm);

// New (in the PRESSURE_SAMPLE_INTERVAL block — keep this as the long push):
state.pressure_history.push_long(bme, owm);
```

Then inside the BME accept block (same place as Step 3), add short pressure push:

```rust
// Short pressure: every 3rd accepted BME read = 15 s
if bme_sample_tick % 3 == 0 {
    let bme_hpa = state.indoor_pressure;
    let owm_hpa = state.current_weather.as_ref()
        .and_then(|cw| if cw.pressure_hpa > 0 { Some(cw.pressure_hpa as f32) } else { None });
    state.pressure_history.push_short(bme_hpa, owm_hpa);
}
```

Note: `state.indoor_pressure` is set earlier in the same accept block (line ~1236) so it's safe to read here.

**Step 5: Add SRAM watch — AtomicBool flag + logic in `http_fetch_into`**

`http_fetch_into` already measures `lb` for the bail-early check, so the SRAM watch lives there
naturally. It signals back to the main loop via a static `AtomicBool` for the BME reset, and
calls the NVS save + reboot directly for the full reboot (since it has access to nothing — the
reboot path is handled by returning a special error that the caller acts on, or via a second
static for the reboot signal).

Simplest approach: two statics at the top of `main.rs` (or `http_client.rs`):

```rust
// In main.rs, near the top (after imports):
static SRAM_BME_RESET: AtomicBool = AtomicBool::new(false);
static SRAM_LOW_STREAK: AtomicU32  = AtomicU32::new(0);
```

Add `use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};` if not already present.

**In `http_fetch_into` in `http_client.rs`**, replace the existing bail-early block:

```rust
// Old bail-early:
if largest_block < 4_000 {
    bail!("SRAM too fragmented for TLS ({} KB largest block)", largest_block / 1024);
}

// New — keep the 4 KB hard bail, add 12 KB watch above it:
if largest_block < 12_000 {
    let streak = SRAM_LOW_STREAK.fetch_add(1, Ordering::Relaxed) + 1;

    // First time below threshold: signal main loop to reset BME280.
    if streak == 1 {
        log::warn!(
            "SRAM < 12 KB ({} KB) — signalling BME280 reset (fetch streak {})",
            largest_block / 1024, streak
        );
        SRAM_BME_RESET.store(true, Ordering::Relaxed);
    }

    // Three consecutive fetches below threshold: save history and reboot.
    if streak >= 3 {
        log::error!(
            "SRAM critically low for {} consecutive fetches — saving history and rebooting",
            streak
        );
        // NVS save is handled by the caller — signal via bail so the
        // caller can save before restart.
        bail!("SRAM_REBOOT"); // caller checks for this exact string
    }
} else {
    SRAM_LOW_STREAK.store(0, Ordering::Relaxed);
}

if largest_block < 4_000 {
    bail!("SRAM too fragmented for TLS ({} KB largest block)", largest_block / 1024);
}
```

`http_client.rs` needs `use crate::SRAM_BME_RESET; use crate::SRAM_LOW_STREAK;` (or move the
statics to `http_client.rs` and re-export them).

**In the main loop BME read block**, after the `bme280.is_none()` retry check, add:

```rust
// BME reset requested by SRAM watch in http_fetch_into
if SRAM_BME_RESET.swap(false, Ordering::Relaxed) {
    log::warn!("BME280 reset due to low SRAM — will re-init on next interval");
    bme280 = None;
    bme_reject_streak = 0;
}
```

**In the weather fetch call site** (wherever `http_fetch_into` / `https_get_json` errors are
handled in the weather thread), check for the `"SRAM_REBOOT"` sentinel:

```rust
if let Err(e) = fetch_result {
    if e.to_string().contains("SRAM_REBOOT") {
        history_nvs_save(&state_for_save, &nvs);
        unsafe { esp_idf_sys::esp_restart() };
    }
    // ... existing error handling
}
```

The NVS save + restart happens in whichever thread (weather or alerts) first hits the 3rd
consecutive low fetch. Both threads share the same `SRAM_LOW_STREAK` atomic so the count
accumulates correctly across interleaved fetches.

**Step 6: Add NVS restore call at boot**

After `cfg` is loaded (~line 640) and before the first render:

```rust
history_nvs_restore(&mut state, &nvs);
```

**Step 7: Build**

```bash
cargo +esp build -Zbuild-std=std,panic_abort 2>&1 | grep "^error" | head -20
```

Expected: errors about `history_nvs_save` / `history_nvs_restore` not existing — implement in Task 4.

---

## Task 4: NVS persistence functions in `main.rs`

**Files:**
- Modify: `src/main.rs`
- Modify: `src/config.rs`

NVS blobs are stored as raw bytes using `EspNvs::set_raw` / `get_raw`.
Two keys: `hist_indoor` (~7.7 KB) and `hist_press` (~7.7 KB).

**Step 1: Add NVS keys to `config.rs`**

```rust
pub const KEY_HIST_INDOOR: &str = "hist_indoor";
pub const KEY_HIST_PRESS:  &str = "hist_press";
```

**Step 2: Add `history_nvs_save()` function in `main.rs`**

Place it just before `fn main()`:

```rust
fn history_nvs_save(state: &views::AppState, nvs: &Arc<Mutex<EspNvs<NvsDefault>>>) {
    // Serialise indoor history: two VecDeques (short) + two VecDeques (long)
    // Layout: [short_temp: 480 f32][short_hum: 480 f32][long_temp: 480 f32][long_hum: 480 f32]
    //         + 4 × usize for lengths
    const F: usize = core::mem::size_of::<f32>();
    const N: usize = 480;
    // Total bytes: 4 arrays × N × F + 4 × 8 (usize) = 7680 + 32 = 7712
    let mut buf = vec![0u8; 4 * N * F + 4 * 8];
    let mut off = 0usize;

    // Helper: write VecDeque as contiguous bytes
    fn write_deque(buf: &mut [u8], off: &mut usize, dq: &VecDeque<f32>, max: usize) {
        let len = dq.len().min(max);
        buf[*off..*off + 8].copy_from_slice(&(len as u64).to_le_bytes());
        *off += 8;
        let (s0, s1) = dq.as_slices();
        for &v in s0.iter().chain(s1.iter()).take(len) {
            buf[*off..*off + 4].copy_from_slice(&v.to_le_bytes());
            *off += 4;
        }
        *off += (max - len) * 4; // skip remaining slots so offsets are fixed
    }

    write_deque(&mut buf, &mut off, &state.indoor_temp_history, N);
    write_deque(&mut buf, &mut off, &state.indoor_hum_history, N);
    write_deque(&mut buf, &mut off, &state.indoor_temp_hist_long, N);
    write_deque(&mut buf, &mut off, &state.indoor_hum_hist_long, N);

    if let Ok(mut nvs) = nvs.lock() {
        if let Err(e) = nvs.set_raw(config::KEY_HIST_INDOOR, &buf) {
            log::warn!("NVS hist_indoor save failed: {:?}", e);
        }
    }

    // Pressure history: use PressureHistory's own serialise helper
    let press_buf = state.pressure_history.to_bytes();
    if let Ok(mut nvs) = nvs.lock() {
        if let Err(e) = nvs.set_raw(config::KEY_HIST_PRESS, &press_buf) {
            log::warn!("NVS hist_press save failed: {:?}", e);
        }
    }

    log::info!("History NVS save complete ({} + {} bytes)", buf.len(), press_buf.len());
}
```

**Step 3: Add `history_nvs_restore()` in `main.rs`**

```rust
fn history_nvs_restore(state: &mut views::AppState, nvs: &Arc<Mutex<EspNvs<NvsDefault>>>) {
    const F: usize = core::mem::size_of::<f32>();
    const N: usize = 480;
    let total = 4 * N * F + 4 * 8;
    let mut buf = vec![0u8; total];

    let loaded = if let Ok(nvs) = nvs.lock() {
        nvs.get_raw(config::KEY_HIST_INDOOR, &mut buf).unwrap_or(None).is_some()
    } else {
        false
    };

    if !loaded {
        log::info!("No saved indoor history found");
        return;
    }

    fn read_deque(buf: &[u8], off: &mut usize, dq: &mut VecDeque<f32>, max: usize) {
        let len = u64::from_le_bytes(buf[*off..*off + 8].try_into().unwrap()) as usize;
        *off += 8;
        let len = len.min(max);
        for _ in 0..len {
            let v = f32::from_le_bytes(buf[*off..*off + 4].try_into().unwrap());
            *off += 4;
            dq.push_back(v);
        }
        *off += (max - len) * 4;
    }

    let mut off = 0usize;
    read_deque(&buf, &mut off, &mut state.indoor_temp_history,    N);
    read_deque(&buf, &mut off, &mut state.indoor_hum_history,     N);
    read_deque(&buf, &mut off, &mut state.indoor_temp_hist_long,  N);
    read_deque(&buf, &mut off, &mut state.indoor_hum_hist_long,   N);
    log::info!("Restored indoor history: {} short, {} long temp samples",
        state.indoor_temp_history.len(), state.indoor_temp_hist_long.len());

    // Pressure
    let press_total = pressure_history::PressureHistory::serialised_size();
    let mut pbuf = vec![0u8; press_total];
    let press_loaded = if let Ok(nvs) = nvs.lock() {
        nvs.get_raw(config::KEY_HIST_PRESS, &mut pbuf).unwrap_or(None).is_some()
    } else {
        false
    };
    if press_loaded {
        state.pressure_history.from_bytes(&pbuf);
        log::info!("Restored pressure history");
    }
}
```

**Step 4: Add `to_bytes()` / `from_bytes()` / `serialised_size()` to `PressureHistory`**

In `src/pressure_history.rs`:

```rust
/// Byte size of the serialised form.
pub fn serialised_size() -> usize {
    // short_count + short_idx + CAPACITY × 8 bytes (two f32 per sample)
    // + long_count + long_idx + CAPACITY × 8
    // = 2 × (8 + 8 + 480 × 8) = 2 × 3856 = 7712
    2 * (8 + 8 + SHORT_CAP * 8)
}

pub fn to_bytes(&self) -> Vec<u8> {
    let mut out = Vec::with_capacity(Self::serialised_size());
    let write_ring = |out: &mut Vec<u8>, buf: &[PressureSample], idx: usize, count: usize, cap: usize| {
        out.extend_from_slice(&(count as u64).to_le_bytes());
        out.extend_from_slice(&(idx as u64).to_le_bytes());
        for s in buf.iter().take(cap) {
            let bme = s.bme_hpa.unwrap_or(f32::NAN);
            let owm = s.owm_hpa.unwrap_or(f32::NAN);
            out.extend_from_slice(&bme.to_le_bytes());
            out.extend_from_slice(&owm.to_le_bytes());
        }
    };
    write_ring(&mut out, &self.short, self.short_idx, self.short_count, SHORT_CAP);
    write_ring(&mut out, &self.long, self.long_idx, self.long_count, LONG_CAP);
    out
}

pub fn from_bytes(&mut self, bytes: &[u8]) {
    let read_ring = |bytes: &[u8], off: &mut usize, buf: &mut [PressureSample], idx: &mut usize, count: &mut usize, cap: usize| {
        *count = (u64::from_le_bytes(bytes[*off..*off+8].try_into().unwrap()) as usize).min(cap);
        *off += 8;
        *idx   = (u64::from_le_bytes(bytes[*off..*off+8].try_into().unwrap()) as usize) % cap;
        *off += 8;
        for i in 0..cap {
            let bme = f32::from_le_bytes(bytes[*off..*off+4].try_into().unwrap());
            *off += 4;
            let owm = f32::from_le_bytes(bytes[*off..*off+4].try_into().unwrap());
            *off += 4;
            buf[i] = PressureSample {
                bme_hpa: if bme.is_finite() { Some(bme) } else { None },
                owm_hpa: if owm.is_finite() { Some(owm) } else { None },
            };
        }
    };
    let mut off = 0;
    read_ring(bytes, &mut off, &mut self.short, &mut self.short_idx, &mut self.short_count, SHORT_CAP);
    read_ring(bytes, &mut off, &mut self.long,  &mut self.long_idx,  &mut self.long_count,  LONG_CAP);
}
```

**Step 5: Build**

```bash
cargo +esp build -Zbuild-std=std,panic_abort 2>&1 | grep "^error" | head -20
```

Expected: any remaining type mismatches in the new serialise helpers. Fix until clean.

---

## Task 5: Update `views/indoor.rs` — 2h/24h toggle

**Files:**
- Modify: `src/views/indoor.rs`

**Step 1: Read `plot_range_short` and select buffer references**

At the top of `pub fn draw(...)`, after the existing `let screen_w` / `screen_h` block, add:

```rust
let (temp_s0, temp_s1, temp_len, hum_s0, hum_s1, hum_len) = if state.plot_range_short {
    let (ts0, ts1) = state.indoor_temp_history.as_slices();
    let (hs0, hs1) = state.indoor_hum_history.as_slices();
    (ts0, ts1, state.indoor_temp_history.len(),
     hs0, hs1, state.indoor_hum_history.len())
} else {
    let (ts0, ts1) = state.indoor_temp_hist_long.as_slices();
    let (hs0, hs1) = state.indoor_hum_hist_long.as_slices();
    (ts0, ts1, state.indoor_temp_hist_long.len(),
     hs0, hs1, state.indoor_hum_hist_long.len())
};
```

Remove the old individual `let (temp_s0, temp_s1) = state.indoor_temp_history.as_slices();` lines
and update all downstream references to use the new variables.

**Step 2: Update x-axis label**

Find the existing `"-1h"` / `"-60m"` x-axis label (around line 183) and replace with:

```rust
let x_label = if state.plot_range_short { "-2h" } else { "-24h" };
Text::new(x_label, Point::new(graph_x, x_label_y), label_style).draw(fb).ok();
```

**Step 3: Add tap handler for the graph toggle**

The Indoor view's `handle_gesture` is in `views/mod.rs`'s `AppState::handle_gesture`. Find the
match arm for `View::Indoor` (or add one if absent). Add a `Tap` gesture handler:

```rust
// In handle_gesture, View::Indoor arm:
Gesture::Tap(x, y) => {
    // Tap graph area toggles 2h ↔ 24h
    let graph_y = if self.orientation.is_portrait() { 168 } else { 88 };
    if y >= graph_y {
        self.plot_range_short = !self.plot_range_short;
        self.dirty = true;
        return true;
    }
    false
}
```

**Step 4: Build**

```bash
cargo +esp build -Zbuild-std=std,panic_abort 2>&1 | grep "^error" | head -20
```

---

## Task 6: Update `views/pressure_hvac.rs` — 2h/24h toggle

**Files:**
- Modify: `src/views/pressure_hvac.rs`

**Step 1: Replace `phist.bme_series()` / `phist.owm_series()` calls**

Find (~line 136):
```rust
let bme_pts_raw = phist.bme_series();
let owm_pts = phist.owm_series();
```

Replace with:
```rust
let (bme_pts_raw, owm_pts, bme_mm, owm_mm, num_samples) = if state.plot_range_short {
    (phist.short_bme_series(), phist.short_owm_series(),
     phist.short_bme_min_max(), phist.short_owm_min_max(),
     phist.short_len())
} else {
    (phist.long_bme_series(), phist.long_owm_series(),
     phist.long_bme_min_max(), phist.long_owm_min_max(),
     phist.long_len())
};
```

Update the `min_max` calls downstream to use `bme_mm` / `owm_mm` instead of calling
`phist.bme_min_max()` / `phist.owm_min_max()`.

**Step 2: Update x-axis label**

Find the existing `"-24h"` label (~line 224):
```rust
// Old:
Text::new("-24h", Point::new(graph_x, x_label_y), label_style)
// New:
let x_label = if state.plot_range_short { "-2h" } else { "-24h" };
Text::new(x_label, Point::new(graph_x, x_label_y), label_style)
```

**Step 3: Add tap handler for PressureHvac in `handle_gesture`**

```rust
View::PressureHvac => match gesture {
    Gesture::Tap(x, y) => {
        let graph_top = /* same value as in pressure_hvac.rs */ 30;
        if y >= graph_top {
            self.plot_range_short = !self.plot_range_short;
            self.dirty = true;
            return true;
        }
        false
    }
    _ => false,
},
```

Use the same `graph_top` constant used in `pressure_hvac.rs` (currently `let graph_top = ...` — check the actual value).

**Step 4: Build**

```bash
cargo +esp build -Zbuild-std=std,panic_abort 2>&1 | grep "^error" | head -20
```

---

## Task 7: Full build + version bump

**Files:**
- Modify: `Cargo.toml`

**Step 1: Bump version**

```toml
# Old:
version = "0.2.32"
# New:
version = "0.2.33"
```

**Step 2: Final build**

```bash
cargo +esp build -Zbuild-std=std,panic_abort 2>&1 | tail -5
```

Expected: `Finished ... in ...s` with no errors.

**Step 3: Commit**

```bash
git add src/pressure_history.rs src/views/mod.rs src/views/indoor.rs src/views/pressure_hvac.rs \
        src/main.rs src/config.rs Cargo.toml Cargo.lock
git commit -m "feat: dual 2h/24h sensor history, proactive reboot, NVS persistence (v0.2.33)"
```

---

## Implementation notes

### Memory budget check

Before flashing, verify PSRAM usage:
- 4 × VecDeque<f32>(480) = 4 × ~2 KB = ~8 KB heap (PSRAM with ALWAYSINTERNAL=0)
- 2 × `[PressureSample; 480]` fixed arrays inside `PsBox<PressureHistory>` = ~7.7 KB PSRAM
- NVS save buffers: ~16 KB transient stack/heap allocation only at reboot time — acceptable.

### Toggle state sharing

`plot_range_short` is a single bool shared between Indoor and PressureHvac views. They toggle
independently only if you add separate flags. For v0.2.33 a single shared bool is fine — both views flip together when either graph is tapped.

If you want independent toggles in the future, add `indoor_plot_short` and `pressure_plot_short` separately and update the gesture handlers.

### NVS blob size limit

`esp_idf_svc`'s `EspNvs::set_raw` maps to `nvs_set_blob`, which supports values up to the available NVS page space. With our `CONFIG_PARTITION_TABLE_SINGLE_APP_ENCRYPTED_NVS=y` partition, the NVS region is ~24 KB (check `partition_table.csv`). Two ~8 KB blobs will fit comfortably. If `set_raw` returns an error (e.g. `ESP_ERR_NVS_VALUE_TOO_LONG`), log and skip — history will be lost but the device will boot cleanly.

### Proactive reboot threshold

12 KB (12_000 bytes) was chosen because:
- Normal steady state after first fetch: ~22 KB largest block
- Crash threshold observed: ~22 KB (device locked up at that level during WDT investigation)
- TLS handshake needs ~37 KB to succeed — at 12 KB it's already impossible
- 3 consecutive checks at ~15 ms per main loop tick = 45 ms total, still within render frame time
