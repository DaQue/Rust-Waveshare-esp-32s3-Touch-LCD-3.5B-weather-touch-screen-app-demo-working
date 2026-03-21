#!/usr/bin/env bash
# Flash firmware and clear the serial log.
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

> /tmp/esp_log.txt
echo "Log cleared."

# Kill any running capture script so it releases the serial port
pkill -f burn_in_capture || true
sleep 1

cd "$PROJECT_ROOT"
cargo +esp run -Zbuild-std=std,panic_abort 2>&1 | tee -a /tmp/esp_log.txt

# Restart capture after flash (auto-detects port)
nohup python3 /tmp/burn_in_capture.py >> /tmp/esp_log.txt 2>/tmp/capture_err.txt &
echo "Capture restarted (PID $!)"
