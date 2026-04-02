//! NYX Memory Copy Module

/// Memory copy utilities
pub mod copy {
    use core::ptr;

    /// Copy memory (non-overlapping)
    #[inline]
    pub unsafe fn copy<T>(src: *const T, dst: *mut T, count: usize) {
        ptr::copy_nonoverlapping(src, dst, count);
    }

    /// Copy memory (overlapping allowed)
    #[inline]
    pub unsafe fn copy_overlapping<T>(src: *const T, dst: *mut T, count: usize) {
        ptr::copy(src, dst, count);
    }

    /// Copy bytes
    #[inline]
    pub unsafe fn copy_bytes(dst: *mut u8, src: *const u8, count: usize) {
        ptr::copy_nonoverlapping(src, dst, count);
    }
}
