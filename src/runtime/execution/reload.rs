use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::module_loader::NyxModule;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimeStateSnapshot {
    pub state_slots: BTreeMap<String, String>,
    pub focus_owner: Option<String>,
    pub route_stack: Vec<String>,
    pub scroll_offsets: BTreeMap<String, f32>,
    pub animation_ticks: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReloadSnapshot {
    pub runtime: RuntimeStateSnapshot,
    pub module_versions: BTreeMap<String, u64>,
    pub globals: BTreeMap<String, crate::runtime::execution::nyx_vm::Value>,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub struct ModulePatch {
    pub module_id: String,
    pub source_path: PathBuf,
    pub next: NyxModule,
}

#[derive(Debug, Clone, Default)]
pub struct PatchReport {
    pub patched_modules: Vec<String>,
    pub remounted_boundaries: Vec<String>,
    pub errors: Vec<String>,
    pub reload_triggered: bool,
}
