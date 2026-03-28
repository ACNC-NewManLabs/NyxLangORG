//! NYX Concurrent Layer
//! 
//! Concurrency primitives: Thread, Mutex, RwLock, Channels, etc.

pub mod thread;
pub mod mutex;
pub mod rwlock;
pub mod condvar;
pub mod atomic;
pub mod channels;
pub mod task;
pub mod executor;

/// Initialize concurrent runtime
pub fn init() {
    // Initialize concurrency runtime
}

// Re-exports
pub use thread::Thread;
pub use mutex::Mutex;
pub use rwlock::RwLock;
pub use condvar::Condvar;
pub use channels::Channel;
pub use task::Task;
pub use executor::Executor;
