#!/usr/bin/env bash
# Flash firmware and clear the serial log.
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

> /tmp/esp_log.txt
echo "Log cleared."

cd "$PROJECT_ROOT"
exec cargo +esp run -Zbuild-std=std,panic_abort
