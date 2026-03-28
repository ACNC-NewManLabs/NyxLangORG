//! NYX RwLock Module

use std::sync::{RwLock as StdRwLock, RwLockReadGuard, RwLockWriteGuard};

pub struct RwLock<T> {
    inner: StdRwLock<T>,
}

impl<T> RwLock<T> {
    pub fn new(value: T) -> RwLock<T> {
        RwLock { inner: StdRwLock::new(value) }
    }

    pub fn read(&self) -> RwLockReadGuard<'_, T> {
        self.inner.read().unwrap()
    }

    pub fn write(&self) -> RwLockWriteGuard<'_, T> {
        self.inner.write().unwrap()
    }
}
