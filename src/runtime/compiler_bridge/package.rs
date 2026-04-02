use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::runtime::execution::module_loader::{module_id_from_path, NyxModule, NyxPackage};
use crate::runtime::execution::RuntimeError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetRecord {
    pub logical_path: String,
    pub content_hash: String,
    pub output_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageBuild {
    pub package: NyxPackage,
    pub ordered_modules: Vec<String>,
    pub asset_records: Vec<AssetRecord>,
}

pub fn package_entry(entry_file: &Path, target: &str) -> Result<PackageBuild, RuntimeError> {
    let entry_file = std::fs::canonicalize(entry_file).map_err(|e| {
        RuntimeError::new(format!(
            "failed to canonicalize {}: {e}",
            entry_file.display()
        ))
    })?;
    let root = package_root(&entry_file)?;
    let index = index_modules(&root)?;
    let mut ordered_modules = Vec::new();
    let mut modules = Vec::new();
    let mut queued = BTreeMap::<PathBuf, bool>::new();
    let mut queue = VecDeque::from([entry_file.clone()]);

    while let Some(path) = queue.pop_front() {
        if queued.insert(path.clone(), true).is_some() {
            continue;
        }

        let module = module_from_file(&root, &path, 1)?;
        let source = module.source.clone();
        ordered_modules.push(module.id.clone());
        modules.push(module);

        for dep in parse_dependencies(&source) {
            if let Some(dep_path) = resolve_dependency(&path, &root, &index, &dep) {
                queue.push_back(dep_path);
            }
        }
    }

    let entry_module = module_id_from_path(&root, &entry_file)?;
    Ok(PackageBuild {
        package: NyxPackage {
            entry_module,
            target: target.to_string(),
            modules,
            assets: BTreeMap::new(),
        },
        ordered_modules,
        asset_records: vec![],
    })
}

pub fn module_from_file(root: &Path, file: &Path, version: u64) -> Result<NyxModule, RuntimeError> {
    let source = std::fs::read_to_string(file)
        .map_err(|e| RuntimeError::new(format!("failed reading {}: {e}", file.display())))?;
    let id = module_id_from_path(root, file)?;
    Ok(NyxModule {
        id,
        path: PathBuf::from(file),
        source,
        version,
    })
}

pub fn content_hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub fn stable_toposort(entry: &str, edges: &BTreeMap<String, Vec<String>>) -> Vec<String> {
    let mut out = Vec::new();
    let mut queue = VecDeque::from([entry.to_string()]);
    let mut seen = BTreeMap::<String, bool>::new();
    while let Some(node) = queue.pop_front() {
        if seen.insert(node.clone(), true).is_some() {
            continue;
        }
        out.push(node.clone());
        if let Some(children) = edges.get(&node) {
            let mut ordered = children.clone();
            ordered.sort();
            for child in ordered {
                queue.push_back(child);
            }
        }
    }
    out.sort();
    out
}

fn package_root(entry_file: &Path) -> Result<PathBuf, RuntimeError> {
    let entry_dir = entry_file
        .parent()
        .ok_or_else(|| RuntimeError::new("entry file has no parent directory"))?;

    // If entry_dir is empty, it means the file is in the current directory.
    // Use "." to ensure path methods like ancestors() and join() work as expected for fs operations.
    let search_root = if entry_dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        entry_dir
    };

    for ancestor in search_root.ancestors() {
        if ancestor.join("engine.json").exists() {
            return Ok(ancestor.to_path_buf());
        }
    }
    Ok(search_root.to_path_buf())
}

fn index_modules(root: &Path) -> Result<BTreeMap<String, Vec<PathBuf>>, RuntimeError> {
    let mut files = Vec::new();
    collect_nyx_files(root, &mut files)
        .map_err(|e| RuntimeError::new(format!("failed scanning {}: {e}", root.display())))?;

    let mut index = BTreeMap::<String, Vec<PathBuf>>::new();
    for file in files {
        for alias in module_aliases(root, &file)? {
            index.entry(alias).or_default().push(file.clone());
        }
    }
    Ok(index)
}

fn collect_nyx_files(root: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();

        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
        }

        if path.is_dir() {
            collect_nyx_files(&path, files)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("nyx") {
            files.push(path);
        }
    }
    Ok(())
}

fn module_aliases(root: &Path, file: &Path) -> Result<Vec<String>, RuntimeError> {
    let mut aliases = Vec::new();
    let relative = file.strip_prefix(root).map_err(|_| {
        RuntimeError::new(format!("module {} is outside package root", file.display()))
    })?;
    let relative_no_ext = relative
        .to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches(".nyx")
        .to_string();
    aliases.push(relative_no_ext.clone());

    for prefix in ["src/", "tests/", "stdlib/"] {
        if let Some(stripped) = relative_no_ext.strip_prefix(prefix) {
            aliases.push(stripped.to_string());
        }
    }

    if let Some(stem) = file.file_stem().and_then(|stem| stem.to_str()) {
        aliases.push(stem.to_string());
        if let Some(stripped) = stem.strip_suffix("_unit_tests") {
            aliases.push(format!("{stripped}_tests"));
        }
    }

    if let Some(parent) = file
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
    {
        aliases.push(parent.to_string());
    }

    aliases.sort();
    aliases.dedup();
    Ok(aliases)
}

fn parse_dependencies(source: &str) -> Vec<String> {
    let mut deps = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        for prefix in ["use ", "mod ", "import "] {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                let name = rest
                    .split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .trim_end_matches(';')
                    .replace("::", "/");
                if !name.is_empty() {
                    deps.push(name);
                }
            }
        }
    }
    deps
}

fn resolve_dependency(
    current_file: &Path,
    root: &Path,
    index: &BTreeMap<String, Vec<PathBuf>>,
    dep: &str,
) -> Option<PathBuf> {
    if dep.starts_with("std/") {
        return None;
    }
    let current_dir = current_file.parent()?;
    let direct_candidates = [
        current_dir.join(format!("{dep}.nyx")),
        root.join(format!("{dep}.nyx")),
        root.join("src").join(format!("{dep}.nyx")),
        root.join("stdlib").join(format!("{dep}.nyx")),
        root.join("stdlib").join(dep).join("mod.nyx"),
    ];

    for candidate in direct_candidates {
        if candidate.exists() {
            return Some(candidate);
        }
    }

    let matches = index.get(dep)?;
    if matches.len() == 1 {
        return matches.first().cloned();
    }

    matches
        .iter()
        .find(|path| path.starts_with(current_dir))
        .cloned()
        .or_else(|| matches.first().cloned())
}
