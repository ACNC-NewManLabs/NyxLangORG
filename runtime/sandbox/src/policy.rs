//! Sandbox Policy
//!
//! This module defines the security policies for sandbox execution.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Sandbox policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxPolicy {
    /// Policy name
    pub name: String,
    /// Memory limits
    pub memory: MemoryPolicy,
    /// CPU limits
    pub cpu: CpuPolicy,
    /// Filesystem policy
    pub filesystem: FilesystemPolicy,
    /// Network policy
    pub network: NetworkPolicy,
    /// Syscall restrictions
    pub syscalls: SyscallPolicy,
    /// Environment variables
    pub environment: EnvironmentPolicy,
    /// Time limits
    pub time: TimePolicy,
}

impl Default for SandboxPolicy {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            memory: MemoryPolicy::default(),
            cpu: CpuPolicy::default(),
            filesystem: FilesystemPolicy::default(),
            network: NetworkPolicy::default(),
            syscalls: SyscallPolicy::default(),
            environment: EnvironmentPolicy::default(),
            time: TimePolicy::default(),
        }
    }
}

/// Memory policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPolicy {
    /// Maximum heap size in bytes
    pub max_heap: u64,
    /// Maximum stack size in bytes
    pub max_stack: u64,
    /// Maximum total memory in bytes
    pub max_total: u64,
    /// Enable memory protection
    pub enable_protection: bool,
}

impl Default for MemoryPolicy {
    fn default() -> Self {
        Self {
            max_heap: 128 * 1024 * 1024,  // 128 MB
            max_stack: 8 * 1024 * 1024,   // 8 MB
            max_total: 256 * 1024 * 1024, // 256 MB
            enable_protection: true,
        }
    }
}

/// CPU policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuPolicy {
    /// Maximum CPU time in seconds
    pub max_cpu_time: u64,
    /// Number of CPU cores allowed
    pub max_cores: u32,
    /// CPU affinity (if supported)
    pub affinity: Option<Vec<u32>>,
    /// Enable CPU protection
    pub enable_protection: bool,
}

impl Default for CpuPolicy {
    fn default() -> Self {
        Self {
            max_cpu_time: 30, // 30 seconds
            max_cores: 4,
            affinity: None,
            enable_protection: true,
        }
    }
}

/// Filesystem policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemPolicy {
    /// Access mode
    pub mode: FilesystemMode,
    /// Allowed directories (for whitelist mode)
    pub allowed_dirs: Vec<String>,
    /// Denied directories (for blacklist mode)
    pub denied_dirs: Vec<String>,
    /// Maximum file size
    pub max_file_size: u64,
    /// Allow file creation
    pub allow_create: bool,
    /// Allow file deletion
    pub allow_delete: bool,
    /// Read-only directories
    pub read_only: Vec<String>,
}

impl Default for FilesystemPolicy {
    fn default() -> Self {
        Self {
            mode: FilesystemMode::ReadOnly,
            allowed_dirs: vec!["/tmp".to_string()],
            denied_dirs: vec![],
            max_file_size: 10 * 1024 * 1024, // 10 MB
            allow_create: false,
            allow_delete: false,
            read_only: vec!["/".to_string()],
        }
    }
}

/// Filesystem access mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilesystemMode {
    /// No filesystem access
    None,
    /// Read-only access
    ReadOnly,
    /// Whitelist mode (only allowed paths)
    Whitelist,
    /// Blacklist mode (denied paths)
    Blacklist,
    /// Full access (for trusted code)
    Full,
}

/// Network policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPolicy {
    /// Network access mode
    pub mode: NetworkMode,
    /// Allowed ports
    pub allowed_ports: Vec<u16>,
    /// Denied ports
    pub denied_ports: Vec<u16>,
    /// Allowed addresses (for whitelist mode)
    pub allowed_addresses: Vec<String>,
    /// Maximum connections
    pub max_connections: u32,
    /// Bandwidth limit (bytes per second)
    pub bandwidth_limit: u64,
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        Self {
            mode: NetworkMode::None,
            allowed_ports: vec![],
            denied_ports: vec![],
            allowed_addresses: vec![],
            max_connections: 0,
            bandwidth_limit: 0,
        }
    }
}

/// Network access mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkMode {
    /// No network access
    None,
    /// Localhost only
    Localhost,
    /// Whitelist mode
    Whitelist,
    /// Full access (for trusted code)
    Full,
}

/// Syscall policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyscallPolicy {
    /// Syscall filtering mode
    pub mode: SyscallMode,
    /// Allowed syscalls (for whitelist mode)
    pub allowed_syscalls: HashSet<String>,
    /// Denied syscalls (for blacklist mode)
    pub denied_syscalls: HashSet<String>,
    /// Enable seccomp
    pub enable_seccomp: bool,
}

impl Default for SyscallPolicy {
    fn default() -> Self {
        let mut allowed = HashSet::new();
        // Basic read/write/exit syscalls
        allowed.insert("read".to_string());
        allowed.insert("write".to_string());
        allowed.insert("exit".to_string());
        allowed.insert("brk".to_string());
        allowed.insert("mmap".to_string());
        allowed.insert("mprotect".to_string());
        allowed.insert("munmap".to_string());

        Self {
            mode: SyscallMode::Whitelist,
            allowed_syscalls: allowed,
            denied_syscalls: HashSet::new(),
            enable_seccomp: true,
        }
    }
}

/// Syscall filtering mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyscallMode {
    /// No filtering
    None,
    /// Whitelist mode (only allowed syscalls)
    Whitelist,
    /// Blacklist mode (denied syscalls)
    Blacklist,
}

/// Environment policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentPolicy {
    /// Clear all environment variables
    pub clear_all: bool,
    /// Allowed variables
    pub allowed_vars: HashSet<String>,
    /// Denied variables
    pub denied_vars: HashSet<String>,
    /// Set specific variables
    pub set_vars: HashSet<(String, String)>,
}

impl Default for EnvironmentPolicy {
    fn default() -> Self {
        Self {
            clear_all: true,
            allowed_vars: HashSet::new(),
            denied_vars: HashSet::new(),
            set_vars: HashSet::new(),
        }
    }
}

/// Time policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimePolicy {
    /// Maximum execution time in seconds
    pub max_execution_time: u64,
    /// Maximum idle time in seconds
    pub max_idle_time: u64,
    /// Enable timeout
    pub enable_timeout: bool,
}

impl Default for TimePolicy {
    fn default() -> Self {
        Self {
            max_execution_time: 30,
            max_idle_time: 10,
            enable_timeout: true,
        }
    }
}

/// Policy configuration builder
pub struct PolicyBuilder {
    policy: SandboxPolicy,
}

impl PolicyBuilder {
    /// Create new builder
    pub fn new() -> Self {
        Self {
            policy: SandboxPolicy::default(),
        }
    }

    /// Set policy name
    pub fn name(mut self, name: &str) -> Self {
        self.policy.name = name.to_string();
        self
    }

    /// Set memory limit
    pub fn memory_limit(mut self, max_heap: u64, max_stack: u64, max_total: u64) -> Self {
        self.policy.memory.max_heap = max_heap;
        self.policy.memory.max_stack = max_stack;
        self.policy.memory.max_total = max_total;
        self
    }

    /// Set CPU limit
    pub fn cpu_limit(mut self, max_time: u64, max_cores: u32) -> Self {
        self.policy.cpu.max_cpu_time = max_time;
        self.policy.cpu.max_cores = max_cores;
        self
    }

    /// Set filesystem mode
    pub fn filesystem(mut self, mode: FilesystemMode) -> Self {
        self.policy.filesystem.mode = mode;
        self
    }

    /// Allow directory
    pub fn allow_dir(mut self, dir: &str) -> Self {
        self.policy.filesystem.allowed_dirs.push(dir.to_string());
        self
    }

    /// Set network mode
    pub fn network(mut self, mode: NetworkMode) -> Self {
        self.policy.network.mode = mode;
        self
    }

    /// Allow port
    pub fn allow_port(mut self, port: u16) -> Self {
        self.policy.network.allowed_ports.push(port);
        self
    }

    /// Set execution time limit
    pub fn time_limit(mut self, seconds: u64) -> Self {
        self.policy.time.max_execution_time = seconds;
        self
    }

    /// Build the policy
    pub fn build(self) -> SandboxPolicy {
        self.policy
    }
}

impl Default for PolicyBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Policy configuration wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfig {
    /// Policy name
    pub name: String,
    /// JSON policy content
    pub policy: SandboxPolicy,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_policy() {
        let policy = SandboxPolicy::default();
        assert_eq!(policy.name, "default");
    }

    #[test]
    fn test_policy_builder() {
        let policy = PolicyBuilder::new()
            .name("test")
            .memory_limit(64 * 1024 * 1024, 4 * 1024 * 1024, 128 * 1024 * 1024)
            .cpu_limit(60, 2)
            .filesystem(FilesystemMode::ReadOnly)
            .network(NetworkMode::None)
            .time_limit(120)
            .build();

        assert_eq!(policy.name, "test");
        assert_eq!(policy.memory.max_heap, 64 * 1024 * 1024);
        assert_eq!(policy.cpu.max_cpu_time, 60);
    }
}
