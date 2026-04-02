use crate::runtime::execution::nyx_vm::{Value as AstValue};
use crate::runtime::execution::bytecode_compiler::BytecodeCompiler;
use nyx_vm::bytecode::{Value as VmValue};
use nyx_vm::runtime::{NyxVm as Vm, Frame};
use nyx_vm::VmConfig;
use std::collections::HashMap;

/// Tier 2 Aero-JIT Bridge.
/// Lower-level executor that synchronizes AST state with native machine code.
pub struct AeroJit;

impl AeroJit {
    /// Compiles and executes a loop fragment using the Cranelift JIT engine.
    /// Synchronizes external local variables before and after execution.
    pub fn execute_loop(
        condition: &crate::core::ast::ast_nodes::Expr,
        body: &[crate::core::ast::ast_nodes::Stmt],
        ast_locals: &mut HashMap<String, AstValue>,
    ) -> Result<(), String> {
        // 1. Identify and map relevant local variables to VM registers
        let mut name_to_idx = HashMap::new();
        let mut idx_to_name = HashMap::new();
        let mut initial_vms = Vec::new();
        
        // Ensure deterministic order for stack mapping
        let mut sorted_keys: Vec<_> = ast_locals.keys().collect();
        sorted_keys.sort();

        for name in sorted_keys {
            let val = ast_locals.get(name).expect("Local var not found");
            let idx = name_to_idx.len();
            name_to_idx.insert(name.clone(), idx);
            idx_to_name.insert(idx, name.clone());
            initial_vms.push(Self::ast_to_vm(val)?);
        }

        let num_locals = name_to_idx.len();

        // 2. Compile loop to Bytecode
        let compiler = BytecodeCompiler::new();
        let module = compiler.compile_loop_fragment(condition, body, &name_to_idx)?;

        // 3. Setup VM Runtime with JIT enabled
        let mut config = VmConfig::default();
        config.enable_jit = true;
        let mut vm = Vm::new(config);
        vm.load(module.clone());

        // 4. Force compilation and direct invocation
        let func = module.functions.first().ok_or("Failed to find JIT function")?;
        
        {
            let runtime = vm.runtime_mut();
            for val in initial_vms {
                runtime.stack.push(val);
            }
            let frame = Frame {
                module_name: "main".to_string(),
                function_idx: 0,
                function: func.clone(),
                ip: 0,
                stack_base: 0,
                num_locals,
            };
            runtime.frames.push(frame);
        }

        // --- THE MAGIC: Direct JIT Invocation ---
        #[cfg(feature = "jit")]
        {
            let runtime_ptr = vm.runtime_mut() as *mut nyx_vm::runtime::VmRuntime;
            if let Some(jit_engine) = vm.runtime_mut().jit_engine.as_mut() {
                println!("Aero-JIT: Compiling Loop Fragment...");
                let jit_res = jit_engine.compile_vm("main", 0, func)
                    .map_err(|e| format!("Cranelift Compilation Failed: {}", e))?;
                
                println!("Aero-JIT: Entering Native execution...");
                let native_fn: unsafe extern "C" fn(*mut nyx_vm::runtime::VmRuntime, i32) -> i64 = 
                    unsafe { std::mem::transmute(jit_res.func_ptr) };
                
                let res = unsafe { native_fn(runtime_ptr, 0) };
                println!("Aero-JIT: Native execution finished with code {}.", res);
                if res == -1 {
                    return Err("JIT Native Trap or Execution Error".to_string());
                }
            } else {
                println!("Aero-JIT: Falling back to Bytecode VM (JIT engine missing)...");
                vm.run_function("main", 0, Vec::new()).map_err(|e| format!("{:?}", e))?;
            }
        }
        #[cfg(not(feature = "jit"))]
        {
            vm.run_function("main", 0, Vec::new()).map_err(|e| format!("{:?}", e))?;
        }

        // 6. Synchronize state back to AST interpreter
        {
            let runtime = vm.runtime();
            // Important: We must use the base of the stack where our locals are stored
            let final_locals = &runtime.stack[0..num_locals];
            for (idx, val) in final_locals.iter().enumerate() {
                if let Some(name) = idx_to_name.get(&idx) {
                    ast_locals.insert(name.clone(), Self::vm_to_ast(val)?);
                }
            }
        }

        Ok(())
    }

    fn ast_to_vm(val: &AstValue) -> Result<VmValue, String> {
        match val {
            AstValue::Int(i) => Ok(VmValue::Int(*i)),
            AstValue::Float(f) => Ok(VmValue::Float(*f)),
            AstValue::Bool(b) => Ok(VmValue::Bool(*b)),
            AstValue::Str(s) => Ok(VmValue::String(s.clone())),
            AstValue::Null => Ok(VmValue::Null),
            _ => Err(format!("Unsupported sync type for Aero-JIT: {:?}", val)),
        }
    }

    fn vm_to_ast(val: &VmValue) -> Result<AstValue, String> {
        match val {
            VmValue::Int(i) => Ok(AstValue::Int(*i)),
            VmValue::Float(f) => Ok(AstValue::Float(*f)),
            VmValue::Bool(b) => Ok(AstValue::Bool(*b)),
            VmValue::String(s) => Ok(AstValue::Str(s.clone())),
            VmValue::Null => Ok(AstValue::Null),
            _ => Err(format!("Unsupported return type from Aero-JIT: {:?}", val)),
        }
    }
}
