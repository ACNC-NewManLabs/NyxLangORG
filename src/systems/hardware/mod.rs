//! Nyx Systems Programming Module
//!
//! This module provides low-level systems programming capabilities for Nyx,
//! including unsafe operations, raw pointers, inline assembly, and bare-metal support.

pub mod bare_metal;
pub mod inline_asm;
pub mod io;
pub mod memory;
pub mod pointers;
pub mod unsafe_ops;

pub use crate::asm;
pub use bare_metal::KernelInterface;
pub use io::{IoPort, Port};
pub use memory::{Allocator, MemoryRegion};
pub use pointers::{Ptr, PtrMut};
pub use unsafe_ops::{unsafe_block, UnsafeCell};

/// Systems programming configuration
pub struct SystemsConfig {
    /// Enable unsafe operations
    pub allow_unsafe: bool,
    /// Enable inline assembly
    pub allow_asm: bool,
    /// Enable bare-metal mode
    pub is_bare_metal: bool,
    /// Page size for memory operations
    pub page_size: usize,
}

impl Default for SystemsConfig {
    fn default() -> Self {
        Self {
            allow_unsafe: true,
            allow_asm: true,
            is_bare_metal: false,
            page_size: 4096,
        }
    }
}

/// Initialize systems programming subsystem
pub fn init() -> Result<(), SystemsError> {
    log::info!("Nyx Systems Module initialized");
    Ok(())
}

/// Systems programming errors
#[derive(Debug)]
pub enum SystemsError {
    UnsafeOperation(String),
    InvalidPointer(String),
    AllocationFailed(String),
    InvalidAssembly(String),
    IoError(String),
}

impl std::fmt::Display for SystemsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SystemsError::UnsafeOperation(msg) => write!(f, "Unsafe operation: {}", msg),
            SystemsError::InvalidPointer(msg) => write!(f, "Invalid pointer: {}", msg),
            SystemsError::AllocationFailed(msg) => write!(f, "Allocation failed: {}", msg),
            SystemsError::InvalidAssembly(msg) => write!(f, "Invalid assembly: {}", msg),
            SystemsError::IoError(msg) => write!(f, "I/O error: {}", msg),
        }
    }
}

impl std::error::Error for SystemsError {}

pub type SystemsResult<T> = Result<T, SystemsError>;
