// Copyright (c) 2026 SURYA SEKHAR ROY. All Rights Reserved.
// Nyx Multithreading Engine™
use std::thread;

pub fn spawn_named<F, T>(name: &str, f: F) -> Result<thread::JoinHandle<T>, String>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    thread::Builder::new()
        .name(name.to_string())
        .spawn(f)
        .map_err(|e| e.to_string())
}
