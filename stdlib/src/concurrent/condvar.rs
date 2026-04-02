//! NYX Condvar Module

use std::sync::{Condvar as StdCondvar, MutexGuard};

pub struct Condvar {
    inner: StdCondvar,
}

impl Default for Condvar {
    fn default() -> Self {
        Self::new()
    }
}

impl Condvar {
    pub fn new() -> Condvar {
        Condvar {
            inner: StdCondvar::new(),
        }
    }

    pub fn wait<'a, T>(&self, guard: MutexGuard<'a, T>) -> MutexGuard<'a, T> {
        self.inner.wait(guard).unwrap()
    }

    pub fn notify_one(&self) {
        self.inner.notify_one();
    }

    pub fn notify_all(&self) {
        self.inner.notify_all();
    }
}
