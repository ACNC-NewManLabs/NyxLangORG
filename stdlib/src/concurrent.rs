//! NYX Concurrent Layer
//!
//! Concurrency primitives: Thread, Mutex, RwLock, Channels, etc.

pub mod actor;
pub mod atomic;
pub mod channels;
pub mod condvar;
pub mod executor;
pub mod mutex;
pub mod rwlock;
pub mod task;
pub mod thread;

/// Initialize concurrent runtime
pub fn init() {
    // Initialize concurrency runtime
}

// Re-exports
pub use actor::Actor;
pub use channels::Channel;
pub use condvar::Condvar;
pub use executor::Executor;
pub use mutex::Mutex;
pub use rwlock::RwLock;
pub use task::Task;
pub use thread::Thread;
