pub mod commands;
pub mod package_manager;
pub mod lockfile;
pub mod resolver;
pub mod builder;
pub mod templates;
pub mod project;
pub mod registry;
pub mod tester;
pub mod docgen;
pub mod verifier;
pub mod cache;
pub mod surn;
pub mod surn_tools;

use std::env;
use std::process::{Command, exit};
use colored::*;

/// The Industrial Dispatcher (Sentinel)
/// Prioritizes pre-compiled binaries and falls back to cargo run in dev environments.
pub fn dispatch_tool(crate_path: &str, bin_name: &str, args: Vec<&str>) {
    let base_dir = env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    
    // 1. Production Path: Pre-compiled binaries (Release then Debug)
    let release_bin = base_dir.join("target/release").join(bin_name);
    let debug_bin = base_dir.join("target/debug").join(bin_name);
    let opt_bin = env::var("NYX_BIN_PATH").map(std::path::PathBuf::from).ok()
        .map(|p| p.join(bin_name));

    let mut cmd = if let Some(p) = opt_bin.filter(|p| p.exists()) {
        Command::new(p)
    } else if release_bin.exists() {
        Command::new(release_bin)
    } else if debug_bin.exists() {
        Command::new(debug_bin)
    } else {
        // 2. Development Path: Cargo run fallback
        if base_dir.join("tools").exists() || base_dir.join("Cargo.toml").exists() {
            let mut c = Command::new("cargo");
            c.args(["run", "--quiet", "--manifest-path", crate_path, "--"]);
            c
        } else {
            eprintln!("{} Tool '{}' not found in path or repository.", "Error:".red().bold(), bin_name.yellow());
            eprintln!("{} Try running from the Nyx repository root or compile with 'cargo build --release'.", "Suggestion:".cyan());
            exit(1);
        }
    };

    cmd.args(args);
    
    match cmd.status() {
        Ok(status) => exit(status.code().unwrap_or(1)),
        Err(e) => {
            eprintln!("{} Failed to execute {}: {}", "Critical:".red().bold(), bin_name, e);
            exit(1);
        }
    }
}
