use std::path::PathBuf;
use std::process::Command;
use crate::package_manager::{NyxCargo, Profile};
use crate::resolver::Resolver;
use crate::lockfile::NyxLock;
use crate::cache::CacheManager;
pub struct Builder {
    project_root: PathBuf,
    surn: NyxCargo,
}

impl Builder {
    pub fn new(project_root: PathBuf, surn: NyxCargo) -> Self {
        Self { project_root, surn }
    }

    pub fn build(&self, release: bool) -> Result<(), String> {
        let cache = CacheManager::new();
        
        // 1. Resolve dependencies if load.bolt is missing or out of date
        let lock_path = self.project_root.join("load.bolt");
        let registry_path = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string())).join(".nyx/registry");
        let registry = crate::registry::RegistryClient::new_local(registry_path);
        
        let lock = if !lock_path.exists() {
            println!("    Resolving dependencies...");
            let resolver = Resolver::new(registry.clone());
            let lock = resolver.resolve(&self.surn.dependencies)?;
            std::fs::write(&lock_path, lock.to_string()).map_err(|e| e.to_string())?;
            lock
        } else {
            let content = std::fs::read_to_string(&lock_path).map_err(|e| e.to_string())?;
            NyxLock::parse(&content)?
        };

        // 2. Ensure all packages are cached and built
        let mut include_paths = Vec::new();
        let mut lib_paths = Vec::new();

        for pkg in &lock.packages {
            let pkg_path = if let Some(path) = cache.get_package(&pkg.name, &pkg.version) {
                path
            } else {
                // Determine source URL (in a real registry, this would be computed from the index)
                let url = format!("https://crates.io/api/v1/crates/{}/{}/download", pkg.name, pkg.version);
                cache.download_registry_package(&pkg.name, &pkg.version, &url, &pkg.checksum)?
            };
            
            include_paths.push(pkg_path.clone());

            // Recursive build: Check if dependency has its own load.surn
            let dep_surn_path = pkg_path.join("load.surn");
            if dep_surn_path.exists() {
                println!("    Building dependency {}...", pkg.name);
                let dep_content = std::fs::read_to_string(&dep_surn_path).map_err(|e| e.to_string())?;
                let dep_surn = NyxCargo::parse(&dep_content)?;
                let dep_builder = Builder::new(pkg_path.clone(), dep_surn);
                dep_builder.build(release)?;
                
                // Collect output lib (convention: target/debug/libname.lib)
                let profile_dir = if release { "release" } else { "debug" };
                let lib_path = pkg_path.join("target").join(profile_dir).join(format!("lib{}.lib", pkg.name));
                if lib_path.exists() {
                    lib_paths.push(lib_path);
                }
            }
        }

        let profile = if release {
            self.surn.profiles.get("release")
        } else {
            self.surn.profiles.get("dev")
        };

        println!("   Compiling {} v{}", self.surn.package.name, self.surn.package.version);

        // 3. Prepare targets
        let mut targets = Vec::new();
        targets.push(("bin".to_string(), self.surn.package.name.clone(), "src/main.nyx".to_string()));

        // 4. Run compilation for each target
        for (kind, name, path) in targets {
            self.compile(&kind, &name, &path, profile, release, &include_paths, &lib_paths)?;
        }

        Ok(())
    }

    fn compile(&self, kind: &str, name: &str, path: &str, profile: Option<&Profile>, release: bool, _includes: &[PathBuf], _libs: &[PathBuf]) -> Result<(), String> {
        let output_dir = if release { "target/release" } else { "target/debug" };
        let output_path = self.project_root.join(output_dir).join(name);

        println!("   Compiling {} {} ({})", kind, name, path);

        let mut cmd = Command::new("nyx");
        cmd.arg("build-internal");
        cmd.arg("--path").arg(path);
        cmd.arg("--output").arg(output_path);
        
        for inc in _includes {
            cmd.arg("--include").arg(inc);
        }

        for lib in _libs {
            cmd.arg("--link").arg(lib);
        }

        if let Some(p) = profile {
            println!("    Optimization: {}", p.opt_level);
        }

        Ok(())
    }
}
