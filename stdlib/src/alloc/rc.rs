//! NYX Rc Smart Pointer Module

use std::rc::Rc as StdRc;

/// Rc - Reference Counted pointer (single-threaded)
pub struct Rc<T> {
    inner: StdRc<T>,
}

impl<T> Rc<T> {
    /// Create new Rc
    pub fn new(value: T) -> Rc<T> {
        Rc {
            inner: StdRc::new(value),
        }
    }

    /// Get reference count
    pub fn strong_count(&self) -> usize {
        StdRc::strong_count(&self.inner)
    }

    /// Get reference to value
    pub fn get(&self) -> &T {
        &self.inner
    }
}

impl<T> Clone for Rc<T> {
    fn clone(&self) -> Rc<T> {
        Rc {
            inner: self.inner.clone(),
        }
    }
}
