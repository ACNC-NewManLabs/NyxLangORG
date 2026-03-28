//! Sandbox Manager
//! 
//! This module manages sandbox execution.

use std::collections::HashMap;
// use std::path::Path;

use crate::limits::{CpuLimit, MemoryLimit, ResourceLimits, ResourceUsage};
use crate::policy::{PolicyBuilder, SandboxPolicy};
use crate::monitor::ResourceMonitor;
use crate::{SandboxError, SandboxResult};

#[derive(Debug, Clone)]
pub struct SandboxContext {
    /// Sandbox ID
    pub id: String,
    /// Policy
    pub policy: SandboxPolicy,
    /// Resource limits
    pub limits: ResourceLimits,
    /// Resource usage
    pub usage: ResourceUsage,
    /// Monitor
    pub monitor: ResourceMonitor,
    /// Active flag
    pub active: bool,
}

impl SandboxContext {
    /// Create new context
    pub fn new(id: String, policy: SandboxPolicy) -> Self {
        let limits = ResourceLimits {
            memory: MemoryLimit {
                max_heap: policy.memory.max_heap as usize,
                max_stack: policy.memory.max_stack as usize,
                max_total: policy.memory.max_total as usize,
                page_size: 4096,
            },
            cpu: CpuLimit::new(policy.cpu.max_cpu_time),
            max_file_size: policy.filesystem.max_file_size as usize,
            max_open_files: 64,
            max_processes: 1,
        };

        Self {
            id,
            policy,
            limits,
            usage: ResourceUsage::new(),
            monitor: ResourceMonitor::new(),
            active: false,
        }
    }

    /// Check if resource limits exceeded
    pub fn check_limits(&self) -> SandboxResult<()> {
        // Check memory
        if self.usage.memory_used > self.limits.memory.max_total {
            return Err(SandboxError::ResourceLimitExceeded("Memory limit exceeded".to_string()));
        }

        // Check CPU time
        if self.limits.cpu.is_exceeded() {
            return Err(SandboxError::ResourceLimitExceeded("CPU time limit exceeded".to_string()));
        }

        // Check file size
        if self.limits.max_file_size > 0 && self.usage.file_ops > self.limits.max_file_size as u64 {
            return Err(SandboxError::ResourceLimitExceeded("File size limit exceeded".to_string()));
        }

        Ok(())
    }

    /// Record allocation
    pub fn record_alloc(&mut self, size: usize) -> SandboxResult<()> {
        if !self.limits.memory.can_allocate(self.usage.memory_used, size) {
            return Err(SandboxError::ResourceLimitExceeded("Memory allocation failed".to_string()));
        }

        self.usage.record_alloc(size);
        self.check_limits()
    }

    /// Record deallocation
    pub fn record_dealloc(&mut self, size: usize) {
        self.usage.record_dealloc(size);
    }

    /// Start execution
    pub fn start(&mut self) {
        self.active = true;
        self.limits.cpu.start();
        self.monitor.start();
    }

    /// Stop execution
    pub fn stop(&mut self) {
        self.active = false;
        self.limits.cpu.stop();
        self.monitor.stop();
    }
}

/// Sandbox manager
pub struct SandboxManager {
    /// Active sandboxes
    contexts: HashMap<String, SandboxContext>,
    /// Default policy
    default_policy: SandboxPolicy,
}

impl SandboxManager {
    /// Create new manager
    pub fn new() -> Self {
        Self {
            contexts: HashMap::new(),
            default_policy: SandboxPolicy::default(),
        }
    }

    /// Create with default policy
    pub fn with_policy(policy: SandboxPolicy) -> Self {
        Self {
            contexts: HashMap::new(),
            default_policy: policy,
        }
    }

    /// Create sandbox context
    pub fn create(&mut self, id: &str) -> SandboxResult<&mut SandboxContext> {
        let policy = self.default_policy.clone();
        let context = SandboxContext::new(id.to_string(), policy);
        
        self.contexts.insert(id.to_string(), context);
        
        self.contexts
            .get_mut(id)
            .ok_or_else(|| SandboxError::SystemError("Failed to create sandbox".to_string()))
    }

    /// Create with custom policy
    pub fn create_with_policy(&mut self, id: &str, policy: SandboxPolicy) -> SandboxResult<&mut SandboxContext> {
        let context = SandboxContext::new(id.to_string(), policy);
        
        self.contexts.insert(id.to_string(), context);
        
        self.contexts
            .get_mut(id)
            .ok_or_else(|| SandboxError::SystemError("Failed to create sandbox".to_string()))
    }

    /// Get sandbox context
    pub fn get(&self, id: &str) -> Option<&SandboxContext> {
        self.contexts.get(id)
    }

    /// Get mutable sandbox context
    pub fn get_mut(&mut self, id: &str) -> Option<&mut SandboxContext> {
        self.contexts.get_mut(id)
    }

    /// Destroy sandbox
    pub fn destroy(&mut self, id: &str) -> SandboxResult<()> {
        if let Some(ctx) = self.contexts.get_mut(id) {
            ctx.stop();
        }
        
        self.contexts
            .remove(id)
            .map(|_| ())
            .ok_or_else(|| SandboxError::SystemError("Sandbox not found".to_string()))
    }

    /// Set default policy
    pub fn set_default_policy(&mut self, policy: SandboxPolicy) {
        self.default_policy = policy;
    }

    /// List active sandboxes
    pub fn list(&self) -> Vec<String> {
        self.contexts.keys().cloned().collect()
    }

    /// Check all active sandboxes
    pub fn check_all(&self) -> SandboxResult<()> {
        for (id, ctx) in &self.contexts {
            if ctx.active {
                ctx.check_limits()
                    .map_err(|e| SandboxError::ResourceLimitExceeded(format!("Sandbox {}: {}", id, e)))?;
            }
        }
        Ok(())
    }
}

impl Default for SandboxManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating sandboxes
pub struct SandboxBuilder {
    manager: SandboxManager,
    policy_builder: PolicyBuilder,
    id: String,
}

impl SandboxBuilder {
    /// Create new builder
    pub fn new(id: &str) -> Self {
        Self {
            manager: SandboxManager::new(),
            policy_builder: PolicyBuilder::new(),
            id: id.to_string(),
        }
    }

    /// Set memory limit
    pub fn memory(mut self, max_heap: u64, max_stack: u64, max_total: u64) -> Self {
        self.policy_builder = self.policy_builder.memory_limit(max_heap, max_stack, max_total);
        self
    }

    /// Set CPU time limit
    pub fn cpu_time(mut self, seconds: u64) -> Self {
        self.policy_builder = self.policy_builder.cpu_limit(seconds, 1);
        self
    }

    /// Allow directory
    pub fn allow_dir(mut self, dir: &str) -> Self {
        self.policy_builder = self.policy_builder.allow_dir(dir);
        self
    }

    /// Build and create sandbox
    pub fn create(self) -> SandboxResult<SandboxContext> {
        let policy = self.policy_builder.build();
        let mut manager = self.manager;
        
        let context = manager.create_with_policy(&self.id, policy)?;
        Ok(context.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_manager() {
        let mut manager = SandboxManager::new();
        
        let ctx = manager.create("test").unwrap();
        assert_eq!(ctx.id, "test");
    }

    #[test]
    fn test_sandbox_context() {
        let policy = SandboxPolicy::default();
        let mut ctx = SandboxContext::new("test".to_string(), policy);
        
        ctx.start();
        assert!(ctx.active);
        
        ctx.record_alloc(1000).unwrap();
        assert_eq!(ctx.usage.memory_used, 1000);
        
        ctx.record_dealloc(500);
        assert_eq!(ctx.usage.memory_used, 500);
        
        ctx.stop();
        assert!(!ctx.active);
    }

    #[test]
    fn test_sandbox_builder() {
        let ctx = SandboxBuilder::new("test")
            .memory(64 * 1024 * 1024, 4 * 1024 * 1024, 128 * 1024 * 1024)
            .cpu_time(60)
            .create()
            .unwrap();
        
        assert_eq!(ctx.id, "test");
    }
}

