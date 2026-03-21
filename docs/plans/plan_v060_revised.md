# ESP32 Environmental Dashboard (V0.6.0) — REVISED IMPLEMENTATION PLAN

## GOAL
Add a web dashboard to the existing Waveshare ESP32-S3 firmware with local and remote (Tailscale) access to environmental data while protecting display/sensor loop stability under low-SRAM conditions.

---

## HARDWARE-AWARE MEMORY AND CONCURRENCY MODEL
1. **Loop Isolation Is Mandatory**
   - Display and sensor loops remain independent and must never block on HTTP work.
   - HTTP processing runs on a separate task/core with explicit priority and watchdog visibility.

2. **SRAM Is Control Plane, PSRAM Is Data Plane**
   - SRAM is reserved for small fixed control state, driver/runtime internals, and hot-path loop data.
   - PSRAM is used for large or bursty allocations: history ring storage, HTTP scratch buffers, and large response staging.
   - Any allocation larger than 1KB should default to PSRAM unless proven otherwise.

3. **No Large Inline Containers in SRAM**
   - Do not use large `heapless::*` capacities for history or dashboard payloads.
   - `heapless` is allowed only for small fixed-size control buffers where SRAM residency is intentional.

4. **Flash-Resident Static Assets**
   - Dashboard assets (`index.html`, JS/CSS, icons) are embedded in flash via `include_str!`/`include_bytes!`.
   - Pre-gzip assets before embedding; serve with `Content-Encoding: gzip` to reduce flash usage and transfer time.
   - Do not duplicate full assets into RAM.

5. **Fragmentation Strategy**
   - Preallocate long-lived PSRAM buffers at boot and reuse them.
   - Avoid frequent alloc/free cycles in SRAM request paths.
   - Avoid per-sample temporary allocations during JSON encoding.

6. **Wi-Fi Drop Tolerance**
   - Reuse existing `wifi.rs` reconnect strategy.
   - HTTP server must not start until WiFi is confirmed connected. Bind only after successful join.
   - HTTP failures due to link loss must fail fast and not impact display/sensor tasks.

7. **HTTP Admission Control Is SRAM Protection, Not Security**
   - Local network traffic is trusted; no authentication is required for LAN access.
   - Admission control exists solely to protect SRAM from being exhausted by concurrent heavy
     requests (e.g. `/api/history`) during TLS handshakes or other peak-SRAM moments.
   - Thresholds defined in Stage 0 must be wired into the server from Stage 1 onward.

---

## STAGE 0 — BASELINE SAFETY CHECKS (DO THIS FIRST)
### Implementation
- **PSRAM availability check**: Verify PSRAM exists and report size at boot. Fail-fast with clear
  error log if PSRAM is unavailable — the dashboard design assumes PSRAM for history storage.
  Add this check before any PSRAM allocations are attempted.
- **Partition baseline**: Run `idf.py build` and record current app binary size before Stage 1.
  This establishes the headroom budget for Stage 5 asset embedding.
- Capture baseline memory/runtime metrics before dashboard changes:
  - internal free heap
  - largest internal free block
  - free PSRAM
  - largest free PSRAM block
- Add a lightweight periodic memory log line (e.g., every 5-10s in debug builds).
- Define and commit hard safety threshold constants for HTTP admission control:
  - `SRAM_ADMIT_MIN_BLOCK`: minimum largest-free-block to serve heavy endpoints (history/UI)
  - `SRAM_CRITICAL_BLOCK`: minimum below which all non-essential HTTP is rejected
  - `REBOOT_THRESHOLD`: largest block below this for `REBOOT_STREAK_COUNT` consecutive checks triggers graceful reboot
- Define reboot streak tracking:
  - `REBOOT_STREAK_COUNT`: consecutive critical checks before reboot (recommended: 5, at 5s interval = 25s)
- These constants are used from Stage 1 onward; do not defer enforcement to Stage 6.
- **Logging framework**: Use `log::*` macros consistently. Log level controlled via `CONFIG_LOG_DEFAULT_LEVEL`.
  All memory-related events (admission control triggers, reboot decisions) must emit `WARN` or above.

### Success Criteria
- PSRAM presence/size confirmed in boot logs.
- Current app binary size documented.
- Baseline metrics are visible in logs.
- Threshold constants are committed, documented, and referenced in server code from Stage 1.

---

## STAGE 1 — HTTP SERVER IN MAIN LOOP (POLLING MODEL)
### Architecture Decision
- **HTTP server runs in the main loop via polling**, not as a separate task.
- `EspHttpServer::poll()` is called each loop iteration; it returns quickly when no requests are pending.
- No cross-core synchronization needed — state is shared directly without locks.
- Single watchdog coverage — simpler failure modes.
- If polling overhead becomes measurable (unlikely at this traffic level), revisit.

### Implementation
- Initialize `EspHttpServer` only after WiFi join is confirmed.
- Call `server.poll()` in the main loop alongside display and sensor updates.
- Register `GET /` returning a flash-resident static response (`"ESP32 OK"`).
- Wire in admission control from the start: check `SRAM_ADMIT_MIN_BLOCK` before every handler.
  Return HTTP 503 with a brief message if below threshold.

### Success Criteria
- `http://<ip>/` returns `"ESP32 OK"`.
- Display FPS and sensor cadence remain stable during repeated requests.
- No stack overflow or watchdog resets under basic request load.
- 503 response observed when threshold is manually lowered in testing.

---

## STAGE 2 — SHARED STATE + CURRENT DATA ENDPOINT
### Design Decision (resolved)
- **Chosen approach: Extend `AppState`**. `AppState` already exists and owns sensor data.
  Single-threaded main loop means no locks needed — same task writes and reads.
  Add a `web_snapshot` field to `AppState` with documentation clarifying ownership contract:
  - Main loop writes `web_snapshot` every 5s
  - HTTP handlers read `web_snapshot` directly during `server.poll()` iteration

### Implementation
- Main loop updates the shared snapshot every 5s.
- `GET /api/current` handler reads from `web_snapshot` and serializes directly.
- No synchronization primitives needed — all access is sequential within the main loop.
- Add `Access-Control-Allow-Origin: *` header to all API responses (required if dashboard is ever
  served from a different origin, e.g. during development).

### Success Criteria
- `/api/current` is valid JSON and updates reliably.
- No visible display stutter from API polling.

---

## STAGE 3 — PSRAM HISTORY STORAGE (7 DAYS @ 1-MIN)
### Design Decisions (resolved)
- **Scope: Indoor-only for v0.6.0**. Wind/UV fields deferred to future version. Struct is fixed at
  24 bytes/sample; outdoor extension requires schema migration (new version tag + ring wipe).
- **NVS persistence: Accept loss on reboot**. Checkpoint adds NVS write cycles and complexity.
  Given reboots are rare and memory-pressure-induced (making NVS writes risky), accept that
  history is ephemeral in PSRAM. Document this in user-facing docs.

### Sample Struct (final)

| Field        | Type    | Bytes | Notes                        |
|--------------|---------|-------|------------------------------|
| timestamp    | u32     | 4     | Unix epoch, seconds          |
| temp_c       | f32     | 4     | BME280 temperature           |
| humidity_pct | f32     | 4     | BME280 humidity              |
| pressure_hpa | f32     | 4     | BME280 pressure              |
| _reserved    | [u8; 4] | 4     | Padding for future fields    |
| version      | u8      | 1     | Schema version tag           |
| _pad         | [u8; 3] | 3     | Align to 24 bytes            |

Total: 24 bytes/sample.

### Implementation
- Store history in PSRAM-backed ring storage (not large inline `heapless` capacity).
- Target retention: 10,080 samples (7 days x 24h x 60m).
- Preallocate ring capacity once at boot; avoid grow/shrink at runtime.
- History is lost on reboot — no NVS checkpoint in v0.6.0.

### Memory Budget
- 10,080 samples x 24 bytes = 241,920 bytes (~236KB) at the above struct size.
- Reserve additional PSRAM headroom for HTTP buffers and future schema growth.
- Total PSRAM budget for history: 256KB (allows ~8KB headroom).

### Success Criteria
- History retention reaches target without SRAM regressions.
- No sustained increase in SRAM fragmentation metrics.
- Ring buffer initializes successfully at boot (assert on allocation failure).

---

## STAGE 4 — STREAMING HISTORY API (NO PER-SAMPLE HEAP CHURN)
### Implementation
- Add `GET /api/history` with streaming JSON array output.
- Support `?hours=N` query parameter (default: 24, max: 168). Returning all 10,080 samples
  unconditionally is ~400-600KB of JSON per request; range filtering is required for usability
  and reduces per-request memory pressure significantly.
- Do not build one giant JSON `String`/`Vec<u8>`.
- Do not allocate per sample (avoid `serde_json::to_vec` in loop).
- **Chunked transfer approach**: First, validate `EspHttpServer` supports chunked
  `transfer-encoding: chunked` responses. If yes, stream samples directly to response writer.
  If no, use a PSRAM-backed buffer (32KB) for the response and flush when full.
  Document the chosen approach in code comments.
- Use a reusable preallocated scratch buffer in PSRAM for JSON serialization of individual samples
  (stack-based `utoa`/`ftoa` is preferred over `itoa`/`sprintf` for embedded).

### Success Criteria
- History responses complete without OOM or watchdog reset.
- `?hours=N` filtering works correctly at boundaries.
- Heap usage remains stable across repeated history fetches.
- Chunked transfer mode documented (if applicable).

---

## STAGE 5 — FLASH-RESIDENT DASHBOARD UI
### Pre-Implementation Decisions (resolved)
1. **Charting library: uPlot**. Best size/feature ratio for embedded. ~40KB minified+gzipped.
   - Alternatives rejected: Hand-rolled (too much dev time), Chart.js (larger), Plotly (too large).
   - uPlot handles zoom/pan well and is canvas-based (SVG overhead avoided).
2. **Asset pipeline**: Pre-gzip all assets offline using `zopfli` or `gzip -9`. Embed via
   `include_bytes!` with `const` visibility for compile-time verification.
3. **Remaining flash budget**: Document before and after. Minimum 64KB headroom required
   post-embedding; if less, trim assets or increase partition size first.

### Asset Manifest (v0.6.0)
| Asset | Est. Size (raw) | Est. Size (gz) | Notes |
|-------|-----------------|----------------|-------|
| index.html | 8KB | 3KB | Single-page app shell |
| uPlot (~standalone) | 45KB | 38KB | Include minified uPlot |
| app.js | 10KB | 4KB | Dashboard logic |
| styles.css | 3KB | 1KB | Minimal styling |
| **Total** | ~66KB | ~46KB | |

### Implementation
- Serve `index.html` and static assets from flash with correct content types and
  `Content-Encoding: gzip` for all assets.
- Dashboard pulls `/api/current` and `/api/history?hours=24` on page load.
- CORS headers already in place from Stage 2.
- Consider a lightweight build script (`assets/build.sh`) to automate gzipping and
  verify total embedded size against budget before each build.

### Success Criteria
- Dashboard loads on LAN and renders charts correctly.
- Build fits partition table with documented remaining margin (minimum 64KB).
- Total embedded asset size documented in build output.

---

## STAGE 6 — SRAM GUARDRAIL TUNING AND HARDENING
*Note: Basic admission control (503 on low SRAM) is wired in from Stage 1.
Thresholds are defined in Stage 0. This stage is for tuning against real traffic and
implementing the streak/reboot logic.*

### Reboot Policy (concrete)
When `largest_free_block` drops below `REBOOT_THRESHOLD`:
1. Log `WARN` with memory stats and current uptime.
2. Start streak counter; increment on each critical check.
3. If streak reaches `REBOOT_STREAK_COUNT` (default: 5, at 5s interval = 25s sustained):
   - Log `ERROR` with final memory stats.
   - Attempt graceful shutdown: stop HTTP server, flush any pending sensor data.
   - Call `esp_restart()` after 500ms delay.
4. If `largest_free_block` recovers above `REBOOT_THRESHOLD`, reset streak counter and log `INFO`.

### Degradation Levels (implement in order)
| Level | Trigger | Action |
|-------|---------|--------|
| 0 — Normal | Above `SRAM_ADMIT_MIN_BLOCK` | Full service |
| 1 — Degraded | Below `SRAM_ADMIT_MIN_BLOCK` | Reject `/api/history`; serve `/api/current` only |
| 2 — Critical | Below `SRAM_CRITICAL_BLOCK` | Reject all non-essential HTTP; serve 503 |
| 3 — Reboot pending | `REBOOT_STREAK_COUNT` consecutive Level 2 | Graceful restart |

### Implementation
- Tune `SRAM_ADMIT_MIN_BLOCK` and `SRAM_CRITICAL_BLOCK` against observed metrics from Stages 1-5.
- Add streak duration tracking as described above.
- Log state transitions (Normal → Degraded → Critical) at appropriate levels.

### Success Criteria
- Under memory pressure, device degrades gracefully through levels 0–2.
- Reboot triggers only after sustained critical state; device recovers if pressure eases.
- All state transitions logged at appropriate severity levels.
- Thresholds documented with rationale based on measured data.

---

## STAGE 7 — TAILSCALE REMOTE ACCESS
### Implementation
- Use Linux host as Tailscale subnet router to reach ESP32 LAN IP.
- Put reverse proxy (e.g. nginx or caddy) in front of ESP32 endpoint.
- Restrict reverse proxy to required routes only (`/`, `/api/current`, `/api/history`).
- Enforce tailnet ACL scope and host firewall rules.
- **Authentication**: Tailscale network membership is the access boundary (no additional auth
  needed for personal use). If shared tailnet access is a concern, add basic auth at the
  reverse proxy layer. Document the chosen stance explicitly.
- Traffic within the tailnet is encrypted by Tailscale; no HTTPS termination needed on the ESP32.

### Success Criteria
- Dashboard reachable from tailnet clients.
- Remote exposure is limited to documented routes.
- Auth stance is explicitly documented.

---

## TESTING STRATEGY
### Unit Tests (per stage)
- Stage 1: Test admission control thresholds (mock/freeze memory values).
- Stage 2: Test `web_snapshot` serialization to JSON (valid JSON output, correct field values).
- Stage 3: Test ring buffer push/overwrite logic (boundary conditions, full buffer).
- Stage 4: Test history filtering (`?hours=N` boundaries, empty result, full window).
- Stage 5: Test gzip serving (verify `Content-Encoding` header, asset sizes).
- Use `#[cfg(test)]` modules; run with `cargo test`.

### Integration Tests
- Manual validation matrix below (no CI for ESP target without hardware).
- Consider `std` feature gate for tests that can run on host (ring buffer logic, JSON serialization).

### Validation Matrix (required before merge)
1. Repeated `/api/current` polling at 1Hz for 30+ min: no FPS regression, no crashes.
2. Repeated `/api/history` pulls (default window and `?hours=168`): no progressive heap loss,
   no watchdog reset.
3. Wi-Fi disconnect/reconnect during active polling: server recovers, loops unaffected.
4. **Admission control under pressure**: manually lower `SRAM_ADMIT_MIN_BLOCK` to current free
   block level, then hammer `/api/history` — confirm 503 responses, display/sensor loops
   unaffected, SRAM recovers after load stops.
5. 24h soak test with dashboard active: stable memory trend and no reboot loops.
6. **Reboot trigger test**: artificially set `largest_free_block` below `REBOOT_THRESHOLD`,
   confirm graceful restart after `REBOOT_STREAK_COUNT` intervals.

---

## FINAL RULES
1. Display/sensor loops are priority workloads and must never be blocked by dashboard traffic.
2. Large data structures belong in PSRAM, preallocated and reused.
3. Static assets stay in flash, pre-gzipped, served without whole-file RAM copies.
4. Prefer predictable memory lifetime over allocation convenience in all HTTP code paths.
5. Admission control enforces SRAM safety; local network trust does not override it.
6. **Resolved decisions** (do not revisit without new requirements):
   - Charting: uPlot
   - SharedState: Extend `AppState`
   - NVS persistence: None (accept loss on reboot)
   - History scope: Indoor-only (BME280 only)
   - HTTP server: Polling model in main loop (not separate task)
7. All pre-implementation design decisions must be resolved before the relevant stage begins.
8. Unit tests required per stage before merge; integration tests manual on hardware.
