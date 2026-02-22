Waveshare ESP32-S3 Touch LCD 3.5B Weather Demo (Rust + ESP-IDF)
=================================================================

This project is a practical reference/demo for getting the Waveshare
ESP32-S3-Touch-LCD-3.5B running in Rust. It is intended as a working baseline
you can build from, not a polished end-user product.

Intended Scope
--------------
- Show a complete, working Rust firmware path for this board:
  - display init + rendering
  - touch input
  - Wi-Fi + HTTPS
  - weather + forecast + NWS alerts
  - console configuration commands
  - optional speaker/beep alerts
- Provide a reproducible starting point for your own app work on this hardware.

Current Features
----------------
- Weather dashboard with current conditions + forecast view
- NWS alerts fetch with optional auto-scope discovery and alert beeps
- Touch navigation and orientation support (landscape/portrait/flipped/auto)
- NVS-persisted config (Wi-Fi, API, units, alerts, orientation, metadata)
- Encrypted NVS support
- Serial console commands (`help`) for runtime configuration and diagnostics

Prereqs
-------
- ESP Rust/ESP-IDF toolchain installed
- For this setup, environment is commonly loaded from `/home/david/export-esp.sh`

Build
-----
Preferred build used in this repo:
```
cargo build --target xtensa-esp32s3-espidf --release
```

Flash + Monitor
---------------
Preferred flash/monitor flow:
```
cargo run --target xtensa-esp32s3-espidf --release -- -p /dev/ttyACM0
```

Optional helper script:
```
./scripts/flash.sh
```
or without sudo:
```
./scripts/flash.sh --no-sudo
```

Local Secrets (`wifi.local.rs`)
-------------------------------
- `wifi.local.rs` is git-ignored and intended for local credentials.
- The file is read at build time and used only as fallback defaults.
- Precedence at runtime is:
  - `NVS` values (if previously set via console), then
  - `wifi.local.rs` values embedded at build time, then
  - built-in safe defaults (empty SSID/pass/API key).
- Create/edit flow:
  1. Copy `wifi.local.rs.example` to `wifi.local.rs`.
  2. Edit `WIFI_SSID`, `WIFI_PASS`, `OPENWEATHER_API_KEY`.
  3. Rebuild/flash.
- Console support for migration:
  - `secrets show` (shows whether local fallback values are present at build time)
  - `secrets seed-local` (copies local fallback values into NVS)
- After you set values via console (`wifi set ...`, `api set-key ...`), NVS will override
  `wifi.local.rs` on future boots.

NVS Encryption Transition (No Retyping)
---------------------------------------
- Keep `wifi.local.rs` populated (git-ignored).
- NVS encryption is enabled by project defaults (`CONFIG_NVS_ENCRYPTION=y`) and uses
  ESP-IDF built-in partition table `partitions_singleapp_encr_nvs.csv`.
- Flash and boot the encrypted-NVS firmware once.
- Run:
  - `secrets seed-local`
- This persists local fallback secrets into NVS so future boots can ignore local fallback.
- Verify:
  - `wifi show`
  - `api show`
  - `status`
- Runtime defaults when keys are missing:
  - units: Fahrenheit (`F`)
  - alerts: enabled
  - alerts auto-scope: enabled

Notes
-----
- USB-Serial-JTAG is used for input, not UART0.
- Main task stack is configured as `CONFIG_ESP_MAIN_TASK_STACK_SIZE=32768`.

Troubleshooting
---------------
- No response to typing: make sure the board is using USB-Serial-JTAG, not an
  external USB-UART bridge.
- Build errors about the target: ensure `espup` is installed and the environment
  is loaded with `source /home/david/export-esp.sh`.

Recent Orientation Updates (2026-02-21)
---------------------------------------
- Firmware/package version is now `0.2.3`.
- Runtime screen orientation now supports all 4 physical directions:
  - `Landscape` (USB right)
  - `LandscapeFlipped` (USB left)
  - `Portrait` (USB bottom)
  - `PortraitFlipped` (USB top)
- Auto mode uses the QMI8658 accelerometer with hysteresis to prevent rapid flip noise.
- Locked mode still uses `landscape`/`portrait`, with optional 180-degree flip.
- Orientation changes trigger full redraw.
- Framebuffer reallocation happens only when switching between landscape and portrait dimensions.
- Touch coordinates are remapped per active orientation.
- Swipe directions are normalized so navigation matches on-screen direction in flipped modes.

Console Commands for Orientation
--------------------------------
- `orientation auto`
- `orientation landscape`
- `orientation portrait`
- `orientation flip on`
- `orientation flip off`
- `orientation flip toggle`
- `orientation flip show`

Behavior Notes
--------------
- `orientation flip ...` is available only when locked to `landscape` or `portrait`.
- In `auto` mode, `orientation flip` prints a guidance message and does not apply.
- Orientation mode and flip are persisted in NVS and survive reboot.

NWS Alerts (Current)
--------------------
- The firmware can poll active NWS alerts from `api.weather.gov`.
- NWS calls include required headers:
  - `User-Agent`
  - `Accept: application/geo+json`
- Current manual scope defaults to `area=MO`.
- Backward compatibility: `state=XX` is accepted in config and auto-normalized to `area=XX`.
- Alerts config is persisted in NVS:
  - `alerts_enabled`
  - `alerts_auto_scope`
  - `nws_user_agent`
  - `nws_scope`
  - `nws_zone` (cached auto-discovered forecast zone, e.g. `MOZ061`)
  - `flash_time` metadata
- Alert config changes from console apply at runtime (no reboot required).
- Auto-scope flow:
  - geolocate via `https://ipapi.co/json/`
  - resolve zone via `https://api.weather.gov/points/{lat},{lon}`
  - poll alerts via `alerts/active?zone=<ZONE>`

Console Commands for Alerts / Metadata
--------------------------------------
- `units show|f|c`
- `version` (prints firmware version, e.g. `v0.2.3`)
- `about` (firmware/device summary plus `help` hint)
- `beep advisory|watch|warning|stop` (speaker tone test / stop)
- `alerts show`
- `alerts on`
- `alerts off`
- `alerts beep on|off|show`
- `alerts auto-scope on|off`
- `alerts ua <user-agent>`
- `alerts scope <scope>` (example: `area=MO`, `zone=MOZ061`)
- `alerts zone show|clear`
- `flash show`
- `flash set-time <text>`

Now View Alert UX
-----------------
- Status text color reflects top alert class:
  - Warning: red
  - Watch: yellow
  - Advisory: amber
- Tapping weather icon:
  - No alerts: weather refresh
  - Alerts present: open/close alert details overlay

Problems Encountered and Fixes
------------------------------
- Audio output was initially silent even though `beep` commands were accepted.
  - Root cause: I2S TX data pin mismatch and codec/path setup differences from factory behavior.
  - Fixes:
    - Switched I2S TX output to the factory-working data pin (`GPIO16`).
    - Kept audio path initialization explicit (PA enable + ES8311 init).
    - Aligned tone path to 48 kHz and tuned tone envelopes/patterns.
- Audio tones initially could continue longer than expected.
  - Root cause: I2S/DMA path could hold non-zero samples after command completion.
  - Fixes:
    - Enabled channel auto-clear.
    - Explicitly flushed silence after tones and on stop path.
    - Added `beep stop` console command.
- Startup watchdog warnings occurred during concurrent TLS handshakes.
  - Root cause: weather and alerts HTTPS handshakes started at nearly the same time.
  - Fix:
    - Added startup delay before first alerts poll to avoid handshake collision.
- A render-loop watchdog warning appeared during some icon-heavy draws.
  - Root cause: long uninterrupted pixel loops could starve lower-priority task scheduling.
  - Fix:
    - Added cooperative yields during large framebuffer draw/fill loops.
