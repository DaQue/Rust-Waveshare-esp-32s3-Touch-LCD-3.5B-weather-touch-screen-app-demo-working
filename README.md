Waveshare ESP32-S3 3.5" Weather Station Bring-up
=================================================

This repo is a bring-up playground for an ESP32-S3 board. It currently focuses on
LCD bring-up with a trial harness.

Features
--------
- ESP-IDF + Rust std setup
- USB-Serial-JTAG input for mode selection and trial annotations
- Auto sweep mode (unattended) and interactive sweep mode
- NVS persistence of last trial marker, last mode, last result, and auto sweep count
- Per-trial display reset/cleanup for repeatable bring-up checks

Prereqs
-------
- ESP Rust toolchain installed via `espup`
- ESP environment vars available from `/home/david/export-esp.sh`

Build
-----
```
source /home/david/export-esp.sh
cargo +esp build -Zbuild-std=std,panic_abort
```

Flash + Monitor
--------------
```
source /home/david/export-esp.sh
cargo +esp run -Zbuild-std=std,panic_abort
```

If `sudo` is required for USB access, use the helper:
```
./scripts/flash.sh
```

To skip sudo (if you already have device permissions):
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

Trial Flow
----------
1) Open the monitor (via `cargo +esp run ...` or `./scripts/flash.sh`).
2) At boot, the firmware prompts and waits until you enter a valid mode:
   - `a` (default): auto mode, resume from saved trial
   - `a1`: auto mode from trial #1
   - `i`: interactive mode, resume from saved trial
   - `i1`: interactive mode from trial #1
   - `i <n>`: interactive mode from trial `n`
3) Each trial shows solid green, then a blue box with a yellow X.
4) In interactive mode, enter result codes:
   - `p`, `px`, `pxb`, `gx`, `gxb`, `pbn`, `vjg`, `vjb`, `hjgb`, `hjbb`, `n`, `b`
5) In interactive mode, control commands:
   - `r` rerun current trial
   - `jmp <n>` jump to trial `n`
   - `nvs` print persisted marker/state
   - `q` quit interactive mode
   - `h` show help
6) The harness prints progress with base trial number as `#/N` and resets/cleans up between trials.

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
- Firmware/package version is now `0.2.2`.
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
- `alerts show`
- `alerts on`
- `alerts off`
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
