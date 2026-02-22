use esp_idf_hal::i2c::I2cDriver;
use log::info;
use crate::layout::Orientation;

macro_rules! dbg_touch {
    ($($arg:tt)*) => {
        if crate::debug_flags::is_on(&crate::debug_flags::DEBUG_TOUCH) {
            info!($($arg)*);
        }
    };
}

/// AXS15231B integrated touch controller at I2C address 0x3B.
const TOUCH_ADDR: u8 = 0x3B;

/// 11-byte command to request touch data (from C factory bsp_touch.c).
const TOUCH_READ_CMD: [u8; 11] = [
    0xb5, 0xab, 0xa5, 0x5a, 0x00, 0x00, 0x00, 0x0e, 0x00, 0x00, 0x00,
];

// Touch thresholds matching C factory
const TOUCH_SWIPE_MIN_X_PX: i32 = 64;
const TOUCH_SWIPE_MAX_Y_PX: i32 = 80;
const TOUCH_SWIPE_MIN_Y_PX: i32 = 48;
const TOUCH_SWIPE_COOLDOWN_MS: u32 = 300;
const TOUCH_TAP_MAX_MOVE_PX: i32 = 18;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Gesture {
    None,
    Tap { x: i16, y: i16 },
    SwipeLeft,
    SwipeRight,
    SwipeUp,
    SwipeDown,
}

pub struct TouchState {
    pressed: bool,
    start_x: i16,
    start_y: i16,
    last_x: i16,
    last_y: i16,
    last_swipe_ms: u32,
    poll_count: u32,
    err_count: u32,
    touch_count: u32,
}

impl TouchState {
    pub fn new() -> Self {
        Self {
            pressed: false,
            start_x: 0,
            start_y: 0,
            last_x: 0,
            last_y: 0,
            last_swipe_ms: 0,
            poll_count: 0,
            err_count: 0,
            touch_count: 0,
        }
    }

    /// Poll the touch controller and return any detected gesture in the active
    /// logical screen orientation.
    pub fn poll(
        &mut self,
        i2c: &mut I2cDriver<'_>,
        now_ms: u32,
        orientation: Orientation,
    ) -> Gesture {
        self.poll_count += 1;

        // Log touch stats every 50 polls (~5 seconds at 100ms tick)
        #[allow(clippy::manual_is_multiple_of)]
        if self.poll_count % 50 == 0 {
            dbg_touch!(
                "TOUCH stats: polls={} errs={} touches={}",
                self.poll_count, self.err_count, self.touch_count
            );
        }

        let touch = read_touch(i2c, &mut self.err_count);

        if let Some((px, py)) = touch {
            let (x, y) = map_to_orientation(px, py, orientation);
            self.touch_count += 1;
            if !self.pressed {
                self.pressed = true;
                self.start_x = x;
                self.start_y = y;
                dbg_touch!("TOUCH down at ({}, {})", x, y);
            }
            self.last_x = x;
            self.last_y = y;
            return Gesture::None;
        }

        if !self.pressed {
            return Gesture::None;
        }

        // Finger released - classify gesture
        self.pressed = false;

        let dx = self.last_x as i32 - self.start_x as i32;
        let dy = self.last_y as i32 - self.start_y as i32;
        let abs_dx = dx.abs();
        let abs_dy = dy.abs();

        dbg_touch!(
            "TOUCH up: start=({},{}) end=({},{}) dx={} dy={}",
            self.start_x, self.start_y, self.last_x, self.last_y, dx, dy
        );

        // Tap
        if abs_dx <= TOUCH_TAP_MAX_MOVE_PX && abs_dy <= TOUCH_TAP_MAX_MOVE_PX {
            dbg_touch!("TOUCH -> Tap at ({}, {})", self.last_x, self.last_y);
            return Gesture::Tap {
                x: self.last_x,
                y: self.last_y,
            };
        }

        // Cooldown
        if now_ms.wrapping_sub(self.last_swipe_ms) < TOUCH_SWIPE_COOLDOWN_MS {
            return Gesture::None;
        }

        // Vertical swipe (for forecast scrolling)
        if abs_dy >= TOUCH_SWIPE_MIN_Y_PX && abs_dy >= abs_dx {
            self.last_swipe_ms = now_ms;
            let g = if dy < 0 { Gesture::SwipeUp } else { Gesture::SwipeDown };
            let g = map_swipe_for_orientation(g, orientation);
            dbg_touch!("TOUCH -> {:?}", g);
            return g;
        }

        // Horizontal swipe (page navigation)
        if abs_dx >= TOUCH_SWIPE_MIN_X_PX && abs_dy <= TOUCH_SWIPE_MAX_Y_PX && abs_dx > abs_dy {
            self.last_swipe_ms = now_ms;
            let g = if dx < 0 { Gesture::SwipeLeft } else { Gesture::SwipeRight };
            let g = map_swipe_for_orientation(g, orientation);
            dbg_touch!("TOUCH -> {:?}", g);
            return g;
        }

        dbg_touch!("TOUCH -> unclassified (abs_dx={}, abs_dy={})", abs_dx, abs_dy);
        Gesture::None
    }
}

/// Read touch coordinates from the AXS15231B integrated touch controller.
/// Returns Some((x, y)) in the native portrait panel coordinate space.
fn read_touch(i2c: &mut I2cDriver<'_>, err_count: &mut u32) -> Option<(i16, i16)> {
    let mut data = [0u8; 14];

    // Use write_read (repeated start) matching C factory i2c_master_transmit_receive
    match i2c.write_read(TOUCH_ADDR, &TOUCH_READ_CMD, &mut data, 100) {
        Ok(_) => {}
        Err(_) => {
            *err_count += 1;
            dbg_touch!("TOUCH read error (errs={})", *err_count);
            return None;
        }
    }

    // Byte 1: number of touch points (valid: 1 or 2)
    // Controller returns 0xBC for all bytes when idle.
    let num_points = data[1];
    if num_points == 0 || num_points > 2 {
        return None;
    }

    // Parse first touch point (bytes 2-5)
    let raw_x = (((data[2] & 0x0F) as u16) << 8) | data[3] as u16;
    let raw_y = (((data[4] & 0x0F) as u16) << 8) | data[5] as u16;

    dbg_touch!(
        "TOUCH raw touch: [{:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X}] points={} x={} y={}",
        data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        num_points, raw_x, raw_y
    );

    Some((raw_x as i16, raw_y as i16))
}

fn map_to_orientation(x: i16, y: i16, orientation: Orientation) -> (i16, i16) {
    let px = x.clamp(0, (crate::layout::SCREEN_W_PORTRAIT - 1) as i16);
    let py = y.clamp(0, (crate::layout::SCREEN_H_PORTRAIT - 1) as i16);
    match orientation {
        Orientation::Portrait => (px, py),
        Orientation::PortraitFlipped => (
            (crate::layout::SCREEN_W_PORTRAIT - 1) as i16 - px,
            (crate::layout::SCREEN_H_PORTRAIT - 1) as i16 - py,
        ),
        Orientation::Landscape => {
            // portrait(320x480) -> landscape(480x320)
            let lx = py;
            let ly = (crate::layout::SCREEN_W_PORTRAIT - 1) as i16 - px;
            (lx, ly)
        }
        Orientation::LandscapeFlipped => {
            // portrait(320x480) -> landscape flipped (USB on left)
            let lx = (crate::layout::SCREEN_H_PORTRAIT - 1) as i16 - py;
            let ly = px;
            (lx, ly)
        }
    }
}

fn map_swipe_for_orientation(g: Gesture, orientation: Orientation) -> Gesture {
    match orientation {
        Orientation::LandscapeFlipped | Orientation::PortraitFlipped => match g {
            Gesture::SwipeLeft => Gesture::SwipeRight,
            Gesture::SwipeRight => Gesture::SwipeLeft,
            Gesture::SwipeUp => Gesture::SwipeDown,
            Gesture::SwipeDown => Gesture::SwipeUp,
            _ => g,
        },
        _ => g,
    }
}

/// One-time diagnostic: try multiple approaches to communicate with touch controller.
pub fn probe(i2c: &mut I2cDriver<'_>) {
    info!("=== TOUCH PROBE START ===");

    // Try write_read (repeated start)
    let mut data = [0u8; 14];
    match i2c.write_read(TOUCH_ADDR, &TOUCH_READ_CMD, &mut data, 100) {
        Ok(_) => dbg_touch!("Touch write_read OK: [{:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X}]",
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7]),
        Err(e) => dbg_touch!("Touch write_read FAILED: {:?}", e),
    }

    // Try separate write + read
    match i2c.write(TOUCH_ADDR, &TOUCH_READ_CMD, 100) {
        Ok(_) => {
            let mut data2 = [0u8; 14];
            match i2c.read(TOUCH_ADDR, &mut data2, 100) {
                Ok(_) => dbg_touch!("Touch write+read OK: [{:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X}]",
                    data2[0], data2[1], data2[2], data2[3], data2[4], data2[5], data2[6], data2[7]),
                Err(e) => dbg_touch!("Touch write OK, read FAILED: {:?}", e),
            }
        }
        Err(e) => dbg_touch!("Touch write FAILED: {:?}", e),
    }

    // Try simple single-byte read (basic I2C health check)
    let mut byte = [0u8; 1];
    match i2c.read(TOUCH_ADDR, &mut byte, 100) {
        Ok(_) => dbg_touch!("Touch bare read OK: [{:02X}]", byte[0]),
        Err(e) => dbg_touch!("Touch bare read FAILED: {:?}", e),
    }

    info!("=== TOUCH PROBE END ===");
}
