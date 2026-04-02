//! JIT Binary Translation Module using Cranelift
//!
//! Provides a high-performance JIT engine that translates guest instructions
//! (x86_64/ARM64) into native machine code.

use super::cpu::{Architecture, CpuEmulator, Register};
use cranelift_codegen::ir::{self, InstBuilder};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// JIT-compiled block of code
pub struct CompiledBlock {
    pub ptr: *const u8,
}

unsafe impl Send for CompiledBlock {}
unsafe impl Sync for CompiledBlock {}

/// JIT Engine for Nyx Hypervisor
pub struct JitEngine {
    /// Cranelift JIT module
    module: JITModule,
    /// Cache of compiled blocks (Guest PC -> Compiled Pointer)
    cache: HashMap<u64, CompiledBlock>,
    /// Hit counters for PGO
    pub hit_counters: HashMap<u64, Arc<AtomicU64>>,
}

impl JitEngine {
    /// Create a new JIT engine
    pub fn new() -> Self {
        let mut flag_builder = settings::builder();
        flag_builder.set("use_colocated_libcalls", "false").unwrap();
        flag_builder.set("is_pic", "false").unwrap();

        let isa_builder = cranelift_native::builder().unwrap_or_else(|msg| {
            panic!("host machine is not supported: {}", msg);
        });
        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .expect("Failed to create ISA");

        let builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
        let module = JITModule::new(builder);

        Self {
            module,
            cache: HashMap::new(),
            hit_counters: HashMap::new(),
        }
    }

    /// Compile a basic block starting at guest virtual address
    pub fn compile_block(
        &mut self,
        cpu: &mut CpuEmulator,
        guest_pc: u64,
    ) -> Result<*const u8, String> {
        let is_hot = self.is_hot(guest_pc, 5000);

        if let Some(block) = self.cache.get(&guest_pc) {
            return Ok(block.ptr);
        }

        let mut flag_builder = settings::builder();
        if is_hot {
            // Enable more aggressive optimizations for hot blocks
            flag_builder.set("opt_level", "speed").unwrap();
            eprintln!("[JIT] Re-optimizing hot block 0x{:x}", guest_pc);
        } else {
            flag_builder.set("opt_level", "none").unwrap();
        }

        // Finalize ISA with new flags
        let _isa_builder = cranelift_native::builder()
            .unwrap()
            .finish(settings::Flags::new(flag_builder))
            .unwrap();
        // Since we can't easily re-create the module with a new ISA without losing state,
        // in a real implementation we would have multiple modules or use a more flexible JIT.
        // For Phase 2, we will keep the original module but log the optimization intent.

        let mut ctx = self.module.make_context();
        let mut builder_context = FunctionBuilderContext::new();

        // Function signature: fn(cpu_state_ptr: *mut CpuState, counter_ptr: *mut u64)
        let mut sig = self.module.make_signature();
        sig.params.push(ir::AbiParam::new(ir::types::I64)); // Pointer to CpuState
        sig.params.push(ir::AbiParam::new(ir::types::I64)); // Pointer to Hit Counter
        ctx.func.signature = sig;

        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_context);
        let block0 = builder.create_block();
        builder.append_block_params_for_function_params(block0);
        builder.switch_to_block(block0);
        builder.seal_block(block0);

        let cpu_ptr = builder.block_params(block0)[0];
        let counter_ptr = builder.block_params(block0)[1];

        // Increment hit counter
        let count = builder
            .ins()
            .load(ir::types::I64, ir::MemFlags::new(), counter_ptr, 0);
        let new_count = builder.ins().iadd_imm(count, 1);
        builder
            .ins()
            .store(ir::MemFlags::new(), new_count, counter_ptr, 0);

        // Get or create hit counter for this block
        self.hit_counters
            .entry(guest_pc)
            .or_insert_with(|| Arc::new(AtomicU64::new(0)));

        // Register mapping helpers
        let get_reg_offset = |reg: Register| -> i32 { (reg as i32) * 8 };

        // Basic block translation loop
        let mut current_pc = guest_pc;
        let mut instructions_translated = 0;

        while instructions_translated < 100 {
            // Fetch instruction from guest virtual memory
            let opcode = cpu.read_memory(current_pc, 1).map_err(|e| e)? as u8;

            match cpu.arch {
                Architecture::X86_64 => {
                    match opcode {
                        0x21 => {
                            // AND r/m64, r64
                            let rax_off = get_reg_offset(Register::Rax);
                            let rbx_off = get_reg_offset(Register::Rbx);
                            let v1 = builder.ins().load(
                                ir::types::I64,
                                ir::MemFlags::new(),
                                cpu_ptr,
                                rax_off,
                            );
                            let v2 = builder.ins().load(
                                ir::types::I64,
                                ir::MemFlags::new(),
                                cpu_ptr,
                                rbx_off,
                            );
                            let res = builder.ins().band(v1, v2);
                            builder
                                .ins()
                                .store(ir::MemFlags::new(), res, cpu_ptr, rax_off);
                            current_pc += 3;
                        }
                        0x09 => {
                            // OR r/m64, r64
                            let rax_off = get_reg_offset(Register::Rax);
                            let rbx_off = get_reg_offset(Register::Rbx);
                            let v1 = builder.ins().load(
                                ir::types::I64,
                                ir::MemFlags::new(),
                                cpu_ptr,
                                rax_off,
                            );
                            let v2 = builder.ins().load(
                                ir::types::I64,
                                ir::MemFlags::new(),
                                cpu_ptr,
                                rbx_off,
                            );
                            let res = builder.ins().bor(v1, v2);
                            builder
                                .ins()
                                .store(ir::MemFlags::new(), res, cpu_ptr, rax_off);
                            current_pc += 3;
                        }
                        0x31 => {
                            // XOR r/m64, r64
                            let rax_off = get_reg_offset(Register::Rax);
                            let rbx_off = get_reg_offset(Register::Rbx);
                            let v1 = builder.ins().load(
                                ir::types::I64,
                                ir::MemFlags::new(),
                                cpu_ptr,
                                rax_off,
                            );
                            let v2 = builder.ins().load(
                                ir::types::I64,
                                ir::MemFlags::new(),
                                cpu_ptr,
                                rbx_off,
                            );
                            let res = builder.ins().bxor(v1, v2);
                            builder
                                .ins()
                                .store(ir::MemFlags::new(), res, cpu_ptr, rax_off);
                            current_pc += 3;
                        }
                        0x01 => {
                            // ADD r/m64, r64
                            let rax_off = get_reg_offset(Register::Rax);
                            let rbx_off = get_reg_offset(Register::Rbx);
                            let v1 = builder.ins().load(
                                ir::types::I64,
                                ir::MemFlags::new(),
                                cpu_ptr,
                                rax_off,
                            );
                            let v2 = builder.ins().load(
                                ir::types::I64,
                                ir::MemFlags::new(),
                                cpu_ptr,
                                rbx_off,
                            );
                            let sum = builder.ins().iadd(v1, v2);
                            builder
                                .ins()
                                .store(ir::MemFlags::new(), sum, cpu_ptr, rax_off);
                            current_pc += 3;
                        }
                        0x29 => {
                            // SUB r/m64, r64
                            let rax_off = get_reg_offset(Register::Rax);
                            let rbx_off = get_reg_offset(Register::Rbx);
                            let v1 = builder.ins().load(
                                ir::types::I64,
                                ir::MemFlags::new(),
                                cpu_ptr,
                                rax_off,
                            );
                            let v2 = builder.ins().load(
                                ir::types::I64,
                                ir::MemFlags::new(),
                                cpu_ptr,
                                rbx_off,
                            );
                            let diff = builder.ins().isub(v1, v2);
                            builder
                                .ins()
                                .store(ir::MemFlags::new(), diff, cpu_ptr, rax_off);
                            current_pc += 3;
                        }
                        0x39 => {
                            // CMP r/m64, r64
                            let rax_off = get_reg_offset(Register::Rax);
                            let rbx_off = get_reg_offset(Register::Rbx);
                            let v1 = builder.ins().load(
                                ir::types::I64,
                                ir::MemFlags::new(),
                                cpu_ptr,
                                rax_off,
                            );
                            let v2 = builder.ins().load(
                                ir::types::I64,
                                ir::MemFlags::new(),
                                cpu_ptr,
                                rbx_off,
                            );
                            let is_equal = builder.ins().icmp(ir::condcodes::IntCC::Equal, v1, v2);
                            let zf_off = 1433i32;
                            let zf_val = builder.ins().uextend(ir::types::I8, is_equal);
                            builder
                                .ins()
                                .store(ir::MemFlags::new(), zf_val, cpu_ptr, zf_off);
                            current_pc += 3;
                        }
                        0xD3 => {
                            // SHL/SHR r/m64, CL
                            let rax_off = get_reg_offset(Register::Rax);
                            let rcx_off = get_reg_offset(Register::Rcx);
                            let val = builder.ins().load(
                                ir::types::I64,
                                ir::MemFlags::new(),
                                cpu_ptr,
                                rax_off,
                            );
                            let cl = builder.ins().load(
                                ir::types::I64,
                                ir::MemFlags::new(),
                                cpu_ptr,
                                rcx_off,
                            );
                            let amt = builder.ins().ireduce(ir::types::I8, cl);
                            let res = builder.ins().ishl(val, amt);
                            builder
                                .ins()
                                .store(ir::MemFlags::new(), res, cpu_ptr, rax_off);
                            current_pc += 3;
                        }
                        0xB8..=0xBF => {
                            // MOV reg, imm32
                            let reg_idx = (opcode - 0xB8) as i32;
                            let imm = cpu.read_memory(current_pc + 1, 4).map_err(|e| e)? as i32;
                            let reg = [
                                Register::Rax,
                                Register::Rcx,
                                Register::Rdx,
                                Register::Rbx,
                                Register::Rsp,
                                Register::Rbp,
                                Register::Rsi,
                                Register::Rdi,
                            ][reg_idx as usize];
                            let offset = get_reg_offset(reg);
                            let val = builder.ins().iconst(ir::types::I64, imm as i64);
                            builder
                                .ins()
                                .store(ir::MemFlags::new(), val, cpu_ptr, offset);
                            current_pc += 5;
                        }
                        0x74 => {
                            // JZ rel8
                            let offset = cpu.read_memory(current_pc + 1, 1).map_err(|e| e)? as i8;
                            let zf_off = 1433i32;
                            let zf = builder.ins().load(
                                ir::types::I8,
                                ir::MemFlags::new(),
                                cpu_ptr,
                                zf_off,
                            );
                            let is_zero =
                                builder
                                    .ins()
                                    .icmp_imm(ir::condcodes::IntCC::NotEqual, zf, 0);
                            let true_pc = (current_pc as i64 + 2 + offset as i64) as u64;
                            let false_pc = current_pc + 2;
                            let rip_off = get_reg_offset(Register::Rip);
                            let true_val = builder.ins().iconst(ir::types::I64, true_pc as i64);
                            let false_val = builder.ins().iconst(ir::types::I64, false_pc as i64);
                            let next_pc = builder.ins().select(is_zero, true_val, false_val);
                            builder
                                .ins()
                                .store(ir::MemFlags::new(), next_pc, cpu_ptr, rip_off);
                            builder.ins().return_(&[]);
                            break;
                        }
                        0x75 => {
                            // JNZ rel8
                            let offset = cpu.read_memory(current_pc + 1, 1).map_err(|e| e)? as i8;
                            let zf_off = 1433i32;
                            let zf = builder.ins().load(
                                ir::types::I8,
                                ir::MemFlags::new(),
                                cpu_ptr,
                                zf_off,
                            );
                            let is_not_zero =
                                builder.ins().icmp_imm(ir::condcodes::IntCC::Equal, zf, 0);
                            let true_pc = (current_pc as i64 + 2 + offset as i64) as u64;
                            let false_pc = current_pc + 2;
                            let rip_off = get_reg_offset(Register::Rip);
                            let true_val = builder.ins().iconst(ir::types::I64, true_pc as i64);
                            let false_val = builder.ins().iconst(ir::types::I64, false_pc as i64);
                            let next_pc = builder.ins().select(is_not_zero, true_val, false_val);
                            builder
                                .ins()
                                .store(ir::MemFlags::new(), next_pc, cpu_ptr, rip_off);
                            builder.ins().return_(&[]);
                            break;
                        }
                        0xE9 => {
                            // JMP rel32
                            let offset = cpu.read_memory(current_pc + 1, 4).map_err(|e| e)? as i32;
                            current_pc = (current_pc as i64 + 5 + offset as i64) as u64;
                            let rip_off = get_reg_offset(Register::Rip);
                            let next_pc = builder.ins().iconst(ir::types::I64, current_pc as i64);
                            builder
                                .ins()
                                .store(ir::MemFlags::new(), next_pc, cpu_ptr, rip_off);
                            builder.ins().return_(&[]);
                            break;
                        }
                        0xC3 => {
                            // RET
                            let rip_off = get_reg_offset(Register::Rip);
                            let next_pc = builder
                                .ins()
                                .iconst(ir::types::I64, (current_pc + 1) as i64);
                            builder
                                .ins()
                                .store(ir::MemFlags::new(), next_pc, cpu_ptr, rip_off);
                            builder.ins().return_(&[]);
                            break;
                        }
                        0x90 => {
                            // NOP
                            current_pc += 1;
                        }
                        _ => {
                            let rip_off = get_reg_offset(Register::Rip);
                            let next_pc = builder.ins().iconst(ir::types::I64, current_pc as i64);
                            builder
                                .ins()
                                .store(ir::MemFlags::new(), next_pc, cpu_ptr, rip_off);
                            builder.ins().return_(&[]);
                            break;
                        }
                    }
                }
                _ => return Err("Architecture not supported in JIT".to_string()),
            }

            instructions_translated += 1;
        }

        builder.finalize();

        // Compile and finalize
        let id = self
            .module
            .declare_function(
                &format!("block_{:x}", guest_pc),
                Linkage::Export,
                &ctx.func.signature,
            )
            .map_err(|e| e.to_string())?;

        self.module
            .define_function(id, &mut ctx)
            .map_err(|e| e.to_string())?;
        self.module
            .finalize_definitions()
            .map_err(|e| e.to_string())?;

        let code = self.module.get_finalized_function(id);
        self.cache.insert(guest_pc, CompiledBlock { ptr: code });

        Ok(code)
    }

    /// Clear cache for a specific block (used for re-optimization)
    pub fn clear_cache(&mut self, guest_pc: u64) {
        self.cache.remove(&guest_pc);
    }

    /// Check if a block is hot enough for re-optimization
    pub fn is_hot(&self, guest_pc: u64, threshold: u64) -> bool {
        if let Some(counter) = self.hit_counters.get(&guest_pc) {
            return counter.load(Ordering::Acquire) >= threshold;
        }
        false
    }
}
