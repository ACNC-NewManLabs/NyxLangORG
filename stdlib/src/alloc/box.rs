//! NYX Box Smart Pointer Module

use std::alloc;
use std::mem;
use std::ptr;

/// A pointer type that owns its data on the heap.
pub struct Box<T> {
    ptr: *mut T,
}

impl<T> Box<T> {
    /// Allocate on heap and put value in it
    pub fn new(value: T) -> Box<T> {
        let layout = alloc::Layout::new::<T>();
        let ptr = unsafe { alloc::alloc(layout) } as *mut T;
        unsafe { ptr.write(value) };
        Box { ptr }
    }

    /// Create from raw pointer
    pub fn from_raw(ptr: *mut T) -> Box<T> {
        Box { ptr }
    }

    /// Consume box, return raw pointer
    pub fn into_raw(b: Box<T>) -> *mut T {
        let ptr = b.ptr;
        mem::forget(b);
        ptr
    }

    /// Get mutable reference
    pub fn as_mut(&mut self) -> &mut T {
        unsafe { &mut *self.ptr }
    }

    /// Get reference
    pub fn as_ref(&self) -> &T {
        unsafe { &*self.ptr }
    }

    /// Unwrap the box
    pub fn unwrap(self) -> T {
        let value = unsafe { self.ptr.read() };
        let layout = alloc::Layout::new::<T>();
        unsafe { alloc::dealloc(self.ptr as *mut u8, layout) };
        value
    }
}

impl<T> Drop for Box<T> {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { ptr::drop_in_place(self.ptr) };
            let layout = alloc::Layout::new::<T>();
            unsafe { alloc::dealloc(self.ptr as *mut u8, layout) };
        }
    }
}
