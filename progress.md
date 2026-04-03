# Progress & Known Issues

## Open TODOs

- [ ] **Upgrade ESP-IDF v5.2.3 → v5.3.x+**
  Fixes lwIP `tcpip_thread_handle_msg` crash (LoadProhibited, EXCVADDR=0x1) on
  periodic SNTP re-sync after ~22h uptime. Workaround: removed SNTP callback
  (`EspSntp::new()` instead of `new_with_callback()`). See `src/time_sync.rs`.

## Recently Fixed

- **v0.7.3** — Removed SNTP re-sync callback to work around ESP-IDF v5.2.3 lwIP
  crash. Device was crashing after ~22h uptime on periodic SNTP re-sync.

## Historical: Display Bring-up (resolved ~2026-02-01)

LCD panel (AXS15231B, QSPI) was showing noise/blank screen during initial bring-up.

**Root causes resolved:**
- QSPI command framing: SPI mode 3, cmd bits 32, param bits 8, quad mode enabled
- Panel init: switched to `esp_lcd_new_panel_axs15231b` with vendor init table from factory `bsp_display.c`
- BSP component: `espressif/esp_lcd_axs15231b` v1.0.0 via ESP-IDF component manager
- Draw: full-frame `esp_lcd_panel_draw_bitmap`, always RAMWR (2C), never RAMWRC (3C)
- PMU (AXP2101): BLDO1/BLDO2 + ALDO1-4 + CPUSLDO + DLDO1/2 + DC3/DC5 all enabled over I2C

**Status:** Fully working. See `components/board_power/bsp_axp2101.cpp` and `components/esp_lcd_axs15231b/`.

## Security Notes (reviewed 2026-04-02)

- No hardcoded credentials — NVS-only, good
- HTTP endpoints unauthenticated (local network only, acceptable for now)
- Wildcard CORS on web server — low risk on LAN
- SSID logged in plaintext at INFO level (`src/console/cmd_wifi.rs`, `src/config.rs`)
- API key in OWM URL query string (forced by OWM API design)
- API key masking edge case: keys ≤4 chars shown in full (unlikely with OWM)
