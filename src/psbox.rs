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

// ── PsBoxSlice ─────────────────────────────────────────────────────────────
//
// A PSRAM-backed fixed-length slice, analogous to Box<[T]> but allocated from
// PSRAM.  Used for large arrays inside PsBox structs (e.g. PressureHistory)
// so that Clone never touches internal SRAM.

pub struct PsBoxSlice<T> {
    ptr: *mut T,
    len: usize,
}

unsafe impl<T: Send> Send for PsBoxSlice<T> {}
unsafe impl<T: Sync> Sync for PsBoxSlice<T> {}

impl<T: Default> PsBoxSlice<T> {
    /// Allocate `len` elements in PSRAM, initialised with `T::default()`.
    pub fn new(len: usize) -> Self {
        let byte_size = len * mem::size_of::<T>();
        let ptr = if byte_size == 0 {
            mem::align_of::<T>() as *mut T
        } else {
            let p = unsafe {
                esp_idf_sys::heap_caps_malloc(byte_size, esp_idf_sys::MALLOC_CAP_SPIRAM) as *mut T
            };
            assert!(!p.is_null(), "PsBoxSlice::new: PSRAM alloc failed (size={})", byte_size);
            for i in 0..len {
                unsafe { ptr::write(p.add(i), T::default()) };
            }
            p
        };
        PsBoxSlice { ptr, len }
    }
}

impl<T> core::ops::Deref for PsBoxSlice<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        unsafe { core::slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl<T> core::ops::DerefMut for PsBoxSlice<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        unsafe { core::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

impl<T> Drop for PsBoxSlice<T> {
    fn drop(&mut self) {
        let byte_size = self.len * mem::size_of::<T>();
        if byte_size == 0 {
            return;
        }
        unsafe {
            for i in 0..self.len {
                ptr::drop_in_place(self.ptr.add(i));
            }
            esp_idf_sys::heap_caps_free(self.ptr as *mut _);
        }
    }
}

impl<T: Clone> Clone for PsBoxSlice<T> {
    /// Clone all elements into a new PSRAM allocation — never touches SRAM.
    fn clone(&self) -> Self {
        let byte_size = self.len * mem::size_of::<T>();
        let ptr = if byte_size == 0 {
            mem::align_of::<T>() as *mut T
        } else {
            let p = unsafe {
                esp_idf_sys::heap_caps_malloc(byte_size, esp_idf_sys::MALLOC_CAP_SPIRAM) as *mut T
            };
            assert!(!p.is_null(), "PsBoxSlice::clone: PSRAM alloc failed (size={})", byte_size);
            for i in 0..self.len {
                unsafe { ptr::write(p.add(i), (*self.ptr.add(i)).clone()) };
            }
            p
        };
        PsBoxSlice { ptr, len: self.len }
    }
}
