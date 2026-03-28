use crate::lockfile::NyxLock;
use crate::package_manager::NyxCargo;
use crate::resolver::Resolver;
use crate::registry::RegistryClient;
use crate::cache::CacheManager;
use std::fs;
use std::path::PathBuf;
use colored::*;

/// Main entry point for `nyx install`.
///
/// Behaviour:
/// - If `path` is provided, installs a binary from that path.
/// - Otherwise, reads `load.surn`, resolves/loads the lockfile, then downloads
///   and verifies every dependency.
/// - `offline = true` means: use only the local cache; never touch the network.
pub fn execute(path: Option<String>, offline: bool) -> Result<(), String> {
    if let Some(bin) = path {
        println!("  {} Installing binary: {}", "→".cyan(), bin);
        // Binary installation is a separate concern; delegate to OS tools.
        return Err(format!(
            "Binary installation from '{}' is not yet supported in this version. \
             Use `nyx add` to declare dependencies.",
            bin
        ));
    }

    println!("  {} Resolving project dependencies…", "→".cyan());

    // 1. Read manifest.
    let surn_content = fs::read_to_string("load.surn")
        .map_err(|e| format!("Could not read load.surn: {}", e))?;
    let manifest = NyxCargo::parse(&surn_content)?;

    // 2. Build registry client (respects offline flag).
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let local_index = PathBuf::from(&home).join(".nyx").join("index");

    let registry = if offline {
        println!("  {} Offline mode — using cached registry only.", "⚡".yellow());
        RegistryClient::new_local(local_index)
    } else {
        // Prefer local cache; fall back to remote.
        RegistryClient::new_remote("https://index.crates.io".to_string())
    };

    // 3. Resolve or load lock.
    let lock_path = PathBuf::from("load.bolt");
    let lock = if lock_path.exists() {
        println!("  {} Lockfile found — using locked versions.", "🔒".green());
        let content = fs::read_to_string(&lock_path).map_err(|e| e.to_string())?;
        NyxLock::parse(&content)?
    } else {
        println!("  {} No lockfile — resolving from manifest…", "🔍".cyan());
        let resolver = Resolver::new(registry.clone());
        let lock = resolver.resolve(&manifest.dependencies)?;
        fs::write(&lock_path, lock.to_string())
            .map_err(|e| format!("Could not write load.bolt: {}", e))?;
        println!("  {} Generated load.bolt", "✓".green());
        lock
    };

    // 4. Acquire every package.
    let cache = CacheManager::new();
    let project_root = PathBuf::from(".");

    for pkg in &lock.packages {
        let source = pkg.source.as_deref().unwrap_or("registry+https://index.crates.io");

        if source.starts_with("git+") {
            let git_url = source.trim_start_matches("git+");
            if cache.get_git_package(&pkg.name, git_url, &None).is_none() {
                if offline {
                    return Err(format!(
                        "Offline mode: git package '{}' is not in the local cache.",
                        pkg.name
                    ));
                }
                cache.cache_git_package(&pkg.name, git_url, &None, &None, &None)?;
                println!("  {} {} (git)", "✓".green(), pkg.name);
            } else {
                println!("  {} {} (git, cached)", "✓".green(), pkg.name);
            }
        } else if source.starts_with("path:") {
            let rel = source.trim_start_matches("path:");
            cache.get_path_package(&project_root, rel)?;
            println!("  {} {} (path)", "✓".green(), pkg.name);
        } else {
            // Registry package.
            if cache.get_package(&pkg.name, &pkg.version).is_none() {
                if offline {
                    return Err(format!(
                        "Offline mode: '{}' v{} is not in the local cache.\n\
                         Run `nyx fetch` while online first.",
                        pkg.name, pkg.version
                    ));
                }
                let url = format!(
                    "https://crates.io/api/v1/crates/{}/{}/download",
                    pkg.name, pkg.version
                );
                cache.download_registry_package(&pkg.name, &pkg.version, &url, &pkg.checksum)?;
                println!("  {} {} v{}", "✓".green(), pkg.name, pkg.version);
            } else {
                println!("  {} {} v{} (cached)", "✓".green(), pkg.name, pkg.version);
            }
        }
    }

    println!("\n  {} All dependencies installed successfully.", "✓".green().bold());
    Ok(())
}
