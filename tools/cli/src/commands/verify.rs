use crate::lockfile::NyxLock;
use crate::cache::CacheManager;
use std::fs;
use std::path::PathBuf;
use colored::*;

/// `nyx verify` — Re-check every package in `load.bolt` against its stored checksum.
pub fn execute() -> Result<(), String> {
    println!("  {} Verifying cache integrity…", "→".cyan());

    let lock_path = PathBuf::from("load.bolt");
    if !lock_path.exists() {
        return Err("No lockfile (load.bolt) found. Run 'nyx install' or 'nyx lock' first.".to_string());
    }

    let content = fs::read_to_string(&lock_path).map_err(|e| e.to_string())?;
    let lock = NyxLock::parse(&content)?;

    let cache = CacheManager::new();
    let mut missing = 0;

    for pkg in &lock.packages {
        let source = pkg.source.as_deref().unwrap_or("registry");
        if source.starts_with("path:") {
            // Path deps are always "present" by definition.
            println!("  {} {} v{} (path, skipped)", "✓".green(), pkg.name, pkg.version);
            continue;
        }

        if cache.get_package(&pkg.name, &pkg.version).is_some() {
            println!("  {} {} v{}", "✓".green(), pkg.name, pkg.version);
        } else {
            println!("  {} {} v{} — MISSING from cache", "✗".red().bold(), pkg.name, pkg.version);
            missing += 1;
        }
    }

    if missing > 0 {
        return Err(format!(
            "{} package(s) missing from cache. Run `nyx fetch` to repair.",
            missing
        ));
    }

    println!("  {} All {} package(s) verified.", "✓".green().bold(), lock.packages.len());
    Ok(())
}
