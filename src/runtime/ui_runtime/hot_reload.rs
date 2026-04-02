use std::path::{Path, PathBuf};

use crate::runtime::compiler_bridge::incremental::incremental_patch_set;
use crate::runtime::execution::reload::PatchReport;
use crate::runtime::execution::{RuntimeError, RuntimeSession};

pub fn patch_runtime<R: RuntimeSession>(
    runtime: &mut R,
    entry: &Path,
    changed: &Path,
    version: u64,
) -> Result<PatchReport, RuntimeError> {
    let snapshot = runtime.snapshot_reload_state()?;
    let patches = incremental_patch_set(entry, changed, version)?;
    let report = runtime.patch_modules(patches)?;
    runtime.restore_reload_state(snapshot)?;
    Ok(report)
}

pub fn watched_paths(entry: &Path, engine_root: &Path) -> Vec<PathBuf> {
    let mut out = vec![
        entry.to_path_buf(),
        engine_root.join("engine.json"),
        engine_root.join("src"),
    ];
    out.sort();
    out.dedup();
    out
}
