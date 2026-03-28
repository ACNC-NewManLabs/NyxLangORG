//! NYX Memory Drop Module

/// Deterministic destruction utilities
pub mod drop {
    use core::ptr;

    /// Drop a value in place
    #[inline]
    pub unsafe fn drop<T>(ptr: *mut T) {
        ptr::drop_in_place(ptr);
    }

    /// Drop a range of values
    #[inline]
    pub unsafe fn drop_range<T>(ptr: *mut T, count: usize) {
        for i in 0..count {
            ptr::drop_in_place(ptr.add(i));
        }
    }
}

pub use drop::*;

