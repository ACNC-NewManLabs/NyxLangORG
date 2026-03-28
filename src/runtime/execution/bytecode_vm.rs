use std::collections::BTreeMap;

use super::module_loader::{ModuleHandle, ModuleLoader, NyxPackage};
use super::reload::{ModulePatch, PatchReport, ReloadSnapshot, RuntimeStateSnapshot};
use super::{ModuleInstance, RuntimeError, RuntimeSession, RuntimeValue};
use crate::applications::compiler::compiler_main::Compiler;
use crate::runtime::execution::nyx_vm::Value;

use nyx_vm::runtime::NyxVm;
use nyx_vm::VmConfig;
use nyx_vm::bytecode::Value as VmValue;

pub struct BytecodeRuntimeSession {
    vm: NyxVm,
    loader: ModuleLoader,
    instances: BTreeMap<String, ModuleHandle>,
    next_handle: usize,
    snapshot: RuntimeStateSnapshot,
}

impl BytecodeRuntimeSession {
    pub fn new() -> Self {
        let mut config = VmConfig::default();
        config.enable_jit = true;
        let mut vm = NyxVm::new(config);

        // Register standard natives
        vm.register("print", 1, print_native);
        vm.register("__native_time_ms", 0, time_now_ms_native);

        Self {
            vm,
            loader: ModuleLoader::new(),
            instances: BTreeMap::new(),
            next_handle: 1,
            snapshot: RuntimeStateSnapshot::default(),
        }
    }

    fn lower_package_to_vm(&mut self, pkg: NyxPackage) -> Result<(), RuntimeError> {
        let entry_path = pkg
            .modules
            .iter()
            .find(|m| m.id == pkg.entry_module)
            .map(|m| &m.path)
            .ok_or_else(|| {
                RuntimeError::new(format!("Entry module {} not found", pkg.entry_module))
            })?;

        let mut compiler =
            Compiler::from_registry_files("registry/language.json", "registry/engines.json")
                .map_err(|e| RuntimeError::new(format!("Compiler init error: {}", e)))?;

        let bc_module = compiler
            .compile_to_bytecode(entry_path)
            .map_err(|e| RuntimeError::new(format!("Compilation error: {}", e)))?;

        self.vm.load(bc_module);
        Ok(())
    }
}

impl RuntimeSession for BytecodeRuntimeSession {
    fn load_package(&mut self, pkg: NyxPackage) -> Result<(), RuntimeError> {
        self.loader.load_package(pkg.clone());
        self.lower_package_to_vm(pkg)?;
        self.instances.clear();
        self.next_handle = 1;
        Ok(())
    }

    fn instantiate_module(&mut self, module_id: &str) -> Result<ModuleInstance, RuntimeError> {
        self.loader
            .get(module_id)
            .ok_or_else(|| RuntimeError::new(format!("unknown module '{module_id}'")))?;
        let handle = ModuleHandle(self.next_handle);
        self.next_handle += 1;
        self.instances.insert(module_id.to_string(), handle);
        Ok(ModuleInstance {
            handle,
            module_id: module_id.to_string(),
        })
    }

    fn invoke(&mut self, _entry_symbol: &str, args: Vec<RuntimeValue>) -> Result<RuntimeValue, RuntimeError> {
        let _vm_args: Vec<VmValue> = args.into_iter().map(rt_to_vm_value).collect();
        
        // Find function index for entry_symbol
        // For now, let's assume entry_symbol is 'main' or similar and use run()
        // Or find it in the module.
        
        // Let's assume we want to run the function named entry_symbol in module "main"
        let res = self.vm.run("main") // This should be updated to run specific function if needed
            .map_err(|e| RuntimeError::new(e.to_string()))?;
            
        Ok(vm_to_rt_value(res))
    }

    fn patch_modules(&mut self, changed_modules: Vec<ModulePatch>) -> Result<PatchReport, RuntimeError> {
        let mut report = PatchReport::default();
        for patch in changed_modules {
            report.patched_modules.push(patch.module_id.clone());
            self.loader.patch(patch.next);
            // Hot reload is not yet implemented for the bytecode VM.
        }
        Ok(report)
    }

    fn snapshot_reload_state(&mut self) -> Result<ReloadSnapshot, RuntimeError> {
        Ok(ReloadSnapshot {
            runtime: self.snapshot.clone(),
            module_versions: self
                .loader
                .modules()
                .map(|module| (module.id.clone(), module.version))
                .collect(),
            globals: BTreeMap::new(),
            timestamp: 0,
        })
    }

    fn restore_reload_state(&mut self, snapshot: ReloadSnapshot) -> Result<(), RuntimeError> {
        self.snapshot = snapshot.runtime;
        Ok(())
    }
}

fn rt_to_vm_value(val: RuntimeValue) -> VmValue {
    match val {
        RuntimeValue::Null => VmValue::Null,
        RuntimeValue::Int(i) => VmValue::Int(i),
        RuntimeValue::Float(f) => VmValue::Float(f),
        RuntimeValue::Bool(b) => VmValue::Bool(b),
        RuntimeValue::Str(s) => VmValue::String(s),
        RuntimeValue::Array(arr_rc) => {
            let vm_arr = arr_rc.read().unwrap().iter().map(|v| rt_to_vm_value(v.clone())).collect();
            VmValue::Array(vm_arr)
        }
        RuntimeValue::Object(obj_rc) => {
            let vm_obj = obj_rc.read().unwrap().iter().map(|(k, v)| (k.clone(), rt_to_vm_value(v.clone()))).collect();
            VmValue::Object(vm_obj)
        }
        _ => VmValue::Null,
    }
}

fn vm_to_rt_value(val: VmValue) -> RuntimeValue {
    match val {
        VmValue::Null => RuntimeValue::Null,
        VmValue::Bool(b) => RuntimeValue::Bool(b),
        VmValue::Int(i) => RuntimeValue::Int(i),
        VmValue::Float(f) => RuntimeValue::Float(f),
        VmValue::String(s) => RuntimeValue::Str(s),
        VmValue::Array(arr) => {
            Value::array(arr.into_iter().map(vm_to_rt_value).collect())
        }
        VmValue::Object(obj) => {
            Value::object(obj.into_iter().map(|(k, v)| (k, vm_to_rt_value(v))).collect())
        }
        _ => RuntimeValue::Null,
    }
}

fn print_native(args: &[VmValue]) -> Result<VmValue, String> {
    if let Some(arg) = args.first() {
        println!("{}", format_vm_value(arg));
    }
    Ok(VmValue::Null)
}

fn time_now_ms_native(_args: &[VmValue]) -> Result<VmValue, String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis();
    Ok(VmValue::Int(now as i64))
}

fn format_vm_value(val: &VmValue) -> String {
    match val {
        VmValue::Null => "null".to_string(),
        VmValue::Bool(b) => b.to_string(),
        VmValue::Int(i) => i.to_string(),
        VmValue::Float(f) => f.to_string(),
        VmValue::String(s) => s.clone(),
        VmValue::Array(arr) => {
            let parts: Vec<String> = arr.iter().map(format_vm_value).collect();
            format!("[{}]", parts.join(", "))
        }
        VmValue::Object(obj) => {
            let parts: Vec<String> = obj
                .iter()
                .map(|(k, v)| format!("{}: {}", k, format_vm_value(v)))
                .collect();
            format!("{{ {} }}", parts.join(", "))
        }
        _ => "unknown".to_string(),
    }
}
