#!/bin/bash
# Interactive serial monitor for the ESP32-S3 weather station.
# Exit: Ctrl+A then X (minicom) or Ctrl+A then k (screen)
# If the port is busy: killall minicom; killall screen

BY_ID="/dev/serial/by-id/usb-Espressif_USB_JTAG_serial_debug_unit_80:B5:4E:DA:B6:C8-if00"
PORT="${1:-$BY_ID}"
BAUD="${2:-115200}"
LOGFILE="$(dirname "$0")/../log.txt"

if [ ! -e "$PORT" ]; then
    echo "Port $PORT not found. Is the device connected?"
    exit 1
fi

echo "Connecting to $PORT at $BAUD baud"
echo "Logging to $LOGFILE"
echo "Type 'help' for commands."
echo "If no echo: Ctrl+A then E to toggle local echo"
echo "Exit: Ctrl+A then X"
echo ""

minicom -c on -D "$PORT" -b "$BAUD" -w -8 -C "$LOGFILE"

