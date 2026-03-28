//! NYX Arc Smart Pointer Module

use std::sync::Arc as StdArc;

/// Arc - Atomically Reference Counted pointer
pub struct Arc<T> {
    inner: StdArc<T>,
}

impl<T> Arc<T> {
    /// Create new Arc
    pub fn new(value: T) -> Arc<T> {
        Arc { inner: StdArc::new(value) }
    }

    /// Get reference count
    pub fn strong_count(&self) -> usize {
        StdArc::strong_count(&self.inner)
    }

    /// Get reference to value
    pub fn get(&self) -> &T {
        &self.inner
    }
}

impl<T> Clone for Arc<T> {
    fn clone(&self) -> Arc<T> {
        Arc { inner: self.inner.clone() }
    }
}
