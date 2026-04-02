use crate::core::ast::ast_nodes::{Expr, Stmt, Item, ItemKind, Program, ModuleDecl};
use crate::runtime::execution::nyx_vm::Value;
use crate::runtime::execution::bytecode_compiler::BytecodeCompiler;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum LoopAnalysis {
    /// O(1) - The loop can be replaced with these final variable values.
    Deterministic(HashMap<String, Value>),
    /// Tier 2 - The loop is complex but "numeric-pure" and can be JIT compiled.
    JitReady,
    /// Standard execution - Too complex for optimization.
    Standard,
}

/// The "God-level" AST Optimizer for Nyx.
pub struct AeroOptimizer;

impl AeroOptimizer {
    pub fn optimize_program(program: &mut Program) {
        let mut symbols = HashMap::new();
        for item in &mut program.items {
            Self::optimize_item(item, &mut symbols);
        }
    }

    fn optimize_item(item: &mut Item, symbols: &mut HashMap<String, Value>) {
        match &mut item.kind {
            ItemKind::Function(f) => {
                let mut local_symbols = symbols.clone();
                Self::optimize_block(&mut f.body, &mut local_symbols);
            }
            ItemKind::Module(m) => {
                if let ModuleDecl::Inline { items, .. } = m {
                    let mut module_symbols = symbols.clone();
                    for inner in items {
                        Self::optimize_item(inner, &mut module_symbols);
                    }
                }
            }
            ItemKind::Static(s) => {
                Self::optimize_expr(&mut s.value, symbols);
                if let Some(val) = Self::expr_to_value(&s.value) {
                    symbols.insert(s.name.clone(), val);
                }
            }
            ItemKind::Const(c) => {
                Self::optimize_expr(&mut c.value, symbols);
                if let Some(val) = Self::expr_to_value(&c.value) {
                    symbols.insert(c.name.clone(), val);
                }
            }
            _ => {}
        }
    }

    pub fn optimize_block(stmts: &mut Vec<Stmt>, symbols: &mut HashMap<String, Value>) {
        let mut i = 0;
        while i < stmts.len() {
            Self::optimize_stmt(&mut stmts[i], symbols);
            if Self::is_terminator(&stmts[i]) {
                stmts.truncate(i + 1);
                break;
            }
            i += 1;
        }
    }

    fn optimize_stmt(stmt: &mut Stmt, symbols: &mut HashMap<String, Value>) {
        match stmt {
            Stmt::Let { name, expr, mutable, .. } => {
                Self::optimize_expr(expr, symbols);
                if !*mutable {
                    if let Some(val) = Self::expr_to_value(expr) {
                        symbols.insert(name.clone(), val);
                    }
                }
            }
            Stmt::Assign { value, .. } => Self::optimize_expr(value, symbols),
            Stmt::CompoundAssign { value, .. } => Self::optimize_expr(value, symbols),
            Stmt::If { branches, else_body, .. } => {
                let mut i = 0;
                while i < branches.len() {
                    Self::optimize_expr(&mut branches[i].condition, symbols);
                    Self::optimize_block(&mut branches[i].body, &mut symbols.clone());
                    match &branches[i].condition {
                        Expr::BoolLiteral { value: false, .. } => {
                            branches.remove(i);
                            continue;
                        }
                        Expr::BoolLiteral { value: true, .. } => {
                            branches.truncate(i + 1);
                            *else_body = None;
                            break;
                        }
                        _ => {}
                    }
                    i += 1;
                }
                if let Some(eb) = else_body {
                    Self::optimize_block(eb, &mut symbols.clone());
                }
            }
            Stmt::While { condition, body, .. } => {
                Self::optimize_expr(condition, symbols);
                Self::optimize_block(body, &mut symbols.clone());
            }
            Stmt::Return { expr, .. } => {
                if let Some(e) = expr {
                    Self::optimize_expr(e, symbols);
                }
            }
            _ => {}
        }
    }

    pub fn optimize_expr(expr: &mut Expr, symbols: &HashMap<String, Value>) {
        match expr {
            Expr::Binary { left, op, right, .. } => {
                Self::optimize_expr(left, symbols);
                Self::optimize_expr(right, symbols);
                if let Some(folded) = Self::try_fold_binary(left, op, right) {
                    *expr = folded;
                } else {
                    Self::try_strength_reduction(expr);
                }
            }
            Expr::Unary { right: inner, op, .. } => {
                Self::optimize_expr(inner, symbols);
                if let Some(folded) = Self::try_fold_unary(op, inner) {
                    *expr = folded;
                }
            }
            Expr::Identifier { name, span } => {
                if let Some(val) = symbols.get(name) {
                    if let Some(lit) = Self::value_to_expr(val, *span) {
                        *expr = lit;
                    }
                }
            }
            Expr::Ternary { condition, then_expr, else_expr, .. } => {
                Self::optimize_expr(condition, symbols);
                Self::optimize_expr(then_expr, symbols);
                Self::optimize_expr(else_expr, symbols);
                if let Expr::BoolLiteral { value, .. } = condition.as_ref() {
                    let branch = if *value { then_expr } else { else_expr };
                    let boxed = std::mem::replace(branch, Box::new(Expr::NullLiteral { span: crate::core::lexer::token::Span::default() }));
                    *expr = *boxed;
                }
            }
            Expr::Index { object, index, .. } => {
                Self::optimize_expr(object, symbols);
                Self::optimize_expr(index, symbols);
                if let (Expr::ArrayLiteral { elements, .. }, Expr::IntLiteral { value: idx, .. }) = (object.as_ref(), index.as_ref()) {
                    if *idx >= 0 && (*idx as usize) < elements.len() {
                        let mut item = elements[*idx as usize].clone();
                        Self::optimize_expr(&mut item, symbols);
                        *expr = item;
                    }
                }
            }
            _ => {}
        }
    }

    pub fn analyze_while(
        condition: &Expr,
        body: &[Stmt],
        locals: &HashMap<String, Value>,
    ) -> LoopAnalysis {
        // Tier 1: Symbolic O(1) reduction
        if let Some(upd) = Self::try_symbolic_reduction(condition, body, locals) {
            return LoopAnalysis::Deterministic(upd);
        }

        // Tier 2: Aero-JIT readiness check
        // Check if the loop is "compilable" (numeric-pure, no complex literals/IO)
        if Self::is_jit_compilable(condition, body, locals) {
            return LoopAnalysis::JitReady;
        }

        LoopAnalysis::Standard
    }

    fn try_symbolic_reduction(
        condition: &Expr,
        body: &[Stmt],
        locals: &HashMap<String, Value>,
    ) -> Option<HashMap<String, Value>> {
        let (ind_var, n) = match condition {
            Expr::Binary { left, op, right, .. } if op == "<" => {
                let left_name = if let Expr::Identifier { name, .. } = left.as_ref() {
                    name
                } else {
                    return None;
                };

                let limit = match right.as_ref() {
                    Expr::Identifier { name, .. } => {
                        match locals.get(name) {
                            Some(Value::Int(v)) => *v,
                            _ => return None,
                        }
                    }
                    Expr::IntLiteral { value, .. } => *value,
                    _ => return None,
                };
                (left_name, limit)
            }
            _ => return None,
        };

        if body.len() != 2 { return None; }

        let mut acc_var = None;
        let mut ind_increment = None;

        for stmt in body {
            if let Stmt::Assign { target, value, .. } = stmt {
                if let Expr::Identifier { name, .. } = target {
                    if name == ind_var {
                        ind_increment = match value {
                            Expr::Binary { left, op, right, .. } if op == "+" => {
                                if let (Expr::Identifier { name: l_name, .. }, Expr::IntLiteral { value: r_val, .. }) = (left.as_ref(), right.as_ref()) {
                                    if l_name == ind_var { Some(*r_val) } else { None }
                                } else { None }
                            }
                            _ => None,
                        };
                    } else {
                        acc_var = match value {
                            Expr::Binary { left, op, right, .. } if op == "+" => {
                                if let (Expr::Identifier { name: l_name, .. }, Expr::Identifier { name: r_name, .. }) = (left.as_ref(), right.as_ref()) {
                                    if l_name == name && r_name == ind_var { Some(name.clone()) } else { None }
                                } else { None }
                            }
                            _ => None,
                        };
                    }
                }
            }
        }

        if let (Some(acc), Some(1)) = (acc_var, ind_increment) {
            let start_val = match locals.get(ind_var) {
                Some(Value::Int(v)) => *v,
                _ => 0,
            };
            let initial_acc = match locals.get(&acc) {
                Some(Value::Int(v)) => *v,
                _ => 0,
            };

            if start_val < n {
                let count = n - start_val;
                let sum = (count - 1) * count / 2 + start_val * count;
                let mut updates = HashMap::new();
                updates.insert(acc, Value::Int(initial_acc + sum));
                updates.insert(ind_var.clone(), Value::Int(n));
                return Some(updates);
            }
        }

        None
    }

    fn is_jit_compilable(condition: &Expr, body: &[Stmt], _locals: &HashMap<String, Value>) -> bool {
        // Broadly, if the loop contains only arithmetic, branches, and supported stmts, it can be JIT'd.
        // We'll use the BytecodeCompiler as our verifier.
        let test_compiler = BytecodeCompiler::new();
        let mut test_locals = HashMap::new();
        // Assume variables exist for the test pass
        if let Expr::Identifier { name, .. } = condition { test_locals.insert(name.clone(), 0); }
        
        test_compiler.compile_loop_fragment(condition, body, &test_locals).is_ok()
    }

    fn try_fold_binary(left: &Expr, op: &str, right: &Expr) -> Option<Expr> {
        match (left, right) {
            (Expr::IntLiteral { value: l_val, span }, Expr::IntLiteral { value: r_val, .. }) => {
                match op {
                    "+" => Some(Expr::IntLiteral { value: l_val + r_val, span: *span }),
                    "-" => Some(Expr::IntLiteral { value: l_val - r_val, span: *span }),
                    "*" => Some(Expr::IntLiteral { value: l_val * r_val, span: *span }),
                    "/" if *r_val != 0 => Some(Expr::IntLiteral { value: l_val / r_val, span: *span }),
                    ">" => Some(Expr::BoolLiteral { value: l_val > r_val, span: *span }),
                    "<" => Some(Expr::BoolLiteral { value: l_val < r_val, span: *span }),
                    "==" => Some(Expr::BoolLiteral { value: l_val == r_val, span: *span }),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn try_fold_unary(op: &str, inner: &Expr) -> Option<Expr> {
        match inner {
            Expr::IntLiteral { value, span } if op == "-" => Some(Expr::IntLiteral { value: -value, span: *span }),
            Expr::BoolLiteral { value, span } if op == "!" => Some(Expr::BoolLiteral { value: !value, span: *span }),
            _ => None,
        }
    }

    fn try_strength_reduction(expr: &mut Expr) {
        if let Expr::Binary { left, op, right, span, .. } = expr {
            if *op == "*" {
                if let Expr::IntLiteral { value: 2, .. } = right.as_ref() {
                    let l = std::mem::replace(left.as_mut(), Expr::NullLiteral { span: *span });
                    *expr = Expr::Binary {
                        left: Box::new(l),
                        op: "<<".to_string(),
                        right: Box::new(Expr::IntLiteral { value: 1, span: *span }),
                        span: *span,
                    };
                }
            }
        }
    }

    fn expr_to_value(expr: &Expr) -> Option<Value> {
        match expr {
            Expr::IntLiteral { value, .. } => Some(Value::Int(*value)),
            Expr::FloatLiteral { value, .. } => Some(Value::Float(*value)),
            Expr::BoolLiteral { value, .. } => Some(Value::Bool(*value)),
            Expr::NullLiteral { .. } => Some(Value::Null),
            _ => None,
        }
    }

    fn value_to_expr(val: &Value, span: crate::core::lexer::token::Span) -> Option<Expr> {
        match val {
            Value::Int(v) => Some(Expr::IntLiteral { value: *v, span }),
            Value::Float(v) => Some(Expr::FloatLiteral { value: *v, span }),
            Value::Bool(v) => Some(Expr::BoolLiteral { value: *v, span }),
            Value::Null => Some(Expr::NullLiteral { span }),
            _ => None,
        }
    }

    fn is_terminator(stmt: &Stmt) -> bool {
        matches!(stmt, Stmt::Return { .. } | Stmt::Break { .. } | Stmt::Continue { .. })
    }
}
