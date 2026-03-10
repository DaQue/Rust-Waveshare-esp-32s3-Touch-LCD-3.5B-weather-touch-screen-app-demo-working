// Dual-window pressure history ring buffer (fixed-size, no heap growth).
//
// SHORT buffer: 480 samples at 15 s intervals = 2 h window.
// LONG  buffer: 480 samples at 3 min intervals = 24 h window.
// Either source (BME280 local / OpenWeather remote) may be `None` if
// unavailable at sample time.
//
// The struct lives inside a PsBox<PressureHistory> so both arrays are in PSRAM.

use crate::psbox::PsBoxSlice;

// ── Constants ───────────────────────────────────────────────────────

pub const LONG_PERIOD_SECS:  u32 = 180;  // every 36th BME read @ 5s = 3 min cadence
pub const SHORT_CAP: usize = 480;        // 2 h at 15 s
pub const LONG_CAP:  usize = 480;        // 24 h at 3 min

// ── Sample ──────────────────────────────────────────────────────────

#[derive(Clone, Copy, Default)]
pub struct PressureSample {
    pub bme_hpa: Option<f32>,
    pub owm_hpa: Option<f32>,
}

// ── Ring buffer ─────────────────────────────────────────────────────

#[derive(Clone)]
pub struct PressureHistory {
    short: PsBoxSlice<PressureSample>,  // PSRAM-allocated; clone never touches SRAM
    short_idx: usize,
    short_count: usize,
    long: PsBoxSlice<PressureSample>,
    long_idx: usize,
    long_count: usize,
}

impl PressureHistory {
    pub fn new() -> Self {
        // PsBoxSlice::new allocates directly in PSRAM — avoids both SRAM pressure
        // and the 15 KB stack temporary that would occur with inline fixed arrays.
        Self {
            short: PsBoxSlice::new(SHORT_CAP),
            short_idx: 0,
            short_count: 0,
            long: PsBoxSlice::new(LONG_CAP),
            long_idx: 0,
            long_count: 0,
        }
    }

    /// Push a new short-window sample (every 15 s).
    pub fn push_short(&mut self, bme_hpa: Option<f32>, owm_hpa: Option<f32>) {
        self.short[self.short_idx] = PressureSample { bme_hpa, owm_hpa };
        self.short_idx = (self.short_idx + 1) % SHORT_CAP;
        if self.short_count < SHORT_CAP { self.short_count += 1; }
    }

    /// Push a new long-window sample (every 3 min).
    pub fn push_long(&mut self, bme_hpa: Option<f32>, owm_hpa: Option<f32>) {
        self.long[self.long_idx] = PressureSample { bme_hpa, owm_hpa };
        self.long_idx = (self.long_idx + 1) % LONG_CAP;
        if self.long_count < LONG_CAP { self.long_count += 1; }
    }

    // ── Iterators ───────────────────────────────────────────────────

    fn iter_short(&self) -> impl Iterator<Item = &PressureSample> {
        let start = if self.short_count < SHORT_CAP { 0 } else { self.short_idx };
        (0..self.short_count).map(move |i| &self.short[(start + i) % SHORT_CAP])
    }

    fn iter_long(&self) -> impl Iterator<Item = &PressureSample> {
        let start = if self.long_count < LONG_CAP { 0 } else { self.long_idx };
        (0..self.long_count).map(move |i| &self.long[(start + i) % LONG_CAP])
    }

    // ── Series extraction ───────────────────────────────────────────

    fn extract_series_short(&self, f: impl Fn(&PressureSample) -> Option<f32>) -> Vec<(usize, f32)> {
        let mut out = Vec::new();
        for (i, s) in self.iter_short().enumerate() {
            if let Some(v) = f(s) {
                if v.is_finite() { out.push((i, v)); }
            }
        }
        out
    }

    fn extract_series_long(&self, f: impl Fn(&PressureSample) -> Option<f32>) -> Vec<(usize, f32)> {
        let mut out = Vec::new();
        for (i, s) in self.iter_long().enumerate() {
            if let Some(v) = f(s) {
                if v.is_finite() { out.push((i, v)); }
            }
        }
        out
    }

    pub fn short_bme_series(&self) -> Vec<(usize, f32)> { self.extract_series_short(|s| s.bme_hpa) }
    pub fn short_owm_series(&self) -> Vec<(usize, f32)> { self.extract_series_short(|s| s.owm_hpa) }
    pub fn long_bme_series(&self)  -> Vec<(usize, f32)> { self.extract_series_long(|s| s.bme_hpa) }
    pub fn long_owm_series(&self)  -> Vec<(usize, f32)> { self.extract_series_long(|s| s.owm_hpa) }

    // ── Min / max ───────────────────────────────────────────────────

    fn min_max_short(&self, extract: impl Fn(&PressureSample) -> Option<f32>) -> Option<(f32, f32)> {
        let mut lo = f32::INFINITY;
        let mut hi = f32::NEG_INFINITY;
        let mut found = false;
        for s in self.iter_short() {
            if let Some(v) = extract(s) {
                if v.is_finite() {
                    if v < lo { lo = v; }
                    if v > hi { hi = v; }
                    found = true;
                }
            }
        }
        if found { Some((lo, hi)) } else { None }
    }

    fn min_max_long(&self, extract: impl Fn(&PressureSample) -> Option<f32>) -> Option<(f32, f32)> {
        let mut lo = f32::INFINITY;
        let mut hi = f32::NEG_INFINITY;
        let mut found = false;
        for s in self.iter_long() {
            if let Some(v) = extract(s) {
                if v.is_finite() {
                    if v < lo { lo = v; }
                    if v > hi { hi = v; }
                    found = true;
                }
            }
        }
        if found { Some((lo, hi)) } else { None }
    }

    pub fn short_bme_min_max(&self) -> Option<(f32, f32)> { self.min_max_short(|s| s.bme_hpa) }
    pub fn short_owm_min_max(&self) -> Option<(f32, f32)> { self.min_max_short(|s| s.owm_hpa) }
    pub fn long_bme_min_max(&self)  -> Option<(f32, f32)> { self.min_max_long(|s| s.bme_hpa) }
    pub fn long_owm_min_max(&self)  -> Option<(f32, f32)> { self.min_max_long(|s| s.owm_hpa) }

    pub fn long_len(&self)  -> usize { self.long_count }
    pub fn short_len(&self) -> usize { self.short_count }

    // ── Latest values (from short buffer — most recent data) ────────

    /// Most recent valid BME sample.
    pub fn latest_bme(&self) -> Option<f32> {
        self.latest_value_short(|s| s.bme_hpa)
    }

    /// Most recent valid OWM sample.
    pub fn latest_owm(&self) -> Option<f32> {
        self.latest_value_short(|s| s.owm_hpa)
    }

    fn latest_value_short(&self, extract: impl Fn(&PressureSample) -> Option<f32>) -> Option<f32> {
        if self.short_count == 0 { return None; }
        let start = if self.short_count < SHORT_CAP { 0 } else { self.short_idx };
        for i in 0..self.short_count {
            let rev_i = (start + self.short_count - 1 - i) % SHORT_CAP;
            if let Some(v) = extract(&self.short[rev_i]) {
                if v.is_finite() { return Some(v); }
            }
        }
        None
    }

    /// Average delta (OWM - BME) over the last `n` short-window samples where both exist.
    pub fn delta_owm_bme_recent(&self, n: usize) -> Option<f32> {
        let mut sum = 0.0f32;
        let mut count = 0u32;
        let start = if self.short_count < SHORT_CAP { 0 } else { self.short_idx };
        let take = self.short_count.min(n);
        for i in 0..take {
            let rev_i = (start + self.short_count - 1 - i) % SHORT_CAP;
            let s = &self.short[rev_i];
            if let (Some(bme), Some(owm)) = (s.bme_hpa, s.owm_hpa) {
                if bme.is_finite() && owm.is_finite() {
                    sum += owm - bme;
                    count += 1;
                }
            }
        }
        if count > 0 { Some(sum / count as f32) } else { None }
    }

    // ── NVS serialisation ───────────────────────────────────────────

    /// Byte size of the serialised form.
    pub fn serialised_size() -> usize {
        // short: count(8) + idx(8) + SHORT_CAP × 8 bytes (two f32 per sample)
        // long:  count(8) + idx(8) + LONG_CAP  × 8 bytes
        (8 + 8 + SHORT_CAP * 8) + (8 + 8 + LONG_CAP * 8)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::serialised_size());

        // Write short ring
        out.extend_from_slice(&(self.short_count as u64).to_le_bytes());
        out.extend_from_slice(&(self.short_idx   as u64).to_le_bytes());
        for s in self.short.iter().take(SHORT_CAP) {
            let bme = s.bme_hpa.unwrap_or(f32::NAN);
            let owm = s.owm_hpa.unwrap_or(f32::NAN);
            out.extend_from_slice(&bme.to_le_bytes());
            out.extend_from_slice(&owm.to_le_bytes());
        }

        // Write long ring
        out.extend_from_slice(&(self.long_count as u64).to_le_bytes());
        out.extend_from_slice(&(self.long_idx   as u64).to_le_bytes());
        for s in self.long.iter().take(LONG_CAP) {
            let bme = s.bme_hpa.unwrap_or(f32::NAN);
            let owm = s.owm_hpa.unwrap_or(f32::NAN);
            out.extend_from_slice(&bme.to_le_bytes());
            out.extend_from_slice(&owm.to_le_bytes());
        }

        out
    }

    pub fn load_from_bytes(&mut self, bytes: &[u8]) {
        let mut off = 0usize;

        // Read short ring
        self.short_count = (u64::from_le_bytes(bytes[off..off+8].try_into().unwrap()) as usize).min(SHORT_CAP);
        off += 8;
        self.short_idx = (u64::from_le_bytes(bytes[off..off+8].try_into().unwrap()) as usize) % SHORT_CAP;
        off += 8;
        for i in 0..SHORT_CAP {
            let bme = f32::from_le_bytes(bytes[off..off+4].try_into().unwrap()); off += 4;
            let owm = f32::from_le_bytes(bytes[off..off+4].try_into().unwrap()); off += 4;
            self.short[i] = PressureSample {
                bme_hpa: if bme.is_finite() { Some(bme) } else { None },
                owm_hpa: if owm.is_finite() { Some(owm) } else { None },
            };
        }

        // Read long ring
        self.long_count = (u64::from_le_bytes(bytes[off..off+8].try_into().unwrap()) as usize).min(LONG_CAP);
        off += 8;
        self.long_idx = (u64::from_le_bytes(bytes[off..off+8].try_into().unwrap()) as usize) % LONG_CAP;
        off += 8;
        for i in 0..LONG_CAP {
            let bme = f32::from_le_bytes(bytes[off..off+4].try_into().unwrap()); off += 4;
            let owm = f32::from_le_bytes(bytes[off..off+4].try_into().unwrap()); off += 4;
            self.long[i] = PressureSample {
                bme_hpa: if bme.is_finite() { Some(bme) } else { None },
                owm_hpa: if owm.is_finite() { Some(owm) } else { None },
            };
        }
    }
}
