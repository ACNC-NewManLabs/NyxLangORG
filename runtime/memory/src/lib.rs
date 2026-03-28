// Copyright (c) 2026 SURYA SEKHAR ROY. All Rights Reserved.
// Nyx Secure Memory Architecture™
use std::sync::Arc;
pub struct AllocationGuard {
    limit: usize,
    current: std::sync::atomic::AtomicUsize,
}

impl AllocationGuard {
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            current: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    pub fn try_alloc(&self, size: usize) -> bool {
        let current = self.current.load(std::sync::atomic::Ordering::SeqCst);
        if current + size > self.limit {
            false
        } else {
            self.current.fetch_add(size, std::sync::atomic::Ordering::SeqCst);
            true
        }
    }

    pub fn free(&self, size: usize) {
        self.current.fetch_sub(size, std::sync::atomic::Ordering::SeqCst);
    }
}

pub fn alloc_shared<T>(value: T) -> Arc<T> {
    Arc::new(value)
}

/// Execute a closure with a guaranteed cleanup scope
pub fn with_cleanup<T, F, R>(value: T, f: F) -> R
where
    F: FnOnce(&T) -> R,
{
    let result = f(&value);
    // Explicitly dropping is redundant for most Ts but emphasizes the "cleanup scope" contract
    drop(value);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocation_guard() {
        let guard = AllocationGuard::new(100);
        assert!(guard.try_alloc(50));
        assert!(guard.try_alloc(50));
        assert!(!guard.try_alloc(1));
    }
}
