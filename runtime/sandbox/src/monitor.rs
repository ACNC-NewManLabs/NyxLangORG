//! Resource Monitor
//! 
//! This module provides resource monitoring for sandbox execution.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

/// Resource monitor for tracking usage
#[derive(Debug)]
pub struct ResourceMonitor {
    /// Active flag
    active: Arc<AtomicBool>,
    /// Memory usage
    memory_used: Arc<AtomicUsize>,
    /// Operation count
    ops_count: Arc<AtomicUsize>,
}

impl ResourceMonitor {
    /// Create new monitor
    pub fn new() -> Self {
        Self {
            active: Arc::new(AtomicBool::new(false)),
            memory_used: Arc::new(AtomicUsize::new(0)),
            ops_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Start monitoring
    pub fn start(&self) {
        self.active.store(true, Ordering::SeqCst);
    }

    /// Stop monitoring
    pub fn stop(&self) {
        self.active.store(false, Ordering::SeqCst);
    }

    /// Check if active
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    /// Record memory allocation
    pub fn record_alloc(&self, size: usize) {
        if self.active.load(Ordering::SeqCst) {
            self.memory_used.fetch_add(size, Ordering::SeqCst);
            self.ops_count.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// Record memory deallocation
    pub fn record_dealloc(&self, size: usize) {
        if self.active.load(Ordering::SeqCst) {
            self.memory_used.fetch_sub(size, Ordering::SeqCst);
        }
    }

    /// Get current memory usage
    pub fn memory_used(&self) -> usize {
        self.memory_used.load(Ordering::SeqCst)
    }

    /// Get operation count
    pub fn ops_count(&self) -> usize {
        self.ops_count.load(Ordering::SeqCst)
    }

    /// Reset counters
    pub fn reset(&self) {
        self.memory_used.store(0, Ordering::SeqCst);
        self.ops_count.store(0, Ordering::SeqCst);
    }

    /// Clone as arc
    pub fn clone_arc(&self) -> Self {
        Self {
            active: self.active.clone(),
            memory_used: self.memory_used.clone(),
            ops_count: self.ops_count.clone(),
        }
    }
}

impl Default for ResourceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ResourceMonitor {
    fn clone(&self) -> Self {
        self.clone_arc()
    }
}

/// Monitoring statistics
#[derive(Debug, Clone, Default)]
pub struct MonitorStats {
    /// Active
    pub active: bool,
    /// Memory used
    pub memory_used: usize,
    /// Operations count
    pub ops_count: usize,
}

impl ResourceMonitor {
    /// Get current statistics
    pub fn stats(&self) -> MonitorStats {
        MonitorStats {
            active: self.is_active(),
            memory_used: self.memory_used(),
            ops_count: self.ops_count(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor() {
        let monitor = ResourceMonitor::new();
        
        assert!(!monitor.is_active());
        
        monitor.start();
        assert!(monitor.is_active());
        
        monitor.record_alloc(1000);
        assert_eq!(monitor.memory_used(), 1000);
        
        monitor.record_dealloc(500);
        assert_eq!(monitor.memory_used(), 500);
        
        assert_eq!(monitor.ops_count(), 1);
        
        monitor.stop();
        assert!(!monitor.is_active());
    }

    #[test]
    fn test_monitor_clone() {
        let monitor = ResourceMonitor::new();
        let cloned = monitor.clone();
        
        monitor.start();
        assert!(cloned.is_active());
    }
}

