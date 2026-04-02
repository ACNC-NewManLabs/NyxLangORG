use crate::core::ast::ast_nodes::{Expr, Stmt, Program, ItemKind};
use nyx_vm::emitter::BytecodeEmitter;
use nyx_vm::bytecode::{OpCode, Value, BytecodeModule};
use std::collections::HashMap;

pub struct BytecodeCompiler {
    emitter: BytecodeEmitter,
    pub locals: HashMap<String, usize>,
    next_local: usize,
}

impl Default for BytecodeCompiler {
    fn default() -> Self {
        Self::new()
    }
}

impl BytecodeCompiler {
    pub fn new() -> Self {
        Self {
            emitter: BytecodeEmitter::new(),
            locals: HashMap::new(),
            next_local: 0,
        }
    }

    /// Compiles a standalone loop fragment for Tier 2 Aero-JIT.
    pub fn compile_loop_fragment(
        mut self,
        condition: &Expr,
        body: &[Stmt],
        external_locals: &HashMap<String, usize>,
    ) -> Result<BytecodeModule, String> {
        self.locals = external_locals.clone();
        self.next_local = external_locals.values().max().map(|&v| v + 1).unwrap_or(0);
        
        let _loop_func_idx = self.emitter.create_function("jit_loop", external_locals.len());
        
        let start_idx = self.get_current_instr_idx();
        self.compile_expr(condition)?;
        let jz_idx = self.emit_placeholder(OpCode::JZ, condition.span().start.line);
        
        for stmt in body {
            self.compile_stmt(stmt)?;
        }
        
        self.emitter.jump(start_idx, condition.span().start.line);
        let end_idx = self.get_current_instr_idx();
        self.patch_instr(jz_idx, end_idx);
        
        // Finalize for JIT: push a dummy null and Return
        let null_idx = self.emitter.add_constant(Value::Null);
        self.emitter.push(null_idx, condition.span().start.line);
        self.emitter.ret(condition.span().start.line);
        
        Ok(self.emitter.get_module())
    }

    pub fn compile_program(mut self, program: &Program) -> Result<BytecodeModule, String> {
        let _main_idx = self.emitter.create_function("main", 0);
        
        for item in &program.items {
            match &item.kind {
                ItemKind::Stmt(stmt) => {
                    self.compile_stmt(stmt)?;
                }
                ItemKind::Static(s) => {
                    self.compile_expr(&s.value)?;
                    let idx = self.next_local;
                    self.locals.insert(s.name.clone(), idx);
                    self.next_local += 1;
                    self.emitter.store_local(idx, s.span.start.line);
                }
                _ => {}
            }
        }
        
        let halt_line = program.items.last().map(|i| i.span.start.line).unwrap_or(0);
        self.emitter.ret(halt_line);
        self.emitter.halt(halt_line);
        
        Ok(self.emitter.get_module())
    }

    pub fn compile_expr(&mut self, expr: &Expr) -> Result<(), String> {
        match expr {
            Expr::IntLiteral { value, span } => {
                let idx = self.emitter.add_constant(Value::Int(*value));
                self.emitter.push(idx, span.start.line);
                Ok(())
            }
            Expr::FloatLiteral { value, span } => {
                let idx = self.emitter.add_constant(Value::Float(*value));
                self.emitter.push(idx, span.start.line);
                Ok(())
            }
            Expr::StringLiteral { value, span } => {
                let idx = self.emitter.add_constant(Value::String(value.clone()));
                self.emitter.push(idx, span.start.line);
                Ok(())
            }
            Expr::BoolLiteral { value, span } => {
                let idx = self.emitter.add_constant(Value::Bool(*value));
                self.emitter.push(idx, span.start.line);
                Ok(())
            }
            Expr::NullLiteral { span } => {
                let idx = self.emitter.add_constant(Value::Null);
                self.emitter.push(idx, span.start.line);
                Ok(())
            }
            Expr::Binary { left, op, right, span } => {
                self.compile_expr(left)?;
                self.compile_expr(right)?;
                let opcode = match op.as_str() {
                    "+" => OpCode::ADD,
                    "-" => OpCode::SUB,
                    "*" => OpCode::MUL,
                    "/" => OpCode::DIV,
                    "%" => OpCode::MOD,
                    "==" => OpCode::EQ,
                    "!=" => OpCode::NE,
                    "<" => OpCode::LT,
                    ">" => OpCode::GT,
                    "<=" => OpCode::LE,
                    ">=" => OpCode::GE,
                    _ => return Err(format!("Unsupported binary op: {}", op)),
                };
                self.emitter.emit_binary(opcode, span.start.line);
                Ok(())
            }
            Expr::Identifier { name, span } => {
                if let Some(&idx) = self.locals.get(name) {
                    self.emitter.load_local(idx, span.start.line);
                    Ok(())
                } else {
                    Err(format!("Undefined variable: {}", name))
                }
            }
            Expr::Call { callee, args, span } => {
                let num_args = args.len();
                for arg in args {
                    self.compile_expr(arg)?;
                }
                if let Expr::Identifier { name, .. } = &**callee {
                    let name_idx = self.emitter.add_constant(Value::String(name.clone()));
                    self.emitter.emit_instr(OpCode::CallExt, vec![name_idx as i32, num_args as i32], span.start.line);
                    Ok(())
                } else {
                    Err("Only direct identifier calls supported in JIT".to_string())
                }
            }
            _ => Err(format!("Expr not supported in Bytecode JIT: {:?}", expr)),
        }
    }

    pub fn compile_stmt(&mut self, stmt: &Stmt) -> Result<(), String> {
        match stmt {
            Stmt::Expr(expr) => {
                self.compile_expr(expr)?;
                self.emitter.emit_instr(OpCode::POP, vec![], expr.span().start.line);
                Ok(())
            }
            Stmt::Let { name, expr, span, .. } => {
                self.compile_expr(expr)?;
                let idx = if let Some(&i) = self.locals.get(name) {
                    i
                } else {
                    let i = self.next_local;
                    self.locals.insert(name.clone(), i);
                    self.next_local += 1;
                    i
                };
                self.emitter.store_local(idx, span.start.line);
                Ok(())
            }
            Stmt::While { condition, body, span } => {
                let start_idx = self.get_current_instr_idx();
                self.compile_expr(condition)?;
                let jz_idx = self.emit_placeholder(OpCode::JZ, span.start.line);
                for s in body { self.compile_stmt(s)?; }
                self.emitter.jump(start_idx, span.start.line);
                let end_idx = self.get_current_instr_idx();
                self.patch_instr(jz_idx, end_idx);
                Ok(())
            }
            Stmt::If { branches, else_body, span: _ } => {
                let mut end_jumps = Vec::new();
                for br in branches {
                    self.compile_expr(&br.condition)?;
                    let jz_idx = self.emit_placeholder(OpCode::JZ, br.condition.span().start.line);
                    for s in &br.body { self.compile_stmt(s)?; }
                    end_jumps.push(self.emit_placeholder(OpCode::JMP, br.condition.span().end.line));
                    self.patch_instr(jz_idx, self.get_current_instr_idx());
                }
                if let Some(eb) = else_body {
                    for s in eb { self.compile_stmt(s)?; }
                }
                let end_idx = self.get_current_instr_idx();
                for ej in end_jumps { self.patch_instr(ej, end_idx); }
                Ok(())
            }
            Stmt::Assign { target, value, span } => {
                if let Expr::Identifier { name, .. } = target {
                    self.compile_expr(value)?;
                    if let Some(&idx) = self.locals.get(name) {
                        self.emitter.store_local(idx, span.start.line);
                        Ok(())
                    } else {
                        Err(format!("Undefined variable: {}", name))
                    }
                } else {
                    Err("Only identifiers supported for assignment in JIT".to_string())
                }
            }
            Stmt::Print { expr } => {
                self.compile_expr(expr)?;
                let name_idx = self.emitter.add_constant(Value::String("print".to_string()));
                self.emitter.emit_instr(OpCode::CallExt, vec![name_idx as i32, 1], expr.span().start.line);
                self.emitter.emit_instr(OpCode::POP, vec![], expr.span().start.line);
                Ok(())
            }
            _ => Err(format!("Stmt not supported in Bytecode JIT: {:?}", stmt)),
        }
    }

    fn get_current_instr_idx(&self) -> usize { self.emitter.current_instr_idx() }
    fn emit_placeholder(&mut self, opcode: OpCode, line: usize) -> usize {
        let idx = self.get_current_instr_idx();
        self.emitter.emit_instr(opcode, vec![0], line);
        idx
    }
    fn patch_instr(&mut self, instr_idx: usize, target: usize) {
        self.emitter.patch_instr_operand(instr_idx, 0, target as i32);
    }
}
