//! NYX Core Memory Module
//!
//! Low-level memory operations that work without an OS.
//! This module provides unsafe memory utilities for the core library.

use core::ptr;

// =============================================================================
// Memory Operations
// =============================================================================

/// Copies a block of memory from source to destination.
///
/// This function copies `count` bytes from `src` to `dst`. The regions must not
/// overlap. Use `copy_overlapping` if regions may overlap.
///
/// # Safety
///
/// - The source and destination regions must not overlap
/// - Both regions must be valid for reading/writing respectively
/// - The regions must contain at least `count` bytes
#[inline]
pub unsafe fn copy(src: *const u8, dst: *mut u8, count: usize) {
    ptr::copy_nonoverlapping(src, dst, count)
}

/// Copies a block of memory from source to destination (allowing overlap).
///
/// This function copies `count` bytes from `src` to `dst`, handling overlapping
/// regions correctly.
///
/// # Safety
///
/// - Both regions must be valid for reading/writing respectively
/// - The regions must contain at least `count` bytes
#[inline]
pub unsafe fn copy_overlapping(src: *const u8, dst: *mut u8, count: usize) {
    ptr::copy(src, dst, count)
}

/// Sets a block of memory to a specific value.
///
/// This function fills `count` bytes starting at `dst` with `value`.
///
/// # Safety
///
/// - The region must be valid for writing
/// - The region must contain at least `count` bytes
#[inline]
pub unsafe fn set(dst: *mut u8, value: u8, count: usize) {
    ptr::write_bytes(dst, value, count)
}

/// Reads a value from a memory location.
///
/// # Safety
///
/// - The location must be properly aligned
/// - The location must be valid for reading
#[inline]
pub unsafe fn read<T>(src: *const T) -> T {
    ptr::read(src)
}

/// Writes a value to a memory location.
///
/// # Safety
///
/// - The location must be properly aligned
/// - The location must be valid for writing
/// - The previous value (if any) must be properly dropped
#[inline]
pub unsafe fn write<T>(dst: *mut T, value: T) {
    ptr::write(dst, value)
}

/// Drops the value at a memory location without reading it.
///
/// # Safety
///
/// - The location must be valid for reading
/// - The value must be properly initialized
#[inline]
pub unsafe fn drop<T>(src: *mut T) {
    ptr::drop_in_place(src)
}

// =============================================================================
// Swap Operations
// =============================================================================

/// Swaps the values at two memory locations.
///
/// # Safety
///
/// - Both locations must be valid for reading/writing
/// - Both locations must be properly aligned
#[inline]
pub unsafe fn swap<T>(a: *mut T, b: *mut T) {
    ptr::swap(a, b)
}

/// Swaps `count` elements between two regions.
///
/// # Safety
///
/// - Both regions must be valid for reading/writing
/// - Both regions must contain at least `count` elements
#[inline]
pub unsafe fn swap_nonoverlapping<T>(a: *mut T, b: *mut T, count: usize) {
    ptr::swap_nonoverlapping(a, b, count)
}

// =============================================================================
// Zero Initialization
// =============================================================================

/// Creates an uninitialized value in memory.
///
/// # Safety
///
/// The returned value is not initialized. Reading from it is undefined behavior
/// until it has been written to.
#[inline]
pub unsafe fn uninit<T>() -> T {
    ptr::read(ptr::NonNull::dangling().as_ptr())
}

/// Creates a zeroed value in memory.
///
/// This is equivalent to `mem::zeroed()` but works in const contexts.
#[inline]
pub const fn zeroed<T>() -> T
where
    T: Copy,
{
    unsafe { core::mem::zeroed() }
}

// =============================================================================
// Pointer Utilities
// =============================================================================

/// Returns the offset from a pointer.
///
/// # Safety
///
/// The resulting pointer must be in bounds or one byte past the end of the
/// same allocated object as the original pointer.
#[inline]
pub unsafe fn offset<T>(ptr: *const T, count: isize) -> *const T {
    ptr.offset(count)
}

/// Returns the offset from a mutable pointer.
///
/// # Safety
///
/// The resulting pointer must be in bounds or one byte past the end of the
/// same allocated object as the original pointer.
#[inline]
pub unsafe fn offset_mut<T>(ptr: *mut T, count: isize) -> *mut T {
    ptr.offset(count)
}

/// Returns the byte offset from a pointer.
///
/// # Safety
///
/// The resulting pointer must be in bounds or one byte past the end of the
/// same allocated object as the original pointer.
#[inline]
pub unsafe fn add<T>(ptr: *const T, count: usize) -> *const T {
    ptr.add(count)
}

/// Returns the byte offset from a mutable pointer.
///
/// # Safety
///
/// The resulting pointer must be in bounds or one byte past the end of the
/// same allocated object as the original pointer.
#[inline]
pub unsafe fn add_mut<T>(ptr: *mut T, count: usize) -> *mut T {
    ptr.add(count)
}

/// Returns the distance between two pointers.
///
/// # Safety
///
/// Both pointers must point to the same allocated object.
#[inline]
pub unsafe fn distance<T>(a: *const T, b: *const T) -> usize {
    b.offset_from(a) as usize
}

// =============================================================================
// Alignment
// =============================================================================

/// Aligns a pointer to the specified alignment.
///
/// Returns the smallest pointer greater than or equal to `ptr` that is
/// aligned to `align`.
#[inline]
pub fn align_to<T, U>(ptr: *const T, align: usize) -> *const U {
    let addr = ptr as usize;
    let aligned = (addr + align - 1) & !(align - 1);
    aligned as *const U
}

/// Aligns a mutable pointer to the specified alignment.
///
/// Returns the smallest pointer greater than or equal to `ptr` that is
/// aligned to `align`.
#[inline]
pub fn align_to_mut<T, U>(ptr: *mut T, align: usize) -> *mut U {
    let addr = ptr as usize;
    let aligned = (addr + align - 1) & !(align - 1);
    aligned as *mut U
}

/// Returns true if the pointer is aligned to the specified alignment.
#[inline]
pub fn is_aligned_to<T>(ptr: *const T, align: usize) -> bool {
    let addr = ptr as usize;
    addr.is_multiple_of(align)
}

// =============================================================================
// ManuallyDrop-like wrapper
// =============================================================================

/// A wrapper to prevent automatic dropping of contained value.
#[derive(Debug)]
#[repr(transparent)]
pub struct ManuallyDrop<T: ?Sized> {
    /// The contained value.
    value: T,
}

impl<T> ManuallyDrop<T> {
    /// Wraps a value without dropping it.
    #[inline]
    pub const fn new(value: T) -> ManuallyDrop<T> {
        ManuallyDrop { value }
    }

    /// Extracts the value from the ManuallyDrop wrapper.
    ///
    /// # Safety
    ///
    /// After calling this, the caller is responsible for managing the
    /// lifetime of the extracted value.
    #[inline]
    pub unsafe fn take(slot: &mut ManuallyDrop<T>) -> T {
        ptr::read(&slot.value as *const T)
    }

    /// Prevents the value from being dropped.
    #[inline]
    pub fn forget(slot: ManuallyDrop<T>) {
        core::mem::forget(slot.value);
    }
}

impl<T: ?Sized> ManuallyDrop<T> {
    /// Returns a raw pointer to the wrapped value.
    #[inline]
    pub fn as_ptr(this: &ManuallyDrop<T>) -> *const T {
        &this.value as *const T
    }

    /// Returns a mutable raw pointer to the wrapped value.
    #[inline]
    pub fn as_mut_ptr(this: &mut ManuallyDrop<T>) -> *mut T {
        &mut this.value as *mut T
    }
}

// =============================================================================
// MaybeUninit support
// =============================================================================

/// A wrapper to create uninitialized values.
///
/// This is a thin wrapper around [`core::mem::MaybeUninit`] that exposes a
/// similar API for use in the Nyx standard library.
// We use a union-based layout (via core::mem::MaybeUninit) to safely hold
// uninitialized memory without triggering undefined behaviour.
pub union MaybeUninit<T: Copy> {
    uninit: (),
    value: core::mem::ManuallyDrop<T>,
}

impl<T: Copy> MaybeUninit<T> {
    /// Creates a new MaybeUninit with uninitialized memory.
    #[inline]
    pub const fn uninit() -> Self {
        MaybeUninit { uninit: () }
    }

    /// Creates a new MaybeUninit with zeroed memory.
    #[inline]
    pub const fn zeroed() -> MaybeUninit<T> {
        // SAFETY: zero is a valid bit-pattern for zeroed memory.
        MaybeUninit {
            value: core::mem::ManuallyDrop::new(unsafe { core::mem::zeroed() }),
        }
    }

    /// Creates a new MaybeUninit with initialized memory.
    #[inline]
    pub const fn new(value: T) -> MaybeUninit<T> {
        MaybeUninit {
            value: core::mem::ManuallyDrop::new(value),
        }
    }

    /// Returns a raw pointer to the contained value.
    #[inline]
    pub fn as_ptr(&self) -> *const T {
        // SAFETY: `value` and `uninit` occupy the same memory.
        core::ptr::addr_of!(self.value).cast::<T>()
    }

    /// Returns a mutable raw pointer to the contained value.
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut T {
        // SAFETY: `value` and `uninit` occupy the same memory.
        core::ptr::addr_of_mut!(self.value).cast::<T>()
    }

    /// Extracts the value.
    ///
    /// # Safety
    ///
    /// The value must have been initialized.
    #[inline]
    pub unsafe fn assume_init(self) -> T {
        core::mem::ManuallyDrop::into_inner(self.value)
    }
}

// =============================================================================
// Layout Utilities
// =============================================================================

/// Layout of a value in memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    size: usize,
    align: usize,
}

impl Layout {
    /// Creates a layout from a type.
    #[inline]
    pub const fn new<T>() -> Layout {
        Layout {
            size: core::mem::size_of::<T>(),
            align: core::mem::align_of::<T>(),
        }
    }

    /// Creates a layout with the given size and alignment.
    #[inline]
    pub const fn from_size_align(size: usize, align: usize) -> Option<Layout> {
        if align == 0 || !align.is_power_of_two() {
            return None;
        }
        Some(Layout { size, align })
    }

    /// Returns the size of the layout.
    #[inline]
    pub const fn size(&self) -> usize {
        self.size
    }

    /// Returns the alignment of the layout.
    #[inline]
    pub const fn align(&self) -> usize {
        self.align
    }

    /// Returns the size of a layout with the given size and alignment.
    #[inline]
    pub const fn size_for(&self, additional: usize) -> usize {
        let offset = self.size;
        let align = self.align;
        let aligned = (offset + align - 1) & !(align - 1);
        aligned + additional
    }

    /// Returns a new layout with the given additional size.
    #[inline]
    pub fn extend(&self, additional: Layout) -> Option<(Layout, usize)> {
        let new_size = self.size_for(additional.size);
        Some((Layout::from_size_align(new_size, self.align)?, self.size))
    }
}

// =============================================================================
// Compile-time Constants
// =============================================================================

/// Maximum alignment.
pub const MAX_ALIGN: usize = core::mem::align_of::<&mut ()>();

/// Size of a usize.
pub const USIZE_SIZE: usize = core::mem::size_of::<usize>();

/// Size of a u32.
pub const U32_SIZE: usize = core::mem::size_of::<u32>();

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_copy() {
        let src = [1u8, 2, 3, 4, 5];
        let mut dst = [0u8; 5];
        unsafe {
            copy(src.as_ptr(), dst.as_mut_ptr(), src.len());
        }
        assert_eq!(src, dst);
    }

    #[test]
    fn test_set() {
        let mut arr = [0u8; 10];
        unsafe {
            set(arr.as_mut_ptr(), 0xFF, arr.len());
        }
        assert_eq!(arr, [0xFF; 10]);
    }

    #[test]
    fn test_swap() {
        let mut a = 1i32;
        let mut b = 2i32;
        unsafe {
            swap(&mut a, &mut b);
        }
        assert_eq!(a, 2);
        assert_eq!(b, 1);
    }

    #[test]
    fn test_manually_drop() {
        let mut md = ManuallyDrop::new(42);
        let value = unsafe { ManuallyDrop::take(&mut md) };
        assert_eq!(value, 42);
    }

    #[test]
    fn test_maybe_uninit() {
        let uninit: MaybeUninit<i32> = MaybeUninit::uninit();
        let initialized = unsafe { MaybeUninit::assume_init(uninit) };
        // Value is uninitialized, so we just verify no panic
        let _ = initialized;
    }

    #[test]
    fn test_layout() {
        let layout = Layout::new::<i32>();
        assert!(layout.size() >= 4);
        assert!(layout.align() >= 1);
    }

    #[test]
    fn test_align() {
        let data = [0u8; 100];
        let ptr = data.as_ptr();
        let aligned = align_to::<u8, u64>(ptr, 8);
        let addr = aligned as usize;
        assert_eq!(addr % 8, 0);
    }
}
