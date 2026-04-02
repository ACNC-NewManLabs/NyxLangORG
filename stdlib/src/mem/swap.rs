//! NYX Memory Swap Module

/// Memory swap utilities
pub mod swap {
    use core::ptr;

    /// Swap two values
    #[inline]
    pub unsafe fn swap<T>(a: *mut T, b: *mut T) {
        ptr::swap(a, b);
    }

    /// Swap two values (non-overlapping)
    #[inline]
    pub unsafe fn swap_nonoverlapping<T>(a: *mut T, b: *mut T, count: usize) {
        ptr::swap_nonoverlapping(a, b, count);
    }
}

pub use swap::*;
