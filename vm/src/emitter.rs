//! Bytecode Emitter
//! 
//! This module provides utilities for creating bytecode programs.

use crate::bytecode::{BytecodeInstr, BytecodeModule, Function, OpCode, Value};

/// Emitter for creating bytecode programs
pub struct BytecodeEmitter {
    module: BytecodeModule,
    current_function: Option<usize>,
    locals: Vec<String>,
}

impl BytecodeEmitter {
    /// Create new emitter
    pub fn new() -> Self {
        Self {
            module: BytecodeModule::new("main".to_string()),
            current_function: None,
            locals: Vec::new(),
        }
    }

    /// Create a simple function with instructions
    pub fn create_function(&mut self, name: &str, arity: usize) -> usize {
        let func = Function {
            name: name.to_string(),
            arity,
            num_locals: arity + 16, // Some space for locals
            instructions: Vec::new(),
            constants: Vec::new(),
            upvalues: Vec::new(),
            line_info: Vec::new(),
        };

        let idx = self.module.add_function(func);
        self.current_function = Some(idx);
        self.locals.clear();

        idx
    }

    /// Add a constant and return its index
    pub fn add_constant(&mut self, value: Value) -> usize {
        self.module.add_constant(value)
    }

    /// Emit a PUSH instruction (push constant to stack)
    pub fn push(&mut self, const_idx: usize, line: usize) {
        self.emit_instr(OpCode::PUSH, vec![const_idx as i32], line);
    }

    /// Emit a PUSHM instruction (push module constant to stack)
    pub fn push_module(&mut self, const_idx: usize, line: usize) {
        self.emit_instr(OpCode::PushM, vec![const_idx as i32], line);
    }

    /// Emit GetGlobalM instruction
    pub fn get_global_module(&mut self, const_idx: usize, line: usize) {
        self.emit_instr(OpCode::GetGlobalM, vec![const_idx as i32], line);
    }

    /// Emit SetGlobalM instruction
    pub fn set_global_module(&mut self, const_idx: usize, line: usize) {
        self.emit_instr(OpCode::SetGlobalM, vec![const_idx as i32], line);
    }

    /// Emit a binary instruction
    pub fn emit_binary(&mut self, op: OpCode, line: usize) {
        self.emit_instr(op, vec![], line);
    }

    /// Emit an instruction
    pub fn emit_instr(&mut self, opcode: OpCode, operands: Vec<i32>, line: usize) {
        if let Some(idx) = self.current_function {
            if let Some(func) = self.module.functions.get_mut(idx) {
                func.instructions.push(BytecodeInstr::new(opcode, operands, line));
            }
        }
    }

    /// Emit return
    pub fn ret(&mut self, line: usize) {
        self.emit_instr(OpCode::RET, vec![], line);
    }

    /// Emit halt
    pub fn halt(&mut self, line: usize) {
        self.emit_instr(OpCode::HALT, vec![], line);
    }

    /// Emit jump
    pub fn jump(&mut self, target: usize, line: usize) {
        self.emit_instr(OpCode::JMP, vec![target as i32], line);
    }

    /// Emit conditional jump
    pub fn jump_if_zero(&mut self, target: usize, line: usize) {
        self.emit_instr(OpCode::JZ, vec![target as i32], line);
    }

    /// Emit call
    pub fn call(&mut self, func_idx: usize, num_args: usize, line: usize) {
        self.emit_instr(OpCode::CALL, vec![func_idx as i32, num_args as i32], line);
    }

    /// Emit dynamic call (callee on stack, use CALL -1)
    pub fn call_dynamic(&mut self, num_args: usize, line: usize) {
        self.emit_instr(OpCode::CALL, vec![-1, num_args as i32], line);
    }

    /// Emit closure creation
    pub fn closure(&mut self, func_idx: usize, num_upvalues: usize, line: usize) {
        self.emit_instr(
            OpCode::CLOSURE,
            vec![func_idx as i32, num_upvalues as i32],
            line,
        );
    }

    /// Emit by-reference closure creation (local indices appended)
    pub fn closure_ref(&mut self, func_idx: usize, local_indices: &[usize], line: usize) {
        let mut operands = Vec::with_capacity(2 + local_indices.len());
        operands.push(func_idx as i32);
        operands.push(local_indices.len() as i32);
        for idx in local_indices {
            operands.push(*idx as i32);
        }
        self.emit_instr(OpCode::ClosureRef, operands, line);
    }

    /// Emit by-reference closure creation from stack values
    pub fn closure_ref_stack(&mut self, func_idx: usize, num_upvalues: usize, line: usize) {
        self.emit_instr(
            OpCode::ClosureRefStack,
            vec![func_idx as i32, num_upvalues as i32],
            line,
        );
    }

    /// Emit load local
    pub fn load_local(&mut self, idx: usize, line: usize) {
        self.emit_instr(OpCode::LOAD, vec![idx as i32], line);
    }

    /// Emit store local
    pub fn store_local(&mut self, idx: usize, line: usize) {
        self.emit_instr(OpCode::STORE, vec![idx as i32], line);
    }

    /// Emit print
    pub fn print(&mut self, line: usize) {
        // This would be lowered to specific instructions
        self.emit_instr(OpCode::NOP, vec![], line);
    }

    /// Get the emitted module
    pub fn get_module(self) -> BytecodeModule {
        self.module
    }
}

impl Default for BytecodeEmitter {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple bytecode optimizer
pub struct BytecodeOptimizer;

impl BytecodeOptimizer {
    /// Optimize a function
    pub fn optimize(func: &mut Function) {
        // Remove NOPs
        func.instructions.retain(|i| i.opcode != OpCode::NOP);
        
        // Remove redundant pushes
        let mut last_push = false;
        for instr in &func.instructions {
            match instr.opcode {
                OpCode::PUSH => {
                    if last_push {
                        // Could replace with POP
                    }
                    last_push = true;
                }
                OpCode::POP | OpCode::RET | OpCode::JMP | OpCode::CALL => {
                    last_push = false;
                }
                _ => {
                    last_push = false;
                }
            }
        }
    }
}

/// Helper to build a simple hello world program
pub fn build_hello_world() -> BytecodeModule {
    let mut emitter = BytecodeEmitter::new();
    
    // Create main function
    emitter.create_function("main", 0);
    
    // Add string constant "Hello, World!"
    let const_idx = emitter.add_constant(Value::String("Hello, World!".to_string()));
    
    // Push the string
    emitter.push(const_idx, 1);
    
    // Print (placeholder)
    emitter.print(1);
    
    // Return
    emitter.ret(2);
    
    // Halt
    emitter.halt(2);
    
    emitter.get_module()
}

/// Helper to build an add program
pub fn build_add_program() -> BytecodeModule {
    let mut emitter = BytecodeEmitter::new();
    
    // Create main function
    emitter.create_function("main", 0);
    
    // Add constant 10
    let const1 = emitter.add_constant(Value::Int(10));
    emitter.push(const1, 1);
    
    // Add constant 20
    let const2 = emitter.add_constant(Value::Int(20));
    emitter.push(const2, 1);
    
    // Add
    emitter.emit_binary(OpCode::ADD, 1);
    
    // Return
    emitter.ret(2);
    
    // Halt
    emitter.halt(2);
    
    emitter.get_module()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emitter_new() {
        let emitter = BytecodeEmitter::new();
        assert_eq!(emitter.module.name, "main");
    }

    #[test]
    fn test_create_function() {
        let mut emitter = BytecodeEmitter::new();
        let idx = emitter.create_function("test", 2);
        assert_eq!(idx, 0);
    }

    #[test]
    fn test_hello_world() {
        let module = build_hello_world();
        assert!(!module.functions.is_empty());
    }

    #[test]
    fn test_add_program() {
        let module = build_add_program();
        assert!(!module.functions.is_empty());
    }

    #[test]
    fn test_optimizer() {
        let mut func = Function {
            name: "test".to_string(),
            arity: 0,
            num_locals: 0,
            instructions: vec![
                BytecodeInstr::new(OpCode::NOP, vec![], 0),
                BytecodeInstr::new(OpCode::HALT, vec![], 1),
            ],
            constants: Vec::new(),
            upvalues: Vec::new(),
            line_info: vec![],
        };
        
        BytecodeOptimizer::optimize(&mut func);
        
        // NOP should be removed
        assert!(!func.instructions.iter().any(|i| i.opcode == OpCode::NOP));
    }
}
