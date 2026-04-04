use std::sync::atomic::Ordering;
/// Render thread: owns framebuffer + LCD context, draws AppState on demand.
use std::sync::{Arc, Mutex};

use embedded_graphics::prelude::OriginDimensions;

use crate::{debug_flags, framebuffer, layout, views};

pub(crate) fn spawn_render_thread(
    render_slot: Arc<Mutex<Option<views::AppState>>>,
    fb: framebuffer::Framebuffer,
    ctx: framebuffer::LcdContext,
) {
    std::thread::Builder::new()
        .name("render".into())
        .stack_size(24576)
        .spawn(move || {
            let mut fb = fb;
            let mut current_orientation = layout::Orientation::Landscape;
            // fb_orientation tracks the actual framebuffer pixel dimensions so
            // flush_to_panel always receives an orientation that matches the FB.
            // It diverges from current_orientation when a realloc is skipped due
            // to low DMA memory — in that case we keep flushing with the old
            // orientation until a realloc eventually succeeds.
            let mut fb_orientation = layout::Orientation::Landscape;
            loop {
                // Mutex::lock() uses a FreeRTOS semaphore — never spins.
                // When the slot is None, yield for one tick so IDLE0 runs.
                let snapshot = loop {
                    let s = render_slot.lock().unwrap().take();
                    match s {
                        Some(s) => break s,
                        None => {
                            unsafe { esp_idf_sys::vTaskDelay(1) };
                        }
                    }
                };
                // Only reallocate framebuffer when the logical pixel dimensions
                // change (portrait <-> landscape).  Landscape <-> LandscapeFlipped
                // shares the same pixel dimensions — reallocating would hold TWO
                // DMA buffers simultaneously and permanently fragment the heap.
                if snapshot.orientation != current_orientation {
                    log::info!(
                        "orientation change: {:?} -> {:?}",
                        current_orientation,
                        snapshot.orientation
                    );
                    let (w, h) = layout::framebuffer_dims(snapshot.orientation);
                    let sz = fb.size();
                    if w != sz.width || h != sz.height {
                        // Guard: only reallocate if there is enough DMA-capable SRAM.
                        let dma_free = unsafe {
                            esp_idf_sys::heap_caps_get_largest_free_block(
                                esp_idf_sys::MALLOC_CAP_DMA,
                            )
                        };
                        if dma_free >= 13_000 {
                            fb = framebuffer::Framebuffer::new(w, h);
                            fb_orientation = snapshot.orientation;
                        } else {
                            log::warn!(
                                "orientation change: skipping FB realloc — DMA free only {} bytes",
                                dma_free
                            );
                        }
                    } else {
                        // Same pixel dimensions (e.g. Landscape <-> LandscapeFlipped).
                        fb_orientation = snapshot.orientation;
                    }
                    current_orientation = snapshot.orientation;
                }
                // Yield before draw so IDLE1 runs at frame start. draw_current_view
                // can be long and the post-flush yield alone is too late under PSRAM
                // bus contention.
                unsafe { esp_idf_sys::vTaskDelay(1) };
                let mut snapshot = snapshot;
                if snapshot.orientation != fb_orientation {
                    snapshot.orientation = fb_orientation;
                }
                views::draw_current_view(&mut fb, &snapshot);
                debug_flags::RENDER_FLUSH_ACTIVE.store(true, Ordering::Release);
                ctx.flush_fb(&fb, fb_orientation);
                debug_flags::RENDER_FLUSH_ACTIVE.store(false, Ordering::Release);
                // Yield after flush as well (belt-and-suspenders).
                unsafe { esp_idf_sys::vTaskDelay(1) };
            }
        })
        .expect("failed to spawn render thread");
}
