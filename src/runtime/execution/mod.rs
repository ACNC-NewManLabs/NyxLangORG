use std::collections::BTreeMap;
use std::path::PathBuf;

pub mod aero_jit;
pub mod agent_bridge;
pub mod bytecode_compiler;
pub mod bytecode_vm;
pub mod concurrent_bridge;
pub mod config_bridge;
pub mod df_engine;
pub mod df_kernels;
pub mod dist_bridge;
pub mod driver_mock;
pub mod gpu_bridge;
pub mod jit_compiler;
pub mod kernel_compiler;
pub mod loop_optimizer;
pub mod module_loader;
pub mod native_bridge;
pub mod net_bridge;
pub mod nyx_server;
pub mod nyx_shell_client;
pub mod nyx_table_writer;
pub mod nyx_vm;
pub mod optimizer;
pub mod reload;
pub mod session;
pub mod simd_kernels;
pub mod sql_planner;
pub mod stdlib_bridge;
pub mod transaction_context;
pub mod ui_runtime;
pub mod wal_engine;

pub use bytecode_vm::BytecodeRuntimeSession;
pub use module_loader::{ModuleHandle, ModuleLoader, NyxModule, NyxPackage};
pub use reload::{ModulePatch, PatchReport, ReloadSnapshot, RuntimeStateSnapshot};

pub use nyx_vm::{eval_repl_line, NyxVm, Value, VmConfig};
pub type RuntimeValue = Value;

#[derive(Debug, Clone)]
pub struct RuntimeError {
    pub message: String,
}

impl RuntimeError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for RuntimeError {}

impl From<nyx_vm::EvalError> for RuntimeError {
    fn from(value: nyx_vm::EvalError) -> Self {
        Self {
            message: nyx_vm::format_eval_error(&value),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModuleInstance {
    pub handle: ModuleHandle,
    pub module_id: String,
}

pub trait RuntimeSession {
    fn load_package(&mut self, package: NyxPackage) -> Result<(), RuntimeError>;
    fn instantiate_module(&mut self, module_id: &str) -> Result<ModuleInstance, RuntimeError>;
    fn invoke(
        &mut self,
        entry_symbol: &str,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, RuntimeError>;
    fn patch_modules(
        &mut self,
        changed_modules: Vec<ModulePatch>,
    ) -> Result<PatchReport, RuntimeError>;
    fn snapshot_reload_state(&mut self) -> Result<ReloadSnapshot, RuntimeError>;
    fn restore_reload_state(&mut self, snapshot: ReloadSnapshot) -> Result<(), RuntimeError>;
}

#[derive(Debug, Clone)]
pub struct RuntimeSessionConfig {
    pub entry_file: PathBuf,
    pub engine_root: PathBuf,
    pub runtime_name: String,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeMetadata {
    pub module_versions: BTreeMap<String, u64>,
}
