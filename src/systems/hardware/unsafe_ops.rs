//! Unsafe Operations Module

pub fn unsafe_block<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    f()
}

#[allow(dead_code)]
pub struct UnsafeCell<T> {
    value: std::cell::UnsafeCell<T>,
}

impl<T> UnsafeCell<T> {
    pub const fn new(value: T) -> Self {
        Self {
            value: std::cell::UnsafeCell::new(value),
        }
    }
}
