//! NYX Task Module

use std::thread::JoinHandle;

pub struct Task<T> {
    handle: JoinHandle<T>,
}

impl<T> Task<T> {
    pub fn from_handle(handle: JoinHandle<T>) -> Task<T> {
        Task { handle }
    }

    pub fn join(self) -> std::thread::Result<T> {
        self.handle.join()
    }
}
