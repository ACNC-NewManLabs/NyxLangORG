//! Web Preview CLI Transport Adapter
//!
//! This module provides the CLI interface for web preview and build commands.
//! All runtime logic has been moved to the runtime host modules.

pub use crate::runtime::host::web_host::{WebBuildOptions, WebDevOptions};

/// Start the web development server
pub fn dev(opts: WebDevOptions) -> Result<(), String> {
    crate::runtime::host::web_host::dev(opts)
}

/// Build for web deployment
pub fn build(opts: WebBuildOptions) -> Result<(), String> {
    crate::runtime::host::web_host::build(opts)
}

/// Serve a pre-built static directory
pub fn serve(opts: WebDevOptions) -> Result<(), String> {
    crate::runtime::host::web_host::serve(opts)
}
