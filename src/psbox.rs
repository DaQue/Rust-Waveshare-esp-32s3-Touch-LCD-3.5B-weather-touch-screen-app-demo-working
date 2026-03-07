/// PSRAM-backed owning pointer, analogous to Box but allocated from PSRAM.
///
/// ESP32-S3 has 8 MB of PSRAM but only ~317 KB of internal SRAM.  Internal
/// SRAM is shared with mbedTLS TLS contexts (~37 KB contiguous needed) and
/// the lwIP/WiFi stack.  Storing large fixed-size structs (HvacDetector,
/// PressureHistory) in PSRAM via PsBox keeps the internal SRAM heap clean
/// and unfragmented so TLS handshakes can always succeed.
///
/// # Safety
/// Requires `CONFIG_SPIRAM_USE_CAPS_ALLOC=y` (already set in sdkconfig):
/// `heap_caps_malloc(MALLOC_CAP_SPIRAM)` must be available.
use core::ops::{Deref, DerefMut};
use core::{mem, ptr};

pub struct PsBox<T>(*mut T);

// PSRAM is accessible from both cores; the contained T is solely owned.
unsafe impl<T: Send> Send for PsBox<T> {}
unsafe impl<T: Sync> Sync for PsBox<T> {}

impl<T> PsBox<T> {
    pub fn new(val: T) -> Self {
        let size = mem::size_of::<T>();
        if size == 0 {
            // Zero-sized type: use a dangling but non-null aligned pointer.
            return PsBox(mem::align_of::<T>() as *mut T);
        }
        let ptr = unsafe {
            esp_idf_sys::heap_caps_malloc(
                size,
                esp_idf_sys::MALLOC_CAP_SPIRAM,
            ) as *mut T
        };
        assert!(!ptr.is_null(), "PsBox: PSRAM alloc failed (size={})", size);
        unsafe { ptr::write(ptr, val) };
        PsBox(ptr)
    }
}

impl<T> Deref for PsBox<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.0 }
    }
}

impl<T> DerefMut for PsBox<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.0 }
    }
}

impl<T> Drop for PsBox<T> {
    fn drop(&mut self) {
        if mem::size_of::<T>() == 0 {
            return;
        }
        unsafe {
            ptr::drop_in_place(self.0);
            esp_idf_sys::heap_caps_free(self.0 as *mut _);
        }
    }
}

impl<T: Clone> Clone for PsBox<T> {
    /// Allocate a new PSRAM block and clone the contained value into it.
    fn clone(&self) -> Self {
        PsBox::new((**self).clone())
    }

    /// Update the contained value **in-place** — no new PSRAM alloc.
    /// For plain-old-data types (arrays, scalars) this is a memcpy-like
    /// operation: T::clone() builds the new value on the stack, then
    /// overwrites the existing PSRAM slot.  No allocation, no free.
    fn clone_from(&mut self, source: &Self) {
        unsafe { *self.0 = (**source).clone() };
    }
}
