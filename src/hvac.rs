use core::fmt;

// ── Fast detection constants (5s sample rate) ───────────────────────
const FAST_WINDOW: usize = 3;          // slope over 3 intervals = 15s of data
const FAST_BUF_SIZE: usize = FAST_WINDOW + 1; // need N+1 samples
const FAST_CONFIRM: u8 = 1;            // min candidate_count to commit (1 = commit on first observation)

// ── Slope thresholds (C/min) ────────────────────────────────────────
const HEAT_ON_SLOPE: f32 = 0.03;
const COOL_ON_SLOPE: f32 = -0.03;
const HEAT_OFF_SLOPE: f32 = 0.01;
const COOL_OFF_SLOPE: f32 = -0.01;

// ── History constants (30s record rate) ────────────────────────────
const SHORT_CYCLE_MINS: u32 = 4;
const HISTORY_SIZE: usize = 2880;      // 24h at 30s/sample

// ── HvacState ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HvacState { Idle, Heating, Cooling }

impl fmt::Display for HvacState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HvacState::Idle    => write!(f, "IDLE"),
            HvacState::Heating => write!(f, "HEATING"),
            HvacState::Cooling => write!(f, "COOLING"),
        }
    }
}

// ── Stats ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Default)]
pub struct HvacModeStats {
    pub total_minutes: u32,
    pub cycles: u32,
    pub avg_cycle_mins: f32,
    pub longest_cycle_mins: u32,
    pub short_cycles: u32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct HvacStats {
    pub heat: HvacModeStats,
    pub cool: HvacModeStats,
    pub history_minutes: u32,
}

// ── Detector ────────────────────────────────────────────────────────

pub struct HvacDetector {
    // Detection state
    current_state: HvacState,
    state_start_ms: u32,
    candidate_state: HvacState,
    candidate_count: u8,

    // Fast detection ring (raw temps)
    fast_buf: [f32; FAST_BUF_SIZE],
    fast_idx: usize,
    fast_count: usize,
    fast_period_mins: f32,  // detect_period_secs / 60

    // Slow history ring
    history: [HvacState; HISTORY_SIZE],
    hist_idx: usize,
    hist_count: usize,
    hist_period_mins: f32,  // record_period_secs / 60
}

impl HvacDetector {
    pub fn new(detect_period_secs: f32, record_period_secs: f32) -> Self {
        Self {
            current_state: HvacState::Idle,
            state_start_ms: 0,
            candidate_state: HvacState::Idle,
            candidate_count: 0,
            fast_buf: [0.0; FAST_BUF_SIZE],
            fast_idx: 0,
            fast_count: 0,
            fast_period_mins: detect_period_secs / 60.0,
            history: [HvacState::Idle; HISTORY_SIZE],
            hist_idx: 0,
            hist_count: 0,
            hist_period_mins: record_period_secs / 60.0,
        }
    }

    /// Call every `detect_period_secs` (e.g. 5s). Updates current state.
    pub fn detect(&mut self, temp_c: f32, now_ms: u32) {
        self.fast_buf[self.fast_idx] = temp_c;
        self.fast_idx = (self.fast_idx + 1) % FAST_BUF_SIZE;
        self.fast_count = self.fast_count.min(FAST_BUF_SIZE - 1) + 1;

        // Anchor state_start_ms to first real timestamp so duration display is meaningful from startup
        if self.fast_count == 1 {
            self.state_start_ms = now_ms;
        }

        if self.fast_count < FAST_BUF_SIZE {
            return; // not enough samples yet
        }

        let oldest = self.fast_idx; // oldest slot after increment
        let newest = (self.fast_idx + FAST_BUF_SIZE - 1) % FAST_BUF_SIZE;
        let elapsed_mins = FAST_WINDOW as f32 * self.fast_period_mins;
        let slope = (self.fast_buf[newest] - self.fast_buf[oldest]) / elapsed_mins;

        let proposed = match self.current_state {
            HvacState::Idle => {
                if slope >= HEAT_ON_SLOPE { HvacState::Heating }
                else if slope <= COOL_ON_SLOPE { HvacState::Cooling }
                else { HvacState::Idle }
            }
            HvacState::Heating => {
                if slope < HEAT_OFF_SLOPE { HvacState::Idle } else { HvacState::Heating }
            }
            HvacState::Cooling => {
                if slope > COOL_OFF_SLOPE { HvacState::Idle } else { HvacState::Cooling }
            }
        };

        if proposed != self.current_state {
            if proposed == self.candidate_state {
                self.candidate_count += 1;
            } else {
                self.candidate_state = proposed;
                self.candidate_count = 1;
            }
            if self.candidate_count >= FAST_CONFIRM {
                self.current_state = proposed;
                self.state_start_ms = now_ms;
                self.candidate_count = 0;
            }
        } else {
            self.candidate_count = 0;
            self.candidate_state = self.current_state;
        }
    }

    /// Call every `record_period_secs` (e.g. 30s). Appends current state to history.
    pub fn record(&mut self) {
        self.history[self.hist_idx] = self.current_state;
        self.hist_idx = (self.hist_idx + 1) % HISTORY_SIZE;
        if self.hist_count < HISTORY_SIZE {
            self.hist_count += 1;
        }
    }

    pub fn state(&self) -> HvacState { self.current_state }

    pub fn state_duration_secs(&self, now_ms: u32) -> u32 {
        now_ms.wrapping_sub(self.state_start_ms) / 1000
    }

    pub fn history_count(&self) -> usize { self.hist_count }

    pub fn stats(&self) -> HvacStats {
        if self.hist_count == 0 { return HvacStats::default(); }

        let mut heat = HvacModeStats::default();
        let mut cool = HvacModeStats::default();

        let start = if self.hist_count < HISTORY_SIZE { 0 } else { self.hist_idx };
        let mut prev_state = HvacState::Idle;
        let mut run_len: u32 = 0;
        let mut first = true;

        for i in 0..self.hist_count {
            let idx = (start + i) % HISTORY_SIZE;
            let s = self.history[idx];
            if first || s != prev_state {
                if !first {
                    Self::close_run(prev_state, run_len, self.hist_period_mins, &mut heat, &mut cool);
                }
                prev_state = s;
                run_len = 1;
                first = false;
            } else {
                run_len += 1;
            }
        }
        if !first {
            Self::close_run(prev_state, run_len, self.hist_period_mins, &mut heat, &mut cool);
        }

        if heat.cycles > 0 { heat.avg_cycle_mins = heat.total_minutes as f32 / heat.cycles as f32; }
        if cool.cycles > 0 { cool.avg_cycle_mins = cool.total_minutes as f32 / cool.cycles as f32; }

        HvacStats {
            heat,
            cool,
            history_minutes: (self.hist_count as f32 * self.hist_period_mins).round() as u32,
        }
    }

    fn close_run(
        state: HvacState, run_len_samples: u32, period_mins: f32,
        heat: &mut HvacModeStats, cool: &mut HvacModeStats,
    ) {
        let target = match state {
            HvacState::Heating => heat,
            HvacState::Cooling => cool,
            HvacState::Idle => return,
        };
        let run_mins = (run_len_samples as f32 * period_mins).round() as u32;
        target.total_minutes += run_mins;
        target.cycles += 1;
        if run_mins > target.longest_cycle_mins { target.longest_cycle_mins = run_mins; }
        if run_mins < SHORT_CYCLE_MINS { target.short_cycles += 1; }
    }
}
