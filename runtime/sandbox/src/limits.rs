//! Resource Limits
//!
//! This module defines resource limits for sandbox execution.

use std::time::Instant;

/// Memory limit configuration
#[derive(Debug, Clone)]
pub struct MemoryLimit {
    /// Maximum heap size in bytes
    pub max_heap: usize,
    /// Maximum stack size in bytes
    pub max_stack: usize,
    /// Maximum total memory in bytes
    pub max_total: usize,
    /// Page size
    pub page_size: usize,
}

impl Default for MemoryLimit {
    fn default() -> Self {
        Self {
            max_heap: 128 * 1024 * 1024,  // 128 MB
            max_stack: 8 * 1024 * 1024,   // 8 MB
            max_total: 256 * 1024 * 1024, // 256 MB
            page_size: 4096,              // 4 KB pages
        }
    }
}

impl MemoryLimit {
    /// Create new memory limit
    pub fn new(max_heap: usize, max_stack: usize, max_total: usize) -> Self {
        Self {
            max_heap,
            max_stack,
            max_total,
            page_size: 4096,
        }
    }

    /// Check if allocation would exceed limit
    pub fn can_allocate(&self, current: usize, size: usize) -> bool {
        current + size <= self.max_total
    }

    /// Get number of pages for size
    pub fn pages_for(&self, size: usize) -> usize {
        size.div_ceil(self.page_size)
    }
}

/// CPU limit configuration
#[derive(Debug, Clone)]
pub struct CpuLimit {
    /// Maximum CPU time in seconds
    pub max_time: u64,
    /// Start time
    start_time: Option<Instant>,
    /// Time used
    time_used: u64,
}

impl Default for CpuLimit {
    fn default() -> Self {
        Self {
            max_time: 30,
            start_time: None,
            time_used: 0,
        }
    }
}

impl CpuLimit {
    /// Create new CPU limit
    pub fn new(max_time: u64) -> Self {
        Self {
            max_time,
            start_time: None,
            time_used: 0,
        }
    }

    /// Start timing
    pub fn start(&mut self) {
        self.start_time = Some(Instant::now());
    }

    /// Stop timing
    pub fn stop(&mut self) {
        if let Some(start) = self.start_time.take() {
            self.time_used += start.elapsed().as_secs();
        }
    }

    /// Check if time limit exceeded
    pub fn is_exceeded(&self) -> bool {
        self.time_used >= self.max_time
    }

    /// Get remaining time
    pub fn remaining(&self) -> u64 {
        self.max_time.saturating_sub(self.time_used)
    }

    /// Update time used
    pub fn add_time(&mut self, secs: u64) {
        self.time_used += secs;
    }
}

/// Resource limits container
#[derive(Debug, Clone, Default)]
pub struct ResourceLimits {
    /// Memory limits
    pub memory: MemoryLimit,
    /// CPU limits
    pub cpu: CpuLimit,
    /// Maximum file size
    pub max_file_size: usize,
    /// Maximum number of open files
    pub max_open_files: usize,
    /// Maximum number of processes
    pub max_processes: usize,
}

impl ResourceLimits {
    /// Create new limits
    pub fn new(memory: MemoryLimit, cpu: CpuLimit) -> Self {
        Self {
            memory,
            cpu,
            max_file_size: 10 * 1024 * 1024, // 10 MB
            max_open_files: 64,
            max_processes: 1,
        }
    }

    /// Create restrictive limits
    pub fn restrictive() -> Self {
        Self {
            memory: MemoryLimit::new(
                64 * 1024 * 1024,  // 64 MB heap
                4 * 1024 * 1024,   // 4 MB stack
                128 * 1024 * 1024, // 128 MB total
            ),
            cpu: CpuLimit::new(10),     // 10 seconds
            max_file_size: 1024 * 1024, // 1 MB
            max_open_files: 16,
            max_processes: 1,
        }
    }

    /// Create moderate limits
    pub fn moderate() -> Self {
        Self {
            memory: MemoryLimit::new(
                128 * 1024 * 1024, // 128 MB heap
                8 * 1024 * 1024,   // 8 MB stack
                256 * 1024 * 1024, // 256 MB total
            ),
            cpu: CpuLimit::new(30),          // 30 seconds
            max_file_size: 10 * 1024 * 1024, // 10 MB
            max_open_files: 64,
            max_processes: 1,
        }
    }

    /// Create permissive limits
    pub fn permissive() -> Self {
        Self {
            memory: MemoryLimit::new(
                512 * 1024 * 1024,  // 512 MB heap
                16 * 1024 * 1024,   // 16 MB stack
                1024 * 1024 * 1024, // 1 GB total
            ),
            cpu: CpuLimit::new(300),          // 5 minutes
            max_file_size: 100 * 1024 * 1024, // 100 MB
            max_open_files: 256,
            max_processes: 4,
        }
    }
}

/// Track resource usage
#[derive(Debug, Clone, Default)]
pub struct ResourceUsage {
    /// Current memory usage
    pub memory_used: usize,
    /// Peak memory usage
    pub memory_peak: usize,
    /// CPU time used in seconds
    pub cpu_time: u64,
    /// Peak CPU time
    pub cpu_time_peak: u64,
    /// Number of allocations
    pub allocations: u64,
    /// Number of deallocations
    pub deallocations: u64,
    /// Number of file operations
    pub file_ops: u64,
    /// Number of syscalls
    pub syscalls: u64,
}

impl ResourceUsage {
    /// Create new usage tracker
    pub fn new() -> Self {
        Self::default()
    }

    /// Record memory allocation
    pub fn record_alloc(&mut self, size: usize) {
        self.memory_used += size;
        self.allocations += 1;
        if self.memory_used > self.memory_peak {
            self.memory_peak = self.memory_used;
        }
    }

    /// Record memory deallocation
    pub fn record_dealloc(&mut self, size: usize) {
        self.memory_used = self.memory_used.saturating_sub(size);
        self.deallocations += 1;
    }

    /// Record CPU time
    pub fn record_cpu(&mut self, secs: u64) {
        self.cpu_time += secs;
        if self.cpu_time > self.cpu_time_peak {
            self.cpu_time_peak = self.cpu_time;
        }
    }

    /// Record syscall
    pub fn record_syscall(&mut self) {
        self.syscalls += 1;
    }

    /// Record file operation
    pub fn record_file_op(&mut self) {
        self.file_ops += 1;
    }

    /// Reset counters
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Get memory usage percentage
    pub fn memory_percent(&self, limit: &MemoryLimit) -> f64 {
        if limit.max_total == 0 {
            return 0.0;
        }
        (self.memory_used as f64 / limit.max_total as f64) * 100.0
    }

    /// Get CPU usage percentage
    pub fn cpu_percent(&self, limit: &CpuLimit) -> f64 {
        if limit.max_time == 0 {
            return 0.0;
        }
        (self.cpu_time as f64 / limit.max_time as f64) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_limit() {
        let limit = MemoryLimit::default();
        assert!(limit.can_allocate(0, 1000));
        assert!(!limit.can_allocate(limit.max_total - 100, 200));
    }

    #[test]
    fn test_cpu_limit() {
        let mut limit = CpuLimit::new(10);
        assert!(!limit.is_exceeded());

        limit.add_time(11);
        assert!(limit.is_exceeded());
    }

    #[test]
    fn test_resource_usage() {
        let mut usage = ResourceUsage::new();

        usage.record_alloc(1000);
        assert_eq!(usage.memory_used, 1000);

        usage.record_dealloc(500);
        assert_eq!(usage.memory_used, 500);

        usage.record_alloc(1000);
        assert_eq!(usage.memory_peak, 1500);
    }

    #[test]
    fn test_resource_limits_presets() {
        let restrictive = ResourceLimits::restrictive();
        assert!(restrictive.memory.max_heap < 128 * 1024 * 1024);

        let permissive = ResourceLimits::permissive();
        assert!(permissive.memory.max_heap > 256 * 1024 * 1024);
    }
}
