use semver::{Version, VersionReq};
use std::collections::HashMap;
use crate::package_manager::Dependency;
use crate::registry::{RegistryClient, IndexEntry};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A single package that was selected during resolution.
#[derive(Debug, Clone)]
pub struct ResolvedPackage {
    pub name: String,
    pub version: Version,
    pub checksum: String,
    pub dependencies: Vec<String>, // "name version" strings
    pub source: String,
}

// ---------------------------------------------------------------------------
// Resolver
// ---------------------------------------------------------------------------

pub struct Resolver {
    registry: RegistryClient,
}

impl Resolver {
    pub fn new(registry: RegistryClient) -> Self {
        Self { registry }
    }

    /// Resolve a full dependency graph and return a deterministic NyxLock.
    pub fn resolve(
        &self,
        root_deps: &HashMap<String, Dependency>,
    ) -> Result<crate::lockfile::NyxLock, String> {
        let mut selections: HashMap<String, ResolvedPackage> = HashMap::new();

        // Build a sorted worklist so resolution order is deterministic.
        let mut worklist: Vec<(String, String)> = root_deps
            .iter()
            .filter_map(|(name, dep)| {
                // Skip optional deps at root unless they are explicitly requested.
                if let Dependency::Detailed(d) = dep {
                    if d.optional.unwrap_or(false) {
                        return None;
                    }
                    // Skip registry lookups for git and path dependencies
                    if d.git.is_some() || d.path.is_some() {
                        return None;
                    }
                }
                let req = Self::dep_version_req(dep);
                Some((name.clone(), req))
            })
            .collect();
        worklist.sort_by(|a, b| a.0.cmp(&b.0));

        // Resolve the full worklist; all git/path deps receive a synthetic entry.
        for (name, req_str) in &worklist {
            self.resolve_one(name, req_str, &mut selections)?;
        }

        // Build the lock, adding git/path deps that were in the worklist but
        // not resolved via registry.
        for (name, dep) in root_deps {
            if let Dependency::Detailed(d) = dep {
                if d.git.is_some() || d.path.is_some() {
                    if !selections.contains_key(name) {
                        let source = if let Some(g) = &d.git {
                            format!("git+{}", g)
                        } else if let Some(p) = &d.path {
                            format!("path:{}", p)
                        } else {
                            "unknown".to_string()
                        };
                        selections.insert(name.clone(), ResolvedPackage {
                            name: name.clone(),
                            version: Version::parse("0.0.0").unwrap(),
                            checksum: String::new(),
                            dependencies: vec![],
                            source,
                        });
                    }
                }
            }
        }

        // Sort packages alphabetically for deterministic lockfile output.
        let mut packages: Vec<crate::lockfile::LockedPackage> = selections
            .into_iter()
            .map(|(_, pkg)| crate::lockfile::LockedPackage {
                name: pkg.name,
                version: pkg.version.to_string(),
                source: Some(pkg.source),
                checksum: pkg.checksum,
                dependencies: pkg.dependencies,
            })
            .collect();
        packages.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(crate::lockfile::NyxLock { version: 1, packages })
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Resolve a single package by name and semver requirement string,
    /// recursively pulling in transitive dependencies.
    fn resolve_one(
        &self,
        name: &str,
        req_str: &str,
        selections: &mut HashMap<String, ResolvedPackage>,
    ) -> Result<(), String> {
        // Parse the version requirement, treating bare "X.Y" as "^X.Y."
        let req_str_normalised = Self::normalise_req(req_str);
        let req = VersionReq::parse(&req_str_normalised)
            .map_err(|e| format!("Invalid version requirement for '{}': {} (req: '{}')", name, e, req_str_normalised))?;

        // If already selected, verify compatibility.
        if let Some(existing) = selections.get(name) {
            if !req.matches(&existing.version) {
                return Err(format!(
                    "Version conflict for '{}': already selected {} but also need {}",
                    name, existing.version, req_str_normalised
                ));
            }
            // Already satisfied – skip.
            return Ok(());
        }

        // Fetch candidate versions from registry index.
        let candidates = self.registry.get_versions(name)
            .map_err(|e| format!("Registry error for '{}': {}", name, e))?;

        // Filter to compatible versions and sort descending (prefer latest).
        let mut compatible: Vec<(Version, IndexEntry)> = candidates
            .into_iter()
            .filter_map(|entry| {
                let v = Version::parse(&entry.vers).ok()?;
                if req.matches(&v) { Some((v, entry)) } else { None }
            })
            .collect();
        compatible.sort_by(|(a, _), (b, _)| b.cmp(a));

        if compatible.is_empty() {
            return Err(format!(
                "No version of '{}' matches requirement '{}'",
                name, req_str_normalised
            ));
        }

        // CDCL-style backtracking: try each candidate, newest first.
        let mut last_err = String::new();
        for (version, entry) in &compatible {
            // Tentatively select this version.
            let dep_names: Vec<String> = entry.deps.iter()
                .filter(|d| !d.optional)
                .map(|d| format!("{} {}", d.name, d.req))
                .collect();

            let resolved = ResolvedPackage {
                name: name.to_string(),
                version: version.clone(),
                checksum: entry.cksum.clone(),
                dependencies: dep_names,
                source: "registry+https://index.crates.io".to_string(),
            };

            // Snapshot current selections before tentative insertion.
            let snapshot = selections.clone();
            selections.insert(name.to_string(), resolved);

            // Attempt to satisfy transitive deps (sorted for determinism).
            let mut sub_deps: Vec<(&str, &str)> = entry.deps.iter()
                .filter(|d| !d.optional)
                .map(|d| (d.name.as_str(), d.req.as_str()))
                .collect();
            sub_deps.sort_by_key(|(n, _)| *n);

            let mut conflict = false;
            for (sub_name, sub_req) in sub_deps {
                if let Err(e) = self.resolve_one(sub_name, sub_req, selections) {
                    last_err = e;
                    conflict = true;
                    break;
                }
            }

            if !conflict {
                return Ok(());
            }

            // Backtrack: restore snapshot and try next candidate.
            *selections = snapshot;
        }

        Err(if last_err.is_empty() {
            format!("Could not resolve '{}' with requirement '{}'", name, req_str_normalised)
        } else {
            format!("Could not resolve '{}': {}", name, last_err)
        })
    }

    /// Return a normalised version requirement string suitable for semver parsing.
    fn normalise_req(req: &str) -> String {
        let req = req.trim();
        // Already has a comparator prefix – use as-is.
        if req.starts_with('^') || req.starts_with('~') || req.starts_with('>')
            || req.starts_with('<') || req.starts_with('=') || req == "*" {
            return req.to_string();
        }
        // Bare version string like "1.0" or "1" – treat as caret (compatible).
        format!("^{}", req)
    }

    /// Extract the version requirement string from a Dependency enum.
    fn dep_version_req(dep: &Dependency) -> String {
        match dep {
            Dependency::Simple(v) => v.clone(),
            Dependency::Detailed(d) => d.version.clone().unwrap_or_else(|| "*".to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    use crate::registry::{IndexDependency, RegistryClient};

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn write_index(index_dir: &std::path::Path, name: &str, entries: &[IndexEntry]) {
        // Compute shard path using the same logic as RegistryClient.
        let n = name.to_lowercase();
        let shard = match n.len() {
            1 => index_dir.join("1").join(&n),
            2 => index_dir.join("2").join(&n),
            3 => index_dir.join("3").join(&n[0..1]).join(&n),
            _ => index_dir.join(&n[0..2]).join(&n[2..4]).join(&n),
        };
        fs::create_dir_all(shard.parent().unwrap()).unwrap();
        let content: String = entries
            .iter()
            .map(|e| serde_json::to_string(e).unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&shard, content).unwrap();
    }

    fn simple_entry(vers: &str, deps: Vec<(&str, &str)>) -> IndexEntry {
        IndexEntry {
            vers: vers.to_string(),
            deps: deps.into_iter().map(|(n, r)| IndexDependency {
                name: n.to_string(),
                req: r.to_string(),
                features: vec![],
                optional: false,
                target: None,
            }).collect(),
            cksum: format!("sha256:{}", vers.replace('.', "")),
        }
    }

    // -----------------------------------------------------------------------
    // Test: transitive resolution
    // -----------------------------------------------------------------------
    #[test]
    fn test_transitive_resolution() {
        let tmp = tempdir().unwrap();
        let idx = tmp.path().join("index");

        // jit 1.0.0 depends on cranelift 0.110.0
        write_index(&idx, "jit", &[simple_entry("1.0.0", vec![("cranelift", "0.110")])]);
        // cranelift has no deps
        write_index(&idx, "cranelift", &[simple_entry("0.110.0", vec![])]);

        let registry = RegistryClient::new_local(idx);
        let resolver = Resolver::new(registry);

        let mut deps = HashMap::new();
        deps.insert("jit".to_string(), Dependency::Simple("1.0".to_string()));

        let lock = resolver.resolve(&deps).expect("resolution should succeed");

        assert_eq!(lock.packages.len(), 2, "should have jit + cranelift");

        let jit = lock.packages.iter().find(|p| p.name == "jit").unwrap();
        assert_eq!(jit.version, "1.0.0");
        assert_eq!(jit.dependencies, vec!["cranelift 0.110"]);

        let cl = lock.packages.iter().find(|p| p.name == "cranelift").unwrap();
        assert_eq!(cl.version, "0.110.0");
    }

    // -----------------------------------------------------------------------
    // Test: deterministic sort in lockfile
    // -----------------------------------------------------------------------
    #[test]
    fn test_lockfile_deterministic_sort() {
        let tmp = tempdir().unwrap();
        let idx = tmp.path().join("index");

        write_index(&idx, "zebra", &[simple_entry("1.0.0", vec![])]);
        write_index(&idx, "apple", &[simple_entry("2.0.0", vec![])]);
        write_index(&idx, "mango", &[simple_entry("3.0.0", vec![])]);

        let registry = RegistryClient::new_local(idx);
        let resolver = Resolver::new(registry);

        let mut deps = HashMap::new();
        deps.insert("zebra".to_string(), Dependency::Simple("1.0".to_string()));
        deps.insert("apple".to_string(), Dependency::Simple("2.0".to_string()));
        deps.insert("mango".to_string(), Dependency::Simple("3.0".to_string()));

        let lock = resolver.resolve(&deps).expect("resolution should succeed");

        let names: Vec<&str> = lock.packages.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["apple", "mango", "zebra"], "packages must be sorted alphabetically");

        // Verify two serializations are byte-identical.
        let s1 = lock.to_string();
        let s2 = lock.to_string();
        assert_eq!(s1, s2, "lockfile must be deterministic");
    }

    // -----------------------------------------------------------------------
    // Test: version conflict detection
    // -----------------------------------------------------------------------
    #[test]
    fn test_conflict_detection() {
        let tmp = tempdir().unwrap();
        let idx = tmp.path().join("index");

        // pkg-a requires pkg-b ^1.0, but root also requires pkg-b ^2.0 — unresolvable.
        write_index(&idx, "pkg-a", &[simple_entry("1.0.0", vec![("pkg-b", "^1.0")])]);
        write_index(&idx, "pkg-b", &[
            simple_entry("1.0.0", vec![]),
            simple_entry("2.0.0", vec![]),
        ]);

        let registry = RegistryClient::new_local(idx);
        let resolver = Resolver::new(registry);

        let mut deps = HashMap::new();
        deps.insert("pkg-a".to_string(), Dependency::Simple("1.0".to_string()));
        deps.insert("pkg-b".to_string(), Dependency::Simple("^2.0".to_string()));

        let result = resolver.resolve(&deps);
        assert!(result.is_err(), "should fail due to version conflict");
        let err = result.unwrap_err();
        // The error should mention pkg-b and conflict.
        assert!(
            err.contains("pkg-b") || err.contains("conflict"),
            "error message should mention the conflicting package, got: {}", err
        );
    }

    // -----------------------------------------------------------------------
    // Test: multiple compatible versions (picks latest)
    // -----------------------------------------------------------------------
    #[test]
    fn test_picks_latest_compatible() {
        let tmp = tempdir().unwrap();
        let idx = tmp.path().join("index");

        write_index(&idx, "lib", &[
            simple_entry("1.0.0", vec![]),
            simple_entry("1.2.3", vec![]),
            simple_entry("1.9.0", vec![]),
            simple_entry("2.0.0", vec![]), // incompatible with ^1
        ]);

        let registry = RegistryClient::new_local(idx);
        let resolver = Resolver::new(registry);

        let mut deps = HashMap::new();
        deps.insert("lib".to_string(), Dependency::Simple("^1.0".to_string()));

        let lock = resolver.resolve(&deps).expect("should resolve");
        assert_eq!(lock.packages[0].version, "1.9.0", "should pick latest ^1 compatible");
    }

    // -----------------------------------------------------------------------
    // Test: already-compatible transitive dep is reused
    // -----------------------------------------------------------------------
    #[test]
    fn test_shared_dependency_reuse() {
        let tmp = tempdir().unwrap();
        let idx = tmp.path().join("index");

        // Both pkg-a and pkg-b depend on shared ^1.0 — should resolve to one entry.
        write_index(&idx, "pkg-a", &[simple_entry("1.0.0", vec![("shared", "^1.0")])]);
        write_index(&idx, "pkg-b", &[simple_entry("1.0.0", vec![("shared", "^1.0")])]);
        write_index(&idx, "shared", &[simple_entry("1.5.0", vec![])]);

        let registry = RegistryClient::new_local(idx);
        let resolver = Resolver::new(registry);

        let mut deps = HashMap::new();
        deps.insert("pkg-a".to_string(), Dependency::Simple("^1.0".to_string()));
        deps.insert("pkg-b".to_string(), Dependency::Simple("^1.0".to_string()));

        let lock = resolver.resolve(&deps).expect("should resolve");
        // Should have pkg-a, pkg-b, shared — not two copies of shared.
        let shared_count = lock.packages.iter().filter(|p| p.name == "shared").count();
        assert_eq!(shared_count, 1, "shared dep must appear exactly once");
    }
}
