//! NYX Executor Module

use super::task::Task;
use std::thread;

pub struct Executor;

impl Executor {
    pub fn spawn<F, T>(f: F) -> Task<T>
    where
        F: FnOnce() -> T,
        F: Send + 'static,
        T: Send + 'static,
    {
        Task::from_handle(thread::spawn(f))
    }
}
