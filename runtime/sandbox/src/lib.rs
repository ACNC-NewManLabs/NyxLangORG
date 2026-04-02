//! Nyx Sandbox Runtime™
//!
//! **Copyright (c) 2026 SURYA SEKHAR ROY. All Rights Reserved.**
//!
//! This module provides a secure execution sandbox with resource limits,
//! syscall filtering, and process isolation.

pub mod limits;
pub mod manager;
pub mod monitor;
pub mod policy;

pub use limits::{CpuLimit, MemoryLimit, ResourceLimits};
pub use manager::SandboxManager;
pub use monitor::ResourceMonitor;
pub use policy::{FilesystemPolicy, NetworkPolicy, PolicyConfig, SandboxPolicy};

/// Sandbox version
pub const SANDBOX_VERSION: &str = "1.0.0";

/// Initialize the sandbox subsystem
pub fn init() -> Result<(), SandboxError> {
    log::info!("Nyx Sandbox v{} initialized", SANDBOX_VERSION);
    Ok(())
}

/// Sandbox error types
#[derive(Debug)]
pub enum SandboxError {
    /// Policy violation
    PolicyViolation(String),
    /// Resource limit exceeded
    ResourceLimitExceeded(String),
    /// System error
    SystemError(String),
    /// Permission denied
    PermissionDenied(String),
    /// Invalid configuration
    InvalidConfig(String),
    /// Not supported on this platform
    NotSupported(String),
}

impl std::fmt::Display for SandboxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SandboxError::PolicyViolation(msg) => write!(f, "Policy violation: {}", msg),
            SandboxError::ResourceLimitExceeded(msg) => {
                write!(f, "Resource limit exceeded: {}", msg)
            }
            SandboxError::SystemError(msg) => write!(f, "System error: {}", msg),
            SandboxError::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            SandboxError::InvalidConfig(msg) => write!(f, "Invalid configuration: {}", msg),
            SandboxError::NotSupported(msg) => write!(f, "Not supported: {}", msg),
        }
    }
}

impl std::error::Error for SandboxError {}

/// Sandbox result type
pub type SandboxResult<T> = Result<T, SandboxError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_init() {
        assert!(init().is_ok());
    }
}
