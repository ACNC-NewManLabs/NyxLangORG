use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::runtime::compiler_bridge::package::package_entry;
use crate::runtime::execution::module_loader::{ModuleHandle, ModuleLoader, NyxPackage};
use crate::runtime::execution::native_bridge::{register_host_natives, NativeBridgeConfig};
use crate::runtime::execution::reload::{
    ModulePatch, PatchReport, ReloadSnapshot, RuntimeStateSnapshot,
};
use crate::runtime::execution::stdlib_bridge::register_stdlib;
use crate::runtime::execution::{ModuleInstance, RuntimeError, RuntimeSession, RuntimeValue};

use super::nyx_vm::{EvalError, NyxVm, Value};

#[derive(Debug, Clone)]
pub struct EngineInfo {
    pub name: String,
    pub version: Option<String>,
    pub modules: usize,
}

#[derive(Debug, Clone, Copy)]
pub enum NativeRuntime {
    Headless,
    WebPreview,
}

#[derive(Debug, Deserialize)]
struct EngineManifest {
    name: String,
    entry: Option<String>,
    modules: Vec<String>,
    version: Option<String>,
}

pub struct AstRuntimeSession {
    vm: NyxVm,
    loader: ModuleLoader,
    package: Option<NyxPackage>,
    engine_roots: Vec<PathBuf>,
}

impl AstRuntimeSession {
    pub fn new(entry_file: &Path, runtime: NativeRuntime) -> Result<Self, RuntimeError> {
        let mut engine_roots = Vec::new();
        
        let ui_root = ui_engine_root();
        if ui_root.exists() {
            if let Err(e) = preflight_engine(&ui_root) {
                eprintln!("UI Engine preflight failed: {e}");
                return Err(RuntimeError::new(e));
            }
            engine_roots.push(ui_root.clone());
        }

        let std_root = stdlib_root();
        if std_root.exists() {
            engine_roots.extend(discover_engines(&std_root));
        }

        let ai_root = std_root.parent().unwrap_or_else(|| Path::new(".")).join("engines").join("ai");
        if ai_root.exists() {
            if let Err(e) = preflight_engine(&ai_root) {
                eprintln!("AI Engine preflight failed: {e}");
            } else {
                engine_roots.push(ai_root);
            }
        }

        let mut vm = NyxVm::new();
        let bridge = NativeBridgeConfig {
            asset_root: ui_root.join("fonts"),
            runtime_name: match runtime {
                NativeRuntime::Headless => "headless".to_string(),
                NativeRuntime::WebPreview => "web".to_string(),
            },
        };
        register_host_natives(&mut vm, &bridge);
        register_stdlib(&mut vm);
        vm.set_stdlib_path(std_root);
        let package = package_entry(entry_file, "ast")?.package;
        let mut session = Self {
            vm,
            loader: ModuleLoader::new(),
            package: None,
            engine_roots,
        };
        session.load_package(package)?;
        Ok(session)
    }

    pub fn vm_mut(&mut self) -> &mut NyxVm {
        &mut self.vm
    }
}

impl RuntimeSession for AstRuntimeSession {
    fn load_package(&mut self, package: NyxPackage) -> Result<(), RuntimeError> {
        self.loader.load_package(package.clone());
        for root in &self.engine_roots {
            if let Err(e) = self.vm.load_engine_from_manifest(root, "engine.json") {
                eprintln!("Failed to load engine from {}: {}", root.display(), e.message);
                return Err(RuntimeError::from(e));
            }
        }
        for module in &package.modules {
            self.vm.load_file("", &module.path)?;
        }
        self.package = Some(package);
        Ok(())
    }

    fn instantiate_module(&mut self, module_id: &str) -> Result<ModuleInstance, RuntimeError> {
        self.loader
            .get(module_id)
            .ok_or_else(|| RuntimeError::new(format!("unknown module '{module_id}'")))?;
        Ok(ModuleInstance {
            handle: ModuleHandle(1),
            module_id: module_id.to_string(),
        })
    }

    fn invoke(
        &mut self,
        entry_symbol: &str,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, RuntimeError> {
        self.vm
            .call_function(entry_symbol, args)
            .map_err(RuntimeError::from)
    }

    fn patch_modules(
        &mut self,
        changed_modules: Vec<ModulePatch>,
    ) -> Result<PatchReport, RuntimeError> {
        let mut report = PatchReport::default();
        for patch in changed_modules {
            self.loader.patch(patch.next.clone());
            self.vm.load_file("", &patch.source_path)?;
            report.patched_modules.push(patch.module_id);
        }
        Ok(report)
    }

    fn snapshot_reload_state(&mut self) -> Result<ReloadSnapshot, RuntimeError> {
        let runtime = RuntimeStateSnapshot {
            state_slots: Default::default(),
            focus_owner: None,
            route_stack: vec![],
            scroll_offsets: Default::default(),
            animation_ticks: Default::default(),
        };
        let module_versions = self
            .loader
            .modules()
            .map(|module| (module.id.clone(), module.version))
            .collect();
        let globals: BTreeMap<String, Value> = self.vm.globals.clone().into_iter().collect();
        Ok(ReloadSnapshot {
            runtime,
            module_versions,
            globals,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        })
    }

    fn restore_reload_state(&mut self, _snapshot: ReloadSnapshot) -> Result<(), RuntimeError> {
        Ok(())
    }
}

pub fn execute_app(input: &Path) -> Result<Value, EvalError> {
    let mut session =
        AstRuntimeSession::new(input, NativeRuntime::Headless).map_err(to_eval_error)?;
    session.vm.execute_main()
}

pub fn execute_bytecode_app(input: &Path) -> Result<Value, EvalError> {
    use super::BytecodeRuntimeSession;
    let mut session = BytecodeRuntimeSession::new();
    let package = crate::runtime::compiler_bridge::package::package_entry(input, "bytecode")
        .map_err(|e| to_eval_error(RuntimeError::new(e.to_string())))?
        .package;
    session.load_package(package).map_err(to_eval_error)?;
    session.invoke("main", vec![]).map_err(to_eval_error)
}

pub fn build_session(
    input: &Path,
    runtime: NativeRuntime,
) -> Result<AstRuntimeSession, RuntimeError> {
    AstRuntimeSession::new(input, runtime)
}

pub fn load_ui_engine_info() -> Result<EngineInfo, String> {
    let root = ui_engine_root();
    let manifest_path = root.join("engine.json");
    let raw = std::fs::read_to_string(&manifest_path).map_err(|e| {
        format!(
            "failed to read ui engine manifest {}: {e}",
            manifest_path.display()
        )
    })?;
    let manifest: EngineManifest =
        serde_json::from_str(&raw).map_err(|e| format!("invalid ui engine manifest JSON: {e}"))?;
    Ok(EngineInfo {
        name: manifest.name,
        version: manifest.version,
        modules: manifest.modules.len(),
    })
}

pub fn preflight_engine(root: &Path) -> Result<(), String> {
    let manifest_path = root.join("engine.json");
    let raw = std::fs::read_to_string(&manifest_path).map_err(|e| {
        format!(
            "failed to read engine manifest {}: {e}",
            manifest_path.display()
        )
    })?;
    let manifest: EngineManifest =
        serde_json::from_str(&raw).map_err(|e| format!("invalid engine manifest JSON: {e}"))?;

    for rel in &manifest.modules {
        let module_path = root.join(rel.trim_start_matches("./"));
        if !module_path.exists() {
            return Err(format!(
                "engine module missing: {} (from {})",
                module_path.display(),
                manifest_path.display()
            ));
        }
    }

    if let Some(entry) = manifest.entry {
        let entry_path = root.join(entry.trim_start_matches("./"));
        if !entry_path.exists() {
            return Err(format!(
                "engine entry missing: {} (from {})",
                entry_path.display(),
                manifest_path.display()
            ));
        }
    }

    Ok(())
}

pub fn discover_engines(root: &Path) -> Vec<PathBuf> {
    let mut engines = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("engine.json").exists() {
                engines.push(path);
            }
        }
    }
    engines
}

pub fn preflight_ui_engine() -> Result<(), String> {
    let root = ui_engine_root();
    if !root.exists() {
        return Ok(()); // Optional
    }
    preflight_engine(&root)
}

pub fn stdlib_root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/stdlib"))
}

pub fn ui_engine_root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/engines/ui_engine"))
}

fn to_eval_error(message: RuntimeError) -> EvalError {
    EvalError {
        message: message.message,
        stack: vec![],
    }
}
