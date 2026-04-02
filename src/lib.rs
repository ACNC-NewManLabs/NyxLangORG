//! Nyx Universal Core Architecture™
//!
//! **Copyright (c) 2026 SURYA SEKHAR ROY. All Rights Reserved.**
//!
//! Layered architecture:
//! - core: Domain-agnostic compiler core (lexer, parser, ast, sema)
//! - runtime: Virtual execution, sandbox, concurrency
//! - systems: Low-level IR, LLVM backends, hardware interfaces
//! - extensions: Plugins and domain-specific features
//! - applications: CLI, UI Runner, overall tools

pub mod accessibility;
pub mod applications;
pub mod core;
pub mod devtools;
pub mod extensions;
pub mod graphics;
pub mod runtime;
pub mod systems;
