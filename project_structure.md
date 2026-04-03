# Project Structure — waveshare_esp32-s3-touch-lcd-3p5b

## Thread architecture

| Thread | Spawned by | Purpose |
|---|---|---|
| `main` | OS | Init, shared state, main loop (5s tick) |
| `render` | `render::spawn_render_thread()` | Owns framebuffer + LCD, draws `AppState` on demand |
| `console` | `console::spawn_console()` | Serial command dispatch |
| `sensor_poll` | `sensor_poll::spawn_sensor_poll()` | BME280 + QMI8658 reads every 5s |
| `weather_task` | `weather_task::spawn_weather_task()` | OWM fetch every 5min, NWS alerts every 3min |
| `web_server` | `web_server::start_web_server()` | Embedded HTTP server |

Threads communicate via `Arc<Mutex<AppState>>` snapshot posted to `render_slot`, plus `Arc<AtomicBool>` flags for side-channel signals.

---

## Key shared state

**`AppState`** (`src/views/mod.rs`) — snapshot cloned into render slot each tick. Contains current weather, indoor sensor data, history refs, HVAC state, active view, orientation.

**`Config`** (`src/config.rs`) — runtime config loaded from NVS. WiFi SSID, API key, units, orientation mode, alert settings. Shared as `Arc<Mutex<Config>>`.

**`HistoryRing`** (`src/history_ring.rs`) — PSRAM ring buffer, 10,080 samples × 24B = ~236 KB. One `HistorySample` per minute, 7-day window. Shared as `Arc<Mutex<HistoryRing>>`.

**`WebSnapshot`** (`src/web_server.rs`) — flattened snapshot for `/api/current`, updated every 5s by main loop. Shared as `Arc<Mutex<WebSnapshot>>`.

---

## Source modules

### Core / plumbing
| File | Key items |
|---|---|
| `main.rs` | Entry point, all thread spawning, global atomics (`SRAM_BME_RESET`, `SRAM_DO_REBOOT`), timing constants, `now_ms()`, `esp_check()` |
| `psbox.rs` | `PsBox<T>`, `PsBoxSlice<T>`, `PsramRing` — PSRAM allocators; use instead of `Box`/`Vec` for large data |
| `framebuffer.rs` | `Framebuffer`, `LcdContext` — owns DMA buffer, `flush_fb()` drives panel |
| `layout.rs` | `Orientation`, `framebuffer_dims()`, coordinate helpers |
| `render.rs` | `spawn_render_thread()` — polls render slot, calls `views::draw_current_view()`, handles orientation change + FB realloc |
| `debug_flags.rs` | `AtomicBool` flags per module (touch, bme280, wifi, weather, imu); `REQUEST_HISTORY_SAVE`; `RENDER_FLUSH_ACTIVE` |
| `config.rs` | `Config` struct, NVS load/save, `OrientationMode` |

### Sensors
| File | Key items |
|---|---|
| `bme280_sensor.rs` | `Bme280Sensor::new()`, `read()` → `(temp_f, humidity, pressure_hpa)` |
| `qmi8658.rs` | IMU driver, `read_accel_gyro()` — used for orientation detection |
| `sensor_poll.rs` | `spawn_sensor_poll()` — drives BME280 + IMU on 5s interval, posts to shared state |

### Weather
| File | Key items |
|---|---|
| `weather/mod.rs` | `CurrentWeather`, `ForecastRow`, `Forecast`, `NwsAlert` structs; `fetch_current()`, `fetch_forecast()`, `fetch_alerts()` |
| `weather/parse.rs` | OWM JSON → `CurrentWeather`/`Forecast`; maps OWM icon codes to `WeatherIcon` |
| `weather/geo.rs` | IP geolocation → lat/lon → NWS zone lookup |
| `weather_task.rs` | `spawn_weather_task()` — orchestrates fetch loop, retry logic, alert beep triggers |
| `weather_icons.rs` | `WeatherIcon` enum, 54 BMP assets embedded at compile time |
| `http_client.rs` | `http_fetch_into()` — PSRAM-buffered HTTPS GET, connection reuse, SRAM guard |

### History & persistence
| File | Key items |
|---|---|
| `history_ring.rs` | `HistoryRing`, `HistorySample` (24B, repr C), `HISTORY_CAP=10080` |
| `history_nvs.rs` | Ping-pong NVS slots (gen counter), `save_history()`, `restore_history()` |
| `pressure_history.rs` | `PressureHistory` — trend calculation (rising/falling/steady) stored in PSRAM via `PsBox` |
| `hvac.rs` | `HvacDetector`, `HvacState` (Idle/Heating/Cooling) — inferred from temp delta |

### Views
| File | Key items |
|---|---|
| `views/mod.rs` | `View` enum, `AppState`, swipe navigation (next/prev), `draw_current_view()` |
| `views/splash.rs` | Boot screen, `draw_splash()` |
| `views/nav_menu.rs` | Main menu, group navigation |
| `views/now.rs` | Home weather screen |
| `views/forecast.rs` | 4-day forecast cards |
| `views/indoor.rs` | Indoor temp/humidity/pressure |
| `views/hvac.rs` | HVAC state display |
| `views/pressure_hvac.rs` | Pressure trend + HVAC combined |
| `views/settings.rs` | WiFi/API config UI (touch keyboard) |
| `views/about.rs` | Firmware version, uptime, memory |
| `views/warning.rs` | NWS alert overlay |
| `views/wifi_scan.rs` | AP scan results |
| `views/i2c_scan.rs` | I2C bus scanner |

### Console commands
| File | Commands |
|---|---|
| `console/mod.rs` | `spawn_console()`, dispatch, `help`/`about`/`status`/`reboot`/`history`/`log` |
| `console/cmd_wifi.rs` | `wifi set/show/scan/clear`, `api set-key/set-query/show/clear`, `secrets` |
| `console/cmd_device.rs` | `units`, `flash`, `orientation`, `i2c`, `imu`, `debug`, `beep` |
| `console/cmd_alerts.rs` | `alerts on/off/show/silence/beep/ua/scope/zone/test` |

### Networking
| File | Key items |
|---|---|
| `wifi.rs` | `connect_wifi()`, reconnect logic |
| `time_sync.rs` | `sync_time(tz)` → `EspSntp` (keep alive); `format_local_time()` |
| `web_server.rs` | `start_web_server()`, endpoints: `GET /`, `/api/current`, `/api/history?hours=N`, `/api/alerts`, `/api/silence` |

### Hardware / drivers
| File/Dir | Purpose |
|---|---|
| `touch.rs` | Touch driver, `Gesture` enum (Tap, SwipeLeft, SwipeRight, etc.) |
| `speaker.rs` | Buzzer tones: advisory / watch / warning |
| `components/board_power/` | AXP2101 PMU init (C++) — called from Rust via FFI |
| `components/esp_lcd_axs15231b/` | AXS15231B QSPI display panel driver |
| `components/XPowersLib/` | PMU support library |

---

## Memory model

- **Internal SRAM** (~50 KB free at runtime) — stack, TLS contexts, lwIP
- **PSRAM** (~7.5 MB free) — `HistoryRing`, `PressureHistory`, `HvacDetector`, HTTP body buffer, framebuffer DMA
- Rule: anything > ~4 KB goes in PSRAM via `PsBox` / `PsBoxSlice`
- SRAM low-water guards: `SRAM_ADMIT_MIN_BLOCK=20KB`, `SRAM_CRITICAL_BLOCK=12KB`, `REBOOT_THRESHOLD=7KB`

---

## Navigation model

Swipe left/right moves within a group. Group boundaries open `NavMenu`. Center-header tap returns to `Now`.

```
Weather:  Now ↔ Forecast
Sensors:  Indoor ↔ Hvac ↔ PressureHvac
System:   Settings ↔ About ↔ WifiScan ↔ I2cScan
Special:  NavMenu, Warning (overlay)
```
