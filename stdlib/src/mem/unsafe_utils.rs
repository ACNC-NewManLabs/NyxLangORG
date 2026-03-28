//! NYX Memory Unsafe Utilities Module

/// Unsafe utilities
pub mod unsafe_utils {
    /// Assume initialized - for use with MaybeUninit
    #[inline]
    pub unsafe fn assume_init<T>(value: core::mem::MaybeUninit<T>) -> T {
        value.assume_init()
    }
}

pub use unsafe_utils::*;
