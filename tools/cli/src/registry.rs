use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use semver::Version;
use crate::cache::CacheManager;

// ---------------------------------------------------------------------------
// Index types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IndexEntry {
    pub vers: String,
    pub deps: Vec<IndexDependency>,
    pub cksum: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IndexDependency {
    pub name: String,
    pub req: String,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub optional: bool,
    pub target: Option<String>,
}

// ---------------------------------------------------------------------------
// Registry client
// ---------------------------------------------------------------------------

/// A client that can read package metadata from either a local index directory
/// or a remote HTTP registry, with automatic local caching.
#[derive(Clone)]
pub struct RegistryClient {
    index_path: Option<PathBuf>,   // local-only registry (e.g. in tests)
    remote_url: Option<String>,    // remote registry with local index cache
    offline: bool,                 // refuse all network access
}

impl RegistryClient {
    // -----------------------------------------------------------------------
    // Constructors
    // -----------------------------------------------------------------------

    /// Local-only registry (no network access; used in tests and local setups).
    pub fn new_local(index_path: PathBuf) -> Self {
        Self { index_path: Some(index_path), remote_url: None, offline: false }
    }

    /// Remote registry with transparent local caching.
    pub fn new_remote(url: String) -> Self {
        Self { index_path: None, remote_url: Some(url), offline: false }
    }

    /// Remote registry in offline mode – serves from cache only.
    #[allow(dead_code)]
    pub fn new_offline() -> Self {
        Self { index_path: None, remote_url: None, offline: true }
    }

    /// Return a copy of this client that operates offline.
    #[allow(dead_code)]
    pub fn as_offline(&self) -> Self {
        Self { offline: true, ..self.clone() }
    }

    // -----------------------------------------------------------------------
    // Metadata lookup
    // -----------------------------------------------------------------------

    /// Fetch all known versions of `package_name` from the index.
    pub fn get_versions(&self, package_name: &str) -> Result<Vec<IndexEntry>, String> {
        let name = package_name.to_lowercase();
        let shard_path = Self::shard_path(&name);

        let content = if let Some(local) = &self.index_path {
            // Explicit local index (tests / local registry).
            let path = local.join(&shard_path);
            if !path.exists() {
                return Err(format!("'{}' not found in local registry index", name));
            }
            fs::read_to_string(&path).map_err(|e| e.to_string())?
        } else {
            let cache = CacheManager::new();
            // Try local index cache first.
            if let Some(cached_path) = cache.get_cached_index(shard_path.to_str().unwrap_or("")) {
                fs::read_to_string(&cached_path).map_err(|e| e.to_string())?
            } else if self.offline {
                return Err(format!(
                    "Offline mode: '{}' not in local cache. Run `nyx fetch` first.",
                    name
                ));
            } else if let Some(remote) = &self.remote_url {
                // Fetch from remote and cache locally.
                let url = format!("{}/{}", remote.trim_end_matches('/'), shard_path.display());
                let resp = reqwest::blocking::get(&url)
                    .map_err(|e| format!("Failed to fetch index for '{}': {}", name, e))?;
                if !resp.status().is_success() {
                    return Err(format!(
                        "'{}' not found in remote registry (HTTP {})",
                        name, resp.status()
                    ));
                }
                let text = resp.text().map_err(|e| e.to_string())?;
                // Cache for future offline / fast use.
                cache.cache_index(shard_path.to_str().unwrap_or(""), &text).ok();
                text
            } else {
                return Err("No registry source configured.".to_string());
            }
        };

        let mut entries = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() { continue; }
            let entry: IndexEntry = serde_json::from_str(line)
                .map_err(|e| format!("Failed to parse registry entry for '{}': {}", name, e))?;
            entries.push(entry);
        }
        Ok(entries)
    }

    /// For a given package and a semantic version constraint (e.g. "^1.2.0"),
    /// queries the index, parses all available versions, filters those matching,
    /// and returns the highest version (along with its metadata).
    #[allow(dead_code)]
    pub fn latest_matching(
        &self,
        package_name: &str,
        req_str: &str,
    ) -> Result<Option<IndexEntry>, String> {
        let req = semver::VersionReq::parse(req_str)
            .map_err(|e| format!("Invalid requirement '{}': {}", req_str, e))?;
        let mut entries = self.get_versions(package_name)?;
        entries.sort_by(|a, b| {
            let va = Version::parse(&a.vers).unwrap_or(Version::new(0, 0, 0));
            let vb = Version::parse(&b.vers).unwrap_or(Version::new(0, 0, 0));
            vb.cmp(&va) // Descending
        });

        let found = entries.into_iter()
            .find(|e| Version::parse(&e.vers).map(|v| req.matches(&v)).unwrap_or(false));
            
        Ok(found)
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Compute the shard path for a package name following the standard layout:
    ///   len 1 → 1/<name>
    ///   len 2 → 2/<name>
    ///   len 3 → 3/<first-char>/<name>
    ///   len 4+ → <first-2>/<next-2>/<name>
    pub fn shard_path(name: &str) -> PathBuf {
        match name.len() {
            1 => PathBuf::from("1").join(name),
            2 => PathBuf::from("2").join(name),
            3 => PathBuf::from("3").join(&name[0..1]).join(name),
            _ => PathBuf::from(&name[0..2]).join(&name[2..4]).join(name),
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

    fn write_entry(index: &std::path::Path, name: &str, entry: &IndexEntry) {
        let shard = RegistryClient::shard_path(&name.to_lowercase());
        let path = index.join(&shard);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let line = serde_json::to_string(entry).unwrap();
        let existing = fs::read_to_string(&path).unwrap_or_default();
        fs::write(&path, format!("{}{}\n", existing, line)).unwrap();
    }

    #[test]
    fn test_shard_path_1_char() {
        assert_eq!(RegistryClient::shard_path("a"), PathBuf::from("1").join("a"));
    }

    #[test]
    fn test_shard_path_2_chars() {
        assert_eq!(RegistryClient::shard_path("ab"), PathBuf::from("2").join("ab"));
    }

    #[test]
    fn test_shard_path_3_chars() {
        assert_eq!(RegistryClient::shard_path("jit"), PathBuf::from("3").join("j").join("jit"));
    }

    #[test]
    fn test_shard_path_long() {
        assert_eq!(
            RegistryClient::shard_path("cranelift"),
            PathBuf::from("cr").join("an").join("cranelift")
        );
    }

    #[test]
    fn test_get_versions_local() {
        let tmp = tempdir().unwrap();
        let entry = IndexEntry {
            vers: "1.0.0".to_string(),
            deps: vec![],
            cksum: "blake3:abc".to_string(),
        };
        write_entry(tmp.path(), "foo", &entry);

        let client = RegistryClient::new_local(tmp.path().to_path_buf());
        let versions = client.get_versions("foo").unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].vers, "1.0.0");
    }

    #[test]
    fn test_latest_matching() {
        let tmp = tempdir().unwrap();
        for v in &["1.0.0", "1.5.0", "2.0.0"] {
            let entry = IndexEntry { vers: v.to_string(), deps: vec![], cksum: "x".to_string() };
            write_entry(tmp.path(), "mylib", &entry);
        }

        let client = RegistryClient::new_local(tmp.path().to_path_buf());
        let best = client.latest_matching("mylib", "^1.0").unwrap().unwrap();
        assert_eq!(best.vers, "1.5.0");
    }

    #[test]
    fn test_offline_no_network() {
        let client = RegistryClient::new_offline();
        let result = client.get_versions("something-not-cached");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Offline mode"));
    }
}
