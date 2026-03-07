# Waveshare ESP32-S3 Touch LCD 3.5B — Rust Weather Dashboard

A fully functional weather dashboard firmware for the Waveshare ESP32-S3-Touch-LCD-3.5B,
written in Rust on top of ESP-IDF. Serves as both a daily-use dashboard and a practical
reference for Rust + ESP-IDF development on this hardware.

> **Status**: All dashboard features are working and stable on hardware. The files
> `current_issues.md` and `display_trials.md` are LCD bring-up lab notes from early
> development — useful if you are experimenting with panel configs, but not required for
> normal use.

---

## Table of Contents
- [Features](#features)
- [Gallery](#gallery)
- [Hardware](#hardware)
- [Prerequisites](#prerequisites)
- [Quick Start](#quick-start)
- [Build and Flash](#build-and-flash)
- [Credentials and Secrets](#credentials-and-secrets)
- [Serial Console](#serial-console)
- [Navigation Reference](#navigation-reference)
- [Architecture Notes](#architecture-notes)
- [Data Sources](#data-sources)
- [Project Layout](#project-layout)
- [Known Behaviors](#known-behaviors)

---

## Features

### Weather
- Current conditions: temperature (°F/°C toggle), feels-like, humidity, wind, pressure,
  condition text, and weather icons
- Outdoor temperature trend arrow (rising/falling/steady) based on recent fetch history
- 4-day forecast with icons and high/low temps
- Hourly drill-down per forecast day (tap a day row, scroll with swipe up/down)
- NWS weather alerts with severity color-coding (Warning/Watch/Advisory), overlay panel,
  optional auto-scope zone discovery, and speaker beep alerts

### Indoor Sensors (BME280)
- Current temperature, humidity, and pressure readings
- 1-hour rolling history graphs for temperature and humidity
- Auto-retry if sensor is absent at boot — reconnects within 30s of becoming available

### HVAC Detection
- Real-time heating/cooling state detection based on indoor temperature slope
- Cycle stats: total runtime, cycle count, average and longest cycle, short-cycle warnings
- 24-hour HVAC state history bar

### Pressure Trend
- 24-hour pressure graph with both BME280 (local) and OpenWeatherMap (remote) sources
- Delta readout comparing local vs remote
- Combined view with HVAC history on the same screen

### Navigation — 9 Views in 3 Groups

| Group   | Views                              |
|---------|------------------------------------|
| Weather | Now, Forecast                      |
| Sensors | Indoor, HVAC, Pressure+HVAC        |
| System  | I2C Scan, WiFi Scan, About         |

- Swipe left/right to move within the current group
- Reaching a group boundary (or swiping left from Now) opens the NavMenu hub
- Tap a NavMenu group card to jump directly to that group's entry view
- Swipe right from NavMenu returns to Now
- Header taps: left = back, center = home (Now), right = forward

### System Views
- **I2C Scan**: lists detected device addresses on the bus
- **WiFi Scan**: lists nearby networks with RSSI
- **About**: firmware version, uptime, heap stats, and device info

### Configuration
- All settings persisted in **encrypted NVS** and survive reboot
- Configurable at runtime via serial console — no reflash required
- Settings: Wi-Fi credentials, OpenWeather API key, location query, temperature units,
  NWS alerts, beep, auto-scope, user agent, zone, screen orientation and flip

---

## Gallery

![Dashboard](IMG_0431.png)
![Display](display02162601.jpg)

---

## Hardware

- **Board**: Waveshare ESP32-S3-Touch-LCD-3.5B
- **Display**: AXS15231B QSPI (320×480 native portrait, software-rotated to 480×320 landscape)
- **Touch**: CST3240 capacitive touch controller (I2C 0x3B)
- **IMU**: QMI8658 accelerometer/gyro for auto-rotation with hysteresis
- **Audio**: ES8311 codec + onboard speaker for alert beeps
- **PMU**: AXP2101 power management
- **Sensor**: Optional BME280 on I2C for indoor temperature, humidity, and pressure
- **Orientations**: Landscape, LandscapeFlipped, Portrait, PortraitFlipped (auto or locked)

---

## Prerequisites

- Rust ESP toolchain installed via [`espup`](https://github.com/esp-rs/espup)
  (so `cargo +esp` works)
- ESP-IDF environment exported: `source ~/export-esp.sh` (path varies per install)
- `espflash` installed for flashing
- `minicom` or `screen` for serial monitor

---

## Quick Start

1. Prepare credentials — see [Credentials and Secrets](#credentials-and-secrets)
2. Build and flash:
   ```
   cargo +esp run -Zbuild-std=std,panic_abort
   ```
3. Open the serial console, type `help`, and configure your location and API keys

---

## Build and Flash

Build, flash, and open monitor in one step:
```
cargo +esp run -Zbuild-std=std,panic_abort
```

Flash only (no monitor):
```
espflash flash target/xtensa-esp32s3-espidf/debug/waveshare_esp32-s3-touch-lcd-3p5b
```

For a release build, add `--release` to `cargo run` and flash from:
```
espflash flash target/xtensa-esp32s3-espidf/release/waveshare_esp32-s3-touch-lcd-3p5b
```

Monitor separately:
```
minicom -D /dev/ttyACM0 -b 115200
```

Helper scripts — use these if `cargo run` can't access the serial port without sudo:
```
./scripts/flash.sh          # flash with elevated permissions
./scripts/monitor.sh /dev/ttyACM0 115200   # monitor only
```

---

## Credentials and Secrets

### Option A — `wifi.local.rs` (build-time fallback)
1. Copy `wifi.local.rs.example` to `wifi.local.rs` (git-ignored)
2. Edit `WIFI_SSID`, `WIFI_PASS`, and `OPENWEATHER_API_KEY`
3. Build and flash

These values are compiled in as fallback defaults. NVS values set via the console take
precedence over `wifi.local.rs` on all future boots.

### Option B — Serial console at runtime
```
wifi set <ssid> <password>
api set-key <openweather_api_key>
api set-query lat=38.80&lon=-90.62
```

### Migrating to encrypted NVS (no re-typing)
- Keep `wifi.local.rs` populated
- Flash the firmware (NVS encryption is enabled by default via sdkconfig)
- Run `secrets seed-local` from the console to move values into encrypted NVS
- Verify with `wifi show` and `api show`

---

## Serial Console

Connect via `minicom -D /dev/ttyACM0 -b 115200`. Enable local echo with Ctrl+A E.
Type `help` for the full command list. Key commands:

```
status                           system status and heap stats
version                          firmware version
about                            firmware/device summary
reboot                           reboot device

wifi set <ssid> <pass>           set Wi-Fi credentials
wifi show                        show current Wi-Fi config
wifi scan                        scan for nearby networks
api set-key <key>                set OpenWeather API key
api set-query <query>            set location (e.g. lat=38.8&lon=-90.6)
units f|c|show                   temperature units

alerts on|off                    enable/disable NWS alerts
alerts beep on|off               enable/disable speaker beeps
alerts auto-scope on|off         auto-discover NWS zone from IP geolocation
alerts zone show|clear           show or clear cached NWS zone
alerts test warning              inject a fake warning for UI testing
alerts ua <string>               set NWS User-Agent header
alerts scope <scope>             set NWS scope (e.g. area=MO, zone=MOZ061)

orientation auto|landscape|portrait
orientation flip on|off|toggle|show

i2c scan                         rescan I2C bus
imu read                         one-shot IMU reading
debug <module>                   toggle debug logging (touch/bme280/wifi/weather/imu/all)
debug show                       show current debug flag state
beep advisory|watch|warning|stop speaker tone test / stop

secrets show                     show whether wifi.local.rs fallback values are compiled in
secrets seed-local               one-time copy of wifi.local.rs values into encrypted NVS
flash show                       show flash metadata
flash set-time <text>            set flash timestamp metadata
```

---

## Navigation Reference

```
Swipe left            advance within group; opens NavMenu at group boundary
Swipe right           go back within group; opens NavMenu at left boundary (not Now)
Swipe left from Now   opens NavMenu directly
Tap header left       go back within group; NavMenu at left boundary
Tap header center     go to Now (home)
Tap header right      advance within group; NavMenu at right boundary
NavMenu tap card      jump to group entry (Weather=Now / Sensors=Indoor / System=I2C Scan)
NavMenu swipe right   return to Now
```

**Now view:**
```
Tap temperature area   toggle °F/°C
Tap weather icon       no alerts: force weather refresh  |  alerts present: open overlay
Tap forecast card      navigate to Forecast view
```

**Forecast view:**
```
Tap a day row          open hourly drill-down
Swipe up/down          scroll hourly list
Swipe left             close hourly (if open); NavMenu at boundary
```

---

## Architecture Notes

- **Render thread**: dedicated thread owns the framebuffer and LCD context. The main loop
  clones `AppState` and sends it via a sync channel. The render thread flushes at its own
  pace without blocking the main loop.
- **Zero-heap AppState clone**: all `AppState` fields use `heapless` fixed-size types.
  Clone is a memcpy — no heap allocation on the hot path.
- **PSRAM**: framebuffer (300KB), DMA buffer (12.5KB), and HTTP body buffer (32KB) all
  allocated from PSRAM via `heap_caps_malloc`. Internal SRAM is reserved for TLS and stacks.
- **Touch debounce**: 3 consecutive polls (60ms) required before confirming a press,
  preventing spurious taps from the CST3240 controller.
- **Orientation**: framebuffer is reallocated only when switching between landscape and
  portrait dimensions — Landscape↔LandscapeFlipped reuses the same 480×320 buffer.
- **NVS encryption**: enabled by default using ESP-IDF's built-in encrypted NVS partition.
- **I2C**: runs at 100kHz (touch controller requires slower speed; 400kHz causes errors).

---

## Data Sources

- **OpenWeatherMap** (`api.openweathermap.org`): current conditions and 5-day/3-hour
  forecast. Requires a free API key.
- **National Weather Service** (`api.weather.gov`): active alert feed. No key required
  but a User-Agent header is mandatory (set via `alerts ua`).
- **ipapi.co**: IP-based geolocation used for NWS auto-scope zone discovery and default
  location query on first boot. No account required. To avoid this call entirely, set
  your location manually (`api set-query lat=…&lon=…`) and disable auto-scope
  (`alerts auto-scope off`).

---

## Project Layout

```
src/
  views/          UI view modules (now, forecast, indoor, hvac, pressure_hvac,
                  i2c_scan, wifi_scan, about, nav_menu, warning)
  icons/          BMP icon assets (26 files: 13 types × 80px and 36px)
  main.rs         main loop, sensor reads, weather fetch coordination
  bme280_sensor.rs BME280 driver (trimmed-mean read, calibration)
  hvac.rs         HVAC slope-detection and history
  pressure_history.rs 24-hour pressure ring buffer
  touch.rs        CST3240 touch driver + gesture classifier
  framebuffer.rs  PSRAM framebuffer + panel flush (4 orientation paths)
  layout.rs       colors, card/text drawing helpers, orientation constants
  weather.rs      OWM JSON parsing, icon mapping
  http_client.rs  HTTPS client with PSRAM body buffer
  config.rs       NVS config load/save
  console.rs      serial console command handler
  wifi.rs         Wi-Fi connect with retry
components/
  esp_lcd_axs15231b/  QSPI LCD panel driver
  board_power/        TCA9554 GPIO expander, LCD reset sequencing
  XPowersLib/         AXP2101 PMU driver
scripts/
  flash.sh        flash helper
  monitor.sh      monitor helper
tools/
  gen_icons.py    generate BMP icon assets from vector definitions
  preview_icons.py preview icons on dark background
wifi.local.rs.example   credentials template (copy to wifi.local.rs, git-ignored)
sdkconfig.defaults      ESP-IDF base configuration
```

---

## Known Behaviors

- **WiFi**: fails on attempt 1, connects on attempt 2 — normal for this hardware.
- **First weather fetch**: may fail at boot due to TLS memory pressure; self-recovers
  on the next retry cycle (~30s).
- **NWS cert errors**: intermittent PK verify failures (`FFFFBD70`) occur occasionally
  and self-recover within 1–2 retry cycles. OWM fetches are unaffected.
- **Outdoor trend arrow**: requires 3 weather fetches (~20 min after boot) before
  appearing. Nothing is drawn until sufficient history exists.
- **Pressure graph**: first line segment appears after ~5 minutes (2 samples at 5-min
  cadence).
- **HVAC stats section**: shows `(collecting data...)` for the first ~50 minutes
  (requires 10 history samples at 30s record rate).
- **BME280 absent at boot**: firmware retries init every 30 seconds until the sensor
  responds. No reboot required.
