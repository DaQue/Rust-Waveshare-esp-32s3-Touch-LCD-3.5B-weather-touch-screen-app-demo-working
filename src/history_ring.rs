/// PSRAM-backed 7-day sensor history ring buffer.
///
/// Stores one sample per minute (10,080 samples = 7 days × 24h × 60m).
/// Backed by `PsBoxSlice<HistorySample>` — no internal SRAM used.
/// History is ephemeral: lost on reboot (no NVS checkpoint in v0.6.0).
///
/// # Memory budget
/// 10,080 samples × 24 bytes = 241,920 bytes (~236 KB) in PSRAM.
use crate::psbox::PsBoxSlice;

pub const HISTORY_CAP: usize = 10_080;   // 7 days × 24h × 60m
pub const SAMPLE_VERSION: u8 = 3;

/// One minute of indoor sensor data. Fixed at 24 bytes (repr C, aligned).
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct HistorySample {
    pub timestamp:    u32,      // Unix epoch seconds
    pub temp_f:       f32,      // Indoor temperature °F
    pub humidity_pct: f32,      // Indoor relative humidity %
    pub pressure_hpa: f32,      // Indoor pressure hPa (altitude-corrected)
    pub hvac_state:          u8,  // HvacState: 0=Idle 1=Heating 2=Cooling
    pub outdoor_temp_u8:     u8,  // outdoor °F encoded: 0=no data, else (temp_f+41) as u8
    pub outdoor_press_u16:   u16, // OWM sea-level pressure × 10 (e.g. 10132 = 1013.2 hPa); 0 = no data
    pub version:      u8,         // Schema version — currently SAMPLE_VERSION
    pub _pad:         [u8; 3],    // Padding to 24 bytes
}

const _: () = assert!(
    core::mem::size_of::<HistorySample>() == 24,
    "HistorySample must be exactly 24 bytes"
);

/// Fixed-capacity ring buffer backed by PSRAM.
pub struct HistoryRing {
    data: PsBoxSlice<HistorySample>,
    head: usize,   // index of oldest element
    len:  usize,   // number of valid elements (0..=HISTORY_CAP)
}

unsafe impl Send for HistoryRing {}
unsafe impl Sync for HistoryRing {}

impl HistoryRing {
    /// Allocate the ring buffer in PSRAM. Panics if PSRAM is unavailable.
    pub fn new() -> Self {
        Self {
            data: PsBoxSlice::new(HISTORY_CAP),
            head: 0,
            len: 0,
        }
    }

    #[allow(dead_code)]
    pub fn len(&self)      -> usize { self.len }
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool  { self.len == 0 }

    /// Push a new sample. Overwrites the oldest when full.
    pub fn push(&mut self, sample: HistorySample) {
        let tail = (self.head + self.len) % HISTORY_CAP;
        self.data[tail] = sample;
        if self.len == HISTORY_CAP {
            self.head = (self.head + 1) % HISTORY_CAP;
        } else {
            self.len += 1;
        }
    }

    /// Iterate samples oldest-first, optionally limited to the most recent
    /// `hours` hours. Pass `hours = 0` to return all samples.
    pub fn iter_recent(&self, hours: u32) -> impl Iterator<Item = &HistorySample> {
        let cap     = HISTORY_CAP;
        let count   = if hours == 0 {
            self.len
        } else {
            (hours as usize * 60).min(self.len)
        };
        let start   = self.len.saturating_sub(count);
        let offset  = (self.head + start) % cap;

        HistoryIter {
            data:    &self.data,
            offset,
            remaining: count,
        }
    }
}

struct HistoryIter<'a> {
    data:      &'a PsBoxSlice<HistorySample>,
    offset:    usize,
    remaining: usize,
}

impl<'a> Iterator for HistoryIter<'a> {
    type Item = &'a HistorySample;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let item = &self.data[self.offset];
        self.offset = (self.offset + 1) % HISTORY_CAP;
        self.remaining -= 1;
        Some(item)
    }
}
