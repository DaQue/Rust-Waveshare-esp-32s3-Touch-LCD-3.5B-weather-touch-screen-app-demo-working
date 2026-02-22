use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{Dimensions, OriginDimensions, Size},
    pixelcolor::{raw::RawU16, Rgb565},
    prelude::*,
    primitives::Rectangle,
    Pixel,
};

/// Logical framebuffer dimensions (landscape orientation).
pub const FB_WIDTH: u32 = 480;
pub const FB_HEIGHT: u32 = 320;

/// Physical panel dimensions (portrait, native).
pub const PANEL_WIDTH: u32 = 320;
pub const PANEL_HEIGHT: u32 = 480;

/// Number of panel rows sent per DMA chunk.
pub const CHUNK_LINES: i32 = 20;

const LCD_OPCODE_WRITE_CMD: u32 = 0x02;
const LCD_CMD_RASET: u32 = 0x2B;

/// RGB565 framebuffer backed by a PSRAM allocation.
/// Supports landscape (480x320, rotated on flush) and portrait
/// (320x480, direct flush) logical rendering.
pub struct Framebuffer {
    buf: *mut u16,
    len: usize,
    width: u32,
    height: u32,
    dma_buf: *mut u8,
    dma_bytes: usize,
}

impl Framebuffer {
    pub fn new(width: u32, height: u32) -> Self {
        let pixels = (width * height) as usize;
        let bytes = pixels * core::mem::size_of::<u16>();
        let ptr = unsafe {
            esp_idf_sys::heap_caps_malloc(bytes, esp_idf_sys::MALLOC_CAP_SPIRAM) as *mut u16
        };
        assert!(!ptr.is_null(), "PSRAM framebuffer alloc failed ({} bytes)", bytes);
        unsafe { core::ptr::write_bytes(ptr, 0, pixels); }

        // Persistent DMA buffer for panel transfers (panel_width * chunk_lines * 2 bytes)
        let dma_pixels = (PANEL_WIDTH as usize) * (CHUNK_LINES as usize);
        let dma_bytes = dma_pixels * 2;
        let dma_buf = unsafe {
            esp_idf_sys::heap_caps_malloc(
                dma_bytes,
                esp_idf_sys::MALLOC_CAP_DMA
                    | esp_idf_sys::MALLOC_CAP_INTERNAL
                    | esp_idf_sys::MALLOC_CAP_8BIT,
            ) as *mut u8
        };
        assert!(!dma_buf.is_null(), "DMA buffer alloc failed ({} bytes)", dma_bytes);

        Self {
            buf: ptr,
            len: pixels,
            width,
            height,
            dma_buf,
            dma_bytes,
        }
    }

    fn as_slice(&self) -> &[u16] {
        unsafe { core::slice::from_raw_parts(self.buf, self.len) }
    }

    fn as_mut_slice(&mut self) -> &mut [u16] {
        unsafe { core::slice::from_raw_parts_mut(self.buf, self.len) }
    }

    pub fn clear_color(&mut self, color: Rgb565) {
        let raw = RawU16::from(color).into_inner();
        self.as_mut_slice().fill(raw);
    }

    /// Flush the landscape framebuffer to the portrait panel with 90° CW rotation.
    pub fn flush_to_panel(
        &self,
        io: esp_idf_sys::esp_lcd_panel_io_handle_t,
        panel: esp_idf_sys::esp_lcd_panel_handle_t,
        orientation: crate::layout::Orientation,
    ) {
        let (need_w, need_h) = if orientation.is_landscape() {
            (480, 320)
        } else {
            (320, 480)
        };
        assert!(
            self.width == need_w && self.height == need_h,
            "framebuffer {}x{} does not match orientation {:?} expected {}x{}",
            self.width,
            self.height,
            orientation,
            need_w,
            need_h
        );
        match orientation {
            crate::layout::Orientation::Landscape => self.flush_landscape_rotated(io, panel),
            crate::layout::Orientation::LandscapeFlipped => {
                self.flush_landscape_rotated_flipped(io, panel)
            }
            crate::layout::Orientation::Portrait => self.flush_portrait_direct(io, panel),
            crate::layout::Orientation::PortraitFlipped => {
                self.flush_portrait_direct_flipped(io, panel)
            }
        }
    }

    fn flush_landscape_rotated(
        &self,
        io: esp_idf_sys::esp_lcd_panel_io_handle_t,
        panel: esp_idf_sys::esp_lcd_panel_handle_t,
    ) {
        let dma_slice = unsafe {
            core::slice::from_raw_parts_mut(self.dma_buf, self.dma_bytes)
        };
        let fb = self.as_slice();
        let fb_w = self.width as usize;
        let fb_h = self.height as usize;

        let pw = PANEL_WIDTH as i32;
        let ph = PANEL_HEIGHT as i32;

        let mut py = 0i32;
        while py < ph {
            let py_end = (py + CHUNK_LINES).min(ph);
            let _rows = (py_end - py) as usize;

            // panel(px, py) <- fb(py, fb_h-1-px)
            let mut di = 0usize;
            for row in py..py_end {
                for px in 0..pw {
                    let lx = row as usize;
                    let ly = fb_h - 1 - px as usize;
                    let pixel = fb[ly * fb_w + lx];
                    dma_slice[di] = (pixel >> 8) as u8;
                    dma_slice[di + 1] = (pixel & 0xFF) as u8;
                    di += 2;
                }
            }

            send_raset(io, py, py_end);

            unsafe {
                esp_idf_sys::esp_lcd_panel_draw_bitmap(
                    panel,
                    0,
                    py,
                    pw,
                    py_end,
                    dma_slice.as_ptr().cast(),
                );
            }

            py = py_end;
        }
    }

    fn flush_portrait_direct(
        &self,
        io: esp_idf_sys::esp_lcd_panel_io_handle_t,
        panel: esp_idf_sys::esp_lcd_panel_handle_t,
    ) {
        let dma_slice = unsafe { core::slice::from_raw_parts_mut(self.dma_buf, self.dma_bytes) };
        let fb = self.as_slice();
        let pw = PANEL_WIDTH as i32;
        let ph = PANEL_HEIGHT as i32;
        let fb_w = self.width as usize;

        let mut py = 0i32;
        while py < ph {
            let py_end = (py + CHUNK_LINES).min(ph);

            let mut di = 0usize;
            for row in py..py_end {
                let row_start = row as usize * fb_w;
                for px in 0..pw {
                    let pixel = fb[row_start + px as usize];
                    dma_slice[di] = (pixel >> 8) as u8;
                    dma_slice[di + 1] = (pixel & 0xFF) as u8;
                    di += 2;
                }
            }

            send_raset(io, py, py_end);

            unsafe {
                esp_idf_sys::esp_lcd_panel_draw_bitmap(
                    panel,
                    0,
                    py,
                    pw,
                    py_end,
                    dma_slice.as_ptr().cast(),
                );
            }

            py = py_end;
        }
    }

    fn flush_portrait_direct_flipped(
        &self,
        io: esp_idf_sys::esp_lcd_panel_io_handle_t,
        panel: esp_idf_sys::esp_lcd_panel_handle_t,
    ) {
        let dma_slice = unsafe { core::slice::from_raw_parts_mut(self.dma_buf, self.dma_bytes) };
        let fb = self.as_slice();
        let pw = PANEL_WIDTH as i32;
        let ph = PANEL_HEIGHT as i32;
        let fb_w = self.width as usize;

        let mut py = 0i32;
        while py < ph {
            let py_end = (py + CHUNK_LINES).min(ph);

            let mut di = 0usize;
            for row in py..py_end {
                let src_y = (ph - 1 - row) as usize;
                let row_start = src_y * fb_w;
                for px in 0..pw {
                    let src_x = (pw - 1 - px) as usize;
                    let pixel = fb[row_start + src_x];
                    dma_slice[di] = (pixel >> 8) as u8;
                    dma_slice[di + 1] = (pixel & 0xFF) as u8;
                    di += 2;
                }
            }

            send_raset(io, py, py_end);

            unsafe {
                esp_idf_sys::esp_lcd_panel_draw_bitmap(
                    panel,
                    0,
                    py,
                    pw,
                    py_end,
                    dma_slice.as_ptr().cast(),
                );
            }

            py = py_end;
        }
    }

    fn flush_landscape_rotated_flipped(
        &self,
        io: esp_idf_sys::esp_lcd_panel_io_handle_t,
        panel: esp_idf_sys::esp_lcd_panel_handle_t,
    ) {
        let dma_slice = unsafe { core::slice::from_raw_parts_mut(self.dma_buf, self.dma_bytes) };
        let fb = self.as_slice();
        let fb_w = self.width as usize;
        let _fb_h = self.height as usize;
        let pw = PANEL_WIDTH as i32;
        let ph = PANEL_HEIGHT as i32;

        let mut py = 0i32;
        while py < ph {
            let py_end = (py + CHUNK_LINES).min(ph);

            // panel(px,py) <- fb(fb_w-1-py, px)
            let mut di = 0usize;
            for row in py..py_end {
                for px in 0..pw {
                    let lx = fb_w - 1 - row as usize;
                    let ly = px as usize;
                    let pixel = fb[ly * fb_w + lx];
                    dma_slice[di] = (pixel >> 8) as u8;
                    dma_slice[di + 1] = (pixel & 0xFF) as u8;
                    di += 2;
                }
            }

            send_raset(io, py, py_end);
            unsafe {
                esp_idf_sys::esp_lcd_panel_draw_bitmap(
                    panel,
                    0,
                    py,
                    pw,
                    py_end,
                    dma_slice.as_ptr().cast(),
                );
            }
            py = py_end;
        }
    }
}

impl OriginDimensions for Framebuffer {
    fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }
}

impl DrawTarget for Framebuffer {
    type Color = Rgb565;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        let w = self.width;
        let h = self.height;
        let buf = self.as_mut_slice();
        let mut wrote = 0usize;
        for Pixel(point, color) in pixels {
            let x = point.x;
            let y = point.y;
            if x >= 0 && y >= 0 && (x as u32) < w && (y as u32) < h {
                let idx = (y as u32 * w + x as u32) as usize;
                buf[idx] = RawU16::from(color).into_inner();
                wrote = wrote.wrapping_add(1);
                // Long icon/frame draws can starve IDLE0; yield periodically.
                if wrote.is_multiple_of(1024) {
                    unsafe { esp_idf_sys::vTaskDelay(0) };
                }
            }
        }
        Ok(())
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        let raw = RawU16::from(color).into_inner();
        let display = self.bounding_box();
        let area = area.intersection(&display);
        let w = self.width;
        let buf = self.as_mut_slice();
        let mut wrote = 0usize;
        for y in area.rows() {
            let row_start = (y as u32 * w) as usize;
            for x in area.columns() {
                buf[row_start + x as usize] = raw;
                wrote = wrote.wrapping_add(1);
                if wrote.is_multiple_of(2048) {
                    unsafe { esp_idf_sys::vTaskDelay(0) };
                }
            }
        }
        Ok(())
    }
}

impl Drop for Framebuffer {
    fn drop(&mut self) {
        unsafe {
            esp_idf_sys::heap_caps_free(self.buf.cast());
            esp_idf_sys::heap_caps_free(self.dma_buf.cast());
        }
    }
}

fn qspi_cmd(raw: u32) -> i32 {
    ((LCD_OPCODE_WRITE_CMD << 24) | ((raw & 0xFF) << 8)) as i32
}

fn send_raset(io: esp_idf_sys::esp_lcd_panel_io_handle_t, y_start: i32, y_end: i32) {
    let y_end_incl = y_end - 1;
    let params: [u8; 4] = [
        ((y_start >> 8) & 0xFF) as u8,
        (y_start & 0xFF) as u8,
        ((y_end_incl >> 8) & 0xFF) as u8,
        (y_end_incl & 0xFF) as u8,
    ];
    unsafe {
        esp_idf_sys::esp_lcd_panel_io_tx_param(
            io,
            qspi_cmd(LCD_CMD_RASET),
            params.as_ptr().cast(),
            params.len(),
        );
    }
}
