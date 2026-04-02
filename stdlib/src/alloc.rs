//! NYX Allocation Layer
//!
//! Allocation systems including heap, arena, pool, stack allocators
//! and smart pointers: Box<T>, Arc<T>, Rc<T>, Unique<T>

pub mod arc;
pub mod arena;
pub mod r#box;
pub mod heap;
pub mod pool;
pub mod rc;
pub mod stack;
pub mod unique;

/// Initialize the allocator
pub fn init() {
    heap::init();
}

/// Shutdown the allocator
pub fn shutdown() {
    // Cleanup allocator
}

// Re-exports
pub use arc::Arc;
pub use heap::Heap;
pub use r#box::Box;
pub use rc::Rc;
pub use unique::Unique;
