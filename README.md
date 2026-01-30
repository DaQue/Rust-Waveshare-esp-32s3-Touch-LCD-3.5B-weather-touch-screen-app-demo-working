Waveshare ESP32-S3 3.5" Weather Station Bring-up
=================================================

This repo is a Phase 0 bring-up for an ESP32-S3 board. It proves USB serial
two-way communication and gives a steady heartbeat on the log.

Features
--------
- ESP-IDF + Rust std setup
- USB-Serial-JTAG RX/TX loop
- Commands: `ping` + Enter -> `pong`, anything else -> `ping`
- `alive` log once per second

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

Serial Test
-----------
1) Open the monitor (via `cargo +esp run ...`).
2) Type `ping` and press Enter. Expect `pong`.
3) Type any other text and press Enter. Expect `ping`.

Notes
-----
- USB-Serial-JTAG is used for RX/TX, not UART0.
- `sdkconfig.defaults` sets `CONFIG_ESP_MAIN_TASK_STACK_SIZE=8192`.

Troubleshooting
---------------
- No response to typing: make sure the board is using USB-Serial-JTAG, not an
  external USB-UART bridge.
- Build errors about the target: ensure `espup` is installed and the environment
  is loaded with `source /home/david/export-esp.sh`.
