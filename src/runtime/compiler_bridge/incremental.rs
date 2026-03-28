use std::path::Path;

use crate::runtime::compiler_bridge::dependency_graph::DependencyGraph;
use crate::runtime::compiler_bridge::package::module_from_file;
use crate::runtime::execution::reload::ModulePatch;
use crate::runtime::execution::RuntimeError;

pub fn incremental_patch_set(
    entry: &Path,
    changed_file: &Path,
    next_version: u64,
) -> Result<Vec<ModulePatch>, RuntimeError> {
    let root = entry
        .parent()
        .ok_or_else(|| RuntimeError::new("entry file has no parent directory"))?;
    let graph = DependencyGraph::discover_from_entry(entry)?;
    let changed_id = changed_file
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| RuntimeError::new("invalid changed file name"))?;

    let impacted = graph.impacted_modules(changed_id);
    let mut patches = Vec::new();
    for module_id in impacted {
        let source_path = graph
            .module_files
            .get(&module_id)
            .cloned()
            .unwrap_or_else(|| changed_file.to_path_buf());
        let next = module_from_file(root, &source_path, next_version)?;
        patches.push(ModulePatch {
            module_id,
            source_path,
            next,
        });
    }
    Ok(patches)
}
