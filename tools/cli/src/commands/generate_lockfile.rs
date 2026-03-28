use crate::package_manager::NyxCargo;
use crate::resolver::Resolver;
use crate::registry::RegistryClient;
use std::fs;
use std::path::PathBuf;
use colored::*;

/// `nyx lock` — Generate `load.bolt` without downloading any packages.
pub fn execute() -> Result<(), String> {
    println!("  {} Generating load.bolt lockfile…", "→".cyan());

    let surn_content = fs::read_to_string("load.surn")
        .map_err(|e| format!("Could not read load.surn: {}", e))?;
    let manifest = NyxCargo::parse(&surn_content)?;

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let _local_index = PathBuf::from(&home).join(".nyx").join("index");

    let registry = RegistryClient::new_remote("https://index.crates.io".to_string());

    let resolver = Resolver::new(registry);
    let lock = resolver.resolve(&manifest.dependencies)?;

    let lock_str = lock.to_string();
    fs::write("load.bolt", &lock_str)
        .map_err(|e| format!("Could not write load.bolt: {}", e))?;

    println!(
        "  {} load.bolt written ({} package(s)).",
        "✓".green().bold(),
        lock.packages.len()
    );
    Ok(())
}
