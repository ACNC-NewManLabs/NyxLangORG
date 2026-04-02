//! NYX Core Pointer Module
//!
//! Raw pointer utilities for the core library.
//! Provides pointer operations that work without an OS.

use core::fmt;

// =============================================================================
// Raw Pointer Types
// =============================================================================

/// A raw pointer type for unique ownership without aliasing.
///
/// This is similar to Rust's Unique<T> but designed for the NYX core library.
#[repr(transparent)]
pub struct NonNull<T: ?Sized> {
    ptr: *const T,
}

impl<T: ?Sized> NonNull<T> {
    /// Creates a new NonNull from a raw pointer.
    ///
    /// # Safety
    ///
    /// The pointer must be non-null and properly aligned.
    #[inline]
    pub unsafe fn new_unchecked(ptr: *mut T) -> NonNull<T> {
        NonNull { ptr }
    }

    /// Creates a new NonNull from a raw pointer, returning None if null.
    #[inline]
    pub fn new(ptr: *mut T) -> Option<NonNull<T>> {
        if ptr.is_null() {
            None
        } else {
            Some(NonNull { ptr })
        }
    }

    /// Returns the raw pointer.
    #[inline]
    pub fn as_ptr(self: &NonNull<T>) -> *mut T {
        self.ptr as *mut T
    }

    /// Returns a reference to the contained value.
    ///
    /// # Safety
    ///
    /// The pointer must be valid for the lifetime 'a.
    #[inline]
    pub unsafe fn as_ref(self: &NonNull<T>) -> &T {
        &*self.ptr
    }

    /// Returns a mutable reference to the contained value.
    ///
    /// # Safety
    ///
    /// The pointer must be valid for the lifetime 'a and no other
    /// references to the same data must exist.
    #[inline]
    pub unsafe fn as_mut(self: &mut NonNull<T>) -> &mut T {
        &mut *(self.ptr as *mut T)
    }

    /// Creates a NonNull that points to invalid memory.
    ///
    /// This is also called a "dangling" pointer.
    pub const fn dangling() -> NonNull<T>
    where
        T: Sized,
    {
        NonNull {
            ptr: core::ptr::null_mut::<T>().wrapping_add(1),
        }
    }
}

impl<T> NonNull<[T]> {
    /// Creates a NonNull for a slice of length n.
    #[inline]
    pub const fn new_slice(ptr: *mut T, len: usize) -> Option<NonNull<[T]>> {
        if ptr.is_null() && len != 0 {
            None
        } else {
            Some(NonNull {
                ptr: core::ptr::slice_from_raw_parts_mut(ptr, len),
            })
        }
    }

    /// Creates a NonNull that points to invalid memory of length n.
    #[inline]
    pub const fn dangling_slice(n: usize) -> NonNull<[T]> {
        let ptr = core::mem::align_of::<T>() as *mut T;
        NonNull {
            ptr: core::ptr::slice_from_raw_parts_mut(ptr, n),
        }
    }
}

impl<T: ?Sized> Clone for NonNull<T> {
    #[inline]
    fn clone(&self) -> NonNull<T> {
        NonNull { ptr: self.ptr }
    }
}

impl<T: ?Sized> Copy for NonNull<T> {}

impl<T: ?Sized> fmt::Debug for NonNull<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Pointer::fmt(&self.ptr, f)
    }
}

// =============================================================================
// Shared Pointer (simulating &T)
// =============================================================================

/// Represents a borrow of some data.
///
/// This is a marker type for documentation purposes, representing
/// the concept of an immutable reference.
#[repr(transparent)]
pub struct Ref<'a, T: ?Sized> {
    data: *const T,
    _marker: core::marker::PhantomData<&'a T>,
}

impl<'a, T: ?Sized> Ref<'a, T> {
    /// Creates a new Ref from a raw pointer.
    ///
    /// # Safety
    ///
    /// The pointer must be valid for the lifetime 'a.
    #[inline]
    pub unsafe fn new(ptr: *const T) -> Ref<'a, T> {
        Ref {
            data: ptr,
            _marker: core::marker::PhantomData,
        }
    }

    /// Returns the raw pointer.
    #[inline]
    pub fn as_ptr(&self) -> *const T {
        self.data
    }
}

impl<T: ?Sized> Copy for Ref<'_, T> {}
impl<'a, T: ?Sized> Clone for Ref<'a, T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

// =============================================================================
// Mutable Reference (simulating &mut T)
// =============================================================================

/// Represents an exclusive borrow of some data.
///
/// This is a marker type for documentation purposes, representing
/// the concept of a mutable reference.
#[repr(transparent)]
pub struct RefMut<'a, T: ?Sized> {
    data: *mut T,
    _marker: core::marker::PhantomData<&'a mut T>,
}

impl<'a, T: ?Sized> RefMut<'a, T> {
    /// Creates a new RefMut from a raw pointer.
    ///
    /// # Safety
    ///
    /// The pointer must be valid for the lifetime 'a and no other
    /// references to the same data must exist.
    #[inline]
    pub unsafe fn new(ptr: *mut T) -> RefMut<'a, T> {
        RefMut {
            data: ptr,
            _marker: core::marker::PhantomData,
        }
    }

    /// Returns the raw pointer.
    #[inline]
    pub fn as_ptr(&self) -> *mut T {
        self.data
    }
}

impl<T: ?Sized> Copy for RefMut<'_, T> {}
impl<'a, T: ?Sized> Clone for RefMut<'a, T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

// =============================================================================
// Pointer Cast Utilities
// =============================================================================

/// Reinterprets a pointer of one type as a pointer of another type.
///
/// # Safety
///
/// The caller must ensure that the pointer is properly aligned
/// and valid for the new type.
#[inline]
pub unsafe fn cast<T, U>(ptr: *const T) -> *const U {
    ptr as *const U
}

/// Reinterprets a mutable pointer of one type as a mutable pointer of another type.
///
/// # Safety
///
/// The caller must ensure that the pointer is properly aligned
/// and valid for the new type.
#[inline]
pub unsafe fn cast_mut<T, U>(ptr: *mut T) -> *mut U {
    ptr as *mut U
}

/// Reinterprets a pointer as a pointer to a sized type.
///
/// # Safety
///
/// The caller must ensure that the pointer is valid for reading/writing
/// the sized type.
#[inline]
pub unsafe fn cast_to<T: ?Sized, U>(ptr: *const T) -> *const U {
    ptr as *const U
}

/// Reinterprets a mutable pointer as a mutable pointer to a sized type.
///
/// # Safety
///
/// The caller must ensure that the pointer is valid for reading/writing
/// the sized type.
#[inline]
pub unsafe fn cast_to_mut<T: ?Sized, U>(ptr: *mut T) -> *mut U {
    ptr as *mut U
}

// =============================================================================
// Pointer Validation
// =============================================================================

/// Returns true if the pointer is null.
#[inline]
pub fn is_null<T: ?Sized>(ptr: *const T) -> bool {
    ptr.is_null()
}

#[inline]
pub fn is_aligned<T>(ptr: *const T) -> bool {
    let addr = ptr as *const () as usize;
    addr.is_multiple_of(core::mem::align_of::<T>())
}

/// Returns the address of the pointer.
#[inline]
pub fn addr<T: ?Sized>(ptr: *const T) -> usize {
    ptr as *const () as usize
}

// =============================================================================
// Weak Reference
// =============================================================================

/// A weak reference to a heap-allocated object.
///
/// This is similar to std::rc::Weak but implemented at the core level.
pub struct Weak<T: ?Sized> {
    ptr: *const T,
}

impl<T: ?Sized> Weak<T> {
    /// Creates a new Weak pointer from a raw pointer.
    ///
    /// # Safety
    ///
    /// The pointer must be valid and properly managed.
    #[inline]
    pub unsafe fn new(ptr: *const T) -> Weak<T> {
        Weak { ptr }
    }

    /// Attempts to upgrade the weak reference to a strong reference.
    ///
    /// Returns None if the object has been dropped.
    #[inline]
    pub fn upgrade(&self) -> Option<NonNull<T>> {
        // This is a simplified version; real implementation would
        // check reference counts
        NonNull::new(self.ptr as *mut T)
    }

    /// Returns the raw pointer.
    #[inline]
    pub fn as_ptr(&self) -> *const T {
        self.ptr
    }
}

impl<T: ?Sized> Clone for Weak<T> {
    #[inline]
    fn clone(&self) -> Weak<T> {
        Weak { ptr: self.ptr }
    }
}

impl<T: ?Sized> Copy for Weak<T> {}

impl<T: ?Sized> fmt::Debug for Weak<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Weak({:p})", self.ptr)
    }
}

// =============================================================================

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nonnull_create() {
        let value = 42;
        let ptr = &value as *const i32 as *mut i32;
        let nonnull = NonNull::new(ptr);
        assert!(nonnull.is_some());
    }

    #[test]
    fn test_nonnull_dangling() {
        let dangling = NonNull::<i32>::dangling();
        assert!(!dangling.as_ptr().is_null());
    }

    #[test]
    fn test_nonnull_slice() {
        let arr = [1, 2, 3, 4, 5];
        let ptr = arr.as_ptr() as *mut i32;
        let nonnull = NonNull::new_slice(ptr, 5);
        assert!(nonnull.is_some());
    }

    #[test]
    fn test_is_null() {
        let ptr: *const i32 = core::ptr::null();
        assert!(is_null(ptr));

        let value = 42;
        let ptr = &value as *const i32;
        assert!(!is_null(ptr));
    }

    #[test]
    fn test_is_aligned() {
        let value = 42i32;
        let ptr = &value as *const i32;
        assert!(is_aligned(ptr));
    }

    #[test]
    fn test_weak() {
        let value = 42;
        let ptr = &value as *const i32;
        let weak = unsafe { Weak::new(ptr) };
        let strong = weak.upgrade();
        assert_eq!(strong.is_some(), true);
    }

    #[test]
    fn test_cast() {
        let value: i32 = 42;
        let ptr = &value as *const i32;
        let casted: *const u32 = unsafe { cast(ptr) };
        assert!(!casted.is_null());
    }
}
