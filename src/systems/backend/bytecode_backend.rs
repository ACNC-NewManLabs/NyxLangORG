use crate::systems::ir::nyx_ir::{BinaryOp, Instruction, IrFunction, Module, Value};
use nyx_vm::bytecode::{
    BytecodeInstr, BytecodeModule, Function as VmFunction, OpCode, Value as VmValue,
};
use std::collections::HashMap;

pub struct BytecodeBackend;

impl BytecodeBackend {
    pub fn lower_to_bytecode(&self, module: &Module) -> Result<BytecodeModule, String> {
        let mut bc_module = BytecodeModule::new("main".to_string());

        // Registry of function names to indices
        let mut fn_indices = HashMap::new();
        for (i, func) in module.functions.iter().enumerate() {
            fn_indices.insert(func.name.clone(), i);
        }

        for func in &module.functions {
            let mut lowerer = FunctionLowerer::new(func, &fn_indices);
            let vm_func = lowerer.lower()?;
            bc_module.add_function(vm_func);
        }

        Ok(bc_module)
    }
}

struct FunctionLowerer<'a> {
    ir_func: &'a IrFunction,
    fn_indices: &'a HashMap<String, usize>,
    locals: HashMap<String, u32>,
    local_counter: u32,
    instructions: Vec<BytecodeInstr>,
    labels: HashMap<String, usize>,
    pending_jumps: Vec<(usize, String)>,
    constants: Vec<VmValue>,
}

impl<'a> FunctionLowerer<'a> {
    fn new(ir_func: &'a IrFunction, fn_indices: &'a HashMap<String, usize>) -> Self {
        let mut locals = HashMap::new();
        let mut local_counter = 0;

        // Params are the first locals
        for param in &ir_func.params {
            locals.insert(param.name.clone(), local_counter);
            local_counter += 1;
        }

        Self {
            ir_func,
            fn_indices,
            locals,
            local_counter,
            instructions: Vec::new(),
            labels: HashMap::new(),
            pending_jumps: Vec::new(),
            constants: Vec::new(),
        }
    }

    fn lower(&mut self) -> Result<VmFunction, String> {
        for instr in &self.ir_func.instructions {
            self.lower_instr(instr)?;
        }

        // Patch jumps
        let pending = std::mem::take(&mut self.pending_jumps);
        for (instr_idx, label_name) in pending {
            let target_pc = *self
                .labels
                .get(&label_name)
                .ok_or_else(|| format!("Undefined label: {}", label_name))?;
            self.instructions[instr_idx].operands[0] = target_pc as i32;
        }

        Ok(VmFunction {
            name: self.ir_func.name.clone(),
            arity: self.ir_func.params.len(),
            num_locals: self.local_counter as usize,
            instructions: self.instructions.clone(),
            constants: self.constants.clone(),
            upvalues: Vec::new(),
            line_info: Vec::new(),
        })
    }

    fn get_or_create_local(&mut self, name: &str) -> u32 {
        if let Some(&idx) = self.locals.get(name) {
            idx
        } else {
            let idx = self.local_counter;
            self.locals.insert(name.to_string(), idx);
            self.local_counter += 1;
            idx
        }
    }

    fn add_constant(&mut self, val: VmValue) -> usize {
        let idx = self.constants.len();
        self.constants.push(val);
        idx
    }

    fn push_value(&mut self, val: &Value) -> Result<(), String> {
        match val {
            Value::Int(n) => {
                let const_idx = self.add_constant(VmValue::Int(*n));
                self.instructions.push(BytecodeInstr::with_operand(
                    OpCode::PUSH,
                    const_idx as i32,
                    0,
                ));
            }
            Value::Float(f) => {
                let const_idx = self.add_constant(VmValue::Float(*f));
                self.instructions.push(BytecodeInstr::with_operand(
                    OpCode::PUSH,
                    const_idx as i32,
                    0,
                ));
            }
            Value::Bool(b) => {
                let const_idx = self.add_constant(VmValue::Bool(*b));
                self.instructions.push(BytecodeInstr::with_operand(
                    OpCode::PUSH,
                    const_idx as i32,
                    0,
                ));
            }
            Value::Str(s) => {
                let const_idx = self.add_constant(VmValue::String(s.clone()));
                self.instructions.push(BytecodeInstr::with_operand(
                    OpCode::PUSH,
                    const_idx as i32,
                    0,
                ));
            }
            Value::Null => {
                let const_idx = self.add_constant(VmValue::Null);
                self.instructions.push(BytecodeInstr::with_operand(
                    OpCode::PUSH,
                    const_idx as i32,
                    0,
                ));
            }
            Value::Local(name) | Value::Temp(name) => {
                let idx = self.get_or_create_local(name);
                self.instructions
                    .push(BytecodeInstr::with_operand(OpCode::LOAD, idx as i32, 0));
            }
        }
        Ok(())
    }

    fn lower_instr(&mut self, instr: &Instruction) -> Result<(), String> {
        match instr {
            Instruction::Let { name, value } => {
                self.push_value(value)?;
                let idx = self.get_or_create_local(name);
                self.instructions
                    .push(BytecodeInstr::with_operand(OpCode::STORE, idx as i32, 0));
            }
            Instruction::Binary { dst, op, lhs, rhs } => {
                self.push_value(lhs)?;
                self.push_value(rhs)?;
                let opcode = match op {
                    BinaryOp::Add => OpCode::ADD,
                    BinaryOp::Sub => OpCode::SUB,
                    BinaryOp::Mul => OpCode::MUL,
                    BinaryOp::Div => OpCode::DIV,
                    BinaryOp::Mod => OpCode::MOD,
                    BinaryOp::Eq => OpCode::EQ,
                    BinaryOp::Ne => OpCode::NE,
                    BinaryOp::Lt => OpCode::LT,
                    BinaryOp::Le => OpCode::LE,
                    BinaryOp::Gt => OpCode::GT,
                    BinaryOp::Ge => OpCode::GE,
                    BinaryOp::And => OpCode::AND,
                    BinaryOp::Or => OpCode::OR,
                    BinaryOp::BitAnd => OpCode::BAND,
                    BinaryOp::BitOr => OpCode::BOR,
                    BinaryOp::BitXor => OpCode::BXOR,
                    BinaryOp::Shl => OpCode::SHL,
                    BinaryOp::Shr => OpCode::SHR,
                    _ => return Err(format!("Unsupported binary op: {:?}", op)),
                };
                self.instructions
                    .push(BytecodeInstr::new(opcode, vec![], 0));
                let dst_idx = self.get_or_create_local(dst);
                self.instructions.push(BytecodeInstr::with_operand(
                    OpCode::STORE,
                    dst_idx as i32,
                    0,
                ));
            }
            Instruction::Print { value } => {
                self.push_value(value)?;
                // Call external "print"
                let name_idx = self.add_constant(VmValue::String("print".to_string()));
                self.instructions.push(BytecodeInstr::new(
                    OpCode::CallExt,
                    vec![name_idx as i32, 1],
                    0,
                ));
            }
            Instruction::Return { value } => {
                if let Some(v) = value {
                    self.push_value(v)?;
                } else {
                    let const_idx = self.add_constant(VmValue::Null);
                    self.instructions.push(BytecodeInstr::with_operand(
                        OpCode::PUSH,
                        const_idx as i32,
                        0,
                    ));
                }
                self.instructions
                    .push(BytecodeInstr::new(OpCode::RET, vec![], 0));
            }
            Instruction::Label(name) => {
                self.labels.insert(name.clone(), self.instructions.len());
            }
            Instruction::Jump(name) => {
                let idx = self.instructions.len();
                self.instructions
                    .push(BytecodeInstr::with_operand(OpCode::JMP, 0, 0));
                self.pending_jumps.push((idx, name.clone()));
            }
            Instruction::Branch {
                cond,
                then_label,
                else_label,
            } => {
                self.push_value(cond)?;
                let jnz_idx = self.instructions.len();
                self.instructions
                    .push(BytecodeInstr::with_operand(OpCode::JNZ, 0, 0));
                self.pending_jumps.push((jnz_idx, then_label.clone()));

                let jmp_idx = self.instructions.len();
                self.instructions
                    .push(BytecodeInstr::with_operand(OpCode::JMP, 0, 0));
                self.pending_jumps.push((jmp_idx, else_label.clone()));
            }
            Instruction::Call { dst, callee, args } => {
                for arg in args {
                    self.push_value(arg)?;
                }
                if let Some(&idx) = self.fn_indices.get(callee) {
                    self.instructions.push(BytecodeInstr::new(
                        OpCode::CALL,
                        vec![idx as i32, args.len() as i32],
                        0,
                    ));
                } else {
                    // Try external call
                    let name_idx = self.add_constant(VmValue::String(callee.clone()));
                    self.instructions.push(BytecodeInstr::new(
                        OpCode::CallExt,
                        vec![name_idx as i32, args.len() as i32],
                        0,
                    ));
                }
                let dst_idx = self.get_or_create_local(dst);
                self.instructions.push(BytecodeInstr::with_operand(
                    OpCode::STORE,
                    dst_idx as i32,
                    0,
                ));
            }
            Instruction::StructInit {
                dst,
                struct_name: _,
                fields,
            } => {
                // How to handle structs? The VM has NewObj and SetField.
                self.instructions
                    .push(BytecodeInstr::new(OpCode::NewObj, vec![], 0));
                for (field_name, value) in fields {
                    self.instructions
                        .push(BytecodeInstr::new(OpCode::DUP, vec![], 0));
                    self.push_value(value)?;
                    let name_idx = self.add_constant(VmValue::String(field_name.clone()));
                    self.instructions.push(BytecodeInstr::with_operand(
                        OpCode::SetField,
                        name_idx as i32,
                        0,
                    ));
                }
                let dst_idx = self.get_or_create_local(dst);
                self.instructions.push(BytecodeInstr::with_operand(
                    OpCode::STORE,
                    dst_idx as i32,
                    0,
                ));
            }
            Instruction::StructGet {
                dst,
                struct_name: _,
                base,
                field,
            } => {
                let base_idx = self.get_or_create_local(base);
                self.instructions.push(BytecodeInstr::with_operand(
                    OpCode::LOAD,
                    base_idx as i32,
                    0,
                ));
                let name_idx = self.add_constant(VmValue::String(field.clone()));
                self.instructions.push(BytecodeInstr::with_operand(
                    OpCode::GetField,
                    name_idx as i32,
                    0,
                ));
                let dst_idx = self.get_or_create_local(dst);
                self.instructions.push(BytecodeInstr::with_operand(
                    OpCode::STORE,
                    dst_idx as i32,
                    0,
                ));
            }
            Instruction::ArrayInit {
                dst,
                elem_ty: _,
                len,
            } => {
                self.push_value(len)?;
                self.instructions
                    .push(BytecodeInstr::new(OpCode::NewArray, vec![], 0));
                let dst_idx = self.get_or_create_local(dst);
                self.instructions.push(BytecodeInstr::with_operand(
                    OpCode::STORE,
                    dst_idx as i32,
                    0,
                ));
            }
            Instruction::ArraySet {
                base,
                elem_ty: _,
                index,
                value,
            } => {
                let base_idx = self.get_or_create_local(base);
                self.instructions.push(BytecodeInstr::with_operand(
                    OpCode::LOAD,
                    base_idx as i32,
                    0,
                ));
                self.push_value(index)?;
                self.push_value(value)?;
                self.instructions
                    .push(BytecodeInstr::new(OpCode::SetIndex, vec![], 0));
                // SetIndex usually pops everything.
            }
            Instruction::ArrayGet {
                dst,
                elem_ty: _,
                base,
                index,
            } => {
                let base_idx = self.get_or_create_local(base);
                self.instructions.push(BytecodeInstr::with_operand(
                    OpCode::LOAD,
                    base_idx as i32,
                    0,
                ));
                self.push_value(index)?;
                self.instructions
                    .push(BytecodeInstr::new(OpCode::GetIndex, vec![], 0));
                let dst_idx = self.get_or_create_local(dst);
                self.instructions.push(BytecodeInstr::with_operand(
                    OpCode::STORE,
                    dst_idx as i32,
                    0,
                ));
            }
            _ => return Err(format!("Instruction {:?} not yet implemented", instr)),
        }
        Ok(())
    }
}
