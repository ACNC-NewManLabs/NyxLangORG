//! Nyx Hypervisor - Virtual Machine Monitor
//! 
//! This module provides a complete virtualization platform capable of
//! running operating systems inside Nyx without external hypervisors.

pub mod cpu;
pub mod memory;
pub mod devices;
pub mod vm;
pub mod hypercall;

pub use cpu::{CpuEmulator, CpuState, Register, CpuMode};
pub use memory::{VirtualMemory, PageTable, GuestPhysicalAddr, HostVirtualAddr};
pub use devices::{DeviceManager, VirtioDevice, ConsoleDevice, BlockDevice, NetworkDevice};
pub use vm::{VirtualMachine, VmConfig, VmState};
pub use hypercall::{Hypercall, HypercallResult};

/// Maximum number of CPUs per VM
pub const MAX_VM_CPUS: usize = 8;

/// Maximum memory per VM (256GB)
pub const MAX_VM_MEMORY: u64 = 256 * 1024 * 1024 * 1024;

/// Page size for virtualization
pub const VIRT_PAGE_SIZE: usize = 4096;

/// Hypervisor version
pub const HYPERVISOR_VERSION: &str = "1.0.0";

/// Initialize the hypervisor subsystem
pub fn init() -> Result<(), HypervisorError> {
    log::info!("Nyx Hypervisor v{} initialized", HYPERVISOR_VERSION);
    Ok(())
}

/// Hypervisor errors
#[derive(Debug)]
pub enum HypervisorError {
    VmCreationFailed(String),
    CpuInitFailed(String),
    MemoryAllocationFailed(String),
    DeviceError(String),
    InvalidConfig(String),
    EmulationError(String),
    UnsupportedFeature(String),
}

impl std::fmt::Display for HypervisorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HypervisorError::VmCreationFailed(msg) => write!(f, "VM creation failed: {}", msg),
            HypervisorError::CpuInitFailed(msg) => write!(f, "CPU init failed: {}", msg),
            HypervisorError::MemoryAllocationFailed(msg) => write!(f, "Memory allocation failed: {}", msg),
            HypervisorError::DeviceError(msg) => write!(f, "Device error: {}", msg),
            HypervisorError::InvalidConfig(msg) => write!(f, "Invalid config: {}", msg),
            HypervisorError::EmulationError(msg) => write!(f, "Emulation error: {}", msg),
            HypervisorError::UnsupportedFeature(msg) => write!(f, "Unsupported feature: {}", msg),
        }
    }
}

impl std::error::Error for HypervisorError {}

pub type HypervisorResult<T> = Result<T, HypervisorError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hypervisor_init() {
        assert!(init().is_ok());
    }
}

