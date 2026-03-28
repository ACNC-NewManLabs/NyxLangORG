//! NYX Memory Pointer Module

/// Pointer operations
pub mod ptr {
    use core::ptr;

    /// Copies memory from source to destination (non-overlapping)
    #[inline]
    pub unsafe fn copy<T>(src: *const T, dst: *mut T, count: usize) {
        ptr::copy_nonoverlapping(src, dst, count);
    }

    /// Copies memory from source to destination (overlapping allowed)
    #[inline]
    pub unsafe fn copy_overlapping<T>(src: *const T, dst: *mut T, count: usize) {
        ptr::copy(src, dst, count);
    }

    /// Sets memory to a value
    #[inline]
    pub unsafe fn set<T: Clone>(dst: *mut T, value: &T, count: usize) {
        for i in 0..count {
            ptr::write(dst.add(i), value.clone());
        }
    }

    /// Reads a value from a pointer
    #[inline]
    pub unsafe fn read<T>(src: *const T) -> T {
        ptr::read(src)
    }

    /// Writes a value to a pointer
    #[inline]
    pub unsafe fn write<T>(dst: *mut T, value: T) {
        ptr::write(dst, value)
    }

    /// Drops a value at a pointer
    #[inline]
    pub unsafe fn drop<T>(src: *mut T) {
        ptr::drop_in_place(src);
    }

    /// Swaps two values
    #[inline]
    pub unsafe fn swap<T>(a: *mut T, b: *mut T) {
        ptr::swap(a, b);
    }

    /// Creates a dangling pointer
    #[inline]
    pub fn dangling<T>() -> *mut T {
        ptr::NonNull::dangling().as_ptr()
    }

    /// Creates a null pointer
    #[inline]
    pub fn null<T>() -> *mut T {
        ptr::null_mut()
    }

    /// Checks if pointer is null
    #[inline]
    pub fn is_null<T>(ptr: *const T) -> bool {
        ptr.is_null()
    }
}

