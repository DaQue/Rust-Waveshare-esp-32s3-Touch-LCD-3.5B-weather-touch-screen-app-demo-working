// 24-hour pressure history ring buffer (fixed-size, no heap growth).
//
// Stores BME280 (local) and OpenWeather (remote) pressure at 5-minute
// intervals. 288 samples = 24 hours.  Either source may be `None` if
// unavailable at sample time.
//
// Tuning:
// - `SAMPLE_PERIOD_SECS`: how often to push a new sample (300 = 5 min)
// - `CAPACITY`: ring size (288 = 24h at 5-min cadence)

// ── Constants ───────────────────────────────────────────────────────

pub const SAMPLE_PERIOD_SECS: u32 = 300; // 5 minutes
pub const WINDOW_SECS: u32 = 24 * 3600;
pub const CAPACITY: usize = (WINDOW_SECS / SAMPLE_PERIOD_SECS) as usize; // 288

// ── Sample ──────────────────────────────────────────────────────────

#[derive(Clone, Copy, Default)]
pub struct PressureSample {
    pub bme_hpa: Option<f32>,
    pub owm_hpa: Option<f32>,
}

// ── Ring buffer ─────────────────────────────────────────────────────

pub struct PressureHistory {
    buf: [PressureSample; CAPACITY],
    idx: usize,
    count: usize,
}

impl PressureHistory {
    pub fn new() -> Self {
        Self {
            buf: [PressureSample::default(); CAPACITY],
            idx: 0,
            count: 0,
        }
    }

    /// Push a new 5-minute sample.  Either field may be `None`.
    pub fn push(&mut self, bme_hpa: Option<f32>, owm_hpa: Option<f32>) {
        self.buf[self.idx] = PressureSample { bme_hpa, owm_hpa };
        self.idx = (self.idx + 1) % CAPACITY;
        if self.count < CAPACITY {
            self.count += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.count
    }

    /// Iterate samples oldest → newest.
    fn iter(&self) -> impl Iterator<Item = &PressureSample> {
        let start = if self.count < CAPACITY { 0 } else { self.idx };
        (0..self.count).map(move |i| &self.buf[(start + i) % CAPACITY])
    }

    /// Extract indexed BME pressure points for graphing.
    /// Returns `(index, value)` pairs; gaps (None) are skipped.
    pub fn bme_series(&self) -> Vec<(usize, f32)> {
        self.extract_series(|s| s.bme_hpa)
    }

    /// Extract indexed OWM pressure points for graphing.
    pub fn owm_series(&self) -> Vec<(usize, f32)> {
        self.extract_series(|s| s.owm_hpa)
    }

    fn extract_series(&self, f: impl Fn(&PressureSample) -> Option<f32>) -> Vec<(usize, f32)> {
        let mut out = Vec::new();
        for (i, s) in self.iter().enumerate() {
            if let Some(v) = f(s) {
                if v.is_finite() {
                    out.push((i, v));
                }
            }
        }
        out
    }

    /// Min/max across valid samples for a given extractor.
    fn min_max(&self, extract: impl Fn(&PressureSample) -> Option<f32>) -> Option<(f32, f32)> {
        let mut lo = f32::INFINITY;
        let mut hi = f32::NEG_INFINITY;
        let mut found = false;
        for s in self.iter() {
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

    pub fn bme_min_max(&self) -> Option<(f32, f32)> {
        self.min_max(|s| s.bme_hpa)
    }

    pub fn owm_min_max(&self) -> Option<(f32, f32)> {
        self.min_max(|s| s.owm_hpa)
    }

    /// Most recent valid BME sample, if available.
    pub fn latest_bme(&self) -> Option<f32> {
        self.latest_value(|s| s.bme_hpa)
    }

    /// Most recent valid OWM sample, if available.
    pub fn latest_owm(&self) -> Option<f32> {
        self.latest_value(|s| s.owm_hpa)
    }

    fn latest_value(&self, extract: impl Fn(&PressureSample) -> Option<f32>) -> Option<f32> {
        if self.count == 0 {
            return None;
        }
        let start = if self.count < CAPACITY { 0 } else { self.idx };
        for i in 0..self.count {
            let rev_i = (start + self.count - 1 - i) % CAPACITY;
            if let Some(v) = extract(&self.buf[rev_i]) {
                if v.is_finite() {
                    return Some(v);
                }
            }
        }
        None
    }

    /// Average delta (OWM - BME) over the last `n` samples where both exist.
    /// Used for the comparison readout.
    pub fn delta_owm_bme_recent(&self, n: usize) -> Option<f32> {
        let mut sum = 0.0f32;
        let mut count = 0u32;
        // Walk backwards from newest
        let start = if self.count < CAPACITY { 0 } else { self.idx };
        let take = self.count.min(n);
        for i in 0..take {
            let rev_i = (start + self.count - 1 - i) % CAPACITY;
            let s = &self.buf[rev_i];
            if let (Some(bme), Some(owm)) = (s.bme_hpa, s.owm_hpa) {
                if bme.is_finite() && owm.is_finite() {
                    sum += owm - bme;
                    count += 1;
                }
            }
        }
        if count > 0 {
            Some(sum / count as f32)
        } else {
            None
        }
    }
}
