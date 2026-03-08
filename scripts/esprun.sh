#!/usr/bin/env bash
# Build, flash, and monitor the ESP32-S3 — logs to log.txt alongside terminal.
# Usage: esprun.sh [--sudo]   (sudo only needed if not in the dialout group)
# Exit monitor: Ctrl+A then X

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ESP_ENV="/home/david/export-esp.sh"
BUILD_FLAGS="-Zbuild-std=std,panic_abort"
LOGFILE="$ROOT_DIR/log.txt"
USER_HOME="$HOME"
USER_CARGO_HOME="${CARGO_HOME:-$USER_HOME/.cargo}"
USER_RUSTUP_HOME="${RUSTUP_HOME:-$USER_HOME/.rustup}"

use_sudo=0
if [[ "${1-}" == "--sudo" ]]; then
  use_sudo=1
fi

if [[ ! -f "$ESP_ENV" ]]; then
  echo "ESP env not found at $ESP_ENV"
  exit 1
fi

cmd="cd \"$ROOT_DIR\" && source \"$ESP_ENV\" && export HOME=\"$USER_HOME\" CARGO_HOME=\"$USER_CARGO_HOME\" RUSTUP_HOME=\"$USER_RUSTUP_HOME\" PATH=\"$USER_CARGO_HOME/bin:\$PATH\" && cargo +esp run $BUILD_FLAGS"

echo "Logging to $LOGFILE"

if [[ $use_sudo -eq 0 ]]; then
  script -q -a "$LOGFILE" -c "bash -lc '$cmd'"
else
  script -q -a "$LOGFILE" -c "sudo bash -lc '$cmd'"
fi
