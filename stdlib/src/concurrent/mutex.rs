//! NYX Mutex Module

use std::sync::{Mutex as StdMutex, MutexGuard};

pub struct Mutex<T> {
    inner: StdMutex<T>,
}

impl<T> Mutex<T> {
    pub fn new(value: T) -> Mutex<T> {
        Mutex { inner: StdMutex::new(value) }
    }
    pub fn lock(&self) -> MutexGuard<'_, T> {
        self.inner.lock().unwrap()
    }
}
