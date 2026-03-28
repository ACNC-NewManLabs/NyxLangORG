use crate::lockfile::NyxLock;
use crate::cache::CacheManager;
use std::fs;
use std::path::PathBuf;
use colored::*;

/// `nyx fetch` — Download everything in `load.bolt` without building.
pub fn execute() -> Result<(), String> {
    println!("  {} Fetching dependencies from lockfile…", "→".cyan());

    let lock_path = PathBuf::from("load.bolt");
    if !lock_path.exists() {
        return Err("No lockfile (load.bolt) found. Run 'nyx install' or 'nyx lock' first.".to_string());
    }

    let content = fs::read_to_string(&lock_path).map_err(|e| e.to_string())?;
    let lock = NyxLock::parse(&content)?;

    let cache = CacheManager::new();
    let project_root = PathBuf::from(".");
    let mut fetched = 0;

    for pkg in &lock.packages {
        let source = pkg.source.as_deref().unwrap_or("registry+https://index.crates.io");

        if source.starts_with("git+") {
            let git_url = source.trim_start_matches("git+");
            if cache.get_git_package(&pkg.name, git_url, &None).is_none() {
                cache.cache_git_package(&pkg.name, git_url, &None, &None, &None)?;
                println!("  {} {} (git)", "✓".green(), pkg.name);
                fetched += 1;
            } else {
                println!("  {} {} (git, already cached)", "·".dimmed(), pkg.name);
            }
        } else if source.starts_with("path:") {
            let rel = source.trim_start_matches("path:");
            cache.get_path_package(&project_root, rel)?;
            println!("  {} {} (path)", "·".dimmed(), pkg.name);
        } else {
            if cache.get_package(&pkg.name, &pkg.version).is_none() {
                let url = format!(
                    "https://crates.io/api/v1/crates/{}/{}/download",
                    pkg.name, pkg.version
                );
                cache.download_registry_package(&pkg.name, &pkg.version, &url, &pkg.checksum)?;
                println!("  {} {} v{}", "✓".green(), pkg.name, pkg.version);
                fetched += 1;
            } else {
                println!("  {} {} v{} (already cached)", "·".dimmed(), pkg.name, pkg.version);
            }
        }
    }

    if fetched == 0 {
        println!("  {} All packages already in cache.", "✓".green());
    } else {
        println!("\n  {} Fetched {} package(s).", "✓".green().bold(), fetched);
    }
    Ok(())
}
