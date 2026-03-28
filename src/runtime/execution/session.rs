//! Runtime Session API Implementation
//!
//! This module provides the unified session-based runtime interface
//! that wraps the existing Nyx VM behind a clean API.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::runtime::compiler_bridge::package::package_entry;
use crate::runtime::execution::module_loader::{ModuleHandle, ModuleLoader, NyxPackage};
use crate::runtime::execution::native_bridge::{register_host_natives, NativeBridgeConfig};
use crate::runtime::execution::nyx_vm::{EvalError, NyxVm, Value};
use crate::runtime::execution::reload::{ModulePatch, PatchReport, ReloadSnapshot, RuntimeStateSnapshot};

/// Runtime session configuration
#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub entry_file: PathBuf,
    pub engine_root: PathBuf,
    pub runtime_name: String,
}

/// Module instance handle
#[derive(Debug, Clone)]
pub struct ModuleInstance {
    pub handle: ModuleHandle,
    pub module_id: String,
    pub exports: BTreeMap<String, Value>,
}

/// Runtime session error
#[derive(Debug, Clone)]
pub struct SessionError {
    pub message: String,
}

impl SessionError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self { message: msg.into() }
    }
}

impl From<EvalError> for SessionError {
    fn from(e: EvalError) -> Self {
        Self { message: e.message }
    }
}

/// Main runtime session that provides the unified API
pub struct RuntimeSession {
    vm: Arc<Mutex<NyxVm>>,
    module_loader: ModuleLoader,
    config: SessionConfig,
    loaded_packages: BTreeMap<String, NyxPackage>,
    module_instances: BTreeMap<String, ModuleInstance>,
    state_snapshots: Vec<RuntimeStateSnapshot>,
    initialized: bool,
}

impl RuntimeSession {
    fn module_prefix_for_file(entry_file: &Path, module_path: &Path) -> String {
        if module_path == entry_file {
            return String::new();
        }
        module_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string()
    }

    fn native_bridge_config(&self) -> NativeBridgeConfig {
        NativeBridgeConfig {
            asset_root: self.config.engine_root.join("fonts"),
            runtime_name: self.config.runtime_name.clone(),
        }
    }

    fn bootstrap_vm(&mut self) -> Result<(), SessionError> {
        let mut vm = self
            .vm
            .lock()
            .map_err(|_| SessionError::new("VM lock poisoned"))?;
        *vm = NyxVm::new();
        let bridge = self.native_bridge_config();
        register_host_natives(&mut vm, &bridge);
        crate::runtime::execution::stdlib_bridge::register_stdlib(&mut vm);

        let mut engine_roots = Vec::new();
        if self.config.engine_root.exists() {
            engine_roots.push(self.config.engine_root.clone());
        }

        let std_root = crate::runtime::execution::ui_runtime::stdlib_root();
        if std_root.exists() {
            vm.set_stdlib_path(std_root.clone());
            engine_roots.extend(crate::runtime::execution::ui_runtime::discover_engines(&std_root));
        }

        for root in engine_roots {
            vm.load_engine_from_manifest(&root, "engine.json")?;
        }

        Ok(())
    }

    fn load_package_into_vm(&mut self, package: &NyxPackage) -> Result<(), SessionError> {
        let mut vm = self
            .vm
            .lock()
            .map_err(|_| SessionError::new("VM lock poisoned"))?;

        for module in &package.modules {
            let prefix = if module.id == package.entry_module {
                String::new()
            } else {
                Self::module_prefix_for_file(&self.config.entry_file, &module.path)
            };
            vm.load_file(prefix, &module.path)?;
        }
        Ok(())
    }

    /// Create a new runtime session
    pub fn new(config: SessionConfig) -> Result<Self, SessionError> {
        let vm = NyxVm::new();
        
        Ok(Self {
            vm: Arc::new(Mutex::new(vm)),
            module_loader: ModuleLoader::new(),
            config,
            loaded_packages: BTreeMap::new(),
            module_instances: BTreeMap::new(),
            state_snapshots: Vec::new(),
            initialized: false,
        })
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Initialize the session with the engine modules
    pub fn initialize(&mut self) -> Result<(), SessionError> {
        self.reload_entry_package()?;
        self.initialized = true;
        Ok(())
    }

    pub fn reload_entry_package(&mut self) -> Result<PatchReport, SessionError> {
        // Rebuild the package from the entry file (dependency scan), then reload the VM so
        // deleted/renamed symbols are not left behind.
        let build = package_entry(&self.config.entry_file, "ast")
            .map_err(|e| SessionError::new(e.message))?;
        let mut package = build.package;

        // Avoid double-loading engine modules if the package root happens to include them.
        // The engine is loaded explicitly from `engine_root/engine.json`.
        package.modules.retain(|m| {
            m.path == self.config.entry_file || !m.path.starts_with(&self.config.engine_root)
        });

        let snapshot = self.snapshot_reload_state()?;
        self.state_snapshots.push(snapshot.runtime.clone());

        self.bootstrap_vm()?;
        self.loaded_packages.clear();
        self.module_instances.clear();
        self.load_package(package.clone())?;
        self.restore_reload_state(snapshot)?;

        Ok(PatchReport {
            patched_modules: package.modules.iter().map(|m| m.id.clone()).collect(),
            remounted_boundaries: Vec::new(),
            errors: Vec::new(),
            reload_triggered: true,
        })
    }

    /// Load a package into the runtime
    pub fn load_package(&mut self, package: NyxPackage) -> Result<(), SessionError> {
        let package_id = package.entry_module.clone();

        self.module_loader.load_package(package.clone());
        self.load_package_into_vm(&package)?;

        self.loaded_packages.insert(package_id, package);
        self.module_instances.clear();
        Ok(())
    }

    /// Instantiate a module and get its exports
    pub fn instantiate_module(&mut self, module_id: &str) -> Result<ModuleInstance, SessionError> {
        if let Some(instance) = self.module_instances.get(module_id) {
            return Ok(instance.clone());
        }

        // Get the module's exports (in practice, would inspect module exports)
        let exports = BTreeMap::new();
        
        let instance = ModuleInstance {
            handle: ModuleHandle(self.module_instances.len()),
            module_id: module_id.to_string(),
            exports,
        };
        
        self.module_instances.insert(module_id.to_string(), instance.clone());
        Ok(instance)
    }

    /// Invoke a function in the runtime
    pub fn invoke(&mut self, entry_symbol: &str, args: Vec<Value>) -> Result<Value, SessionError> {
        let mut vm = self.vm.lock().map_err(|_| SessionError::new("VM lock poisoned"))?;
        
        // Try to call as function first
        if vm.has_function(entry_symbol) {
            return vm.call_function(entry_symbol, args)
                .map_err(|e| SessionError::from(e));
        }
        
        // Try as component render
        Err(SessionError::new(format!("Symbol '{}' not found", entry_symbol)))
    }

    /// Patch modules with changes (hot reload)
    pub fn patch_modules(&mut self, changed_modules: Vec<ModulePatch>) -> Result<PatchReport, SessionError> {
        if changed_modules.is_empty() {
            return Ok(PatchReport {
                patched_modules: Vec::new(),
                remounted_boundaries: Vec::new(),
                errors: Vec::new(),
                reload_triggered: false,
            });
        }

        // Full reload to avoid stale symbols in `NyxVm` when functions are removed/renamed.
        // This is the correct default until the VM tracks per-module symbol ownership.
        let patched_modules = changed_modules.iter().map(|p| p.module_id.clone()).collect();
        match self.reload_entry_package() {
            Ok(mut report) => {
                report.patched_modules = patched_modules;
                Ok(report)
            }
            Err(e) => Ok(PatchReport {
                patched_modules: Vec::new(),
                remounted_boundaries: Vec::new(),
                errors: vec![e.message],
                reload_triggered: true,
            }),
        }
    }

    /// Take a snapshot of the current runtime state
    pub fn snapshot_reload_state(&mut self) -> Result<ReloadSnapshot, SessionError> {
        let vm = self.vm.lock().map_err(|_| SessionError::new("VM lock poisoned"))?;
        
        // Snapshot globals that can be restored after reload
        let globals: BTreeMap<String, Value> = vm.globals.clone().into_iter().collect();
        let module_versions = self
            .module_loader
            .modules()
            .map(|module| (module.id.clone(), module.version))
            .collect();
        
        Ok(ReloadSnapshot {
            runtime: RuntimeStateSnapshot::default(),
            module_versions,
            globals,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        })
    }

    /// Restore runtime state from a snapshot
    pub fn restore_reload_state(&mut self, snapshot: ReloadSnapshot) -> Result<(), SessionError> {
        let mut vm = self.vm.lock().map_err(|_| SessionError::new("VM lock poisoned"))?;
        
        // Restore global values that still exist
        for (key, value) in snapshot.globals {
            if vm.globals.contains_key(&key) {
                vm.globals.insert(key, value);
            }
        }
        
        Ok(())
    }

    /// Get the underlying VM for direct access (wrapping)
    pub fn vm(&self) -> Arc<Mutex<NyxVm>> {
        self.vm.clone()
    }

    /// Get config
    pub fn config(&self) -> &SessionConfig {
        &self.config
    }

    /// Render a route and get the result
    pub fn render_route(&mut self, path: &str) -> Result<Value, SessionError> {
        let mut vm = self.vm.lock().map_err(|_| SessionError::new("VM lock poisoned"))?;
        vm.render_http_via_routes_or_app(path)
            .map_err(|e| SessionError::from(e))
    }

    /// Render app fragment for hot reload
    pub fn render_fragment(&mut self) -> Result<String, SessionError> {
        let mut vm = self.vm.lock().map_err(|_| SessionError::new("VM lock poisoned"))?;
        vm.render_app_fragment()
            .map_err(|e| SessionError::from(e))
    }
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            entry_file: PathBuf::from("main.nyx"),
            engine_root: PathBuf::from("engines/ui_engine"),
            runtime_name: "default".to_string(),
        }
    }
}
