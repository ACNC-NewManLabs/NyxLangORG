//! NYX Pool Allocator Module

/// Pool allocator for fixed-size objects
pub struct Pool<T> {
    free_list: Vec<*mut T>,
    _marker: core::marker::PhantomData<T>,
}

impl<T> Pool<T> {
    /// Create new pool
    pub fn new() -> Pool<T> {
        Pool {
            free_list: Vec::new(),
            _marker: core::marker::PhantomData,
        }
    }

    /// Allocate from pool
    pub fn alloc(&mut self) -> *mut T {
        if let Some(ptr) = self.free_list.pop() {
            ptr
        } else {
            Box::into_raw(Box::new(unsafe { std::mem::zeroed() }))
        }
    }

    /// Deallocate to pool
    pub fn dealloc(&mut self, ptr: *mut T) {
        self.free_list.push(ptr);
    }
}
