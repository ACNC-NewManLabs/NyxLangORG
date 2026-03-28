use std::path::{Path, PathBuf};

use super::runtime_host::HostError;

#[derive(Debug, Clone)]
pub struct AssetHost {
    pub root: PathBuf,
}

impl AssetHost {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn read(&self, asset_id: &str) -> Result<Vec<u8>, HostError> {
        std::fs::read(self.root.join(asset_id)).map_err(|e| HostError::new(e.to_string()))
    }
}
