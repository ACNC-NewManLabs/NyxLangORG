//! NYX Unique Pointer Module

/// Unique pointer (non-shared ownership)
pub struct Unique<T> {
    ptr: *mut T,
}

impl<T> Unique<T> {
    /// Create new unique pointer
    pub fn new(value: T) -> Unique<T> {
        let ptr = Box::into_raw(Box::new(value));
        Unique { ptr }
    }

    /// Get raw pointer
    pub fn as_ptr(&self) -> *mut T {
        self.ptr
    }

    /// Get reference
    pub fn get(&self) -> &T {
        unsafe { &*self.ptr }
    }

    /// Get mutable reference
    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.ptr }
    }
}

impl<T> Drop for Unique<T> {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { let _ = Box::from_raw(self.ptr); };
        }
    }
}
