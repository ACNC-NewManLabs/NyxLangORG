#[cfg(not(target_arch = "wasm32"))]
use std::thread;

#[cfg(not(target_arch = "wasm32"))]
pub fn spawn_task<F, T>(f: F) -> thread::JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    thread::spawn(f)
}
