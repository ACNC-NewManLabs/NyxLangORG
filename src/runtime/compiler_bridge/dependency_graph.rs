use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::runtime::execution::RuntimeError;

#[derive(Debug, Clone, Default)]
pub struct DependencyGraph {
    pub edges: BTreeMap<String, BTreeSet<String>>,
    pub module_files: BTreeMap<String, PathBuf>,
}

impl DependencyGraph {
    pub fn discover_from_entry(entry: &Path) -> Result<Self, RuntimeError> {
        let mut graph = Self::default();
        let module_id = entry
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| RuntimeError::new("invalid entry filename"))?
            .to_string();
        let source = std::fs::read_to_string(entry)
            .map_err(|e| RuntimeError::new(format!("failed reading {}: {e}", entry.display())))?;
        graph
            .module_files
            .insert(module_id.clone(), entry.to_path_buf());
        graph.edges.insert(module_id, parse_imports(&source));
        Ok(graph)
    }

    pub fn impacted_modules(&self, changed: &str) -> Vec<String> {
        let mut impacted = BTreeSet::from([changed.to_string()]);
        let mut changed_any = true;
        while changed_any {
            changed_any = false;
            for (module, deps) in &self.edges {
                if deps.iter().any(|dep| impacted.contains(dep)) && impacted.insert(module.clone())
                {
                    changed_any = true;
                }
            }
        }
        impacted.into_iter().collect()
    }
}

fn parse_imports(source: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("use ") {
            let name = rest
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .trim_end_matches(';')
                .replace("::", "/");
            if !name.is_empty() {
                out.insert(name);
            }
        }
    }
    out
}
