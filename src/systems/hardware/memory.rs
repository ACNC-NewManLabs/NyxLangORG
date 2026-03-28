//! Memory Operations Module
//! Provides low-level memory operations and physical page allocation.

use core::alloc::{GlobalAlloc, Layout};

pub struct Allocator;

unsafe impl GlobalAlloc for Allocator {
    /// # Safety
    /// Hardware constraints apply.
    unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
        core::ptr::null_mut()
    }
    /// # Safety
    /// Hardware constraints apply.
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

pub struct MemoryRegion;
