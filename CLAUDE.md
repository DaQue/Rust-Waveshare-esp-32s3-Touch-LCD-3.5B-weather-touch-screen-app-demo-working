# Claude Code Rules — waveshare_esp32-s3-touch-lcd-3p5b

## Project orientation
- BSP: local dep at `../ws_s3_3p5_bsp` (path dep in Cargo.toml)
- Display: AXS15231B panel, QSPI, custom component in `components/`
- Views: `src/views/` — 10 screens, nav via touch
- Secrets: extracted from `wifi.local.rs` at build time via `build.rs` (never committed)
- Full module map, thread architecture, and memory model: see `project_structure.md`

## Build command
```
cargo fmt && cargo +esp build -Zbuild-std=std,panic_abort
```
Always run `cargo fmt` before building. Build-only (no flash). Confirm clean before committing.

## Flash workflow
1. Run build command — confirm it passes
2. Bump `version` in `Cargo.toml`
3. Commit (include version in message suffix, e.g. `fix: ... (v0.7.2)`)
4. Offer `./scripts/flash.sh` — never flash without asking the user first

`scripts/flash.sh` pipes `cargo +esp run` through `tee -a /tmp/esp_log.txt`.

## Version bumping
- Bump `Cargo.toml` version for every flash-worthy change
- Format: `major.minor.patch` — patch for fixes, minor for new features

## Linting (cargo clippy)
Run strict linting under these conditions:

```bash
cargo clippy -- -D warnings
```

- Every 5 successful builds
- Any version bump ≥ +0.1.0 (minor or major)
- Treat all warnings as errors — fix before committing

## WiFi credentials / API key
- Never commit to git
- Never log or display
- Stored in NVS only, set via serial console:
  - `wifi set <ssid> <pass>`
  - `api set-key <key>`

## Secrets in general
- `wifi_ssid`, `wifi_pass`, `wx_api_key` are NVS-only — never in source, never in logs

## Discuss before coding
- Don't change code while the user is still asking questions
- Wait for explicit approval ("go", "yes", "do it") before making edits

## Build before flash
- Always run the build command and confirm clean before offering to flash
- Never ask the user to flash without a successful build in the same session

## Known issues / TODOs
- **ESP-IDF v5.2.3 → v5.3.x upgrade needed**: lwIP `tcpip_thread_handle_msg` crashes
  (LoadProhibited, EXCVADDR=0x1) on periodic SNTP re-sync after ~22h uptime.
  Workaround in place: removed SNTP callback, using `EspSntp::new()` instead of
  `new_with_callback()`. Root fix requires ESP-IDF upgrade.

## Log file
- `/tmp/esp_log.txt` — appended by `scripts/flash.sh` via `tee -a`
- Useful for post-mortem crash analysis (search for `Guru Meditation`, `panic`, `rst:0x`)

## Keeping this file current
- Update CLAUDE.md when: new workarounds land, rules change, or TODOs are resolved
- Update `progress.md` when: fixes are made, security issues are noted, or TODOs change
- Warn the user if CLAUDE.md reaches 200 lines — time to trim
- Never let a session end with a significant decision unrecorded
