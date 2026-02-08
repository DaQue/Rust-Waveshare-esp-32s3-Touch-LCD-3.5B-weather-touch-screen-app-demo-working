# Factory Demo Firmware Backup (2026-02-08)

Source build directory:
`/media/david/Shared/rust/factory_demo_fresh_20260208-105617/ESP-IDF/01_factory/build`

Backed up flashed images:
- `bin/bootloader.bin` @ `0x0`
- `bin/partition-table.bin` @ `0x8000`
- `bin/factory.bin` @ `0x10000`

Flash parameters:
- mode: `dio`
- freq: `80m`
- size: `16MB`

Equivalent flash command (adjust port):
```bash
esptool.py --chip esp32s3 --before default_reset --after hard_reset \
  write_flash --flash_mode dio --flash_freq 80m --flash_size 16MB \
  0x0 bin/bootloader.bin 0x8000 bin/partition-table.bin 0x10000 bin/factory.bin
```
