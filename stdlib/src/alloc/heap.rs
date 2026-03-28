//! NYX Heap Allocator Module

/// Initialize heap allocator
pub fn init() {
    // Initialize default allocator
}

/// Allocate memory
pub unsafe fn alloc<T>(count: usize) -> *mut T {
    let layout = std::alloc::Layout::array::<T>(count).unwrap();
    std::alloc::alloc(layout) as *mut T
}

/// Deallocate memory
pub unsafe fn dealloc<T>(ptr: *mut T, count: usize) {
    let layout = std::alloc::Layout::array::<T>(count).unwrap();
    std::alloc::dealloc(ptr as *mut u8, layout);
}

/// Reallocate memory
pub unsafe fn realloc<T>(ptr: *mut T, old_count: usize, new_count: usize) -> *mut T {
    let old_layout = std::alloc::Layout::array::<T>(old_count).unwrap();
    let new_layout = std::alloc::Layout::array::<T>(new_count).unwrap();
    std::alloc::realloc(ptr as *mut u8, old_layout, new_layout.size()) as *mut T
}

/// Heap structure
pub struct Heap;

impl Heap {
    /// Create new heap
    pub fn new() -> Heap {
        Heap
    }
}
