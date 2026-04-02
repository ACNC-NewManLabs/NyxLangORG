//! Raw Pointer Types Module
//!
//! Provides raw pointer types for direct memory access in systems programming.

use std::fmt;

/// Immutable raw pointer type (equivalent to *const T)
#[derive(Clone, Copy)]
pub struct Ptr<T> {
    ptr: *const T,
}

impl<T> Ptr<T> {
    /// Create a new raw pointer from a reference
    pub fn new(ptr: &T) -> Self {
        Self { ptr }
    }

    /// Create a raw pointer from a raw address
    /// # Safety
    /// The address must be valid for type T
    /// # Safety
    /// Hardware constraints apply.
    pub unsafe fn from_addr(addr: usize) -> Self {
        Self {
            ptr: addr as *const T,
        }
    }

    /// Get the raw pointer address
    pub fn addr(self) -> usize {
        self.ptr as usize
    }

    /// Get the underlying raw pointer
    pub fn as_ptr(self) -> *const T {
        self.ptr
    }

    /// Convert to mutable pointer
    /// # Safety
    /// The caller must ensure exclusive mutable access
    /// # Safety
    /// Hardware constraints apply.
    pub unsafe fn as_mut(self) -> PtrMut<T> {
        PtrMut {
            ptr: self.ptr as *mut T,
        }
    }

    /// Read the value at the pointer
    /// # Safety
    /// The pointer must point to valid data
    /// # Safety
    /// Hardware constraints apply.
    pub unsafe fn read(self) -> T
    where
        T: Copy,
    {
        std::ptr::read(self.ptr)
    }

    /// Get a reference to the value
    /// # Safety
    /// The pointer must point to valid data
    /// # Safety
    /// Hardware constraints apply.
    pub unsafe fn as_ref(&self) -> &T {
        &*self.ptr
    }

    /// Check if pointer is null
    pub fn is_null(self) -> bool {
        self.ptr.is_null()
    }

    /// Offset the pointer by n elements
    /// # Safety
    /// The resulting pointer must be in bounds or one byte past the end
    /// # Safety
    /// Hardware constraints apply.
    pub unsafe fn offset(self, n: isize) -> Self {
        Self {
            ptr: self.ptr.offset(n),
        }
    }

    /// Add offset to pointer
    /// # Safety
    /// Hardware constraints apply.
    pub unsafe fn add(self, count: usize) -> Self {
        Self {
            ptr: self.ptr.add(count),
        }
    }

    /// Subtract offset from pointer
    /// # Safety
    /// Hardware constraints apply.
    pub unsafe fn sub(self, count: usize) -> Self {
        Self {
            ptr: self.ptr.sub(count),
        }
    }

    /// Cast pointer to another type
    pub fn cast<U>(self) -> Ptr<U> {
        Ptr {
            ptr: self.ptr as *const U,
        }
    }
}

impl<T> fmt::Debug for Ptr<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Ptr({:p})", self.ptr)
    }
}

impl<T> fmt::Pointer for Ptr<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:p}", self.ptr)
    }
}

/// Mutable raw pointer type (equivalent to *mut T)
#[derive(Clone, Copy)]
pub struct PtrMut<T> {
    ptr: *mut T,
}

impl<T> PtrMut<T> {
    /// Create a new mutable raw pointer from a mutable reference
    pub fn new(ptr: &mut T) -> Self {
        Self { ptr }
    }

    /// Create a mutable raw pointer from a raw address
    /// # Safety
    /// The address must be valid for type T
    /// # Safety
    /// Hardware constraints apply.
    pub unsafe fn from_addr(addr: usize) -> Self {
        Self {
            ptr: addr as *mut T,
        }
    }

    /// Get the raw pointer address
    pub fn addr(self) -> usize {
        self.ptr as usize
    }

    /// Get the underlying raw mutable pointer
    pub fn as_ptr(self) -> *mut T {
        self.ptr
    }

    /// Convert to immutable pointer
    pub fn as_const(self) -> Ptr<T> {
        Ptr { ptr: self.ptr }
    }

    /// Write a value to the pointer location
    /// # Safety
    /// The pointer must point to valid memory for writing
    /// # Safety
    /// Hardware constraints apply.
    pub unsafe fn write(self, value: T) {
        std::ptr::write(self.ptr, value);
    }

    /// Write a value to the pointer location without dropping the old value
    /// # Safety
    /// The pointer must point to valid memory for writing
    /// # Safety
    /// Hardware constraints apply.
    pub unsafe fn write_volatile(self, value: T) {
        std::ptr::write_volatile(self.ptr, value);
    }

    /// Get a mutable reference to the value
    /// # Safety
    /// The pointer must point to valid data
    /// # Safety
    /// Hardware constraints apply.
    pub unsafe fn as_mut(&mut self) -> &mut T {
        &mut *self.ptr
    }

    /// Get an immutable reference to the value
    /// # Safety
    /// The pointer must point to valid data
    /// # Safety
    /// Hardware constraints apply.
    pub unsafe fn as_ref(&self) -> &T {
        &*self.ptr
    }

    /// Read the value at the pointer
    /// # Safety
    /// The pointer must point to valid data
    /// # Safety
    /// Hardware constraints apply.
    pub unsafe fn read(self) -> T
    where
        T: Copy,
    {
        std::ptr::read(self.ptr)
    }

    /// Read the value at the pointer (volatile)
    /// # Safety
    /// The pointer must point to valid data
    /// # Safety
    /// Hardware constraints apply.
    pub unsafe fn read_volatile(self) -> T
    where
        T: Copy,
    {
        std::ptr::read_volatile(self.ptr)
    }

    /// Check if pointer is null
    pub fn is_null(self) -> bool {
        self.ptr.is_null()
    }

    /// Offset the pointer by n elements
    /// # Safety
    /// The resulting pointer must be in bounds or one byte past the end
    /// # Safety
    /// Hardware constraints apply.
    pub unsafe fn offset(self, n: isize) -> Self {
        Self {
            ptr: self.ptr.offset(n),
        }
    }

    /// Add offset to pointer
    /// # Safety
    /// Hardware constraints apply.
    pub unsafe fn add(self, count: usize) -> Self {
        Self {
            ptr: self.ptr.add(count),
        }
    }

    /// Subtract offset from pointer
    /// # Safety
    /// Hardware constraints apply.
    pub unsafe fn sub(self, count: usize) -> Self {
        Self {
            ptr: self.ptr.sub(count),
        }
    }

    /// Cast pointer to another type
    pub fn cast<U>(self) -> PtrMut<U> {
        PtrMut {
            ptr: self.ptr as *mut U,
        }
    }

    /// Swap values at two pointer locations
    /// # Safety
    /// Both pointers must point to valid memory
    /// # Safety
    /// Hardware constraints apply.
    pub unsafe fn swap(self, other: PtrMut<T>) {
        std::ptr::swap(self.ptr, other.ptr);
    }

    /// Replace value at pointer with new value, return old value
    /// # Safety
    /// The pointer must point to valid memory
    /// # Safety
    /// Hardware constraints apply.
    pub unsafe fn replace(self, value: T) -> T {
        std::ptr::replace(self.ptr, value)
    }

    /// Take value from pointer, leaving zeroed memory
    /// # Safety
    /// The pointer must point to valid memory and T must implement Default
    /// # Safety
    /// Hardware constraints apply.
    pub unsafe fn take(self) -> T
    where
        T: Default,
    {
        core::mem::take(&mut *self.ptr)
    }
}

impl<T> fmt::Debug for PtrMut<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PtrMut({:p})", self.ptr)
    }
}

impl<T> fmt::Pointer for PtrMut<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:p}", self.ptr)
    }
}

/// Pointer arithmetic operations
pub mod pointer_arithmetic {
    use super::{Ptr, PtrMut};

    /// Calculate distance between two pointers
    /// # Safety
    /// Both pointers must point to the same allocated object
    /// # Safety
    /// Hardware constraints apply.
    pub unsafe fn distance<T>(from: Ptr<T>, to: Ptr<T>) -> isize {
        to.as_ptr().offset_from(from.as_ptr())
    }

    /// Calculate distance between two mutable pointers
    /// # Safety
    /// Both pointers must point to the same allocated object
    /// # Safety
    /// Hardware constraints apply.
    pub unsafe fn distance_mut<T>(from: PtrMut<T>, to: PtrMut<T>) -> isize {
        to.as_ptr().offset_from(from.as_ptr())
    }

    /// Align pointer to specified alignment
    /// # Safety
    /// The pointer must be valid
    /// # Safety
    /// Hardware constraints apply.
    pub unsafe fn align_to<T, const ALIGN: usize>(ptr: Ptr<T>) -> Ptr<T> {
        let addr = ptr.addr();
        let aligned = (addr + ALIGN - 1) & !(ALIGN - 1);
        Ptr::from_addr(aligned)
    }

    /// Align mutable pointer to specified alignment
    /// # Safety
    /// The pointer must be valid
    /// # Safety
    /// Hardware constraints apply.
    pub unsafe fn align_to_mut<T, const ALIGN: usize>(ptr: PtrMut<T>) -> PtrMut<T> {
        let addr = ptr.addr();
        let aligned = (addr + ALIGN - 1) & !(ALIGN - 1);
        PtrMut::from_addr(aligned)
    }

    /// Check if pointer is aligned to specified alignment
    pub fn is_aligned<T, const ALIGN: usize>(ptr: Ptr<T>) -> bool {
        ptr.addr().is_multiple_of(ALIGN)
    }

    /// Check if mutable pointer is aligned to specified alignment
    pub fn is_aligned_mut<T, const ALIGN: usize>(ptr: PtrMut<T>) -> bool {
        ptr.addr().is_multiple_of(ALIGN)
    }
}

/// Null pointer constant
pub const fn null<T>() -> Ptr<T> {
    Ptr {
        ptr: std::ptr::null(),
    }
}

/// Null mutable pointer constant
pub fn null_mut<T>() -> PtrMut<T> {
    PtrMut {
        ptr: std::ptr::null_mut(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ptr_operations() {
        let value = 42;
        let ptr = Ptr::new(&value);

        unsafe {
            assert_eq!(*ptr.as_ref(), 42);
            assert!(!ptr.is_null());
        }
    }

    #[test]
    fn test_ptr_mut_operations() {
        let mut value = 42;
        let mut ptr = PtrMut::new(&mut value);

        unsafe {
            ptr.write(100);
            assert_eq!(*ptr.as_mut(), 100);
        }
    }

    #[test]
    fn test_null_ptr() {
        let ptr: Ptr<i32> = null();
        assert!(ptr.is_null());
    }

    #[test]
    fn test_pointer_cast() {
        let value = 42i32;
        let ptr = Ptr::new(&value);
        let cast_ptr: Ptr<u8> = ptr.cast();

        assert!(!cast_ptr.is_null());
    }
}
