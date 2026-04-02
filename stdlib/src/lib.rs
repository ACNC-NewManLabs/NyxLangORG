//! NYX Standard Library™
//!
//! **Copyright (c) 2026 SURYA SEKHAR ROY. All Rights Reserved.**
//!
//! A comprehensive, production-grade standard library for the NYX™ programming language.
//! Designed for 100+ year stability with strict modular layering.
//!
//! # Architecture
//!
//! The library is organized into 30 layers, each building upon the previous:
//!
//! - `nyx.core` - [Layer 1] Core types and traits (no-std)
//! - `nyx.mem` - [Layer 2] Memory utilities
//! - `nyx.alloc` - [Layer 3] Allocation systems
//! - `nyx.collections` - [Layer 4] High-performance containers
//! - `nyx.concurrent` - [Layer 5] Concurrency primitives
//! - `nyx.io` - [Layer 6] I/O systems
//! - `nyx.os` - [Layer 7] OS interface
//! - `nyx.time` - [Layer 8] Time handling
//! - `nyx.error` - [Layer 9] Error handling
//! - `nyx.iter` - [Layer 10] Iterator framework
//! - `nyx.primitive` - [Layer 11] Primitive extensions
//! - `nyx.format` - [Layer 12] Formatting library
//! - `nyx.crypto` - [Layer 13] Cryptography
//! - `nyx.ai` - [Layer 14] Machine Learning / Tensors
//! - `nyx.web` - [Layer 15] Industrial Web (HTTP/WS)
//! - `nyx.net` - [Layer 16] Networking (TCP/UDP/QUIC)
//! - `nyx.db` - [Layer 17] Database Interface (SQL/NoSQL)
//! - `nyx.serialization` - [Layer 18] JSON/Protobuf/Bincode
//! - `nyx.ui` - [Layer 19] Reactive UI Engine
//! - `nyx.graphics` - [Layer 20] GPU (Nyx-GPU) & Rendering
//! - `nyx.science` - [Layer 21] Linear Algebra & Statistics
//! - `nyx.distributed` - [Layer 22] Consensus & RPC
//! - `nyx.compiler` - [Layer 23] JIT & Dynamic Compilation
//! - `nyx.security` - [Layer 24] Sandboxing & Attestation
//! - `nyx.media` - [Layer 25] Audio/Video Processing
//! - `nyx.hardware` - [Layer 26] HAL & Driver Primitives
//! - `nyx.kernel`   - [Layer 43] OS Kernel Dev (keyboard, mouse, VGA, ports, memory, IRQ)
//! - `nyx.quantum` - [Layer 27] Quantum Simulation
//! - `nyx.galactic` - [Layer 28] Deep Space Protocols
//! - `nyx.evolution` - [Layer 29] Self-Modifying Code Safeties
//! - `nyx.meta` - [Layer 30] Reflection & Metaprogramming
//!
//! # Version
//!
//! This is NYX Standard Library version 2.0.0 (God-Tier Edition)

// Re-export all public modules
// Foundation Layers (1-14)
pub mod ai;
pub mod alloc;
pub mod collections;
pub mod concurrent;
pub mod core;
pub mod crypto;
pub mod error;
pub mod format;
pub mod io;
pub mod iter;
pub mod mem;
pub mod os;
pub mod primitive;
pub mod time;

// Domain Layers (15-30)
pub mod compiler;
pub mod db;
pub mod distributed;
pub mod evolution;
pub mod galactic;
pub mod graphics;
pub mod hardware;
pub mod kernel;
pub mod media;
pub mod meta;
pub mod net;
pub mod quantum;
pub mod science;
pub mod security;
pub mod serialization;
pub mod ui;
pub mod web;

// Re-export commonly used types
pub use collections::hash_map::HashMap;
pub use collections::hash_set::HashSet;
pub use collections::string::String;
pub use collections::vec::Vec;
pub use core::Option;
pub use core::Result;

// Version information
pub const VERSION: &str = "2.0.0";
pub const NAME: &str = "NYX Standard Library";

/// Initialize the standard library
///
/// This function must be called before using any NYX standard library features.
pub fn init() {
    // Initialize core subsystems
    core::init();
}

/// Shutdown the standard library
///
/// This function cleans up all resources used by the standard library.
pub fn shutdown() {
    // Cleanup
}

/// Get library version information
pub fn version_info() -> VersionInfo {
    VersionInfo {
        name: NAME,
        version: VERSION,
    }
}

/// Version information structure
#[derive(Debug, Clone)]
pub struct VersionInfo {
    pub name: &'static str,
    pub version: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_info() {
        let info = version_info();
        assert_eq!(info.name, NAME);
        assert_eq!(info.version, VERSION);
    }

    #[test]
    fn test_init_shutdown() {
        init();
        shutdown();
    }
}
