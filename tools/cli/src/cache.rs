use std::path::{Path, PathBuf};
use std::fs;
use std::io::Read;
use crate::verifier::ChecksumVerifier;

// ---------------------------------------------------------------------------
// Cache directory layout: ~/.nyx/
//   packages/
//     registry/<name>/<version>/   ← extracted registry packages
//     git/<name>/<url-hash>/       ← git clones
//   index/                         ← cached remote registry index files
// ---------------------------------------------------------------------------

pub struct CacheManager {
    /// Root cache directory: `~/.nyx`
    root: PathBuf,
}

impl CacheManager {
    pub fn new() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let root = PathBuf::from(home).join(".nyx");
        fs::create_dir_all(root.join("packages").join("registry")).ok();
        fs::create_dir_all(root.join("packages").join("git")).ok();
        fs::create_dir_all(root.join("index")).ok();
        Self { root }
    }

    // -----------------------------------------------------------------------
    // Registry packages
    // -----------------------------------------------------------------------

    /// Return the cached registry package directory if it exists.
    pub fn get_package(&self, name: &str, version: &str) -> Option<PathBuf> {
        let dir = self.registry_dir(name, version);
        if dir.exists() { Some(dir) } else { None }
    }

    /// Download a registry package tarball, verify its BLAKE3/SHA256 checksum,
    /// extract it into the cache, and return the directory path.
    pub fn download_registry_package(
        &self,
        name: &str,
        version: &str,
        url: &str,
        expected_cksum: &str,
    ) -> Result<PathBuf, String> {
        let dir = self.registry_dir(name, version);
        if dir.exists() {
            return Ok(dir);
        }

        println!("    Downloading {} v{} …", name, version);

        let mut response = reqwest::blocking::get(url)
            .map_err(|e| format!("Download failed for {} v{}: {}", name, version, e))?;
        if !response.status().is_success() {
            return Err(format!("HTTP {} downloading {} v{}", response.status(), name, version));
        }

        let mut buf = Vec::new();
        response.read_to_end(&mut buf).map_err(|e| e.to_string())?;

        // Verify checksum (supports both "blake3:…" and bare SHA256 hex).
        println!("    Verifying checksum …");
        let actual = if expected_cksum.starts_with("blake3:") {
            format!("blake3:{}", ChecksumVerifier::compute_blake3(&buf))
        } else if expected_cksum.starts_with("sha256:") {
            format!("sha256:{}", ChecksumVerifier::compute_sha256(&buf))
        } else {
            ChecksumVerifier::compute_sha256(&buf)
        };

        if actual != expected_cksum && !expected_cksum.is_empty() {
            return Err(format!(
                "Checksum mismatch for {} v{}\n  expected: {}\n  actual:   {}",
                name, version, expected_cksum, actual
            ));
        }

        // Extract.
        println!("    Extracting …");
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let gz = flate2::read::GzDecoder::new(&buf[..]);
        let mut archive = tar::Archive::new(gz);
        archive.unpack(&dir).map_err(|e| format!("Extraction failed: {}", e))?;

        Ok(dir)
    }

    // -----------------------------------------------------------------------
    // Git packages  (uses git2 crate for real cloning)
    // -----------------------------------------------------------------------

    fn git_dir(&self, name: &str, url: &str) -> PathBuf {
        let url_hash = format!("{:x}", md5::compute(url));
        self.root.join("packages").join("git").join(name).join(url_hash)
    }

    /// Return the cached git clone directory if it exists.
    pub fn get_git_package(
        &self,
        name: &str,
        git_url: &str,
        _rev: &Option<String>,
    ) -> Option<PathBuf> {
        let dir = self.git_dir(name, git_url);
        if dir.exists() { Some(dir) } else { None }
    }

    /// Clone (or fetch) a git repository using the `git2` crate.
    pub fn cache_git_package(
        &self,
        name: &str,
        git_url: &str,
        branch: &Option<String>,
        tag: &Option<String>,
        rev: &Option<String>,
    ) -> Result<PathBuf, String> {
        let dir = self.git_dir(name, git_url);

        if dir.exists() {
            // Already cloned – just return.
            return Ok(dir);
        }

        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

        println!("    Cloning {} from {} …", name, git_url);

        // Use git2 for a proper clone.
        let mut fo = git2::FetchOptions::new();
        fo.download_tags(git2::AutotagOption::All);

        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fo);

        if let Some(b) = branch {
            builder.branch(b);
        }

        let repo = builder
            .clone(git_url, &dir)
            .map_err(|e| format!("git clone failed for {}: {}", name, e))?;

        // Checkout the requested rev or tag.
        let target = rev.as_ref().or(tag.as_ref());
        if let Some(t) = target {
            let obj = repo
                .revparse_single(t)
                .map_err(|e| format!("git revparse '{}' failed: {}", t, e))?;
            repo.checkout_tree(&obj, None)
                .map_err(|e| format!("git checkout failed: {}", e))?;
            repo.set_head_detached(obj.id())
                .map_err(|e| format!("git set-head failed: {}", e))?;
        }

        Ok(dir)
    }

    // -----------------------------------------------------------------------
    // Local path dependencies
    // -----------------------------------------------------------------------

    /// Validate that a local path dependency exists and return its absolute path.
    pub fn get_path_package(
        &self,
        project_root: &Path,
        rel_path: &str,
    ) -> Result<PathBuf, String> {
        let abs = project_root.join(rel_path);
        if abs.exists() {
            Ok(abs)
        } else {
            Err(format!(
                "Path dependency not found: '{}' (resolved to {:?})",
                rel_path, abs
            ))
        }
    }

    // -----------------------------------------------------------------------
    // Registry index caching (for offline / incremental updates)
    // -----------------------------------------------------------------------

    /// Return the locally-cached registry index file path for a given shard.
    pub fn get_cached_index(&self, shard: &str) -> Option<PathBuf> {
        let path = self.root.join("index").join(shard);
        if path.exists() { Some(path) } else { None }
    }

    /// Write fetched registry index content to local cache.
    pub fn cache_index(&self, shard: &str, content: &str) -> Result<(), String> {
        let path = self.root.join("index").join(shard);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&path, content).map_err(|e| e.to_string())
    }

    // -----------------------------------------------------------------------
    // Build artifacts cleanup
    // -----------------------------------------------------------------------

    pub fn clean_local(&self, project_root: &Path, release_only: bool) -> Result<(), String> {
        let target = project_root.join("target");
        let path = if release_only { target.join("release") } else { target.clone() };
        if path.exists() {
            fs::remove_dir_all(&path).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    fn registry_dir(&self, name: &str, version: &str) -> PathBuf {
        self.root
            .join("packages")
            .join("registry")
            .join(name)
            .join(version)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_path_dep_exists() {
        let tmp = tempdir().unwrap();
        let sub = tmp.path().join("mylib");
        fs::create_dir_all(&sub).unwrap();
        let cm = CacheManager::new();
        let result = cm.get_path_package(tmp.path(), "mylib");
        assert!(result.is_ok());
    }

    #[test]
    fn test_path_dep_missing() {
        let tmp = tempdir().unwrap();
        let cm = CacheManager::new();
        let result = cm.get_path_package(tmp.path(), "nope");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_index_cache_roundtrip() {
        let cm = CacheManager::new();
        let shard = "test-shard-roundtrip";
        let content = r#"{"vers":"1.0.0","deps":[],"cksum":"abc"}"#;
        cm.cache_index(shard, content).expect("cache_index should succeed");
        let cached = cm.get_cached_index(shard);
        assert!(cached.is_some());
        let read = fs::read_to_string(cached.unwrap()).unwrap();
        assert_eq!(read, content);
    }
}
