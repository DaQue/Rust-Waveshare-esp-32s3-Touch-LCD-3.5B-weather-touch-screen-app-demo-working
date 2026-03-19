# Rust Project Context

## Cargo.toml
[package]
name = "waveshare_esp32-s3-touch-lcd-3p5b"
version = "0.4.19"
edition = "2021"

[dependencies]
anyhow = "1"
log = "0.4"

esp-idf-sys = { version = "0.36", features = ["binstart"] }
esp-idf-svc = "0.51"
esp-idf-hal = "0.45"

embedded-graphics = "0.8"
profont = "0.7"
tinybmp = "0.6"

serde = { version = "1", default-features = false, features = ["derive", "alloc"] }
serde_json = { version = "1", default-features = false, features = ["alloc"] }

libc = "0.2"
heapless = "0.8"
embedded-svc = "0.28"

[build-dependencies]
embuild = { version = "0.32", features = ["espidf"] }

[profile.dev]
panic = "abort"

[profile.release]
panic = "abort"

[package.metadata.esp-idf-sys]

[[package.metadata.esp-idf-sys.extra_components]]
component_dirs = ["components"]

## Module Tree
src/bme280_sensor.rs
src/config.rs
src/console.rs
src/debug_flags.rs
src/framebuffer.rs
src/http_client.rs
src/hvac.rs
src/layout.rs
src/main.rs
src/pressure_history.rs
src/psbox.rs
src/qmi8658.rs
src/speaker.rs
src/time_sync.rs
src/touch.rs
src/views/about.rs
src/views/forecast.rs
src/views/hvac.rs
src/views/i2c_scan.rs
src/views/indoor.rs
src/views/mod.rs
src/views/nav_menu.rs
src/views/now.rs
src/views/pressure_hvac.rs
src/views/settings.rs
src/views/warning.rs
src/views/wifi_scan.rs
src/weather_icons.rs
src/weather.rs
src/wifi.rs

## Public Structs / Enums / Traits
src/bme280_sensor.rs:17:pub struct Bme280Reading {
src/bme280_sensor.rs:23:pub struct Bme280 {
src/config.rs:42:pub enum OrientationMode {
src/config.rs:67:pub struct Config {
src/framebuffer.rs:27:pub struct Framebuffer {
src/hvac.rs:21:pub enum HvacState { Idle, Heating, Cooling }
src/hvac.rs:36:pub struct HvacModeStats {
src/hvac.rs:45:pub struct HvacStats {
src/hvac.rs:54:pub struct HvacDetector {
src/layout.rs:68:pub enum Orientation {
src/pressure_history.rs:21:pub struct PressureSample {
src/pressure_history.rs:29:pub struct PressureHistory {
src/psbox.rs:15:pub struct PsBox<T>(*mut T);
src/psbox.rs:86:pub struct PsBoxSlice<T> {
src/psbox.rs:172:pub struct PsramRing {
src/qmi8658.rs:32:pub struct ImuReading {
src/speaker.rs:16:pub enum AlertTone {
src/speaker.rs:49:pub struct Speaker<'d> {
src/touch.rs:31:pub enum Gesture {
src/touch.rs:41:pub struct TouchState {
src/views/mod.rs:30:pub enum View {
src/views/mod.rs:88:pub struct AppState {
src/views/nav_menu.rs:19:pub enum NavTap {
src/views/settings.rs:15:pub enum SettingsTap {
src/weather.rs:14:pub struct CurrentWeather {
src/weather.rs:28:pub struct ForecastRow {
src/weather.rs:41:pub struct HourlyEntry {
src/weather.rs:52:pub struct ForecastDay {
src/weather.rs:58:pub struct Forecast {
src/weather.rs:65:pub enum AlertKind {
src/weather.rs:85:pub struct WeatherAlert {
src/weather_icons.rs:20:pub enum WeatherIcon {
src/wifi.rs:10:pub struct WifiResult {

## Public Functions
src/bme280_sensor.rs:48:    pub fn init(i2c: &mut I2cDriver<'_>) -> Option<Self> {
src/bme280_sensor.rs:170:    pub fn read(&self, i2c: &mut I2cDriver<'_>) -> Option<Bme280Reading> {
src/config.rs:49:    pub fn as_str(self) -> &'static str {
src/config.rs:57:    pub fn parse(s: &str) -> Option<Self> {
src/config.rs:114:pub fn local_secret_fallbacks() -> (Option<String>, Option<String>, Option<String>) {
src/config.rs:122:pub fn weather_query_needs_autodiscovery(query: &str) -> bool {
src/config.rs:133:    pub fn load(nvs: &EspNvs<NvsDefault>) -> Config {
src/config.rs:225:    pub fn save_wifi(nvs: &mut EspNvs<NvsDefault>, ssid: &str, pass: &str) -> Result<()> {
src/config.rs:232:    pub fn save_weather_api_key(nvs: &mut EspNvs<NvsDefault>, key: &str) -> Result<()> {
src/config.rs:238:    pub fn save_weather_query(nvs: &mut EspNvs<NvsDefault>, query: &str) -> Result<()> {
src/config.rs:244:    pub fn save_use_celsius(nvs: &mut EspNvs<NvsDefault>, celsius: bool) -> Result<()> {
src/config.rs:251:    pub fn save_timezone(nvs: &mut EspNvs<NvsDefault>, tz: &str) -> Result<()> {
src/config.rs:257:    pub fn save_orientation_mode(
src/config.rs:266:    pub fn save_orientation_flip(nvs: &mut EspNvs<NvsDefault>, flip: bool) -> Result<()> {
src/config.rs:272:    pub fn save_flash_time(nvs: &mut EspNvs<NvsDefault>, flash_time: &str) -> Result<()> {
src/config.rs:278:    pub fn save_alerts_enabled(nvs: &mut EspNvs<NvsDefault>, enabled: bool) -> Result<()> {
src/config.rs:284:    pub fn save_alerts_beep(nvs: &mut EspNvs<NvsDefault>, enabled: bool) -> Result<()> {
src/config.rs:290:    pub fn save_alerts_auto_scope(nvs: &mut EspNvs<NvsDefault>, enabled: bool) -> Result<()> {
src/config.rs:296:    pub fn save_nws_user_agent(nvs: &mut EspNvs<NvsDefault>, user_agent: &str) -> Result<()> {
src/config.rs:302:    pub fn save_nws_scope(nvs: &mut EspNvs<NvsDefault>, scope: &str) -> Result<()> {
src/config.rs:308:    pub fn save_nws_zone(nvs: &mut EspNvs<NvsDefault>, zone: &str) -> Result<()> {
src/console.rs:12:pub fn spawn_console(
src/debug_flags.rs:28:pub fn is_on(flag: &AtomicBool) -> bool {
src/debug_flags.rs:32:pub fn set(flag: &AtomicBool, val: bool) {
src/debug_flags.rs:36:pub fn toggle(flag: &AtomicBool) -> bool {
src/debug_flags.rs:42:pub fn status_line() -> String {
src/debug_flags.rs:53:pub fn request_orientation_mode(mode: crate::config::OrientationMode) {
src/debug_flags.rs:62:pub fn take_orientation_mode_request() -> Option<crate::config::OrientationMode> {
src/debug_flags.rs:71:pub fn request_orientation_flip(flip: bool) {
src/debug_flags.rs:75:pub fn take_orientation_flip_request() -> Option<bool> {
src/debug_flags.rs:83:pub fn request_beep_tone(tone: i8) {
src/debug_flags.rs:88:pub fn take_beep_tone_request() -> Option<i8> {
src/debug_flags.rs:95:pub fn request_beep_stop() {
src/debug_flags.rs:99:pub fn take_beep_stop_request() -> bool {
src/framebuffer.rs:37:    pub fn new(width: u32, height: u32) -> Self {
src/framebuffer.rs:77:    pub fn clear_color(&mut self, color: Rgb565) {
src/framebuffer.rs:83:    pub fn flush_to_panel(
src/http_client.rs:272:pub fn https_get_json<T, F>(url: &str, headers: &[(&str, &str)], f: F) -> Result<T>
src/http_client.rs:294:pub fn https_get_with_headers(url: &str, headers: &[(&str, &str)]) -> Result<String> {
src/hvac.rs:75:    pub fn new(detect_period_secs: f32, record_period_secs: f32) -> Self {
src/hvac.rs:93:    pub fn detect(&mut self, temp_c: f32, now_ms: u32) {
src/hvac.rs:145:    pub fn record(&mut self) {
src/hvac.rs:153:    pub fn state(&self) -> HvacState { self.current_state }
src/hvac.rs:155:    pub fn state_duration_secs(&self, now_ms: u32) -> u32 {
src/hvac.rs:159:    pub fn history_count(&self) -> usize { self.hist_count }
src/hvac.rs:161:    pub fn stats(&self) -> HvacStats {
src/layout.rs:76:    pub fn is_landscape(self) -> bool {
src/layout.rs:80:    pub fn is_portrait(self) -> bool {
src/layout.rs:102:pub fn screen_w(orientation: Orientation) -> i32 {
src/layout.rs:110:pub fn screen_h(orientation: Orientation) -> i32 {
src/layout.rs:118:pub fn screen_size(orientation: Orientation) -> (i32, i32) {
src/layout.rs:138:pub fn word_wrap(text: &str, max_chars: usize) -> Vec<String> {
src/layout.rs:188:pub fn draw_vline(fb: &mut Framebuffer, x: i32, y1: i32, y2: i32, color: Rgb565) {
src/layout.rs:197:pub fn draw_hline(fb: &mut Framebuffer, y: i32, color: Rgb565) {
src/layout.rs:207:pub fn draw_card(
src/main.rs:286:pub fn now_ms() -> u32 {
src/pressure_history.rs:39:    pub fn new() -> Self {
src/pressure_history.rs:53:    pub fn push_short(&mut self, bme_hpa: Option<f32>, owm_hpa: Option<f32>) {
src/pressure_history.rs:60:    pub fn push_long(&mut self, bme_hpa: Option<f32>, owm_hpa: Option<f32>) {
src/pressure_history.rs:100:    pub fn short_bme_series(&self) -> Vec<(usize, f32)> { self.extract_series_short(|s| s.bme_hpa) }
src/pressure_history.rs:101:    pub fn short_owm_series(&self) -> Vec<(usize, f32)> { self.extract_series_short(|s| s.owm_hpa) }
src/pressure_history.rs:102:    pub fn long_bme_series(&self)  -> Vec<(usize, f32)> { self.extract_series_long(|s| s.bme_hpa) }
src/pressure_history.rs:103:    pub fn long_owm_series(&self)  -> Vec<(usize, f32)> { self.extract_series_long(|s| s.owm_hpa) }
src/pressure_history.rs:139:    pub fn short_bme_min_max(&self) -> Option<(f32, f32)> { self.min_max_short(|s| s.bme_hpa) }
src/pressure_history.rs:140:    pub fn short_owm_min_max(&self) -> Option<(f32, f32)> { self.min_max_short(|s| s.owm_hpa) }
src/pressure_history.rs:141:    pub fn long_bme_min_max(&self)  -> Option<(f32, f32)> { self.min_max_long(|s| s.bme_hpa) }
src/pressure_history.rs:142:    pub fn long_owm_min_max(&self)  -> Option<(f32, f32)> { self.min_max_long(|s| s.owm_hpa) }
src/pressure_history.rs:144:    pub fn long_len(&self)  -> usize { self.long_count }
src/pressure_history.rs:145:    pub fn short_len(&self) -> usize { self.short_count }
src/pressure_history.rs:150:    pub fn latest_bme(&self) -> Option<f32> {
src/pressure_history.rs:155:    pub fn latest_owm(&self) -> Option<f32> {
src/pressure_history.rs:172:    pub fn delta_owm_bme_recent(&self, n: usize) -> Option<f32> {
src/pressure_history.rs:192:    pub fn delta_owm_bme_stable(&self) -> Option<f32> {
src/pressure_history.rs:213:    pub fn pressure_trend(&self) -> Option<(f32, &'static str)> {
src/pressure_history.rs:247:    pub fn serialised_size() -> usize {
src/pressure_history.rs:255:    pub fn write_bytes(&self, out: &mut [u8]) {
src/pressure_history.rs:278:    pub fn load_from_bytes(&mut self, bytes: &[u8]) {
src/psbox.rs:22:    pub fn new(val: T) -> Self {
src/psbox.rs:96:    pub fn new(len: usize) -> Self {
src/psbox.rs:182:    pub fn new(capacity: usize) -> Self {
src/psbox.rs:186:    pub fn capacity(&self) -> usize { self.data.len() }
src/psbox.rs:187:    pub fn len(&self)      -> usize { self.len }
src/psbox.rs:189:    pub fn is_empty(&self) -> bool  { self.len == 0 }
src/psbox.rs:193:    pub fn push_back(&mut self, v: f32) {
src/psbox.rs:206:    pub fn as_slices(&self) -> (&[f32], &[f32]) {
src/qmi8658.rs:61:pub fn init(i2c: &mut I2cDriver<'_>) -> bool {
src/qmi8658.rs:104:pub fn read(i2c: &mut I2cDriver<'_>) -> Option<ImuReading> {
src/speaker.rs:23:    pub fn from_request(v: i8) -> Option<Self> {
src/speaker.rs:32:    pub fn as_str(self) -> &'static str {
src/speaker.rs:40:    pub fn request_code(self) -> i8 {
src/speaker.rs:80:pub fn enable_pa(i2c: &mut I2cDriver<'_>) -> Result<(), EspError> {
src/speaker.rs:91:pub fn init_audio_path(i2c: &mut I2cDriver<'_>) -> Result<(), EspError> {
src/speaker.rs:97:pub fn init_es8311(i2c: &mut I2cDriver<'_>) -> Result<(), EspError> {
src/speaker.rs:155:    pub fn new(
src/speaker.rs:279:    pub fn play<F: FnMut() -> bool>(&mut self, tone: AlertTone, mut should_stop: F) -> Result<(), EspError> {
src/time_sync.rs:16:pub fn sync_time(tz: &str) -> Result<EspSntp<'static>> {
src/time_sync.rs:55:pub fn format_local_time() -> Option<String> {
src/touch.rs:60:    pub fn new() -> Self {
src/touch.rs:80:    pub fn poll(
src/touch.rs:276:pub fn probe(i2c: &mut I2cDriver<'_>) {
src/views/about.rs:13:pub fn draw(fb: &mut Framebuffer, state: &AppState) {
src/views/forecast.rs:11:pub fn draw(fb: &mut Framebuffer, state: &AppState) {
src/views/hvac.rs:18:pub fn draw(fb: &mut Framebuffer, state: &AppState) {
src/views/i2c_scan.rs:26:pub fn draw(fb: &mut Framebuffer, state: &AppState) {
src/views/indoor.rs:20:pub fn draw(fb: &mut Framebuffer, state: &AppState) {
src/views/mod.rs:52:    pub fn next(self) -> Option<View> {
src/views/mod.rs:68:    pub fn prev(self) -> Option<View> {
src/views/mod.rs:132:    pub fn new() -> Self {
src/views/mod.rs:177:    pub fn screen_w(&self) -> i32 {
src/views/mod.rs:181:    pub fn screen_h(&self) -> i32 {
src/views/mod.rs:186:    pub fn handle_gesture(&mut self, gesture: Gesture) -> bool {
src/views/mod.rs:502:pub fn draw_current_view(fb: &mut Framebuffer, state: &AppState) {
src/views/nav_menu.rs:49:pub fn hit_test(x: i16, y: i16, orientation: Orientation) -> Option<NavTap> {
src/views/nav_menu.rs:127:pub fn draw(fb: &mut Framebuffer, state: &AppState) {
src/views/now.rs:82:pub fn draw(fb: &mut Framebuffer, state: &AppState) {
src/views/pressure_hvac.rs:30:pub fn draw(fb: &mut Framebuffer, state: &AppState) {
src/views/settings.rs:41:pub fn hit_test(x: i16, y: i16, orientation: Orientation) -> Option<SettingsTap> {
src/views/settings.rs:58:pub fn draw(fb: &mut Framebuffer, state: &AppState) {
src/views/warning.rs:18:pub fn draw(fb: &mut Framebuffer, state: &AppState) {
src/views/wifi_scan.rs:13:pub fn draw(fb: &mut Framebuffer, state: &AppState) {
src/weather.rs:73:    pub fn as_str(self) -> &'static str {
src/weather.rs:98:    pub fn kind(&self) -> AlertKind {
src/weather.rs:201:pub fn map_condition_to_icon(weather_id: i32, _icon_code: &str) -> WeatherIcon {
src/weather.rs:259:pub fn parse_current_weather(json: &str) -> Result<CurrentWeather> {
src/weather.rs:314:pub fn parse_forecast(json: &str) -> Result<Forecast> {
src/weather.rs:494:pub fn fetch_weather(
src/weather.rs:521:pub fn format_alert_expiry(iso: &str) -> String {
src/weather.rs:546:pub fn log_alert_to_console(alert: &WeatherAlert) {
src/weather.rs:572:pub fn parse_nws_alerts(json: &str) -> Result<Vec<WeatherAlert>> {
src/weather.rs:606:pub fn fetch_nws_alerts(scope: &str, user_agent: &str) -> Result<Vec<WeatherAlert>> {
src/weather.rs:641:pub fn discover_openweather_query(user_agent: &str) -> Result<String> {
src/weather.rs:646:pub fn discover_nws_zone(user_agent: &str) -> Result<String> {
src/weather_icons.rs:141:    pub fn draw_80(self, fb: &mut Framebuffer, x: i32, y: i32) {
src/weather_icons.rs:148:    pub fn draw_48(self, fb: &mut Framebuffer, x: i32, y: i32) {
src/weather_icons.rs:155:    pub fn draw_36(self, fb: &mut Framebuffer, x: i32, y: i32) {
src/wifi.rs:55:pub fn connect_wifi(
src/wifi.rs:139:pub fn reconnect_existing(
src/wifi.rs:188:pub fn scan_wifi(

## Coding Preferences

### Build & Fix Before Flashing
- ALWAYS run `cargo +esp build -Zbuild-std=std,panic_abort` and fix all errors before
  asking the user to flash or before flashing yourself.
- Do not ask the user to test on hardware until the build is clean.

### Flashing the Board
When a build is ready to flash, prompt the user to run:
```
cargo +esp run -Zbuild-std=std,panic_abort
```

### Watching the Log (no flash)
To monitor the serial output and save to disk without flashing:
```
minicom -D /dev/ttyACM0 -b 115200 -C /tmp/burn_in.log
```

### Version Bump + Commit on Flash-Ready Build
When a build is clean and ready to flash:
1. Bump the patch version in Cargo.toml by 0.0.1.
2. Run `cargo +esp build -Zbuild-std=std,panic_abort` to confirm the version change builds.
3. Create a local git commit with a short message describing what changed.
   Do NOT push yet.

### Minor Version Bump (0.1.0) + Push to GitHub
When told to bump the version by 0.1.0:
1. Bump the minor version in Cargo.toml (e.g. 0.4.x → 0.5.0, reset patch to 0).
2. Run `cargo +esp clippy -Zbuild-std=std,panic_abort` and fix ALL warnings.
3. Run `cargo +esp build -Zbuild-std=std,panic_abort` — must be clean.
4. Review all changes for AI slop:
   - Remove filler comments that restate the code ("// increment counter").
   - Remove unnecessary doc comments on private/obvious items.
   - Remove dead code, unused imports, or gratuitous abstractions introduced during edits.
   - Ensure variable/function names are idiomatic Rust, not verbose AI-style names.
   - Remove any backwards-compatibility shims or _unused suffixes on intentionally removed items.
5. Commit with a clear message, then `git push`.

Context generated on Mon Mar 16 05:37:39 PM CDT 2026
