//! NYX RwLock, Condvar, Atomic, Channels, Task, Executor Modules

pub mod rwlock {
    use std::sync::RwLock as StdRwLock;
    pub struct RwLock<T> { inner: StdRwLock<T> }
    impl<T> RwLock<T> {
        pub fn new(value: T) -> RwLock<T> { RwLock { inner: StdRwLock::new(value) } }
        pub fn read(&self) -> std::sync::RwLockReadGuard<'_, T> { self.inner.read().unwrap() }
        pub fn write(&self) -> std::sync::RwLockWriteGuard<'_, T> { self.inner.write().unwrap() }
    }
}
pub mod condvar {
    use std::sync::Condvar as StdCondvar;
    pub struct Condvar { inner: StdCondvar }
    impl Condvar { pub fn new() -> Condvar { Condvar { inner: StdCondvar::new() } } }
}
pub mod atomic {
    pub use std::sync::atomic::*;
}
pub mod channels {
    use std::sync::mpsc::{channel, Sender, Receiver};
    pub fn channel<T>() -> (Sender<T>, Receiver<T>) { channel() }
}
pub mod task {
    pub struct Task;
}
pub mod executor {
    pub struct Executor;
}

