#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ESP_ENV="/home/david/export-esp.sh"
BUILD_FLAGS="-Zbuild-std=std,panic_abort"
USER_HOME="$HOME"
USER_CARGO_HOME="${CARGO_HOME:-$USER_HOME/.cargo}"
USER_RUSTUP_HOME="${RUSTUP_HOME:-$USER_HOME/.rustup}"

use_sudo=1
if [[ "${1-}" == "--no-sudo" ]]; then
  use_sudo=0
fi

if [[ ! -f "$ESP_ENV" ]]; then
  echo "ESP env not found at $ESP_ENV"
  exit 1
fi

cmd="cd \"$ROOT_DIR\" && source \"$ESP_ENV\" && export HOME=\"$USER_HOME\" CARGO_HOME=\"$USER_CARGO_HOME\" RUSTUP_HOME=\"$USER_RUSTUP_HOME\" PATH=\"$USER_CARGO_HOME/bin:\$PATH\" && command -v cargo >/dev/null || { echo \"cargo not found in PATH\"; exit 1; } && cargo +esp run $BUILD_FLAGS"

if [[ $use_sudo -eq 0 ]]; then
  bash -lc "$cmd"
  exit 0
fi

echo "Flashing via sudo using user rustup/cargo homes."
sudo bash -lc "$cmd"
