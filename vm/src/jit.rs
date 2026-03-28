//! Cranelift-based JIT for Nyx VM

use crate::bytecode::Function;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JitNumKind {
    I64,
    F64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JitRetKind {
    I64,
    F64,
    Bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JitPlan {
    pub num_kind: JitNumKind,
    pub ret_kind: JitRetKind,
}

#[derive(Debug, Clone)]
pub struct JitResult {
    pub func_ptr: *const u8,
    pub arity: usize,
    pub plan: JitPlan,
}

/// VM-aware JIT compiled function (operates on `VmRuntime` state via runtime stubs).
#[derive(Debug, Clone)]
pub struct VmJitResult {
    pub func_ptr: *const u8,
    pub attr_ic_buffer: Option<std::sync::Arc<Vec<crate::runtime::AttributeIC>>>,
    pub index_ic_buffer: Option<std::sync::Arc<Vec<crate::runtime::IndexIC>>>,
}

#[cfg(feature = "jit")]
mod cranelift_impl {
    use super::*;
    use std::collections::HashMap;

    use crate::bytecode::{OpCode, Value};
    use cranelift_codegen::ir::condcodes::IntCC;
    use cranelift_codegen::ir::{types, AbiParam, Function as ClifFunction, InstBuilder, MemFlags, StackSlotData, StackSlotKind};
    use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
    use cranelift_jit::{JITBuilder, JITModule};
    use cranelift_module::{Linkage, Module};
    use cranelift_native;

    pub struct JitEngine {
        module: JITModule,
        functions: HashMap<(String, usize, JitPlan), JitResult>,
        pub(crate) vm_functions: HashMap<(String, usize), VmJitResult>,
        imports: HashMap<String, cranelift_module::FuncId>,
    }

    impl JitEngine {
        pub fn new() -> Result<Self, String> {
            let isa = cranelift_native::builder()
                .map_err(|e| e.to_string())?
                .finish(cranelift_codegen::settings::Flags::new(
                    cranelift_codegen::settings::builder(),
                ))
                .map_err(|e| e.to_string())?;
            let mut builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
            // Register VM-aware runtime stubs.
            builder.symbol("nyx_jit_tick", crate::runtime::nyx_jit_tick as *const u8);
            builder.symbol("nyx_jit_pop_truthy", crate::runtime::nyx_jit_pop_truthy as *const u8);
            builder.symbol("nyx_jit_push_const", crate::runtime::nyx_jit_push_const as *const u8);
            builder.symbol("nyx_jit_pushm", crate::runtime::nyx_jit_pushm as *const u8);
            builder.symbol("nyx_jit_pop", crate::runtime::nyx_jit_pop as *const u8);
            builder.symbol("nyx_jit_dup", crate::runtime::nyx_jit_dup as *const u8);
            builder.symbol("nyx_jit_swap", crate::runtime::nyx_jit_swap as *const u8);
            builder.symbol("nyx_jit_load", crate::runtime::nyx_jit_load as *const u8);
            builder.symbol("nyx_jit_store", crate::runtime::nyx_jit_store as *const u8);
            builder.symbol("nyx_jit_add", crate::runtime::nyx_jit_add as *const u8);
            builder.symbol("nyx_jit_sub", crate::runtime::nyx_jit_sub as *const u8);
            builder.symbol("nyx_jit_mul", crate::runtime::nyx_jit_mul as *const u8);
            builder.symbol("nyx_jit_div", crate::runtime::nyx_jit_div as *const u8);
            builder.symbol("nyx_jit_neg", crate::runtime::nyx_jit_neg as *const u8);
            builder.symbol("nyx_jit_eq", crate::runtime::nyx_jit_eq as *const u8);
            builder.symbol("nyx_jit_ne", crate::runtime::nyx_jit_ne as *const u8);
            builder.symbol("nyx_jit_lt", crate::runtime::nyx_jit_lt as *const u8);
            builder.symbol("nyx_jit_gt", crate::runtime::nyx_jit_gt as *const u8);
            builder.symbol("nyx_jit_le", crate::runtime::nyx_jit_le as *const u8);
            builder.symbol("nyx_jit_ge", crate::runtime::nyx_jit_ge as *const u8);
            builder.symbol("nyx_jit_new_array", crate::runtime::nyx_jit_new_array as *const u8);
            builder.symbol("nyx_jit_new_obj", crate::runtime::nyx_jit_new_obj as *const u8);
            builder.symbol("nyx_jit_get_field", crate::runtime::nyx_jit_get_field as *const u8);
            builder.symbol("nyx_jit_get_field_cached", crate::runtime::nyx_jit_get_field_cached as *const u8);
            builder.symbol("nyx_jit_set_field", crate::runtime::nyx_jit_set_field as *const u8);
            builder.symbol("nyx_jit_set_field_cached", crate::runtime::nyx_jit_set_field_cached as *const u8);
            builder.symbol("nyx_jit_get_index", crate::runtime::nyx_jit_get_index as *const u8);
            builder.symbol("nyx_jit_get_index_cached", crate::runtime::nyx_jit_get_index_cached as *const u8);
            builder.symbol("nyx_jit_set_index", crate::runtime::nyx_jit_set_index as *const u8);
            builder.symbol("nyx_jit_set_index_cached", crate::runtime::nyx_jit_set_index_cached as *const u8);
            builder.symbol("nyx_jit_len", crate::runtime::nyx_jit_len as *const u8);
            builder.symbol("nyx_jit_slice", crate::runtime::nyx_jit_slice as *const u8);
            builder.symbol("nyx_jit_get_global", crate::runtime::nyx_jit_get_global as *const u8);
            builder.symbol("nyx_jit_set_global", crate::runtime::nyx_jit_set_global as *const u8);
            builder.symbol("nyx_jit_get_global_m", crate::runtime::nyx_jit_get_global_m as *const u8);
            builder.symbol("nyx_jit_set_global_m", crate::runtime::nyx_jit_set_global_m as *const u8);
            builder.symbol("nyx_jit_alloc", crate::runtime::nyx_jit_alloc as *const u8);
            builder.symbol("nyx_jit_free", crate::runtime::nyx_jit_free as *const u8);
            builder.symbol("nyx_jit_ret", crate::runtime::nyx_jit_ret as *const u8);
            builder.symbol("nyx_jit_stack_guard", crate::runtime::nyx_jit_stack_guard as *const u8);
            builder.symbol("nyx_jit_check_arity", crate::runtime::nyx_jit_check_arity as *const u8);
            builder.symbol("nyx_jit_halt", crate::runtime::nyx_jit_halt as *const u8);
            builder.symbol("nyx_jit_call", crate::runtime::nyx_jit_call as *const u8);
            builder.symbol("nyx_jit_call_ext", crate::runtime::nyx_jit_call_ext as *const u8);
            builder.symbol("nyx_jit_closure", crate::runtime::nyx_jit_closure as *const u8);
            builder.symbol("nyx_jit_closure_ref", crate::runtime::nyx_jit_closure_ref as *const u8);
            builder.symbol("nyx_jit_get_heap_info", crate::runtime::nyx_jit_get_heap_info as *const u8);
            builder.symbol("nyx_jit_get_stack_info", crate::runtime::nyx_jit_get_stack_info as *const u8);
            builder.symbol("nyx_jit_set_stack_len", crate::runtime::nyx_jit_set_stack_len as *const u8);
            builder.symbol("nyx_jit_pop_n", crate::runtime::nyx_jit_pop_n as *const u8);
            builder.symbol("nyx_jit_push_ic_value", crate::runtime::nyx_jit_push_ic_value as *const u8);
            builder.symbol("nyx_jit_push_index_ic_value", crate::runtime::nyx_jit_push_index_ic_value as *const u8);
            builder.symbol("nyx_jit_closure_ref_stack", crate::runtime::nyx_jit_closure_ref_stack as *const u8);
            builder.symbol("nyx_jit_set_ip", crate::runtime::nyx_jit_set_ip as *const u8);

            let module = JITModule::new(builder);
            Ok(Self {
                module,
                functions: HashMap::new(),
                vm_functions: HashMap::new(),
                imports: HashMap::new(),
            })
        }

        pub fn plan(func: &Function) -> Option<JitPlan> {
            // We now support direct calls up to 32 arguments.
            if func.arity > 32 {
                return None;
            }
            if func.arity > func.num_locals {
                return None;
            }
            if func.instructions.is_empty() {
                return None;
            }
            let mut ret_count = 0usize;
            for instr in &func.instructions {
                if instr.opcode == OpCode::RET {
                    ret_count += 1;
                }
            }
            if ret_count != 1 || func.instructions.last().map(|i| i.opcode) != Some(OpCode::RET) {
                return None;
            }

            let mut seen_int = false;
            let mut seen_float = false;
            for c in &func.constants {
                match c {
                    Value::Int(_) => seen_int = true,
                    Value::Float(_) => seen_float = true,
                    _ => return None,
                }
            }
            if seen_int && seen_float {
                return None;
            }
            let num_kind = if seen_float { JitNumKind::F64 } else { JitNumKind::I64 };

            // Conservative: infer return kind from straight-line stack simulation (single RET at end).
            #[derive(Clone, Copy)]
            enum Ty {
                Num,
                Bool,
            }
            let mut stack: Vec<Ty> = Vec::new();
            for instr in &func.instructions {
                match instr.opcode {
                    OpCode::NOP => {}
                    OpCode::PUSH | OpCode::LOAD => stack.push(Ty::Num),
                    OpCode::POP => {
                        stack.pop()?;
                    }
                    OpCode::DUP => {
                        let top = *stack.last()?;
                        stack.push(top);
                    }
                    OpCode::SWAP => {
                        if stack.len() < 2 {
                            return None;
                        }
                        let len = stack.len();
                        stack.swap(len - 1, len - 2);
                    }
                    OpCode::STORE => {
                        stack.pop()?;
                    }
                    OpCode::ADD | OpCode::SUB | OpCode::MUL | OpCode::DIV | OpCode::NEG => {
                        if instr.opcode == OpCode::NEG {
                            if !matches!(stack.pop()?, Ty::Num) {
                                return None;
                            }
                            stack.push(Ty::Num);
                        } else {
                            if !matches!(stack.pop()?, Ty::Num) || !matches!(stack.pop()?, Ty::Num) {
                                return None;
                            }
                            stack.push(Ty::Num);
                        }
                    }
                    OpCode::EQ | OpCode::NE | OpCode::LT | OpCode::GT | OpCode::LE | OpCode::GE => {
                        if !matches!(stack.pop()?, Ty::Num) || !matches!(stack.pop()?, Ty::Num) {
                            return None;
                        }
                        stack.push(Ty::Bool);
                    }
                    OpCode::JMP => {}
                    OpCode::JZ | OpCode::JNZ => {
                        stack.pop()?;
                    }
                    OpCode::RET => {
                        let ret = stack.pop().unwrap_or(Ty::Num);
                        let ret_kind = match (num_kind, ret) {
                            (_, Ty::Bool) => JitRetKind::Bool,
                            (JitNumKind::I64, Ty::Num) => JitRetKind::I64,
                            (JitNumKind::F64, Ty::Num) => JitRetKind::F64,
                        };
                        return Some(JitPlan { num_kind, ret_kind });
                    }
                    _ => return None,
                }
            }
            None
        }

        pub fn compile(&mut self, module_name: &str, func_idx: usize, func: &Function, plan: JitPlan) -> Result<JitResult, String> {
            if let Some(existing) = self.functions.get(&(module_name.to_string(), func_idx, plan)) {
                return Ok(existing.clone());
            }
            if func.arity > 32 {
                return Err("JIT only supports up to 32 arguments".to_string());
            }

            let mut sig = self.module.make_signature();
            for _ in 0..func.arity {
                sig.params.push(AbiParam::new(match plan.num_kind {
                    JitNumKind::I64 => types::I64,
                    JitNumKind::F64 => types::F64,
                }));
            }
            sig.returns.push(AbiParam::new(match plan.ret_kind {
                JitRetKind::I64 | JitRetKind::Bool => types::I64,
                JitRetKind::F64 => types::F64,
            }));

            let func_id = self
                .module
                .declare_function(
                    &format!("{}_jit_{}", module_name, func_idx),
                    Linkage::Local,
                    &sig,
                )
                .map_err(|e| e.to_string())?;

            let mut ctx = self.module.make_context();
            ctx.func = ClifFunction::with_name_signature(
                cranelift_codegen::ir::UserFuncName::user(0, func_idx as u32),
                sig,
            );
            let mut builder_ctx = FunctionBuilderContext::new();
            let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);
            let entry_block = builder.create_block();
            builder.append_block_params_for_function_params(entry_block);
            builder.switch_to_block(entry_block);

            let mut vars: Vec<Variable> = Vec::with_capacity(func.num_locals);
            for i in 0..func.num_locals {
                let var = Variable::from_u32(i as u32);
                vars.push(var);
                builder.declare_var(
                    var,
                    match plan.num_kind {
                        JitNumKind::I64 => types::I64,
                        JitNumKind::F64 => types::F64,
                    },
                );
            }
            for i in 0..func.arity {
                let val = builder.block_params(entry_block)[i];
                builder.def_var(vars[i], val);
            }
            for i in func.arity..func.num_locals {
                let zero = match plan.num_kind {
                    JitNumKind::I64 => builder.ins().iconst(types::I64, 0),
                    JitNumKind::F64 => builder.ins().f64const(0.0),
                };
                builder.def_var(vars[i], zero);
            }



            // Register-allocating stack: map stack slots to Variables.
            // We need a pre-pass to compute the stack depth at each instruction.
            let mut depths = vec![None; func.instructions.len() + 1];
            let mut stack_worklist = vec![(0, 0)]; // (ip, initial_depth)
            depths[0] = Some(0);
            
            while let Some((ip, depth)) = stack_worklist.pop() {
                if ip >= func.instructions.len() { continue; }
                let instr = &func.instructions[ip];
                let next_depth = match instr.opcode {
                    OpCode::PUSH | OpCode::LOAD | OpCode::DUP | OpCode::ALLOC => depth + 1,
                    OpCode::POP | OpCode::STORE | OpCode::FREE => depth - 1,
                    OpCode::NEG => depth,
                    OpCode::JZ | OpCode::JNZ => depth - 1,
                    OpCode::ADD | OpCode::SUB | OpCode::MUL | OpCode::DIV | OpCode::MOD | OpCode::POW | OpCode::CMP | OpCode::EQ | OpCode::NE | OpCode::LT | OpCode::GT | OpCode::LE | OpCode::GE | OpCode::AND | OpCode::OR | OpCode::XOR | OpCode::BAND | OpCode::BOR | OpCode::BXOR | OpCode::SHL | OpCode::SHR | OpCode::USHR => depth - 1,
                    OpCode::RET | OpCode::HALT => 0,
                    OpCode::JMP => depth,
                    _ => depth,
                };
                
                match instr.opcode {
                    OpCode::JMP => {
                        let target = instr.operands[0] as usize;
                        if depths[target].is_none() {
                            depths[target] = Some(depth);
                            stack_worklist.push((target, depth));
                        }
                    }
                    OpCode::JZ | OpCode::JNZ => {
                        let target = instr.operands[0] as usize;
                        if depths[target].is_none() {
                            depths[target] = Some(depth - 1);
                            stack_worklist.push((target, depth - 1));
                        }
                        if depths[ip + 1].is_none() {
                            depths[ip + 1] = Some(depth - 1);
                            stack_worklist.push((ip + 1, depth - 1));
                        }
                    }
                    OpCode::RET | OpCode::HALT => {}
                    _ => {
                        if depths[ip + 1].is_none() {
                            depths[ip + 1] = Some(next_depth);
                            stack_worklist.push((ip + 1, next_depth));
                        }
                    }
                }
            }

            let max_depth = depths.iter().filter_map(|&d| d).max().unwrap_or(0);
            let mut stack_vars = Vec::with_capacity(max_depth as usize + 1);
            for i in 0..=max_depth {
                let var = Variable::from_u32((func.num_locals + i as usize) as u32);
                stack_vars.push(var);
                builder.declare_var(var, match plan.num_kind {
                    JitNumKind::I64 => types::I64,
                    JitNumKind::F64 => types::F64,
                });
            }

            let mut blocks = Vec::with_capacity(func.instructions.len() + 1);
            for _ in 0..=func.instructions.len() {
                blocks.push(builder.create_block());
            }

            builder.ins().jump(blocks[0], &[]);
            builder.seal_block(entry_block);

            for (ip, instr) in func.instructions.iter().enumerate() {
                builder.switch_to_block(blocks[ip]);
                builder.seal_block(blocks[ip]);

                let current_depth = std::cell::Cell::new(depths[ip].unwrap_or(0));

                let push = |builder: &mut FunctionBuilder, val: cranelift_codegen::ir::Value| {
                    builder.def_var(stack_vars[current_depth.get() as usize], val);
                    current_depth.set(current_depth.get() + 1);
                };
                let pop = |builder: &mut FunctionBuilder| -> Result<cranelift_codegen::ir::Value, String> {
                    if current_depth.get() <= 0 { return Err("Stack underflow in JIT pop".to_string()); }
                    current_depth.set(current_depth.get() - 1);
                    Ok(builder.use_var(stack_vars[current_depth.get() as usize]))
                };
                let peek = |builder: &mut FunctionBuilder, depth_off: i64| -> Result<cranelift_codegen::ir::Value, String> {
                    let idx = (current_depth.get() as i64) - 1 - depth_off;
                    if idx < 0 { return Err("Stack underflow in JIT peek".to_string()); }
                    Ok(builder.use_var(stack_vars[idx as usize]))
                };

                match instr.opcode {
                    OpCode::NOP => {}
                    OpCode::PUSH => {
                        let idx = instr.operands.get(0).copied().unwrap_or(0) as usize;
                        match (plan.num_kind, func.constants.get(idx)) {
                            (JitNumKind::I64, Some(Value::Int(i))) => {
                                let v = builder.ins().iconst(types::I64, *i);
                                push(&mut builder, v)
                            }
                            (JitNumKind::F64, Some(Value::Float(f))) => {
                                let v = builder.ins().f64const(*f);
                                push(&mut builder, v)
                            }
                            _ => return Err("Constant type mismatch for JIT".to_string()),
                        }
                    }
                    OpCode::LOAD => {
                        let idx = instr.operands.get(0).copied().unwrap_or(0) as usize;
                        let v = builder.use_var(vars[idx]);
                        push(&mut builder, v);
                    }
                    OpCode::STORE => {
                        let idx = instr.operands.get(0).copied().unwrap_or(0) as usize;
                        let v = pop(&mut builder)?;
                        builder.def_var(vars[idx], v);
                    }
                    OpCode::POP => {
                        let _ = pop(&mut builder)?;
                    }
                    OpCode::DUP => {
                        let v = peek(&mut builder, 0)?;
                        push(&mut builder, v);
                    }
                    OpCode::SWAP => {
                        let a = pop(&mut builder)?;
                        let b = pop(&mut builder)?;
                        push(&mut builder, a);
                        push(&mut builder, b);
                    }
                    OpCode::ADD | OpCode::SUB | OpCode::MUL | OpCode::DIV => {
                        let b = pop(&mut builder)?;
                        let a = pop(&mut builder)?;
                        let v = match (plan.num_kind, instr.opcode) {
                            (JitNumKind::I64, OpCode::ADD) => builder.ins().iadd(a, b),
                            (JitNumKind::I64, OpCode::SUB) => builder.ins().isub(a, b),
                            (JitNumKind::I64, OpCode::MUL) => builder.ins().imul(a, b),
                            (JitNumKind::I64, OpCode::DIV) => builder.ins().sdiv(a, b),
                            (JitNumKind::F64, OpCode::ADD) => builder.ins().fadd(a, b),
                            (JitNumKind::F64, OpCode::SUB) => builder.ins().fsub(a, b),
                            (JitNumKind::F64, OpCode::MUL) => builder.ins().fmul(a, b),
                            (JitNumKind::F64, OpCode::DIV) => builder.ins().fdiv(a, b),
                            _ => unreachable!(),
                        };
                        push(&mut builder, v);
                    }
                    OpCode::NEG => {
                        let a = pop(&mut builder)?;
                        let v = match plan.num_kind {
                            JitNumKind::I64 => builder.ins().ineg(a),
                            JitNumKind::F64 => builder.ins().fneg(a),
                        };
                        push(&mut builder, v);
                    }
                    OpCode::EQ
                    | OpCode::NE
                    | OpCode::LT
                    | OpCode::GT
                    | OpCode::LE
                    | OpCode::GE => {
                        let b = pop(&mut builder)?;
                        let a = pop(&mut builder)?;
                        let cond = match plan.num_kind {
                            JitNumKind::I64 => {
                                let cc = match instr.opcode {
                                    OpCode::EQ => IntCC::Equal,
                                    OpCode::NE => IntCC::NotEqual,
                                    OpCode::LT => IntCC::SignedLessThan,
                                    OpCode::GT => IntCC::SignedGreaterThan,
                                    OpCode::LE => IntCC::SignedLessThanOrEqual,
                                    OpCode::GE => IntCC::SignedGreaterThanOrEqual,
                                    _ => unreachable!(),
                                };
                                builder.ins().icmp(cc, a, b)
                            }
                            JitNumKind::F64 => {
                                let cc = match instr.opcode {
                                    OpCode::EQ => cranelift_codegen::ir::condcodes::FloatCC::Equal,
                                    OpCode::NE => cranelift_codegen::ir::condcodes::FloatCC::NotEqual,
                                    OpCode::LT => cranelift_codegen::ir::condcodes::FloatCC::LessThan,
                                    OpCode::GT => cranelift_codegen::ir::condcodes::FloatCC::GreaterThan,
                                    OpCode::LE => cranelift_codegen::ir::condcodes::FloatCC::LessThanOrEqual,
                                    OpCode::GE => cranelift_codegen::ir::condcodes::FloatCC::GreaterThanOrEqual,
                                    _ => unreachable!(),
                                };
                                builder.ins().fcmp(cc, a, b)
                            }
                        };
                        let one = builder.ins().iconst(types::I64, 1);
                        let zero = builder.ins().iconst(types::I64, 0);
                        let as_i64 = builder.ins().select(cond, one, zero);
                        let as_num = match plan.num_kind {
                            JitNumKind::I64 => as_i64,
                            JitNumKind::F64 => builder.ins().fcvt_from_sint(types::F64, as_i64),
                        };
                        push(&mut builder, as_num);
                    }
                    OpCode::JMP => {
                        let target = instr.operands.get(0).copied().unwrap_or(0) as usize;
                        if target >= blocks.len() {
                            return Err("Invalid jump target".to_string());
                        }
                        builder.ins().jump(blocks[target], &[]);
                        continue;
                    }
                    OpCode::JZ | OpCode::JNZ => {
                        let target = instr.operands.get(0).copied().unwrap_or(0) as usize;
                        if target >= blocks.len() {
                            return Err("Invalid jump target".to_string());
                        }
                        let cond_val = pop(&mut builder)?;
                        let is_zero = match plan.num_kind {
                            JitNumKind::I64 => builder.ins().icmp_imm(IntCC::Equal, cond_val, 0),
                            JitNumKind::F64 => {
                                let z = builder.ins().f64const(0.0);
                                builder
                                    .ins()
                                    .fcmp(cranelift_codegen::ir::condcodes::FloatCC::Equal, cond_val, z)
                            }
                        };
                        let then_block = if instr.opcode == OpCode::JZ { blocks[target] } else { blocks[ip + 1] };
                        let else_block = if instr.opcode == OpCode::JZ { blocks[ip + 1] } else { blocks[target] };
                        builder.ins().brif(is_zero, then_block, &[], else_block, &[]);
                        continue;
                    }
                    OpCode::RET => {
                        match plan.ret_kind {
                            JitRetKind::I64 => {
                                let ret = pop(&mut builder).unwrap_or_else(|_| builder.ins().iconst(types::I64, 0));
                                builder.ins().return_(&[ret]);
                            }
                            JitRetKind::F64 => {
                                let ret = pop(&mut builder).unwrap_or_else(|_| builder.ins().f64const(0.0));
                                builder.ins().return_(&[ret]);
                            }
                            JitRetKind::Bool => {
                                let ret = pop(&mut builder).unwrap_or_else(|_| match plan.num_kind {
                                    JitNumKind::I64 => builder.ins().iconst(types::I64, 0),
                                    JitNumKind::F64 => builder.ins().f64const(0.0),
                                });
                                let as_i64 = match plan.num_kind {
                                    JitNumKind::I64 => ret,
                                    JitNumKind::F64 => {
                                        let z = builder.ins().f64const(0.0);
                                        let is_nonzero = builder
                                            .ins()
                                            .fcmp(cranelift_codegen::ir::condcodes::FloatCC::NotEqual, ret, z);
                                        let one = builder.ins().iconst(types::I64, 1);
                                        let zero = builder.ins().iconst(types::I64, 0);
                                        builder.ins().select(is_nonzero, one, zero)
                                    }
                                };
                                builder.ins().return_(&[as_i64]);
                            }
                        }
                        continue;
                    }
                    _ => return Err("Unsupported opcode for JIT".to_string()),
                }

                builder.ins().jump(blocks[ip + 1], &[]);
            }

            builder.switch_to_block(blocks[func.instructions.len()]);
            match plan.ret_kind {
                JitRetKind::I64 | JitRetKind::Bool => {
                    let z = builder.ins().iconst(types::I64, 0);
                    builder.ins().return_(&[z]);
                }
                JitRetKind::F64 => {
                    let z = builder.ins().f64const(0.0);
                    builder.ins().return_(&[z]);
                }
            };
            builder.seal_all_blocks();
            builder.finalize();
            let id = func_id;
            self.module.define_function(id, &mut ctx).map_err(|e| e.to_string())?;
            self.module.clear_context(&mut ctx);
            self.module.finalize_definitions().map_err(|e| e.to_string())?;
            let code = self.module.get_finalized_function(id);
            let result = JitResult { func_ptr: code, arity: func.arity, plan };
            self.functions.insert((module_name.to_string(), func_idx, plan), result.clone());
            Ok(result)
        }

        pub fn vm_plan(func: &Function) -> bool {
            if func.instructions.is_empty() {
                return false;
            }
            func.instructions.iter().all(|instr| {
                matches!(
                    instr.opcode,
                    OpCode::HALT
                        | OpCode::NOP
                        | OpCode::RET
                        | OpCode::JMP
                        | OpCode::JZ
                        | OpCode::JNZ
                        | OpCode::PUSH
                        | OpCode::PushM
                        | OpCode::POP
                        | OpCode::DUP
                        | OpCode::SWAP
                        | OpCode::LOAD
                        | OpCode::STORE
                        | OpCode::ADD
                        | OpCode::SUB
                        | OpCode::MUL
                        | OpCode::DIV
                        | OpCode::NEG
                        | OpCode::EQ
                        | OpCode::NE
                        | OpCode::LT
                        | OpCode::GT
                        | OpCode::LE
                        | OpCode::GE
                        | OpCode::ALLOC
                        | OpCode::FREE
                        | OpCode::GetGlobal
                        | OpCode::SetGlobal
                        | OpCode::GetGlobalM
                        | OpCode::SetGlobalM
                        | OpCode::NewArray
                        | OpCode::NewObj
                        | OpCode::GetField
                        | OpCode::SetField
                        | OpCode::GetIndex
                        | OpCode::SetIndex
                        | OpCode::LEN
                        | OpCode::SLICE
                        | OpCode::CALL
                        | OpCode::CallExt
                        | OpCode::CLOSURE
                        | OpCode::ClosureRef
                        | OpCode::ClosureRefStack
                )
            })
        }

        fn import_stub(
            &mut self,
            name: &str,
            params: &[cranelift_codegen::ir::Type],
            returns: &[cranelift_codegen::ir::Type],
            func: &mut ClifFunction,
        ) -> Result<cranelift_codegen::ir::FuncRef, String> {
            let id = if let Some(existing) = self.imports.get(name) {
                *existing
            } else {
                let mut sig = self.module.make_signature();
                for &p in params {
                    sig.params.push(AbiParam::new(p));
                }
                for &r in returns {
                    sig.returns.push(AbiParam::new(r));
                }
                let id = self
                    .module
                    .declare_function(name, Linkage::Import, &sig)
                    .map_err(|e| e.to_string())?;
                self.imports.insert(name.to_string(), id);
                id
            };
            Ok(self.module.declare_func_in_func(id, func))
        }

        pub fn compile_vm(&mut self, module_name: &str, func_idx: usize, func: &Function) -> Result<VmJitResult, String> {
            if let Some(existing) = self.vm_functions.get(&(module_name.to_string(), func_idx)) {
                return Ok(existing.clone());
            }
            if !Self::vm_plan(func) {
                return Err("Function contains unsupported opcodes for VM-aware JIT".to_string());
            }

            let ptr_type = self.module.isa().pointer_type();
            let mut sig = self.module.make_signature();
            sig.params.push(AbiParam::new(ptr_type));
            sig.params.push(AbiParam::new(types::I32));
            sig.returns.push(AbiParam::new(types::I64));

            // Pre-allocate IC buffers for specialization.
            let mut attr_ic_count = 0;
            let mut index_ic_count = 0;
            for (i, instr) in func.instructions.iter().enumerate() {
                if instr.opcode == OpCode::GetField || instr.opcode == OpCode::SetField {
                    if i > 0 && func.instructions[i-1].opcode == OpCode::PUSH {
                        let c_idx = func.instructions[i-1].operands[0] as usize;
                        if let Some(Value::String(_)) = func.constants.get(c_idx) {
                             attr_ic_count += 1;
                        }
                    }
                } else if instr.opcode == OpCode::GetIndex || instr.opcode == OpCode::SetIndex {
                    if i > 0 && func.instructions[i-1].opcode == OpCode::PUSH {
                        let c_idx = func.instructions[i-1].operands[0] as usize;
                        if let Some(Value::Int(_)) = func.constants.get(c_idx) {
                             index_ic_count += 1;
                        }
                    }
                }
            }
            let attr_ic_buffer = if attr_ic_count > 0 {
                let mut v = Vec::with_capacity(attr_ic_count);
                for _ in 0..attr_ic_count {
                    v.push(crate::runtime::AttributeIC::default());
                }
                Some(std::sync::Arc::new(v))
            } else {
                None
            };
            let index_ic_buffer = if index_ic_count > 0 {
                let mut v = Vec::with_capacity(index_ic_count);
                for _ in 0..index_ic_count {
                    v.push(crate::runtime::IndexIC::default());
                }
                Some(std::sync::Arc::new(v))
            } else {
                None
            };
            let mut attr_ic_idx = 0;
            let mut index_ic_idx = 0;

            let func_id = self
                .module
                .declare_function(&format!("{}_vmjit_{}", module_name, func_idx), Linkage::Local, &sig)
                .map_err(|e| e.to_string())?;

            let mut ctx = self.module.make_context();
            ctx.func = ClifFunction::with_name_signature(
                cranelift_codegen::ir::UserFuncName::user(1, func_idx as u32),
                sig,
            );
            let mut builder_ctx = FunctionBuilderContext::new();
            let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);

            let entry_block = builder.create_block();
            builder.append_block_params_for_function_params(entry_block);
            builder.switch_to_block(entry_block);
            builder.seal_block(entry_block);
            let rt_param = builder.block_params(entry_block)[0];
            let initial_ip = builder.block_params(entry_block)[1];

            let stack_guard = self.import_stub("nyx_jit_stack_guard", &[ptr_type], &[types::I64], &mut builder.func)?;
            let guard_res = builder.ins().call(stack_guard, &[rt_param]);
            let guard_res_val = builder.inst_results(guard_res)[0];
            let guard_err = builder.ins().icmp_imm(IntCC::Equal, guard_res_val, -1);
            
            let check_arity = self.import_stub("nyx_jit_check_arity", &[ptr_type, types::I32], &[types::I64], &mut builder.func)?;
            let arity_val = builder.ins().iconst(types::I32, func.arity as i64);
            let arity_res = builder.ins().call(check_arity, &[rt_param, arity_val]);
            let arity_res_val = builder.inst_results(arity_res)[0];
            let arity_err = builder.ins().icmp_imm(IntCC::Equal, arity_res_val, -1);

            let trap_block = builder.create_block();
            let start_block = builder.create_block();
            
            let combined_err = builder.ins().bor(guard_err, arity_err);
            builder.ins().brif(combined_err, trap_block, &[], start_block, &[]);
            builder.seal_block(start_block);
            builder.switch_to_block(start_block);

            let mut blocks = Vec::with_capacity(func.instructions.len() + 1);
            for _ in 0..=func.instructions.len() {
                blocks.push(builder.create_block());
            }
            let exit_block = builder.create_block();
            let yield_block = builder.create_block();

            // Dispatch to the correct block.
            let first_block = builder.func.dfg.block_call(blocks[0], &[]);
            let mut jt_entries = Vec::with_capacity(func.instructions.len());
            for i in 0..func.instructions.len() {
                jt_entries.push(builder.func.dfg.block_call(blocks[i], &[]));
            }
            let jt_data = cranelift_codegen::ir::JumpTableData::new(first_block, &jt_entries);
            let jt = builder.create_jump_table(jt_data);
            builder.ins().br_table(initial_ip, jt);

            let tick = self.import_stub("nyx_jit_tick", &[ptr_type, types::I32], &[types::I64], &mut builder.func)?;
            let pop_truthy = self.import_stub("nyx_jit_pop_truthy", &[ptr_type], &[types::I64], &mut builder.func)?;
            let push_const = self.import_stub("nyx_jit_push_const", &[ptr_type, types::I32], &[types::I64], &mut builder.func)?;
            let pushm = self.import_stub("nyx_jit_pushm", &[ptr_type, types::I32], &[types::I64], &mut builder.func)?;
            let pop = self.import_stub("nyx_jit_pop", &[ptr_type], &[types::I64], &mut builder.func)?;
            let dup = self.import_stub("nyx_jit_dup", &[ptr_type], &[types::I64], &mut builder.func)?;
            let swap = self.import_stub("nyx_jit_swap", &[ptr_type], &[types::I64], &mut builder.func)?;
            let load = self.import_stub("nyx_jit_load", &[ptr_type, types::I32], &[types::I64], &mut builder.func)?;
            let store = self.import_stub("nyx_jit_store", &[ptr_type, types::I32], &[types::I64], &mut builder.func)?;
            let add = self.import_stub("nyx_jit_add", &[ptr_type], &[types::I64], &mut builder.func)?;
            let sub = self.import_stub("nyx_jit_sub", &[ptr_type], &[types::I64], &mut builder.func)?;
            let mul = self.import_stub("nyx_jit_mul", &[ptr_type], &[types::I64], &mut builder.func)?;
            let _div = self.import_stub("nyx_jit_div", &[ptr_type], &[types::I64], &mut builder.func)?;
            let _neg = self.import_stub("nyx_jit_neg", &[ptr_type], &[types::I64], &mut builder.func)?;
            let eq = self.import_stub("nyx_jit_eq", &[ptr_type], &[types::I64], &mut builder.func)?;
            let ne = self.import_stub("nyx_jit_ne", &[ptr_type], &[types::I64], &mut builder.func)?;
            let lt = self.import_stub("nyx_jit_lt", &[ptr_type], &[types::I64], &mut builder.func)?;
            let gt = self.import_stub("nyx_jit_gt", &[ptr_type], &[types::I64], &mut builder.func)?;
            let le = self.import_stub("nyx_jit_le", &[ptr_type], &[types::I64], &mut builder.func)?;
            let ge = self.import_stub("nyx_jit_ge", &[ptr_type], &[types::I64], &mut builder.func)?;
            let new_array = self.import_stub("nyx_jit_new_array", &[ptr_type, types::I32], &[types::I64], &mut builder.func)?;
            let new_obj = self.import_stub("nyx_jit_new_obj", &[ptr_type, types::I32], &[types::I64], &mut builder.func)?;
            let get_field = self.import_stub("nyx_jit_get_field", &[ptr_type], &[types::I64], &mut builder.func)?;
            let set_field = self.import_stub("nyx_jit_set_field", &[ptr_type], &[types::I64], &mut builder.func)?;
            let get_index = self.import_stub("nyx_jit_get_index", &[ptr_type], &[types::I64], &mut builder.func)?;
            let set_index = self.import_stub("nyx_jit_set_index", &[ptr_type], &[types::I64], &mut builder.func)?;
            let len = self.import_stub("nyx_jit_len", &[ptr_type], &[types::I64], &mut builder.func)?;
            let slice = self.import_stub("nyx_jit_slice", &[ptr_type], &[types::I64], &mut builder.func)?;
            let get_global = self.import_stub("nyx_jit_get_global", &[ptr_type, types::I32], &[types::I64], &mut builder.func)?;
            let set_global = self.import_stub("nyx_jit_set_global", &[ptr_type, types::I32], &[types::I64], &mut builder.func)?;
            let get_global_m = self.import_stub("nyx_jit_get_global_m", &[ptr_type, types::I32], &[types::I64], &mut builder.func)?;
            let set_global_m = self.import_stub("nyx_jit_set_global_m", &[ptr_type, types::I32], &[types::I64], &mut builder.func)?;
            let alloc = self.import_stub("nyx_jit_alloc", &[ptr_type, types::I32], &[types::I64], &mut builder.func)?;
            let free = self.import_stub("nyx_jit_free", &[ptr_type], &[types::I64], &mut builder.func)?;
            let ret = self.import_stub("nyx_jit_ret", &[ptr_type], &[types::I64], &mut builder.func)?;
            let halt = self.import_stub("nyx_jit_halt", &[ptr_type], &[types::I64], &mut builder.func)?;
            let jit_call = self.import_stub("nyx_jit_call", &[ptr_type, types::I32, types::I32], &[types::I64], &mut builder.func)?;
            let jit_call_ext = self.import_stub("nyx_jit_call_ext", &[ptr_type, types::I32, types::I32], &[types::I64], &mut builder.func)?;
            let jit_closure = self.import_stub("nyx_jit_closure", &[ptr_type, types::I32, types::I32], &[types::I64], &mut builder.func)?;
            let jit_closure_ref = self.import_stub("nyx_jit_closure_ref", &[ptr_type, types::I32, types::I32, ptr_type], &[types::I64], &mut builder.func)?;
            let jit_closure_ref_stack = self.import_stub("nyx_jit_closure_ref_stack", &[ptr_type, types::I32, types::I32], &[types::I64], &mut builder.func)?;

            let call0 = |builder: &mut FunctionBuilder,
                         callee: cranelift_codegen::ir::FuncRef,
                         rt_param|
             -> cranelift_codegen::ir::Value {
                let call = builder.ins().call(callee, &[rt_param]);
                builder.inst_results(call)[0]
            };
            let call1_i32 = |builder: &mut FunctionBuilder,
                             callee: cranelift_codegen::ir::FuncRef,
                             rt_param,
                             imm: i32|
             -> cranelift_codegen::ir::Value {
                let arg = builder.ins().iconst(types::I32, imm as i64);
                let call = builder.ins().call(callee, &[rt_param, arg]);
                builder.inst_results(call)[0]
            };
            let call2_i32 = |builder: &mut FunctionBuilder,
                              callee: cranelift_codegen::ir::FuncRef,
                              rt_param,
                              arg1: i32,
                              arg2: i32|
             -> cranelift_codegen::ir::Value {
                let a1 = builder.ins().iconst(types::I32, arg1 as i64);
                let a2 = builder.ins().iconst(types::I32, arg2 as i64);
                let call = builder.ins().call(callee, &[rt_param, a1, a2]);
                builder.inst_results(call)[0]
            };
            let call3_i32_ptr = |builder: &mut FunctionBuilder,
                                 callee: cranelift_codegen::ir::FuncRef,
                                 rt_param,
                                 arg1: i32,
                                 arg2: i32,
                                 arg3: cranelift_codegen::ir::Value|
             -> cranelift_codegen::ir::Value {
                let a1 = builder.ins().iconst(types::I32, arg1 as i64);
                let a2 = builder.ins().iconst(types::I32, arg2 as i64);
                let call = builder.ins().call(callee, &[rt_param, a1, a2, arg3]);
                builder.inst_results(call)[0]
            };
            let call4_ptr_ptr = |builder: &mut FunctionBuilder,
                                     callee: cranelift_codegen::ir::FuncRef,
                                     rt_param,
                                     arg1,
                                     arg2,
                                     arg3|
             -> cranelift_codegen::ir::Value {
                let call = builder.ins().call(callee, &[rt_param, arg1, arg2, arg3]);
                builder.inst_results(call)[0]
            };

            for (ip, instr) in func.instructions.iter().enumerate() {
                builder.switch_to_block(blocks[ip]);

                let ip_val = builder.ins().iconst(types::I32, ip as i64);
                let tick_res = builder.ins().call(tick, &[rt_param, ip_val]);
                let tick_res_val = builder.inst_results(tick_res)[0];
                let tick_err = builder.ins().icmp_imm(IntCC::Equal, tick_res_val, -1);
                let after_tick = builder.create_block();
                builder.ins().brif(tick_err, trap_block, &[], after_tick, &[]);
                builder.seal_block(after_tick);
                builder.switch_to_block(after_tick);

                match instr.opcode {
                    OpCode::NOP => {}
                    OpCode::PUSH => {
                        let idx = *instr
                            .operands
                            .get(0)
                            .ok_or_else(|| "PUSH missing operand".to_string())?;
                        let res = call1_i32(&mut builder, push_const, rt_param, idx);
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                    OpCode::PushM => {
                        let idx = *instr
                            .operands
                            .get(0)
                            .ok_or_else(|| "PushM missing operand".to_string())?;
                        let res = call1_i32(&mut builder, pushm, rt_param, idx);
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                    OpCode::POP => {
                        let res = builder.ins().call(pop, &[rt_param]);
                        let res_val = builder.inst_results(res)[0];
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res_val, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                                        OpCode::DUP => {
                        let res = call0(&mut builder, dup, rt_param);
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                    OpCode::SWAP => {
                        let res = call0(&mut builder, swap, rt_param);
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                    OpCode::LOAD => {
                        let idx = *instr
                            .operands
                            .get(0)
                            .ok_or_else(|| "LOAD missing operand".to_string())?;
                        let res = call1_i32(&mut builder, load, rt_param, idx);
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                    OpCode::STORE => {
                        let idx = *instr
                            .operands
                            .get(0)
                            .ok_or_else(|| "STORE missing operand".to_string())?;
                        let res = call1_i32(&mut builder, store, rt_param, idx);
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                    OpCode::ADD => {
                        let stack_info_fn = self.import_stub("nyx_jit_get_stack_info", &[ptr_type], &[ptr_type, types::I64], &mut builder.func)?;
                        let call_stack = builder.ins().call(stack_info_fn, &[rt_param]);
                        let stack_ptr = builder.inst_results(call_stack)[0];
                        let stack_len = builder.inst_results(call_stack)[1];

                        let enough_stack = builder.ins().icmp_imm(IntCC::UnsignedGreaterThanOrEqual, stack_len, 2);
                        let hit_label = builder.create_block();
                        let miss_label = builder.create_block();
                        builder.ins().brif(enough_stack, hit_label, &[], miss_label, &[]);
                        builder.seal_block(hit_label);
                        builder.switch_to_block(hit_label);
                        
                        let idx1 = builder.ins().iadd_imm(stack_len, -1);
                        let idx2 = builder.ins().iadd_imm(stack_len, -2);
                        let off1 = builder.ins().imul_imm(idx1, 88);
                        let off2 = builder.ins().imul_imm(idx2, 88);
                        let addr1 = builder.ins().iadd(stack_ptr, off1);
                        let addr2 = builder.ins().iadd(stack_ptr, off2);
                        
                        let tag1 = builder.ins().load(types::I64, MemFlags::new(), addr1, 0);
                        let tag2 = builder.ins().load(types::I64, MemFlags::new(), addr2, 0);
                        let is_int1 = builder.ins().icmp_imm(IntCC::Equal, tag1, 2);
                        let is_int2 = builder.ins().icmp_imm(IntCC::Equal, tag2, 2);
                        let both_int = builder.ins().band(is_int1, is_int2);
                        
                        let fast_path = builder.create_block();
                        builder.ins().brif(both_int, fast_path, &[], miss_label, &[]);
                        builder.seal_block(fast_path);
                        builder.switch_to_block(fast_path);
                        
                        let v1 = builder.ins().load(types::I64, MemFlags::new(), addr1, 8);
                        let v2 = builder.ins().load(types::I64, MemFlags::new(), addr2, 8);
                        let sum = builder.ins().iadd(v1, v2);
                        builder.ins().store(MemFlags::new(), sum, addr2, 8);
                        
                        let set_stack_len_fn = self.import_stub("nyx_jit_set_stack_len", &[ptr_type, types::I64], &[types::I64], &mut builder.func)?;
                        let new_len = builder.ins().iadd_imm(stack_len, -1);
                        let _ = builder.ins().call(set_stack_len_fn, &[rt_param, new_len]);
                        builder.ins().jump(blocks[ip + 1], &[]);
                        
                        builder.seal_block(miss_label);
                        builder.switch_to_block(miss_label);
                        let res = builder.ins().call(add, &[rt_param]);
                        let res_val = builder.inst_results(res)[0];
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res_val, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                    OpCode::SUB => {
                        let stack_info_fn = self.import_stub("nyx_jit_get_stack_info", &[ptr_type], &[ptr_type, types::I64], &mut builder.func)?;
                        let call_stack = builder.ins().call(stack_info_fn, &[rt_param]);
                        let stack_ptr = builder.inst_results(call_stack)[0];
                        let stack_len = builder.inst_results(call_stack)[1];

                        let enough_stack = builder.ins().icmp_imm(IntCC::UnsignedGreaterThanOrEqual, stack_len, 2);
                        let hit_label = builder.create_block();
                        let miss_label = builder.create_block();
                        builder.ins().brif(enough_stack, hit_label, &[], miss_label, &[]);
                        builder.seal_block(hit_label);
                        builder.switch_to_block(hit_label);
                        
                        let idx1 = builder.ins().iadd_imm(stack_len, -1);
                        let idx2 = builder.ins().iadd_imm(stack_len, -2);
                        let off1 = builder.ins().imul_imm(idx1, 88);
                        let off2 = builder.ins().imul_imm(idx2, 88);
                        let addr1 = builder.ins().iadd(stack_ptr, off1);
                        let addr2 = builder.ins().iadd(stack_ptr, off2);
                        
                        let tag1 = builder.ins().load(types::I64, MemFlags::new(), addr1, 0);
                        let tag2 = builder.ins().load(types::I64, MemFlags::new(), addr2, 0);
                        let is_int1 = builder.ins().icmp_imm(IntCC::Equal, tag1, 2);
                        let is_int2 = builder.ins().icmp_imm(IntCC::Equal, tag2, 2);
                        let both_int = builder.ins().band(is_int1, is_int2);
                        
                        let fast_path = builder.create_block();
                        builder.ins().brif(both_int, fast_path, &[], miss_label, &[]);
                        builder.seal_block(fast_path);
                        builder.switch_to_block(fast_path);
                        
                        let v1 = builder.ins().load(types::I64, MemFlags::new(), addr1, 8);
                        let v2 = builder.ins().load(types::I64, MemFlags::new(), addr2, 8);
                        let diff = builder.ins().isub(v2, v1);
                        builder.ins().store(MemFlags::new(), diff, addr2, 8);
                        
                        let set_stack_len_fn = self.import_stub("nyx_jit_set_stack_len", &[ptr_type, types::I64], &[types::I64], &mut builder.func)?;
                        let new_len = builder.ins().iadd_imm(stack_len, -1);
                        let _ = builder.ins().call(set_stack_len_fn, &[rt_param, new_len]);
                        builder.ins().jump(blocks[ip + 1], &[]);
                        
                        builder.seal_block(miss_label);
                        builder.switch_to_block(miss_label);
                        let res = builder.ins().call(sub, &[rt_param]);
                        let res_val = builder.inst_results(res)[0];
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res_val, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                                        OpCode::MUL => {
                        let stack_info_fn = self.import_stub("nyx_jit_get_stack_info", &[ptr_type], &[ptr_type, types::I64], &mut builder.func)?;
                        let call_stack = builder.ins().call(stack_info_fn, &[rt_param]);
                        let stack_ptr = builder.inst_results(call_stack)[0];
                        let stack_len = builder.inst_results(call_stack)[1];

                        let enough_stack = builder.ins().icmp_imm(IntCC::UnsignedGreaterThanOrEqual, stack_len, 2);
                        let hit_label = builder.create_block();
                        let miss_label = builder.create_block();
                        builder.ins().brif(enough_stack, hit_label, &[], miss_label, &[]);
                        builder.seal_block(hit_label);
                        builder.switch_to_block(hit_label);
                        
                        let idx1 = builder.ins().iadd_imm(stack_len, -1);
                        let idx2 = builder.ins().iadd_imm(stack_len, -2);
                        let off1 = builder.ins().imul_imm(idx1, 88);
                        let off2 = builder.ins().imul_imm(idx2, 88);
                        let addr1 = builder.ins().iadd(stack_ptr, off1);
                        let addr2 = builder.ins().iadd(stack_ptr, off2);
                        
                        let tag1 = builder.ins().load(types::I64, MemFlags::new(), addr1, 0);
                        let tag2 = builder.ins().load(types::I64, MemFlags::new(), addr2, 0);
                        let is_int1 = builder.ins().icmp_imm(IntCC::Equal, tag1, 2);
                        let is_int2 = builder.ins().icmp_imm(IntCC::Equal, tag2, 2);
                        let both_int = builder.ins().band(is_int1, is_int2);
                        
                        let fast_path = builder.create_block();
                        builder.ins().brif(both_int, fast_path, &[], miss_label, &[]);
                        builder.seal_block(fast_path);
                        builder.switch_to_block(fast_path);
                        
                        let v1 = builder.ins().load(types::I64, MemFlags::new(), addr1, 8);
                        let v2 = builder.ins().load(types::I64, MemFlags::new(), addr2, 8);
                        let prod = builder.ins().imul(v1, v2);
                        builder.ins().store(MemFlags::new(), prod, addr2, 8);
                        
                        let set_stack_len_fn = self.import_stub("nyx_jit_set_stack_len", &[ptr_type, types::I64], &[types::I64], &mut builder.func)?;
                        let new_len = builder.ins().iadd_imm(stack_len, -1);
                        let _ = builder.ins().call(set_stack_len_fn, &[rt_param, new_len]);
                        builder.ins().jump(blocks[ip + 1], &[]);
                        
                        builder.seal_block(miss_label);
                        builder.switch_to_block(miss_label);
                        let res = builder.ins().call(mul, &[rt_param]);
                        let res_val = builder.inst_results(res)[0];
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res_val, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                                        OpCode::EQ => {
                        let stack_info_fn = self.import_stub("nyx_jit_get_stack_info", &[ptr_type], &[ptr_type, types::I64], &mut builder.func)?;
                        let call_stack = builder.ins().call(stack_info_fn, &[rt_param]);
                        let stack_ptr = builder.inst_results(call_stack)[0];
                        let stack_len = builder.inst_results(call_stack)[1];

                        let enough_stack = builder.ins().icmp_imm(IntCC::UnsignedGreaterThanOrEqual, stack_len, 2);
                        let hit_label = builder.create_block();
                        let miss_label = builder.create_block();
                        builder.ins().brif(enough_stack, hit_label, &[], miss_label, &[]);
                        builder.seal_block(hit_label);
                        builder.switch_to_block(hit_label);
                        
                        let idx1 = builder.ins().iadd_imm(stack_len, -1);
                        let idx2 = builder.ins().iadd_imm(stack_len, -2);
                        let off1 = builder.ins().imul_imm(idx1, 88);
                        let off2 = builder.ins().imul_imm(idx2, 88);
                        let addr1 = builder.ins().iadd(stack_ptr, off1);
                        let addr2 = builder.ins().iadd(stack_ptr, off2);
                        
                        let tag1 = builder.ins().load(types::I64, MemFlags::new(), addr1, 0);
                        let tag2 = builder.ins().load(types::I64, MemFlags::new(), addr2, 0);
                        let is_int1 = builder.ins().icmp_imm(IntCC::Equal, tag1, 2);
                        let is_int2 = builder.ins().icmp_imm(IntCC::Equal, tag2, 2);
                        let both_int = builder.ins().band(is_int1, is_int2);
                        
                        let fast_path = builder.create_block();
                        builder.ins().brif(both_int, fast_path, &[], miss_label, &[]);
                        builder.seal_block(fast_path);
                        builder.switch_to_block(fast_path);
                        
                        let v1 = builder.ins().load(types::I64, MemFlags::new(), addr1, 8);
                        let v2 = builder.ins().load(types::I64, MemFlags::new(), addr2, 8);
                        let eq_res = builder.ins().icmp(IntCC::Equal, v1, v2);
                        let res_val_imm = builder.ins().uextend(types::I64, eq_res);
                        
                        // Store result (Bool tag 1)
                        let tag_bool = builder.ins().iconst(types::I64, 1);
                        builder.ins().store(MemFlags::new(), tag_bool, addr2, 0);
                        builder.ins().store(MemFlags::new(), res_val_imm, addr2, 8);
                        
                        let set_stack_len = self.import_stub("nyx_jit_set_stack_len", &[ptr_type, types::I64], &[types::I64], &mut builder.func)?;
                        let new_len = builder.ins().iadd_imm(stack_len, -1);
                        let _ = builder.ins().call(set_stack_len, &[rt_param, new_len]);
                        builder.ins().jump(blocks[ip + 1], &[]);
                        
                        builder.seal_block(miss_label);
                        builder.switch_to_block(miss_label);
                        let res = builder.ins().call(eq, &[rt_param]);
                        let res_val = builder.inst_results(res)[0];
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res_val, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                    OpCode::BNOT => {
                        let stack_info_fn = self.import_stub("nyx_jit_get_stack_info", &[ptr_type], &[ptr_type, types::I64], &mut builder.func)?;
                        let call_stack = builder.ins().call(stack_info_fn, &[rt_param]);
                        let stack_ptr = builder.inst_results(call_stack)[0];
                        let stack_len = builder.inst_results(call_stack)[1];
                        
                        let enough_stack = builder.ins().icmp_imm(IntCC::UnsignedGreaterThanOrEqual, stack_len, 1);
                        let hit_label = builder.create_block();
                        let miss_label = builder.create_block();
                        builder.ins().brif(enough_stack, hit_label, &[], miss_label, &[]);
                        builder.seal_block(hit_label);
                        builder.switch_to_block(hit_label);
                        
                        let idx = builder.ins().iadd_imm(stack_len, -1);
                        let off = builder.ins().imul_imm(idx, 88);
                        let addr = builder.ins().iadd(stack_ptr, off);
                        
                        let tag = builder.ins().load(types::I64, MemFlags::new(), addr, 0);
                        let is_int = builder.ins().icmp_imm(IntCC::Equal, tag, 2);
                        
                        let fast_path = builder.create_block();
                        builder.ins().brif(is_int, fast_path, &[], miss_label, &[]);
                        builder.seal_block(fast_path);
                        builder.switch_to_block(fast_path);
                        
                        let v = builder.ins().load(types::I64, MemFlags::new(), addr, 8);
                        let res_v = builder.ins().bnot(v);
                        builder.ins().store(MemFlags::new(), res_v, addr, 8);
                        builder.ins().jump(blocks[ip + 1], &[]);
                        
                        builder.seal_block(miss_label);
                        builder.switch_to_block(miss_label);
                        let bnot_stub = self.import_stub("nyx_jit_bnot", &[ptr_type], &[types::I64], &mut builder.func)?;
                        let res = builder.ins().call(bnot_stub, &[rt_param]);
                        let res_val = builder.inst_results(res)[0];
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res_val, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                    OpCode::MOD => {
                        let stack_info_fn = self.import_stub("nyx_jit_get_stack_info", &[ptr_type], &[ptr_type, types::I64], &mut builder.func)?;
                        let call_stack = builder.ins().call(stack_info_fn, &[rt_param]);
                        let stack_ptr = builder.inst_results(call_stack)[0];
                        let stack_len = builder.inst_results(call_stack)[1];
                        
                        let enough_stack = builder.ins().icmp_imm(IntCC::UnsignedGreaterThanOrEqual, stack_len, 2);
                        let hit_label = builder.create_block();
                        let miss_label = builder.create_block();
                        builder.ins().brif(enough_stack, hit_label, &[], miss_label, &[]);
                        builder.seal_block(hit_label);
                        builder.switch_to_block(hit_label);
                        
                        let idx1 = builder.ins().iadd_imm(stack_len, -1);
                        let idx2 = builder.ins().iadd_imm(stack_len, -2);
                        let off1 = builder.ins().imul_imm(idx1, 88);
                        let off2 = builder.ins().imul_imm(idx2, 88);
                        let addr1 = builder.ins().iadd(stack_ptr, off1);
                        let addr2 = builder.ins().iadd(stack_ptr, off2);
                        
                        let tag1 = builder.ins().load(types::I64, MemFlags::new(), addr1, 0);
                        let tag2 = builder.ins().load(types::I64, MemFlags::new(), addr2, 0);
                        let is_int1 = builder.ins().icmp_imm(IntCC::Equal, tag1, 2);
                        let is_int2 = builder.ins().icmp_imm(IntCC::Equal, tag2, 2);
                        let both_int = builder.ins().band(is_int1, is_int2);
                        
                        let fast_path = builder.create_block();
                        builder.ins().brif(both_int, fast_path, &[], miss_label, &[]);
                        builder.seal_block(fast_path);
                        builder.switch_to_block(fast_path);
                        
                        let v1 = builder.ins().load(types::I64, MemFlags::new(), addr1, 8);
                        let v2 = builder.ins().load(types::I64, MemFlags::new(), addr2, 8);
                        
                        let not_zero = builder.ins().icmp_imm(IntCC::NotEqual, v1, 0);
                        let zero_check_label = builder.create_block();
                        builder.ins().brif(not_zero, zero_check_label, &[], miss_label, &[]);
                        builder.seal_block(zero_check_label);
                        builder.switch_to_block(zero_check_label);
                        
                        let res_v = builder.ins().srem(v2, v1);
                        builder.ins().store(MemFlags::new(), res_v, addr2, 8);
                        
                        let set_stack_len_fn = self.import_stub("nyx_jit_set_stack_len", &[ptr_type, types::I64], &[types::I64], &mut builder.func)?;
                        let new_len = builder.ins().iadd_imm(stack_len, -1);
                        let _ = builder.ins().call(set_stack_len_fn, &[rt_param, new_len]);
                        builder.ins().jump(blocks[ip + 1], &[]);
                        
                        builder.seal_block(miss_label);
                        builder.switch_to_block(miss_label);
                        let mod_stub = self.import_stub("nyx_jit_mod", &[ptr_type], &[types::I64], &mut builder.func)?;
                        let res = builder.ins().call(mod_stub, &[rt_param]);
                        let res_val = builder.inst_results(res)[0];
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res_val, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                    OpCode::BAND | OpCode::BOR | OpCode::BXOR => {
                        let stack_info_fn = self.import_stub("nyx_jit_get_stack_info", &[ptr_type], &[ptr_type, types::I64], &mut builder.func)?;
                        let call_stack = builder.ins().call(stack_info_fn, &[rt_param]);
                        let stack_ptr = builder.inst_results(call_stack)[0];
                        let stack_len = builder.inst_results(call_stack)[1];
                        
                        let enough_stack = builder.ins().icmp_imm(IntCC::UnsignedGreaterThanOrEqual, stack_len, 2);
                        let hit_label = builder.create_block();
                        let miss_label = builder.create_block();
                        builder.ins().brif(enough_stack, hit_label, &[], miss_label, &[]);
                        builder.seal_block(hit_label);
                        builder.switch_to_block(hit_label);
                        
                        let idx1 = builder.ins().iadd_imm(stack_len, -1);
                        let idx2 = builder.ins().iadd_imm(stack_len, -2);
                        let off1 = builder.ins().imul_imm(idx1, 88);
                        let off2 = builder.ins().imul_imm(idx2, 88);
                        let addr1 = builder.ins().iadd(stack_ptr, off1);
                        let addr2 = builder.ins().iadd(stack_ptr, off2);
                        
                        let tag1 = builder.ins().load(types::I64, MemFlags::new(), addr1, 0);
                        let tag2 = builder.ins().load(types::I64, MemFlags::new(), addr2, 0);
                        let is_int1 = builder.ins().icmp_imm(IntCC::Equal, tag1, 2);
                        let is_int2 = builder.ins().icmp_imm(IntCC::Equal, tag2, 2);
                        let both_int = builder.ins().band(is_int1, is_int2);
                        
                        let fast_path = builder.create_block();
                        builder.ins().brif(both_int, fast_path, &[], miss_label, &[]);
                        builder.seal_block(fast_path);
                        builder.switch_to_block(fast_path);
                        
                        let v1 = builder.ins().load(types::I64, MemFlags::new(), addr1, 8);
                        let v2 = builder.ins().load(types::I64, MemFlags::new(), addr2, 8);
                        let bres = match instr.opcode {
                            OpCode::BAND => builder.ins().band(v1, v2),
                            OpCode::BOR => builder.ins().bor(v1, v2),
                            OpCode::BXOR => builder.ins().bxor(v1, v2),
                            _ => unreachable!(),
                        };
                        builder.ins().store(MemFlags::new(), bres, addr2, 8);
                        
                        let set_stack_len_fn = self.import_stub("nyx_jit_set_stack_len", &[ptr_type, types::I64], &[types::I64], &mut builder.func)?;
                        let new_len = builder.ins().iadd_imm(stack_len, -1);
                        let _ = builder.ins().call(set_stack_len_fn, &[rt_param, new_len]);
                        builder.ins().jump(blocks[ip + 1], &[]);
                        
                        builder.seal_block(miss_label);
                        builder.switch_to_block(miss_label);
                        let stub = match instr.opcode {
                            OpCode::BAND => self.import_stub("nyx_jit_band", &[ptr_type], &[types::I64], &mut builder.func)?,
                            OpCode::BOR => self.import_stub("nyx_jit_bor", &[ptr_type], &[types::I64], &mut builder.func)?,
                            OpCode::BXOR => self.import_stub("nyx_jit_bxor", &[ptr_type], &[types::I64], &mut builder.func)?,
                            _ => unreachable!(),
                        };
                        let res = builder.ins().call(stub, &[rt_param]);
                        let res_val = builder.inst_results(res)[0];
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res_val, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                    OpCode::NE => {
                        let res = call0(&mut builder, ne, rt_param);
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                    OpCode::LT => {
                        let res = call0(&mut builder, lt, rt_param);
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                    OpCode::GT => {
                        let res = call0(&mut builder, gt, rt_param);
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                    OpCode::LE => {
                        let res = call0(&mut builder, le, rt_param);
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                    OpCode::GE => {
                        let res = call0(&mut builder, ge, rt_param);
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                    OpCode::NewArray => {
                        let n = *instr
                            .operands
                            .get(0)
                            .ok_or_else(|| "NewArray missing operand".to_string())?;
                        let res = call1_i32(&mut builder, new_array, rt_param, n);
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                    OpCode::NewObj => {
                        let n = *instr
                            .operands
                            .get(0)
                            .ok_or_else(|| "NewObj missing operand".to_string())?;
                        let res = call1_i32(&mut builder, new_obj, rt_param, n);
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                    OpCode::GetField => {
                        let mut specialized = false;
                        if ip > 0 && func.instructions[ip - 1].opcode == OpCode::PUSH {
                            let c_idx = func.instructions[ip - 1].operands[0] as usize;
                            if let Some(Value::String(s)) = func.constants.get(c_idx) {
                                // Specialized constant-keyed access.
                                if let Some(ref ic_buffer) = attr_ic_buffer {
                                    let ic_ptr = &ic_buffer[attr_ic_idx] as *const _ as i64;
                                    attr_ic_idx += 1;
                                    
                                    let ic_val = builder.ins().iconst(ptr_type, ic_ptr);
                                    let name_ptr = s.as_ptr() as i64;
                                    let name_len = s.len() as i64;
                                    let name_ptr_val = builder.ins().iconst(ptr_type, name_ptr);
                                    let name_len_val = builder.ins().iconst(ptr_type, name_len);
                                    
                                    let get_heap_info = self.import_stub("nyx_jit_get_heap_info", &[ptr_type], &[ptr_type, types::I64], &mut builder.func)?;
                                    let get_stack_info = self.import_stub("nyx_jit_get_stack_info", &[ptr_type], &[ptr_type, types::I64], &mut builder.func)?;
                                    let push_ic_value = self.import_stub("nyx_jit_push_ic_value", &[ptr_type, ptr_type], &[types::I64], &mut builder.func)?;
                                    let get_field_cached = self.import_stub("nyx_jit_get_field_cached", &[ptr_type, ptr_type, ptr_type, ptr_type], &[types::I64], &mut builder.func)?;
                                    
                                    let stack_res = builder.ins().call(get_stack_info, &[rt_param]);
                                    let stack_ptr = builder.inst_results(stack_res)[0];
                                    let stack_len = builder.inst_results(stack_res)[1];
                                    
                                    let enough_stack = builder.ins().icmp_imm(IntCC::UnsignedGreaterThanOrEqual, stack_len, 2);
                                    let hit_label = builder.create_block();
                                    let miss_label = builder.create_block();
                                    let next_label = builder.create_block();
                                    
                                    builder.ins().brif(enough_stack, hit_label, &[], miss_label, &[]);
                                    builder.seal_block(hit_label);
                                    builder.switch_to_block(hit_label);
                                    
                                    // Load target from stack: Value size 88, Pointer usize at offset 8, tag at offset 0.
                                    let target_idx = builder.ins().iadd_imm(stack_len, -2);
                                    let target_offset = builder.ins().imul_imm(target_idx, 88);
                                    let target_val_addr = builder.ins().iadd(stack_ptr, target_offset);
                                    let target_tag = builder.ins().load(types::I64, MemFlags::new(), target_val_addr, 0);
                                    let is_pointer = builder.ins().icmp_imm(IntCC::Equal, target_tag, 10);
                                    
                                    let pointer_label = builder.create_block();
                                    builder.ins().brif(is_pointer, pointer_label, &[], miss_label, &[]);
                                    builder.seal_block(pointer_label);
                                    builder.switch_to_block(pointer_label);
                                    
                                    let target_ptr_val = builder.ins().load(ptr_type, MemFlags::new(), target_val_addr, 8);
                                    let ic_target_ptr = builder.ins().load(ptr_type, MemFlags::new(), ic_val, 0);
                                    let ptr_match = builder.ins().icmp(IntCC::Equal, target_ptr_val, ic_target_ptr);
                                    
                                    let match_label = builder.create_block();
                                    builder.ins().brif(ptr_match, match_label, &[], miss_label, &[]);
                                    builder.seal_block(match_label);
                                    builder.switch_to_block(match_label);
                                    
                                    let heap_res = builder.ins().call(get_heap_info, &[rt_param]);
                                    let heap_slots_ptr = builder.inst_results(heap_res)[0];
                                    let heap_slots_len = builder.inst_results(heap_res)[1];
                                    
                                    let in_bounds = builder.ins().icmp(IntCC::UnsignedLessThan, target_ptr_val, heap_slots_len);
                                    let bounds_label = builder.create_block();
                                    builder.ins().brif(in_bounds, bounds_label, &[], miss_label, &[]);
                                    builder.seal_block(bounds_label);
                                    builder.switch_to_block(bounds_label);
                                    
                                    let slot_ptr_offset = builder.ins().imul_imm(target_ptr_val, 8);
                                    let slot_ptr_addr = builder.ins().iadd(heap_slots_ptr, slot_ptr_offset);
                                    let slot_ptr = builder.ins().load(ptr_type, MemFlags::new(), slot_ptr_addr, 0);
                                    let slot_exists = builder.ins().icmp_imm(IntCC::NotEqual, slot_ptr, 0);
                                    
                                    let exists_label = builder.create_block();
                                    builder.ins().brif(slot_exists, exists_label, &[], miss_label, &[]);
                                    builder.seal_block(exists_label);
                                    builder.switch_to_block(exists_label);
                                    
                                    let slot_gen = builder.ins().load(types::I64, MemFlags::new(), slot_ptr, 0);
                                    let slot_ver = builder.ins().load(types::I64, MemFlags::new(), slot_ptr, 8);
                                    let ic_gen = builder.ins().load(types::I64, MemFlags::new(), ic_val, 8);
                                    let ic_ver = builder.ins().load(types::I64, MemFlags::new(), ic_val, 16);
                                    
                                    let gen_match = builder.ins().icmp(IntCC::Equal, slot_gen, ic_gen);
                                    let ver_match = builder.ins().icmp(IntCC::Equal, slot_ver, ic_ver);
                                    let full_match = builder.ins().band(gen_match, ver_match);
                                    
                                    let final_hit_label = builder.create_block();
                                    builder.ins().brif(full_match, final_hit_label, &[], miss_label, &[]);
                                    builder.seal_block(final_hit_label);
                                    builder.switch_to_block(final_hit_label);
                                    
                                    // Full IC hit!
                                    // Pop 2, push IC value.
                                    let _ = builder.ins().call(pop, &[rt_param]);
                                    let _ = builder.ins().call(pop, &[rt_param]);
                                    let push_res = builder.ins().call(push_ic_value, &[rt_param, ic_val]);
                                    let push_val = builder.inst_results(push_res)[0];
                                    let is_push_err = builder.ins().icmp_imm(IntCC::Equal, push_val, -1);
                                    builder.ins().brif(is_push_err, trap_block, &[], next_label, &[]);
                                    
                                    builder.seal_block(miss_label);
                                    builder.switch_to_block(miss_label);
                                    let res = call4_ptr_ptr(&mut builder, get_field_cached, rt_param, ic_val, name_ptr_val, name_len_val);
                                    let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                                    builder.ins().brif(is_err, trap_block, &[], next_label, &[]);
                                    
                                    builder.seal_block(next_label);
                                    builder.switch_to_block(next_label);
                                    builder.ins().jump(blocks[ip + 1], &[]);
                                    specialized = true;
                                }
                            }
                        }
                        
                        if !specialized {
                            let res = call0(&mut builder, get_field, rt_param);
                            let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                            builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        }
                        continue;
                    }
                    OpCode::SetField => {
                        let mut specialized = false;
                        if ip > 0 && func.instructions[ip - 1].opcode == OpCode::PUSH {
                            let c_idx = func.instructions[ip - 1].operands[0] as usize;
                            if let Some(Value::String(s)) = func.constants.get(c_idx) {
                                if let Some(ref ic_buffer) = attr_ic_buffer {
                                    let ic_ptr = &ic_buffer[attr_ic_idx] as *const _ as i64;
                                    attr_ic_idx += 1;
                                    let ic_val = builder.ins().iconst(ptr_type, ic_ptr);
                                    let name_ptr = s.as_ptr() as i64;
                                    let name_len = s.len() as i64;
                                    let name_ptr_val = builder.ins().iconst(ptr_type, name_ptr);
                                    let name_len_val = builder.ins().iconst(ptr_type, name_len);
                                    
                                    let get_heap_info = self.import_stub("nyx_jit_get_heap_info", &[ptr_type], &[ptr_type, types::I64], &mut builder.func)?;
                                    let get_stack_info = self.import_stub("nyx_jit_get_stack_info", &[ptr_type], &[ptr_type, types::I64], &mut builder.func)?;
                                    let set_field_cached = self.import_stub("nyx_jit_set_field_cached", &[ptr_type, ptr_type, ptr_type, ptr_type], &[types::I64], &mut builder.func)?;
                                    
                                    let stack_res = builder.ins().call(get_stack_info, &[rt_param]);
                                    let stack_ptr = builder.inst_results(stack_res)[0];
                                    let stack_len = builder.inst_results(stack_res)[1];
                                    
                                    let enough_stack = builder.ins().icmp_imm(IntCC::UnsignedGreaterThanOrEqual, stack_len, 3);
                                    let hit_label = builder.create_block();
                                    let miss_label = builder.create_block();
                                    let next_label = builder.create_block();
                                    
                                    builder.ins().brif(enough_stack, hit_label, &[], miss_label, &[]);
                                    builder.seal_block(hit_label);
                                    builder.switch_to_block(hit_label);
                                    
                                    // SetField pops [target, field, value].
                                    // Target is at stack_len - 3.
                                    let target_idx = builder.ins().iadd_imm(stack_len, -3);
                                    let target_offset = builder.ins().imul_imm(target_idx, 88);
                                    let target_val_addr = builder.ins().iadd(stack_ptr, target_offset);
                                    let target_tag = builder.ins().load(types::I64, MemFlags::new(), target_val_addr, 0);
                                    let is_pointer = builder.ins().icmp_imm(IntCC::Equal, target_tag, 10);
                                    
                                    let pointer_label = builder.create_block();
                                    builder.ins().brif(is_pointer, pointer_label, &[], miss_label, &[]);
                                    builder.seal_block(pointer_label);
                                    builder.switch_to_block(pointer_label);
                                    
                                    let target_ptr_val = builder.ins().load(ptr_type, MemFlags::new(), target_val_addr, 8);
                                    let ic_target_ptr = builder.ins().load(ptr_type, MemFlags::new(), ic_val, 0);
                                    let ptr_match = builder.ins().icmp(IntCC::Equal, target_ptr_val, ic_target_ptr);
                                    
                                    let match_label = builder.create_block();
                                    builder.ins().brif(ptr_match, match_label, &[], miss_label, &[]);
                                    builder.seal_block(match_label);
                                    builder.switch_to_block(match_label);
                                    
                                    let heap_res = builder.ins().call(get_heap_info, &[rt_param]);
                                    let heap_slots_ptr = builder.inst_results(heap_res)[0];
                                    let heap_slots_len = builder.inst_results(heap_res)[1];
                                    
                                    let in_bounds = builder.ins().icmp(IntCC::UnsignedLessThan, target_ptr_val, heap_slots_len);
                                    let bounds_label = builder.create_block();
                                    builder.ins().brif(in_bounds, bounds_label, &[], miss_label, &[]);
                                    builder.seal_block(bounds_label);
                                    builder.switch_to_block(bounds_label);
                                    
                                    let slot_ptr_offset = builder.ins().imul_imm(target_ptr_val, 8);
                                    let slot_ptr_addr = builder.ins().iadd(heap_slots_ptr, slot_ptr_offset);
                                    let slot_ptr = builder.ins().load(ptr_type, MemFlags::new(), slot_ptr_addr, 0);
                                    let slot_exists = builder.ins().icmp_imm(IntCC::NotEqual, slot_ptr, 0);
                                    
                                    let exists_label = builder.create_block();
                                    builder.ins().brif(slot_exists, exists_label, &[], miss_label, &[]);
                                    builder.seal_block(exists_label);
                                    builder.switch_to_block(exists_label);
                                    
                                    let slot_gen = builder.ins().load(types::I64, MemFlags::new(), slot_ptr, 0);
                                    let slot_ver = builder.ins().load(types::I64, MemFlags::new(), slot_ptr, 8);
                                    let ic_gen = builder.ins().load(types::I64, MemFlags::new(), ic_val, 8);
                                    let ic_ver = builder.ins().load(types::I64, MemFlags::new(), ic_val, 16);
                                    
                                    let gen_match = builder.ins().icmp(IntCC::Equal, slot_gen, ic_gen);
                                    let ver_match = builder.ins().icmp(IntCC::Equal, slot_ver, ic_ver);
                                    let full_match = builder.ins().band(gen_match, ver_match);
                                    
                                    let final_hit_label = builder.create_block();
                                    builder.ins().brif(full_match, final_hit_label, &[], miss_label, &[]);
                                    builder.seal_block(final_hit_label);
                                    builder.switch_to_block(final_hit_label);
                                    
                                    // Full IC hit! Still call the cached stub but we know it's a hit.
                                    let res = call4_ptr_ptr(&mut builder, set_field_cached, rt_param, ic_val, name_ptr_val, name_len_val);
                                    let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                                    builder.ins().brif(is_err, trap_block, &[], next_label, &[]);
                                    
                                    builder.seal_block(miss_label);
                                    builder.switch_to_block(miss_label);
                                    let res_miss = call4_ptr_ptr(&mut builder, set_field_cached, rt_param, ic_val, name_ptr_val, name_len_val);
                                    let is_err_miss = builder.ins().icmp_imm(IntCC::Equal, res_miss, -1);
                                    builder.ins().brif(is_err_miss, trap_block, &[], next_label, &[]);
                                    
                                    builder.seal_block(next_label);
                                    builder.switch_to_block(next_label);
                                    builder.ins().jump(blocks[ip + 1], &[]);
                                    specialized = true;
                                }
                            }
                        }
                        if !specialized {
                            let res = call0(&mut builder, set_field, rt_param);
                            let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                            builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        }
                        continue;
                    }
                    OpCode::GetIndex => {
                        let mut specialized = false;
                        if ip > 0 && func.instructions[ip - 1].opcode == OpCode::PUSH {
                            let c_idx = func.instructions[ip - 1].operands[0] as usize;
                            if let Some(Value::Int(idx)) = func.constants.get(c_idx) {
                                if let Some(ref ic_buffer) = index_ic_buffer {
                                    let ic_ptr = &ic_buffer[index_ic_idx] as *const _ as i64;
                                    index_ic_idx += 1;
                                    let ic_val = builder.ins().iconst(ptr_type, ic_ptr);
                                    let idx_val = builder.ins().iconst(types::I64, *idx);
                                    
                                    let get_heap_info = self.import_stub("nyx_jit_get_heap_info", &[ptr_type], &[ptr_type, types::I64], &mut builder.func)?;
                                    let get_stack_info = self.import_stub("nyx_jit_get_stack_info", &[ptr_type], &[ptr_type, types::I64], &mut builder.func)?;
                                    let push_index_ic_value = self.import_stub("nyx_jit_push_index_ic_value", &[ptr_type, ptr_type], &[types::I64], &mut builder.func)?;
                                    let get_index_cached = self.import_stub("nyx_jit_get_index_cached", &[ptr_type, ptr_type, types::I64], &[types::I64], &mut builder.func)?;
                                    
                                    let stack_res = builder.ins().call(get_stack_info, &[rt_param]);
                                    let stack_ptr = builder.inst_results(stack_res)[0];
                                    let stack_len = builder.inst_results(stack_res)[1];
                                    
                                    let enough_stack = builder.ins().icmp_imm(IntCC::UnsignedGreaterThanOrEqual, stack_len, 2);
                                    let hit_label = builder.create_block();
                                    let miss_label = builder.create_block();
                                    let next_label = builder.create_block();
                                    
                                    builder.ins().brif(enough_stack, hit_label, &[], miss_label, &[]);
                                    builder.seal_block(hit_label);
                                    builder.switch_to_block(hit_label);
                                    
                                    // GetIndex pops [target, index]. Top is index.
                                    // Target is at stack_len - 2.
                                    let target_idx = builder.ins().iadd_imm(stack_len, -2);
                                    let target_offset = builder.ins().imul_imm(target_idx, 88);
                                    let target_val_addr = builder.ins().iadd(stack_ptr, target_offset);
                                    let target_tag = builder.ins().load(types::I64, MemFlags::new(), target_val_addr, 0);
                                    let is_pointer = builder.ins().icmp_imm(IntCC::Equal, target_tag, 10);
                                    
                                    let pointer_label = builder.create_block();
                                    builder.ins().brif(is_pointer, pointer_label, &[], miss_label, &[]);
                                    builder.seal_block(pointer_label);
                                    builder.switch_to_block(pointer_label);
                                    
                                    let target_ptr_val = builder.ins().load(ptr_type, MemFlags::new(), target_val_addr, 8);
                                    let ic_target_ptr = builder.ins().load(ptr_type, MemFlags::new(), ic_val, 0);
                                    let ic_index = builder.ins().load(types::I64, MemFlags::new(), ic_val, 8);
                                    let ptr_match = builder.ins().icmp(IntCC::Equal, target_ptr_val, ic_target_ptr);
                                    let idx_match = builder.ins().icmp(IntCC::Equal, idx_val, ic_index);
                                    let basic_match = builder.ins().band(ptr_match, idx_match);
                                    
                                    let match_label = builder.create_block();
                                    builder.ins().brif(basic_match, match_label, &[], miss_label, &[]);
                                    builder.seal_block(match_label);
                                    builder.switch_to_block(match_label);
                                    
                                    let heap_res = builder.ins().call(get_heap_info, &[rt_param]);
                                    let heap_slots_ptr = builder.inst_results(heap_res)[0];
                                    let heap_slots_len = builder.inst_results(heap_res)[1];
                                    
                                    let in_bounds = builder.ins().icmp(IntCC::UnsignedLessThan, target_ptr_val, heap_slots_len);
                                    let bounds_label = builder.create_block();
                                    builder.ins().brif(in_bounds, bounds_label, &[], miss_label, &[]);
                                    builder.seal_block(bounds_label);
                                    builder.switch_to_block(bounds_label);
                                    
                                    let slot_ptr_offset = builder.ins().imul_imm(target_ptr_val, 8);
                                    let slot_ptr_addr = builder.ins().iadd(heap_slots_ptr, slot_ptr_offset);
                                    let slot_ptr = builder.ins().load(ptr_type, MemFlags::new(), slot_ptr_addr, 0);
                                    let slot_exists = builder.ins().icmp_imm(IntCC::NotEqual, slot_ptr, 0);
                                    
                                    let exists_label = builder.create_block();
                                    builder.ins().brif(slot_exists, exists_label, &[], miss_label, &[]);
                                    builder.seal_block(exists_label);
                                    builder.switch_to_block(exists_label);
                                    
                                    let slot_gen = builder.ins().load(types::I64, MemFlags::new(), slot_ptr, 0);
                                    let slot_ver = builder.ins().load(types::I64, MemFlags::new(), slot_ptr, 8);
                                    let ic_gen = builder.ins().load(types::I64, MemFlags::new(), ic_val, 16);
                                    let ic_ver = builder.ins().load(types::I64, MemFlags::new(), ic_val, 24);
                                    
                                    let gen_match = builder.ins().icmp(IntCC::Equal, slot_gen, ic_gen);
                                    let ver_match = builder.ins().icmp(IntCC::Equal, slot_ver, ic_ver);
                                    let full_match = builder.ins().band(gen_match, ver_match);
                                    
                                    let final_hit_label = builder.create_block();
                                    builder.ins().brif(full_match, final_hit_label, &[], miss_label, &[]);
                                    builder.seal_block(final_hit_label);
                                    builder.switch_to_block(final_hit_label);
                                    
                                    let _ = builder.ins().call(pop, &[rt_param]);
                                    let _ = builder.ins().call(pop, &[rt_param]);
                                    let push_res = builder.ins().call(push_index_ic_value, &[rt_param, ic_val]);
                                    let push_val = builder.inst_results(push_res)[0];
                                    let is_push_err = builder.ins().icmp_imm(IntCC::Equal, push_val, -1);
                                    builder.ins().brif(is_push_err, trap_block, &[], next_label, &[]);
                                    
                                    builder.seal_block(miss_label);
                                    builder.switch_to_block(miss_label);
                                    let res = builder.ins().call(get_index_cached, &[rt_param, ic_val, idx_val]);
                                    let res_val = builder.inst_results(res)[0];
                                    let is_err = builder.ins().icmp_imm(IntCC::Equal, res_val, -1);
                                    builder.ins().brif(is_err, trap_block, &[], next_label, &[]);
                                    
                                    builder.seal_block(next_label);
                                    builder.switch_to_block(next_label);
                                    builder.ins().jump(blocks[ip + 1], &[]);
                                    specialized = true;
                                }
                            }
                        }
                        if !specialized {
                            let res = call0(&mut builder, get_index, rt_param);
                            let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                            builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        }
                        continue;
                    }
                    OpCode::SetIndex => {
                        let mut specialized = false;
                        if ip > 0 && func.instructions[ip - 1].opcode == OpCode::PUSH {
                            let c_idx = func.instructions[ip - 1].operands[0] as usize;
                            if let Some(Value::Int(idx)) = func.constants.get(c_idx) {
                                if let Some(ref ic_buffer) = index_ic_buffer {
                                    let ic_ptr = &ic_buffer[index_ic_idx] as *const _ as i64;
                                    index_ic_idx += 1;
                                    let ic_val = builder.ins().iconst(ptr_type, ic_ptr);
                                    let idx_val = builder.ins().iconst(types::I64, *idx);
                                    
                                    let get_heap_info = self.import_stub("nyx_jit_get_heap_info", &[ptr_type], &[ptr_type, types::I64], &mut builder.func)?;
                                    let get_stack_info = self.import_stub("nyx_jit_get_stack_info", &[ptr_type], &[ptr_type, types::I64], &mut builder.func)?;
                                    let set_index_cached = self.import_stub("nyx_jit_set_index_cached", &[ptr_type, ptr_type, types::I64], &[types::I64], &mut builder.func)?;
                                    
                                    let stack_res = builder.ins().call(get_stack_info, &[rt_param]);
                                    let stack_ptr = builder.inst_results(stack_res)[0];
                                    let stack_len = builder.inst_results(stack_res)[1];
                                    
                                    let enough_stack = builder.ins().icmp_imm(IntCC::UnsignedGreaterThanOrEqual, stack_len, 3);
                                    let hit_label = builder.create_block();
                                    let miss_label = builder.create_block();
                                    let next_label = builder.create_block();
                                    
                                    builder.ins().brif(enough_stack, hit_label, &[], miss_label, &[]);
                                    builder.seal_block(hit_label);
                                    builder.switch_to_block(hit_label);
                                    
                                    // SetIndex pops [target, index, value].
                                    // Target at stack_len - 3.
                                    let target_idx = builder.ins().iadd_imm(stack_len, -3);
                                    let target_offset = builder.ins().imul_imm(target_idx, 88);
                                    let target_val_addr = builder.ins().iadd(stack_ptr, target_offset);
                                    let target_tag = builder.ins().load(types::I64, MemFlags::new(), target_val_addr, 0);
                                    let is_pointer = builder.ins().icmp_imm(IntCC::Equal, target_tag, 10);
                                    
                                    let pointer_label = builder.create_block();
                                    builder.ins().brif(is_pointer, pointer_label, &[], miss_label, &[]);
                                    builder.seal_block(pointer_label);
                                    builder.switch_to_block(pointer_label);
                                    
                                    let target_ptr_val = builder.ins().load(ptr_type, MemFlags::new(), target_val_addr, 8);
                                    let ic_target_ptr = builder.ins().load(ptr_type, MemFlags::new(), ic_val, 0);
                                    let ic_index = builder.ins().load(types::I64, MemFlags::new(), ic_val, 8);
                                    
                                    let ptr_match = builder.ins().icmp(IntCC::Equal, target_ptr_val, ic_target_ptr);
                                    let idx_match = builder.ins().icmp(IntCC::Equal, idx_val, ic_index);
                                    let match_both = builder.ins().band(ptr_match, idx_match);
                                    
                                    let match_label = builder.create_block();
                                    builder.ins().brif(match_both, match_label, &[], miss_label, &[]);
                                    builder.seal_block(match_label);
                                    builder.switch_to_block(match_label);
                                    
                                    let heap_res = builder.ins().call(get_heap_info, &[rt_param]);
                                    let heap_slots_ptr = builder.inst_results(heap_res)[0];
                                    let heap_slots_len = builder.inst_results(heap_res)[1];
                                    
                                    let in_bounds = builder.ins().icmp(IntCC::UnsignedLessThan, target_ptr_val, heap_slots_len);
                                    let bounds_label = builder.create_block();
                                    builder.ins().brif(in_bounds, bounds_label, &[], miss_label, &[]);
                                    builder.seal_block(bounds_label);
                                    builder.switch_to_block(bounds_label);
                                    
                                    let slot_ptr_offset = builder.ins().imul_imm(target_ptr_val, 8);
                                    let slot_ptr_addr = builder.ins().iadd(heap_slots_ptr, slot_ptr_offset);
                                    let slot_ptr = builder.ins().load(ptr_type, MemFlags::new(), slot_ptr_addr, 0);
                                    let slot_exists = builder.ins().icmp_imm(IntCC::NotEqual, slot_ptr, 0);
                                    
                                    let exists_label = builder.create_block();
                                    builder.ins().brif(slot_exists, exists_label, &[], miss_label, &[]);
                                    builder.seal_block(exists_label);
                                    builder.switch_to_block(exists_label);
                                    
                                    let slot_gen = builder.ins().load(types::I64, MemFlags::new(), slot_ptr, 0);
                                    let slot_ver = builder.ins().load(types::I64, MemFlags::new(), slot_ptr, 8);
                                    let ic_gen = builder.ins().load(types::I64, MemFlags::new(), ic_val, 16);
                                    let ic_ver = builder.ins().load(types::I64, MemFlags::new(), ic_val, 24);
                                    
                                    let gen_match = builder.ins().icmp(IntCC::Equal, slot_gen, ic_gen);
                                    let ver_match = builder.ins().icmp(IntCC::Equal, slot_ver, ic_ver);
                                    let full_match = builder.ins().band(gen_match, ver_match);
                                    
                                    let final_hit_label = builder.create_block();
                                    builder.ins().brif(full_match, final_hit_label, &[], miss_label, &[]);
                                    builder.seal_block(final_hit_label);
                                    builder.switch_to_block(final_hit_label);
                                    
                                    // Full IC hit!
                                    let res = builder.ins().call(set_index_cached, &[rt_param, ic_val, idx_val]);
                                    let res_val = builder.inst_results(res)[0];
                                    let is_err = builder.ins().icmp_imm(IntCC::Equal, res_val, -1);
                                    builder.ins().brif(is_err, trap_block, &[], next_label, &[]);
                                    
                                    builder.seal_block(miss_label);
                                    builder.switch_to_block(miss_label);
                                    let res_miss = builder.ins().call(set_index_cached, &[rt_param, ic_val, idx_val]);
                                    let res_val_miss = builder.inst_results(res_miss)[0];
                                    let is_err_miss = builder.ins().icmp_imm(IntCC::Equal, res_val_miss, -1);
                                    builder.ins().brif(is_err_miss, trap_block, &[], next_label, &[]);
                                    
                                    builder.seal_block(next_label);
                                    builder.switch_to_block(next_label);
                                    specialized = true;
                                }
                            }
                        }
                        if !specialized {
                            let res = call0(&mut builder, set_index, rt_param);
                            let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                            builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        }
                        continue;
                    }
                    OpCode::LEN => {
                        let res = call0(&mut builder, len, rt_param);
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                    OpCode::SLICE => {
                        let res = call0(&mut builder, slice, rt_param);
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                    OpCode::GetGlobal => {
                        let idx = *instr
                            .operands
                            .get(0)
                            .ok_or_else(|| "GetGlobal missing operand".to_string())?;
                        let res = call1_i32(&mut builder, get_global, rt_param, idx);
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                    OpCode::SetGlobal => {
                        let idx = *instr
                            .operands
                            .get(0)
                            .ok_or_else(|| "SetGlobal missing operand".to_string())?;
                        let res = call1_i32(&mut builder, set_global, rt_param, idx);
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                    OpCode::GetGlobalM => {
                        let idx = *instr
                            .operands
                            .get(0)
                            .ok_or_else(|| "GetGlobalM missing operand".to_string())?;
                        let res = call1_i32(&mut builder, get_global_m, rt_param, idx);
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                    OpCode::SetGlobalM => {
                        let idx = *instr
                            .operands
                            .get(0)
                            .ok_or_else(|| "SetGlobalM missing operand".to_string())?;
                        let res = call1_i32(&mut builder, set_global_m, rt_param, idx);
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                    OpCode::ALLOC => {
                        let hint = *instr
                            .operands
                            .get(0)
                            .ok_or_else(|| "ALLOC missing operand".to_string())?;
                        let res = call1_i32(&mut builder, alloc, rt_param, hint);
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                    OpCode::FREE => {
                        let res = call0(&mut builder, free, rt_param);
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                    OpCode::JMP => {
                        let target = *instr
                            .operands
                            .get(0)
                            .ok_or_else(|| "JMP missing operand".to_string())? as usize;
                        if target >= func.instructions.len() {
                            return Err("Invalid jump target".to_string());
                        }
                        builder.ins().jump(blocks[target], &[]);
                        continue;
                    }
                    OpCode::JZ | OpCode::JNZ => {
                        let target = *instr.operands.get(0).ok_or("JZ/JNZ missing target")? as usize;
                        if target >= func.instructions.len() {
                            return Err("Invalid jump target".to_string());
                        }
                        
                        let stack_info_fn = self.import_stub("nyx_jit_get_stack_info", &[ptr_type], &[ptr_type, types::I64], &mut builder.func)?;
                        let call_stack = builder.ins().call(stack_info_fn, &[rt_param]);
                        let stack_ptr = builder.inst_results(call_stack)[0];
                        let stack_len = builder.inst_results(call_stack)[1];
                        
                        let enough_stack = builder.ins().icmp_imm(IntCC::UnsignedGreaterThanOrEqual, stack_len, 1);
                        let hit_label = builder.create_block();
                        let miss_label = builder.create_block();
                        builder.ins().brif(enough_stack, hit_label, &[], miss_label, &[]);
                        builder.seal_block(hit_label);
                        builder.switch_to_block(hit_label);
                        
                        let idx = builder.ins().iadd_imm(stack_len, -1);
                        let off = builder.ins().imul_imm(idx, 88);
                        let addr = builder.ins().iadd(stack_ptr, off);
                        
                        let tag = builder.ins().load(types::I64, MemFlags::new(), addr, 0);
                        let val = builder.ins().load(types::I64, MemFlags::new(), addr, 8);
                        
                        let is_int = builder.ins().icmp_imm(IntCC::Equal, tag, 2);
                        let is_bool = builder.ins().icmp_imm(IntCC::Equal, tag, 1);
                        let is_fast = builder.ins().bor(is_int, is_bool);
                        
                        let fast_path = builder.create_block();
                        builder.ins().brif(is_fast, fast_path, &[], miss_label, &[]);
                        builder.seal_block(fast_path);
                        builder.switch_to_block(fast_path);
                        
                        let val_masked = builder.ins().band_imm(val, 0xFF);
                        let safe_val = builder.ins().select(is_bool, val_masked, val);
                        let truthy_val = builder.ins().icmp_imm(IntCC::NotEqual, safe_val, 0);
                        let is_false = builder.ins().icmp_imm(IntCC::Equal, truthy_val, 0);
                        
                        let set_stack_len_fn = self.import_stub("nyx_jit_set_stack_len", &[ptr_type, types::I64], &[types::I64], &mut builder.func)?;
                        let new_len = builder.ins().iadd_imm(stack_len, -1);
                        let _ = builder.ins().call(set_stack_len_fn, &[rt_param, new_len]);
                        
                        let (then_block, else_block) = if instr.opcode == OpCode::JZ {
                            (blocks[target], blocks[ip + 1])
                        } else {
                            (blocks[ip + 1], blocks[target])
                        };
                        builder.ins().brif(is_false, then_block, &[], else_block, &[]);
                        
                        builder.seal_block(miss_label);
                        builder.switch_to_block(miss_label);
                        let call = builder.ins().call(pop_truthy, &[rt_param]);
                        let truthy = builder.inst_results(call)[0];
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, truthy, -1);
                        let after_pop = builder.create_block();
                        builder.ins().brif(is_err, trap_block, &[], after_pop, &[]);
                        builder.seal_block(after_pop);
                        builder.switch_to_block(after_pop);
                        let is_false_slow = builder.ins().icmp_imm(IntCC::Equal, truthy, 0);
                        builder.ins().brif(is_false_slow, then_block, &[], else_block, &[]);
                        continue;
                    }
                                        OpCode::RET => {
                        let res = call0(&mut builder, ret, rt_param);
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                        builder.ins().brif(is_err, trap_block, &[], exit_block, &[]);
                        continue;
                    }
                    OpCode::HALT => {
                        let res = call0(&mut builder, halt, rt_param);
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                        builder.ins().brif(is_err, trap_block, &[], exit_block, &[]);
                        continue;
                    }
                    OpCode::CALL => {
                        let func_raw = *instr.operands.get(0).ok_or("CALL missing func_raw")?;
                        let num_args = *instr.operands.get(1).ok_or("CALL missing num_args")?;
                        
                        // Sync IP BEFORE call, so the caller frame has the correct resume point.
                        let set_ip = self.import_stub("nyx_jit_set_ip", &[ptr_type, types::I32], &[types::I64], &mut builder.func)?;
                        let next_ip = builder.ins().iconst(types::I32, (ip + 1) as i64);
                        let _ = builder.ins().call(set_ip, &[rt_param, next_ip]);
                        
                        let res = call2_i32(&mut builder, jit_call, rt_param, func_raw, num_args);
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                        builder.ins().brif(is_err, trap_block, &[], yield_block, &[]);
                        continue;
                    }
                    OpCode::CallExt => {
                        let name_idx = *instr.operands.get(0).ok_or("CallExt missing name_idx")?;
                        let num_args = *instr.operands.get(1).ok_or("CallExt missing num_args")?;
                        let res = call2_i32(&mut builder, jit_call_ext, rt_param, name_idx, num_args);
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                    OpCode::CLOSURE => {
                        let func_idx = *instr.operands.get(0).ok_or("CLOSURE missing func_idx")?;
                        let num_upvalues = *instr.operands.get(1).ok_or("CLOSURE missing num_upvalues")?;
                        let res = call2_i32(&mut builder, jit_closure, rt_param, func_idx, num_upvalues);
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                    OpCode::ClosureRef => {
                        let func_idx = *instr.operands.get(0).ok_or("ClosureRef missing func_idx")?;
                        let num_upvalues = *instr.operands.get(1).ok_or("ClosureRef missing num_upvalues")? as usize;
                        let upvalue_indices = &instr.operands[2..2 + num_upvalues];
                        
                        // Create a stack slot for the upvalue indices.
                        let slot = builder.create_sized_stack_slot(StackSlotData::new(
                            StackSlotKind::ExplicitSlot,
                            (num_upvalues * 4) as u32,
                            2, // 4-byte alignment
                        ));
                        let addr = builder.ins().stack_addr(ptr_type, slot, 0);
                        for (i, &idx) in upvalue_indices.iter().enumerate() {
                            let val = builder.ins().iconst(types::I32, idx as i64);
                            builder.ins().store(MemFlags::new(), val, addr, (i * 4) as i32);
                        }

                        let res = call3_i32_ptr(&mut builder, jit_closure_ref, rt_param, func_idx, num_upvalues as i32, addr);
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                    OpCode::ClosureRefStack => {
                        let func_idx = *instr.operands.get(0).ok_or("ClosureRefStack missing func_idx")?;
                        let num_upvalues = *instr.operands.get(1).ok_or("ClosureRefStack missing num_upvalues")?;
                        let res = call2_i32(&mut builder, jit_closure_ref_stack, rt_param, func_idx, num_upvalues);
                        let is_err = builder.ins().icmp_imm(IntCC::Equal, res, -1);
                        builder.ins().brif(is_err, trap_block, &[], blocks[ip + 1], &[]);
                        continue;
                    }
                    _ => return Err(format!("Unsupported opcode for VM-aware JIT: {:?}", instr.opcode)),
                }

                builder.ins().jump(blocks[ip + 1], &[]);
            }

            builder.switch_to_block(blocks[func.instructions.len()]);
            let _ = call0(&mut builder, halt, rt_param);
            builder.ins().jump(exit_block, &[]);

             builder.switch_to_block(yield_block);
             let one = builder.ins().iconst(types::I64, 1);
             builder.ins().return_(&[one]);

            builder.switch_to_block(trap_block);
            let neg1 = builder.ins().iconst(types::I64, -1);
            builder.ins().return_(&[neg1]);

            builder.switch_to_block(exit_block);
            let zero = builder.ins().iconst(types::I64, 0);
            builder.ins().return_(&[zero]);

            builder.seal_all_blocks();
            builder.finalize();

            self.module.define_function(func_id, &mut ctx).map_err(|e| e.to_string())?;
            self.module.clear_context(&mut ctx);
            self.module.finalize_definitions().map_err(|e| e.to_string())?;
            let code = self.module.get_finalized_function(func_id);

            let result = VmJitResult { 
                func_ptr: code,
                attr_ic_buffer,
                index_ic_buffer,
            };
            self.vm_functions.insert((module_name.to_string(), func_idx), result.clone());
            Ok(result)
        }
    }

    pub fn call_i64(jit: &JitResult, args: &[i64]) -> i64 {
        assert_eq!(jit.plan.num_kind, JitNumKind::I64);
        assert!(matches!(jit.plan.ret_kind, JitRetKind::I64 | JitRetKind::Bool));
        assert_eq!(jit.arity, args.len());
        unsafe {
            match args.len() {
                0 => {
                    let f: unsafe extern "C" fn() -> i64 = std::mem::transmute(jit.func_ptr);
                    f()
                }
                1 => {
                    let f: unsafe extern "C" fn(i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0])
                }
                2 => {
                    let f: unsafe extern "C" fn(i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1])
                }
                3 => {
                    let f: unsafe extern "C" fn(i64, i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2])
                }
                4 => {
                    let f: unsafe extern "C" fn(i64, i64, i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3])
                }
                5 => {
                    let f: unsafe extern "C" fn(i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4])
                }
                6 => {
                    let f: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5])
                }
                7 => {
                    let f: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6])
                }
                8 => {
                    let f: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7])
                }
                9 => {
                    let f: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8])
                }
                10 => {
                    let f: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9])
                }
                11 => {
                    let f: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10])
                }
                12 => {
                    let f: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11])
                }
                13 => {
                    let f: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12])
                }
                14 => {
                    let f: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13])
                }
                15 => {
                    let f: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14])
                }
                16 => {
                    let f: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15])
                }
                17 => {
                    let f: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16])
                }
                18 => {
                    let f: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16], args[17])
                }
                19 => {
                    let f: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16], args[17], args[18])
                }
                20 => {
                    let f: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16], args[17], args[18], args[19])
                }
                21 => {
                    let f: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16], args[17], args[18], args[19], args[20])
                }
                22 => {
                    let f: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16], args[17], args[18], args[19], args[20], args[21])
                }
                23 => {
                    let f: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16], args[17], args[18], args[19], args[20], args[21], args[22])
                }
                24 => {
                    let f: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16], args[17], args[18], args[19], args[20], args[21], args[22], args[23])
                }
                25 => {
                    let f: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16], args[17], args[18], args[19], args[20], args[21], args[22], args[23], args[24])
                }
                26 => {
                    let f: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16], args[17], args[18], args[19], args[20], args[21], args[22], args[23], args[24], args[25])
                }
                27 => {
                    let f: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16], args[17], args[18], args[19], args[20], args[21], args[22], args[23], args[24], args[25], args[26])
                }
                28 => {
                    let f: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16], args[17], args[18], args[19], args[20], args[21], args[22], args[23], args[24], args[25], args[26], args[27])
                }
                29 => {
                    let f: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16], args[17], args[18], args[19], args[20], args[21], args[22], args[23], args[24], args[25], args[26], args[27], args[28])
                }
                30 => {
                    let f: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16], args[17], args[18], args[19], args[20], args[21], args[22], args[23], args[24], args[25], args[26], args[27], args[28], args[29])
                }
                31 => {
                    let f: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16], args[17], args[18], args[19], args[20], args[21], args[22], args[23], args[24], args[25], args[26], args[27], args[28], args[29], args[30])
                }
                32 => {
                    let f: unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16], args[17], args[18], args[19], args[20], args[21], args[22], args[23], args[24], args[25], args[26], args[27], args[28], args[29], args[30], args[31])
                }
                _ => unreachable!("arity capped to 32 by plan()"),
            }
        }
    }

    pub fn call_f64(jit: &JitResult, args: &[f64]) -> f64 {
        assert_eq!(jit.plan.num_kind, JitNumKind::F64);
        assert_eq!(jit.plan.ret_kind, JitRetKind::F64);
        assert_eq!(jit.arity, args.len());
        unsafe {
            match args.len() {
                0 => {
                    let f: unsafe extern "C" fn() -> f64 = std::mem::transmute(jit.func_ptr);
                    f()
                }
                1 => {
                    let f: unsafe extern "C" fn(f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0])
                }
                2 => {
                    let f: unsafe extern "C" fn(f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1])
                }
                3 => {
                    let f: unsafe extern "C" fn(f64, f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2])
                }
                4 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3])
                }
                5 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4])
                }
                6 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5])
                }
                7 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6])
                }
                8 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7])
                }
                9 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8])
                }
                10 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9])
                }
                11 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10])
                }
                12 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11])
                }
                13 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12])
                }
                14 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13])
                }
                15 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14])
                }
                16 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15])
                }
                17 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16])
                }
                18 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16], args[17])
                }
                19 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16], args[17], args[18])
                }
                20 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16], args[17], args[18], args[19])
                }
                21 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16], args[17], args[18], args[19], args[20])
                }
                22 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16], args[17], args[18], args[19], args[20], args[21])
                }
                23 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16], args[17], args[18], args[19], args[20], args[21], args[22])
                }
                24 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16], args[17], args[18], args[19], args[20], args[21], args[22], args[23])
                }
                25 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16], args[17], args[18], args[19], args[20], args[21], args[22], args[23], args[24])
                }
                26 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16], args[17], args[18], args[19], args[20], args[21], args[22], args[23], args[24], args[25])
                }
                27 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16], args[17], args[18], args[19], args[20], args[21], args[22], args[23], args[24], args[25], args[26])
                }
                28 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16], args[17], args[18], args[19], args[20], args[21], args[22], args[23], args[24], args[25], args[26], args[27])
                }
                29 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16], args[17], args[18], args[19], args[20], args[21], args[22], args[23], args[24], args[25], args[26], args[27], args[28])
                }
                30 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16], args[17], args[18], args[19], args[20], args[21], args[22], args[23], args[24], args[25], args[26], args[27], args[28], args[29])
                }
                31 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16], args[17], args[18], args[19], args[20], args[21], args[22], args[23], args[24], args[25], args[26], args[27], args[28], args[29], args[30])
                }
                32 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16], args[17], args[18], args[19], args[20], args[21], args[22], args[23], args[24], args[25], args[26], args[27], args[28], args[29], args[30], args[31])
                }
                _ => unreachable!("arity capped to 32 by plan()"),
            }
        }
    }

    pub fn call_bool_from_f64(jit: &JitResult, args: &[f64]) -> i64 {
        assert_eq!(jit.plan.num_kind, JitNumKind::F64);
        assert_eq!(jit.plan.ret_kind, JitRetKind::Bool);
        assert_eq!(jit.arity, args.len());
        unsafe {
            match args.len() {
                0 => {
                    let f: unsafe extern "C" fn() -> i64 = std::mem::transmute(jit.func_ptr);
                    f()
                }
                1 => {
                    let f: unsafe extern "C" fn(f64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0])
                }
                2 => {
                    let f: unsafe extern "C" fn(f64, f64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1])
                }
                3 => {
                    let f: unsafe extern "C" fn(f64, f64, f64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2])
                }
                4 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3])
                }
                5 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4])
                }
                6 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5])
                }
                7 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6])
                }
                8 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7])
                }
                9 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8])
                }
                10 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9])
                }
                11 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10])
                }
                12 => {
                    let f: unsafe extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> i64 = std::mem::transmute(jit.func_ptr);
                    f(args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8], args[9], args[10], args[11])
                }
                _ => unreachable!("arity capped to 12 by plan()"),
            }
        }
    }

    /// # Safety
    ///
    /// `rt` must be a valid, non-null pointer to a `VmRuntime` that is properly
    /// initialised and not aliased for the duration of this call.
    pub unsafe fn call_vm(jit: &VmJitResult, rt: *mut crate::runtime::VmRuntime, ip: i32) -> i64 {
        let f: unsafe extern "C" fn(*mut crate::runtime::VmRuntime, i32) -> i64 = std::mem::transmute(jit.func_ptr);
        f(rt, ip)
    }

    pub fn new_engine() -> Result<JitEngine, String> {
        JitEngine::new()
    }

    pub fn plan(func: &Function) -> Option<JitPlan> {
        JitEngine::plan(func)
    }

    pub fn compile(engine: &mut JitEngine, module_name: &str, func_idx: usize, func: &Function, plan: JitPlan) -> Result<JitResult, String> {
        engine.compile(module_name, func_idx, func, plan)
    }

    pub fn vm_plan(func: &Function) -> bool {
        JitEngine::vm_plan(func)
    }

    pub fn compile_vm(engine: &mut JitEngine, module_name: &str, func_idx: usize, func: &Function) -> Result<VmJitResult, String> {
        engine.compile_vm(module_name, func_idx, func)
    }

    pub type Engine = JitEngine;
}

#[cfg(feature = "jit")]
pub use cranelift_impl::{
    call_bool_from_f64, call_f64, call_i64, call_vm, compile, compile_vm, new_engine, plan, vm_plan, Engine,
};

#[cfg(not(feature = "jit"))]
mod nojit_impl {
    use super::*;

    pub struct Engine;

    pub fn new_engine() -> Result<Engine, String> {
        Err("JIT feature not enabled".to_string())
    }

    pub fn plan(_func: &Function) -> Option<JitPlan> {
        None
    }

    pub fn compile(_engine: &mut Engine, _module_name: &str, _func_idx: usize, _func: &Function, _plan: JitPlan) -> Result<JitResult, String> {
        Err("JIT feature not enabled".to_string())
    }

    pub fn call_i64(_jit: &JitResult, _args: &[i64]) -> i64 {
        0
    }

    pub fn call_f64(_jit: &JitResult, _args: &[f64]) -> f64 {
        0.0
    }

    pub fn call_bool_from_f64(_jit: &JitResult, _args: &[f64]) -> i64 {
        0
    }

    pub fn vm_plan(_func: &Function) -> bool {
        false
    }

    pub fn compile_vm(_engine: &mut Engine, _module_name: &str, _func_idx: usize, _func: &Function) -> Result<VmJitResult, String> {
        Err("JIT feature not enabled".to_string())
    }

    /// # Safety
    ///
    /// `_rt` must be a valid, non-null pointer to a `VmRuntime` (no-jit stub, always returns -1).
    pub unsafe fn call_vm(_jit: &VmJitResult, _rt: *mut crate::runtime::VmRuntime, _ip: i32) -> i64 {
        -1
    }
}

#[cfg(not(feature = "jit"))]
pub use nojit_impl::{
    call_bool_from_f64, call_f64, call_i64, call_vm, compile, compile_vm, new_engine, plan, vm_plan, Engine,
};
