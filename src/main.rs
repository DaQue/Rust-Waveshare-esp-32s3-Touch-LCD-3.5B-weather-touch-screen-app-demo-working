use anyhow::Result;
use log::info;
use std::time::{Duration, Instant};

fn main() -> Result<()> {
    esp_idf_sys::link_patches();
    // Use ESP-IDF logger so output goes to UART reliably.
    esp_idf_svc::log::EspLogger::initialize_default();

    info!("BOOT OK (waveshare_s3_3p)");

    init_usb_serial_jtag();

    let mut buf = [0u8; 1];
    let mut line = String::new();
    let mut last_alive = Instant::now();
    let tick_hz = unsafe { esp_idf_sys::configTICK_RATE_HZ as u32 };
    let mut ticks = (10 * tick_hz) / 1000;
    if ticks == 0 {
        ticks = 1;
    }

    loop {
        let n = unsafe {
            esp_idf_sys::usb_serial_jtag_read_bytes(buf.as_mut_ptr().cast(), 1, ticks)
        };
        if n > 0 {
            let b = buf[0];
            match b {
                b'\r' | b'\n' => {
                    if !line.is_empty() {
                        handle_command(&line, ticks);
                        line.clear();
                    }
                }
                0x08 | 0x7f => {
                    line.pop();
                }
                _ => {
                    if let Some(ch) = char::from_u32(b as u32) {
                        if !ch.is_control() {
                            line.push(ch);
                        }
                    }
                }
            }
        } else if n < 0 {
            std::thread::sleep(Duration::from_millis(10));
        }

        if last_alive.elapsed() >= Duration::from_secs(1) {
            write_line("alive", ticks);
            last_alive = Instant::now();
        }
    }
}

fn handle_command(cmd: &str, ticks: u32) {
    let cmd = cmd.trim().to_lowercase();
    match cmd.as_str() {
        "ping" => write_line("pong", ticks),
        "" => {}
        _ => write_line("ping", ticks),
    }
}

fn init_usb_serial_jtag() {
    unsafe {
        let mut cfg = esp_idf_sys::usb_serial_jtag_driver_config_t {
            tx_buffer_size: 1024,
            rx_buffer_size: 1024,
        };
        esp_idf_sys::usb_serial_jtag_driver_install(&mut cfg);
    }
}

fn write_line(msg: &str, ticks: u32) {
    let mut line = String::with_capacity(msg.len() + 2);
    line.push_str(msg);
    line.push_str("\r\n");
    unsafe {
        esp_idf_sys::usb_serial_jtag_write_bytes(line.as_ptr().cast(), line.len(), ticks);
    }
}
