use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::RuntimeError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NyxModule {
    pub id: String,
    pub path: PathBuf,
    pub source: String,
    pub version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NyxPackage {
    pub entry_module: String,
    pub target: String,
    pub modules: Vec<NyxModule>,
    pub assets: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModuleHandle(pub usize);

#[derive(Debug, Default, Clone)]
pub struct ModuleLoader {
    modules: BTreeMap<String, NyxModule>,
}

impl ModuleLoader {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load_package(&mut self, package: NyxPackage) {
        self.modules = package
            .modules
            .into_iter()
            .map(|module| (module.id.clone(), module))
            .collect();
    }

    pub fn get(&self, module_id: &str) -> Option<&NyxModule> {
        self.modules.get(module_id)
    }

    pub fn modules(&self) -> impl Iterator<Item = &NyxModule> {
        self.modules.values()
    }

    pub fn patch(&mut self, module: NyxModule) {
        self.modules.insert(module.id.clone(), module);
    }
}

pub fn module_id_from_path(root: &Path, path: &Path) -> Result<String, RuntimeError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| RuntimeError::new(format!("module {} is outside package root", path.display())))?;
    let mut id = relative.to_string_lossy().replace('\\', "/");
    if let Some(stripped) = id.strip_suffix(".nyx") {
        id = stripped.to_string();
    }
    Ok(id)
}
