//! NYX Thread Module

use std::thread::{self, JoinHandle};

pub struct Thread<T>(JoinHandle<T>);

pub fn spawn<F, T>(f: F) -> Thread<T>
where
    F: FnOnce() -> T,
    F: Send + 'static,
    T: Send + 'static,
{
    Thread(thread::spawn(f))
}

impl<T> Thread<T> {
    pub fn join(self) -> std::thread::Result<T> {
        self.0.join()
    }
}

pub fn current() -> thread::Thread {
    thread::current()
}

pub fn sleep(dur: std::time::Duration) {
    thread::sleep(dur);
}
