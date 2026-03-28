use crate::core::ast::ast_nodes::*;
use crate::systems::ir::nyx_ir::*;
use std::collections::HashMap;

#[derive(Debug)]
pub struct IrBuilder {
    label_counter: usize,
    loop_stack: Vec<LoopLabels>,
    struct_defs: HashMap<String, StructDef>,
    synthetic_structs: HashMap<String, StructDef>,
    local_types: HashMap<String, IrType>,
    generic_fns: HashMap<String, FunctionDecl>,
    impl_methods: HashMap<(String, String, Option<String>), String>,
    pending_monomorphized: Vec<MonoRequest>,
    seen_monomorphized: HashMap<String, String>,
    closure_counter: usize,
    closure_locals: HashMap<String, ClosureInstance>,
    synthetic_functions: Vec<IrFunction>,
    type_subst: Option<HashMap<String, IrType>>,
    defer_stack: Vec<Vec<Vec<Instruction>>>,
}

impl Default for IrBuilder {
    fn default() -> Self {
        Self {
            label_counter: 0,
            loop_stack: Vec::new(),
            struct_defs: HashMap::new(),
            synthetic_structs: HashMap::new(),
            local_types: HashMap::new(),
            generic_fns: HashMap::new(),
            impl_methods: HashMap::new(),
            pending_monomorphized: Vec::new(),
            seen_monomorphized: HashMap::new(),
            closure_counter: 0,
            closure_locals: HashMap::new(),
            synthetic_functions: Vec::new(),
            type_subst: None,
            defer_stack: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct LoopLabels {
    break_label: String,
    continue_label: String,
    label_name: Option<String>,
}

#[derive(Debug, Clone)]
struct MonoRequest {
    name: String,
    decl: FunctionDecl,
    subst: HashMap<String, IrType>,
}

#[derive(Debug, Clone)]
struct ClosureInstance {
    env_struct: String,
    fn_name: String,
    captures: Vec<String>,
}

impl IrBuilder {
    pub fn new_label(&mut self, prefix: &str) -> String {
        let l = format!("{}_{}", prefix, self.label_counter);
        self.label_counter += 1;
        l
    }

    fn emit_defers(&self, out: &mut Vec<Instruction>) {
        // Run defers from innermost scope to outermost,
        // and within each scope from last added to first.
        for scope in self.defer_stack.iter().rev() {
            for defer_instrs in scope.iter().rev() {
                out.extend(defer_instrs.clone());
            }
        }
    }

    fn push_defer_scope(&mut self) {
        self.defer_stack.push(Vec::new());
    }

    fn pop_defer_scope(&mut self) {
        self.defer_stack.pop();
    }

    fn emit_local_defers(&self, out: &mut Vec<Instruction>) {
        if let Some(scope) = self.defer_stack.last() {
            for defer_instrs in scope.iter().rev() {
                out.extend(defer_instrs.clone());
            }
        }
    }

    pub fn build(&self, program: &Program) -> Result<Module, String> {
        let mut builder = IrBuilder::default();
        builder.collect_structs(program)?;
        builder.collect_generic_functions(program);
        builder.collect_impl_methods(program)?;

        let mut functions = Vec::new();
        for item in &program.items {
            match &item.kind {
                ItemKind::Function(f) => {
                    if f.generics.is_empty() {
                        functions.push(builder.lower_fn(f, None, None)?);
                    }
                }
                ItemKind::Impl(ib) => {
                    let self_name = builder.type_path_name(&ib.self_type)?;
                    let trait_name = ib.trait_name.as_ref().map(|t| t.last_name().to_string());
                    for it in &ib.items {
                        if let ImplItem::Method(m) = it {
                            let mangled =
                                builder.method_mangled_name(&self_name, trait_name.as_deref(), &m.name);
                            let mut cloned = m.clone();
                            cloned.name = mangled.clone();
                            if cloned.generics.is_empty() {
                                functions.push(builder.lower_fn(&cloned, None, None)?);
                            } else {
                                builder.generic_fns.insert(mangled, cloned);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        while let Some(req) = builder.pending_monomorphized.pop() {
            let name = req.name.clone();
            functions.push(builder.lower_fn(&req.decl, Some(&req.subst), Some(&name))?);
        }

        functions.extend(builder.synthetic_functions.drain(..));

        let mut structs: Vec<StructDef> = builder
            .struct_defs
            .values()
            .cloned()
            .collect();
        structs.extend(builder.synthetic_structs.values().cloned());
        Ok(Module { structs, functions })
    }

    fn lower_fn(
        &mut self,
        f: &FunctionDecl,
        subst: Option<&HashMap<String, IrType>>,
        name_override: Option<&str>,
    ) -> Result<IrFunction, String> {
        let mut instrs = Vec::new();
        let mut tmp = 0usize;
        self.local_types.clear();
        self.closure_locals.clear();
        let prev_subst = self.type_subst.take();
        self.type_subst = subst.cloned();
        self.push_defer_scope();
        
        let mut params = Vec::new();
        for p in &f.params {
            let ty = self.ast_type_to_ir(&p.param_type);
            self.local_types.insert(p.name.clone(), ty.clone());
            params.push(IrParam {
                name: p.name.clone(),
                ty,
            });
        }
        for stmt in &f.body {
            self.lower_stmt(stmt, &mut instrs, &mut tmp)?;
        }
        
        if !instrs
            .iter()
            .any(|i| matches!(i, Instruction::Return { .. }))
        {
            self.emit_defers(&mut instrs);
            instrs.push(Instruction::Return { value: None });
        }
        
        self.pop_defer_scope();
        let name = name_override.unwrap_or(&f.name).to_string();
        let func = IrFunction {
            name,
            params,
            instructions: instrs,
        };
        self.type_subst = prev_subst;
        Ok(func)
    }

    fn lower_stmt(
        &mut self,
        stmt: &Stmt,
        out: &mut Vec<Instruction>,
        tmp: &mut usize,
    ) -> Result<(), String> {
        match stmt {
            Stmt::Let { name, expr, .. } => {
                let v = self.lower_expr(expr, out, tmp)?;
                self.propagate_closure_value(&v, name);
                out.push(Instruction::Let {
                    name: name.clone(),
                    value: v,
                });
                if let Ok(ty) = self.infer_expr_type(expr) {
                    self.local_types.insert(name.clone(), ty);
                } else if let Stmt::Let {
                    var_type: Some(var_type),
                    ..
                } = stmt
                {
                    let ty = self.ast_type_to_ir(var_type);
                    self.local_types
                        .insert(name.clone(), ty);
                }
                self.propagate_closure_local(expr, name);
            }
            Stmt::Assign { target, value, .. } => {
                let v = self.lower_expr(value, out, tmp)?;
                let name = expr_to_local(target)?;
                self.propagate_closure_value(&v, &name);
                out.push(Instruction::Let {
                    name: name.clone(),
                    value: v,
                });
                if let Ok(ty) = self.infer_expr_type(value) {
                    self.local_types.insert(name.clone(), ty);
                }
                self.propagate_closure_local(value, &name);
            }
            Stmt::CompoundAssign { target, op, value, .. } => {
                let name = expr_to_local(target)?;
                let lhs = Value::Local(name.clone());
                let rhs = self.lower_expr(value, out, tmp)?;
                let ir_op = match op.as_str() {
                    "+=" => BinaryOp::Add,
                    "-=" => BinaryOp::Sub,
                    "*=" => BinaryOp::Mul,
                    "/=" => BinaryOp::Div,
                    "%=" => BinaryOp::Mod,
                    "&=" => BinaryOp::BitAnd,
                    "|=" => BinaryOp::BitOr,
                    "^=" => BinaryOp::BitXor,
                    "<<=" => BinaryOp::Shl,
                    ">>=" => BinaryOp::Shr,
                    _ => return Err(format!("error[E010]: unsupported compound op '{op}'")),
                };
                let dst = format!("_t{}", {
                    let t = *tmp;
                    *tmp += 1;
                    t
                });
                out.push(Instruction::Binary {
                    dst: dst.clone(),
                    op: ir_op,
                    lhs,
                    rhs,
                });
                out.push(Instruction::Let {
                    name,
                    value: Value::Temp(dst),
                });
            }
            Stmt::Return { expr, .. } => {
                let v = if let Some(e) = expr {
                    Some(self.lower_expr(e, out, tmp)?)
                } else {
                    None
                };
                self.emit_defers(out);
                out.push(Instruction::Return { value: v });
            }
            Stmt::Defer { stmt, .. } => {
                let mut deferred_instrs = Vec::new();
                self.lower_stmt(stmt, &mut deferred_instrs, tmp)?;
                if let Some(scope) = self.defer_stack.last_mut() {
                    scope.push(deferred_instrs);
                }
            }
            Stmt::Print { expr } => {
                let v = self.lower_expr(expr, out, tmp)?;
                out.push(Instruction::Print { value: v });
            }
            Stmt::Expr(e) => {
                self.lower_expr(e, out, tmp)?;
            }
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                let end_lbl = self.new_label("if_end");
                let else_lbl = if else_body.is_some() {
                    Some(self.new_label("else"))
                } else {
                    None
                };
                for (i, branch) in branches.iter().enumerate() {
                    let cond = self.lower_expr(&branch.condition, out, tmp)?;
                    let then_lbl = self.new_label("then");
                    let skip_lbl = else_lbl.clone().unwrap_or_else(|| end_lbl.clone());
                    out.push(Instruction::Branch {
                        cond,
                        then_label: then_lbl.clone(),
                        else_label: skip_lbl,
                    });
                    out.push(Instruction::Label(then_lbl));
                    self.push_defer_scope();
                    for s in &branch.body {
                        self.lower_stmt(s, out, tmp)?;
                    }
                    self.emit_local_defers(out);
                    self.pop_defer_scope();
                    out.push(Instruction::Jump(end_lbl.clone()));
                    let _ = i;
                }
                if let Some(el) = &else_lbl {
                    out.push(Instruction::Label(el.clone()));
                    if let Some(eb) = else_body {
                        self.push_defer_scope();
                        for s in eb {
                            self.lower_stmt(s, out, tmp)?;
                        }
                        self.emit_local_defers(out);
                        self.pop_defer_scope();
                    }
                    out.push(Instruction::Jump(end_lbl.clone()));
                }
                out.push(Instruction::Label(end_lbl));
            }
            Stmt::While {
                condition, body, ..
            } => {
                let cond_lbl = self.new_label("while_cond");
                let body_lbl = self.new_label("while_body");
                let end_lbl = self.new_label("while_end");
                self.loop_stack.push(LoopLabels {
                    break_label: end_lbl.clone(),
                    continue_label: cond_lbl.clone(),
                    label_name: None,
                });
                out.push(Instruction::Jump(cond_lbl.clone()));
                out.push(Instruction::Label(cond_lbl.clone()));
                let cond = self.lower_expr(condition, out, tmp)?;
                out.push(Instruction::Branch {
                    cond,
                    then_label: body_lbl.clone(),
                    else_label: end_lbl.clone(),
                });
                out.push(Instruction::Label(body_lbl));
                self.push_defer_scope();
                for s in body {
                    self.lower_stmt(s, out, tmp)?;
                }
                self.emit_local_defers(out);
                self.pop_defer_scope();
                out.push(Instruction::Jump(cond_lbl));
                out.push(Instruction::Label(end_lbl));
                self.loop_stack.pop();
            }
            Stmt::Loop { body, .. } => {
                let lbl = self.new_label("loop");
                let end_lbl = self.new_label("loop_end");
                self.loop_stack.push(LoopLabels {
                    break_label: end_lbl.clone(),
                    continue_label: lbl.clone(),
                    label_name: None,
                });
                out.push(Instruction::Jump(lbl.clone()));
                out.push(Instruction::Label(lbl.clone()));
                self.push_defer_scope();
                for s in body {
                    self.lower_stmt(s, out, tmp)?;
                }
                self.emit_local_defers(out);
                self.pop_defer_scope();
                out.push(Instruction::Jump(lbl));
                out.push(Instruction::Label(end_lbl));
                self.loop_stack.pop();
            }
            Stmt::ForIn {
                var, iter, body, span: _,
            } => {
                self.lower_for_in(var, iter, body, out, tmp)?;
            }
            Stmt::Break { label, .. } => {
                let target = self.resolve_loop_target(label.as_deref())?;
                out.push(Instruction::Jump(target.break_label));
            }
            Stmt::Continue { label, .. } => {
                let target = self.resolve_loop_target(label.as_deref())?;
                out.push(Instruction::Jump(target.continue_label));
            }
            Stmt::Unsafe { .. } => {
                if let Stmt::Unsafe { body, .. } = stmt {
                    self.push_defer_scope();
                    for s in body {
                        self.lower_stmt(s, out, tmp)?;
                    }
                    self.emit_local_defers(out);
                    self.pop_defer_scope();
                }
            }
            Stmt::Match { .. } => {
                self.lower_match_stmt(stmt, out, tmp)?;
            }
            Stmt::InlineAsm { .. } => {
                if let Stmt::InlineAsm {
                    code,
                    outputs,
                    inputs,
                    ..
                } = stmt
                {
                    let mut out_ops = Vec::with_capacity(outputs.len());
                    for op in outputs {
                        let name = expr_to_local(&op.expr)?;
                        out_ops.push(InlineAsmOutput {
                            name,
                            reg: op.reg.clone(),
                        });
                    }
                    let mut in_ops = Vec::with_capacity(inputs.len());
                    for op in inputs {
                        let value = self.lower_expr(&op.expr, out, tmp)?;
                        in_ops.push(InlineAsmInput {
                            value,
                            reg: op.reg.clone(),
                        });
                    }
                    out.push(Instruction::InlineAsm {
                        code: code.clone(),
                        outputs: out_ops,
                        inputs: in_ops,
                    });
                }
            }
        }
        Ok(())
    }

    #[allow(clippy::only_used_in_recursion)]
    fn lower_expr(
        &mut self,
        expr: &Expr,
        out: &mut Vec<Instruction>,
        tmp: &mut usize,
    ) -> Result<Value, String> {
        match expr {
            Expr::IntLiteral(n) => Ok(Value::Int(*n)),
            Expr::BigIntLiteral(_) => {
                Err("error[E110]: big integer literals are not supported in IR lowering".to_string())
            }
            Expr::FloatLiteral(f) => Ok(Value::Float(*f)),
            Expr::StringLiteral(s) => Ok(Value::Str(s.clone())),
            Expr::BoolLiteral(b) => Ok(Value::Bool(*b)),
            Expr::CharLiteral(c) => Ok(Value::Int(*c as i64)),
            Expr::NullLiteral => Ok(Value::Null),
            // css`` literals are evaluated by the VM, not the IR path.
            Expr::CssLiteral(_) => Ok(Value::Null),
            Expr::Identifier(n) => Ok(Value::Local(n.clone())),
            Expr::Path(parts) => Ok(Value::Local(parts.join("::"))),
            Expr::Binary { left, op, right } => {
                let lv = self.lower_expr(left, out, tmp)?;
                let rv = self.lower_expr(right, out, tmp)?;
                if op == "??" {
                    let cond_dst = format!("_t{}", {
                        let t = *tmp;
                        *tmp += 1;
                        t
                    });
                    out.push(Instruction::Binary {
                        dst: cond_dst.clone(),
                        op: BinaryOp::Ne,
                        lhs: lv.clone(),
                        rhs: Value::Null,
                    });
                    let then_lbl = self.new_label("coalesce_then");
                    let else_lbl = self.new_label("coalesce_else");
                    let end_lbl = self.new_label("coalesce_end");
                    let dst = format!("_t{}", {
                        let t = *tmp;
                        *tmp += 1;
                        t
                    });
                    out.push(Instruction::Branch {
                        cond: Value::Temp(cond_dst),
                        then_label: then_lbl.clone(),
                        else_label: else_lbl.clone(),
                    });
                    out.push(Instruction::Label(then_lbl));
                    out.push(Instruction::Let {
                        name: dst.clone(),
                        value: lv,
                    });
                    out.push(Instruction::Jump(end_lbl.clone()));
                    out.push(Instruction::Label(else_lbl));
                    out.push(Instruction::Let {
                        name: dst.clone(),
                        value: rv,
                    });
                    out.push(Instruction::Jump(end_lbl.clone()));
                    out.push(Instruction::Label(end_lbl));
                    return Ok(Value::Temp(dst));
                }
                let ir_op = match op.as_str() {
                    "+" => BinaryOp::Add,
                    "-" => BinaryOp::Sub,
                    "*" => BinaryOp::Mul,
                    "/" => BinaryOp::Div,
                    "%" => BinaryOp::Mod,
                    "==" => BinaryOp::Eq,
                    "!=" => BinaryOp::Ne,
                    "<" => BinaryOp::Lt,
                    "<=" => BinaryOp::Le,
                    ">" => BinaryOp::Gt,
                    ">=" => BinaryOp::Ge,
                    "&&" => BinaryOp::And,
                    "||" => BinaryOp::Or,
                    "&" => BinaryOp::BitAnd,
                    "|" => BinaryOp::BitOr,
                    "^" => BinaryOp::BitXor,
                    "<<" => BinaryOp::Shl,
                    ">>" => BinaryOp::Shr,
                    _ => return Err(format!("error[E010]: unsupported IR op '{op}'")),
                };
                let dst = format!("_t{}", {
                    let t = *tmp;
                    *tmp += 1;
                    t
                });
                out.push(Instruction::Binary {
                    dst: dst.clone(),
                    op: ir_op,
                    lhs: lv,
                    rhs: rv,
                });
                Ok(Value::Temp(dst))
            }
            Expr::Unary { op, right } => {
                let rv = self.lower_expr(right, out, tmp)?;
                let ir_op = match op.as_str() {
                    "-" => BinaryOp::Sub,
                    "!" => BinaryOp::Not,
                    _ => return Err(format!("error[E010]: unsupported unary op '{op}'")),
                };
                let dst = format!("_t{}", {
                    let t = *tmp;
                    *tmp += 1;
                    t
                });
                out.push(Instruction::Binary {
                    dst: dst.clone(),
                    op: ir_op,
                    lhs: Value::Int(0),
                    rhs: rv,
                });
                Ok(Value::Temp(dst))
            }
            Expr::Ternary {
                condition,
                then_expr,
                else_expr,
            } => {
                let cond_val = self.lower_expr(condition, out, tmp)?;
                let then_lbl = self.new_label("tern_then");
                let else_lbl = self.new_label("tern_else");
                let end_lbl = self.new_label("tern_end");
                let dst = format!("_t{}", {
                    let t = *tmp;
                    *tmp += 1;
                    t
                });
                out.push(Instruction::Branch {
                    cond: cond_val,
                    then_label: then_lbl.clone(),
                    else_label: else_lbl.clone(),
                });
                out.push(Instruction::Label(then_lbl));
                let then_val = self.lower_expr(then_expr, out, tmp)?;
                out.push(Instruction::Let {
                    name: dst.clone(),
                    value: then_val,
                });
                out.push(Instruction::Jump(end_lbl.clone()));
                out.push(Instruction::Label(else_lbl));
                let else_val = self.lower_expr(else_expr, out, tmp)?;
                out.push(Instruction::Let {
                    name: dst.clone(),
                    value: else_val,
                });
                out.push(Instruction::Jump(end_lbl.clone()));
                out.push(Instruction::Label(end_lbl));
                Ok(Value::Temp(dst))
            }
            Expr::Call { callee, args } => {
                let callee_str = expr_to_local(callee)?;
                if let Some(closure) = self.closure_locals.get(&callee_str).cloned() {
                    let mut call_args = Vec::new();
                    for cap in &closure.captures {
                        let field_ty = self.struct_field_type(&closure.env_struct, cap)?;
                        let dst = format!("_t{}", {
                            let t = *tmp;
                            *tmp += 1;
                            t
                        });
                        out.push(Instruction::StructGet {
                            dst: dst.clone(),
                            struct_name: closure.env_struct.clone(),
                            base: callee_str.clone(),
                            field: cap.clone(),
                        });
                        self.local_types.insert(dst.clone(), field_ty);
                        call_args.push(Value::Temp(dst));
                    }
                    for a in args {
                        call_args.push(self.lower_expr(a, out, tmp)?);
                    }
                    let dst = format!("_t{}", {
                        let t = *tmp;
                        *tmp += 1;
                        t
                    });
                    out.push(Instruction::Call {
                        dst: dst.clone(),
                        callee: closure.fn_name,
                        args: call_args,
                    });
                    return Ok(Value::Temp(dst));
                }

                let callee_str = self.monomorphize_call_if_needed(&callee_str, args)?;
                let mut arg_vals = Vec::new();
                for a in args {
                    arg_vals.push(self.lower_expr(a, out, tmp)?);
                }
                let dst = format!("_t{}", {
                    let t = *tmp;
                    *tmp += 1;
                    t
                });
                out.push(Instruction::Call {
                    dst: dst.clone(),
                    callee: callee_str,
                    args: arg_vals,
                });
                Ok(Value::Temp(dst))
            }
            Expr::FieldAccess { object, field } => {
                let base = expr_to_local(object)?;
                let struct_ty = self
                    .local_types
                    .get(&base)
                    .cloned()
                    .ok_or_else(|| {
                        format!("error[E110]: unknown struct type for '{base}' in field access")
                    })?;
                let struct_name = match struct_ty {
                    IrType::Struct(n) => n,
                    _ => {
                        return Err(format!(
                            "error[E110]: field access on non-struct value '{base}'"
                        ))
                    }
                };
                let field_ty = self.struct_field_type(&struct_name, field)?;
                let dst = format!("_t{}", {
                    let t = *tmp;
                    *tmp += 1;
                    t
                });
                out.push(Instruction::StructGet {
                    dst: dst.clone(),
                    struct_name,
                    base,
                    field: field.clone(),
                });
                self.local_types.insert(dst.clone(), field_ty);
                Ok(Value::Temp(dst))
            }
            Expr::Cast { expr, .. } => self.lower_expr(expr, out, tmp),
            Expr::Await(e) | Expr::Move(e) | Expr::Deref(e) | Expr::TryOp(e) => {
                self.lower_expr(e, out, tmp)
            }
            Expr::Block(stmts, tail) => {
                self.push_defer_scope();
                for s in stmts {
                    self.lower_stmt(s, out, tmp)?;
                }
                let res = if let Some(expr) = tail {
                    let v = self.lower_expr(expr, out, tmp)?;
                    let dst = format!("_t{}", {
                        let t = *tmp;
                        *tmp += 1;
                        t
                    });
                    out.push(Instruction::Let {
                        name: dst.clone(),
                        value: v,
                    });
                    Ok(Value::Temp(dst))
                } else {
                    Ok(Value::Null)
                };
                self.emit_local_defers(out);
                self.pop_defer_scope();
                res
            }
            Expr::Match { .. } => self.lower_match_expr(expr, out, tmp),
            Expr::StructLiteral { name, fields } => {
                let struct_name = name.clone();
                let def = self
                    .struct_defs
                    .get(&struct_name)
                    .ok_or_else(|| format!("error[E110]: unknown struct '{struct_name}'"))?
                    .clone();
                let mut field_map: HashMap<String, Value> = HashMap::new();
                for f in fields {
                    let v = self.lower_expr(&f.value, out, tmp)?;
                    field_map.insert(f.name.clone(), v);
                }
                for field in &def.fields {
                    if !field_map.contains_key(&field.name) {
                        return Err(format!(
                            "error[E110]: missing field '{}' in struct literal '{}'",
                            field.name, struct_name
                        ));
                    }
                }
                for key in field_map.keys() {
                    if def.fields.iter().all(|f| f.name != *key) {
                        return Err(format!(
                            "error[E110]: unknown field '{}' in struct literal '{}'",
                            key, struct_name
                        ));
                    }
                }
                let dst = format!("_t{}", {
                    let t = *tmp;
                    *tmp += 1;
                    t
                });
                let fields_vec = field_map.into_iter().collect::<Vec<_>>();
                out.push(Instruction::StructInit {
                    dst: dst.clone(),
                    struct_name: struct_name.clone(),
                    fields: fields_vec,
                });
                self.local_types
                    .insert(dst.clone(), IrType::Struct(struct_name));
                Ok(Value::Temp(dst))
            }
            Expr::ArrayLiteral(items) => {
                let elem_ty = self.infer_array_elem_type(items)?;
                let len_val = Value::Int(items.len() as i64);
                let dst = format!("_t{}", {
                    let t = *tmp;
                    *tmp += 1;
                    t
                });
                out.push(Instruction::ArrayInit {
                    dst: dst.clone(),
                    elem_ty: elem_ty.clone(),
                    len: len_val,
                });
                for (idx, item) in items.iter().enumerate() {
                    let v = self.lower_expr(item, out, tmp)?;
                    out.push(Instruction::ArraySet {
                        base: dst.clone(),
                        elem_ty: elem_ty.clone(),
                        index: Value::Int(idx as i64),
                        value: v,
                    });
                }
                self.local_types
                    .insert(dst.clone(), IrType::Array(Box::new(elem_ty)));
                Ok(Value::Temp(dst))
            }
            Expr::ArrayRepeat { value, len } => {
                let elem_ty = self.infer_expr_type(value)?;
                let len_val = self.lower_expr(len, out, tmp)?;
                let dst = format!("_t{}", {
                    let t = *tmp;
                    *tmp += 1;
                    t
                });
                out.push(Instruction::ArrayInit {
                    dst: dst.clone(),
                    elem_ty: elem_ty.clone(),
                    len: len_val.clone(),
                });

                // for i in 0..len { set }
                let i_local = format!("_t{}", {
                    let t = *tmp;
                    *tmp += 1;
                    t
                });
                out.push(Instruction::Let {
                    name: i_local.clone(),
                    value: Value::Int(0),
                });
                let cond_lbl = self.new_label("arrrep_cond");
                let body_lbl = self.new_label("arrrep_body");
                let end_lbl = self.new_label("arrrep_end");

                out.push(Instruction::Jump(cond_lbl.clone()));
                out.push(Instruction::Label(cond_lbl.clone()));
                let cond_tmp = format!("_t{}", {
                    let t = *tmp;
                    *tmp += 1;
                    t
                });
                out.push(Instruction::Binary {
                    dst: cond_tmp.clone(),
                    op: BinaryOp::Lt,
                    lhs: Value::Local(i_local.clone()),
                    rhs: len_val.clone(),
                });
                out.push(Instruction::Branch {
                    cond: Value::Local(cond_tmp.clone()),
                    then_label: body_lbl.clone(),
                    else_label: end_lbl.clone(),
                });

                out.push(Instruction::Label(body_lbl.clone()));
                let v = self.lower_expr(value, out, tmp)?;
                out.push(Instruction::ArraySet {
                    base: dst.clone(),
                    elem_ty: elem_ty.clone(),
                    index: Value::Local(i_local.clone()),
                    value: v,
                });
                let inc_tmp = format!("_t{}", {
                    let t = *tmp;
                    *tmp += 1;
                    t
                });
                out.push(Instruction::Binary {
                    dst: inc_tmp.clone(),
                    op: BinaryOp::Add,
                    lhs: Value::Local(i_local.clone()),
                    rhs: Value::Int(1),
                });
                out.push(Instruction::Let {
                    name: i_local.clone(),
                    value: Value::Local(inc_tmp.clone()),
                });
                out.push(Instruction::Jump(cond_lbl));
                out.push(Instruction::Label(end_lbl));

                self.local_types
                    .insert(dst.clone(), IrType::Array(Box::new(elem_ty)));
                Ok(Value::Temp(dst))
            }
            Expr::Index { object, index } => {
                let base = expr_to_local(object)?;
                let arr_ty = self
                    .local_types
                    .get(&base)
                    .cloned()
                    .ok_or_else(|| format!("error[E110]: unknown array '{base}'"))?;
                let elem_ty = match arr_ty {
                    IrType::Array(inner) => *inner,
                    _ => return Err("error[E110]: index target is not an array".into()),
                };
                let idx_val = self.lower_expr(index, out, tmp)?;
                let dst = format!("_t{}", {
                    let t = *tmp;
                    *tmp += 1;
                    t
                });
                out.push(Instruction::ArrayGet {
                    dst: dst.clone(),
                    elem_ty: elem_ty.clone(),
                    base,
                    index: idx_val,
                });
                self.local_types.insert(dst.clone(), elem_ty);
                Ok(Value::Temp(dst))
            }
            Expr::Slice { .. } => Err("error[E110]: slice expressions are not supported in IR lowering".to_string()),
            Expr::MethodCall { receiver, method, args } => {
                let recv_val = self.lower_expr(receiver, out, tmp)?;
                let recv_ty = self.infer_expr_type(receiver)?;
                let callee =
                    self.resolve_method_callee(&recv_ty, method)
                        .ok_or_else(|| {
                            format!("error[E110]: unknown method '{}' for receiver", method)
                        })?;
                let mut arg_exprs = Vec::with_capacity(args.len() + 1);
                arg_exprs.push((**receiver).clone());
                arg_exprs.extend(args.iter().cloned());
                let callee = self.monomorphize_call_if_needed(&callee, &arg_exprs)?;
                let mut arg_vals = Vec::new();
                arg_vals.push(recv_val);
                for a in args {
                    arg_vals.push(self.lower_expr(a, out, tmp)?);
                }
                let dst = format!("_t{}", {
                    let t = *tmp;
                    *tmp += 1;
                    t
                });
                out.push(Instruction::Call {
                    dst: dst.clone(),
                    callee,
                    args: arg_vals,
                });
                Ok(Value::Temp(dst))
            }
            Expr::TupleLiteral(items) => {
                let mut elem_vals = Vec::new();
                let mut elem_tys = Vec::new();
                for item in items {
                    let v = self.lower_expr(item, out, tmp)?;
                    elem_vals.push(v);
                    elem_tys.push(self.infer_expr_type(item)?);
                }
                let tuple_name = self.ensure_tuple_struct(&elem_tys);
                let dst = format!("_t{}", {
                    let t = *tmp;
                    *tmp += 1;
                    t
                });
                let fields = elem_vals
                    .into_iter()
                    .enumerate()
                    .map(|(idx, v)| (idx.to_string(), v))
                    .collect();
                out.push(Instruction::StructInit {
                    dst: dst.clone(),
                    struct_name: tuple_name.clone(),
                    fields,
                });
                self.local_types
                    .insert(dst.clone(), IrType::Struct(tuple_name));
                Ok(Value::Temp(dst))
            }
            Expr::Closure {
                params,
                return_ty: _,
                body,
            } => {
                let captures = self.collect_closure_captures(body, params)?;
                let mut capture_tys = Vec::new();
                for cap in &captures {
                    let ty = self
                        .local_types
                        .get(cap)
                        .cloned()
                        .unwrap_or(IrType::I64);
                    capture_tys.push(ty);
                }

                let env_name = format!("__closure_env_{}", self.closure_counter);
                let fn_name = format!("__closure_fn_{}", self.closure_counter);
                self.closure_counter += 1;

                let mut fields = Vec::new();
                for (cap, ty) in captures.iter().zip(capture_tys.iter()) {
                    fields.push(StructFieldDef {
                        name: cap.clone(),
                        ty: ty.clone(),
                    });
                }
                let env_def = StructDef {
                    name: env_name.clone(),
                    fields,
                };
                self.synthetic_structs
                    .insert(env_name.clone(), env_def);

                let closure_fn = self.lower_closure_fn(
                    &fn_name,
                    &captures,
                    &capture_tys,
                    params,
                    body,
                )?;
                self.synthetic_functions.push(closure_fn);

                let dst = format!("_t{}", {
                    let t = *tmp;
                    *tmp += 1;
                    t
                });
                let mut init_fields = Vec::new();
                for cap in &captures {
                    init_fields.push((cap.clone(), Value::Local(cap.clone())));
                }
                out.push(Instruction::StructInit {
                    dst: dst.clone(),
                    struct_name: env_name.clone(),
                    fields: init_fields,
                });
                self.local_types
                    .insert(dst.clone(), IrType::Struct(env_name.clone()));
                self.closure_locals.insert(
                    dst.clone(),
                    ClosureInstance {
                        env_struct: env_name,
                        fn_name,
                        captures,
                    },
                );
                Ok(Value::Temp(dst))
            }
            | Expr::BlockLiteral(_)
            | Expr::Range { .. }
            | Expr::Reference { .. }
            | Expr::IfExpr { .. }
            | Expr::AsyncBlock(_)
            | Expr::Loop(_) => {
                Err("error[E110]: unsupported expression in IR lowering".to_string())
            }
        }
    }

    fn lower_for_in(
        &mut self,
        var: &str,
        iter: &Expr,
        body: &[Stmt],
        out: &mut Vec<Instruction>,
        tmp: &mut usize,
    ) -> Result<(), String> {
        let range = self.lower_range_iter(iter, out, tmp)?;

        out.push(Instruction::Let {
            name: var.to_string(),
            value: range.start,
        });
        out.push(Instruction::Let {
            name: range.end_local.clone(),
            value: range.end,
        });

        let cond_lbl = self.new_label("forin_cond");
        let body_lbl = self.new_label("forin_body");
        let end_lbl = self.new_label("forin_end");
        self.loop_stack.push(LoopLabels {
            break_label: end_lbl.clone(),
            continue_label: cond_lbl.clone(),
            label_name: None,
        });

        out.push(Instruction::Jump(cond_lbl.clone()));
        out.push(Instruction::Label(cond_lbl.clone()));

        let cond_tmp = format!("_t{}", {
            let t = *tmp;
            *tmp += 1;
            t
        });
        out.push(Instruction::Binary {
            dst: cond_tmp.clone(),
            op: range.compare_op,
            lhs: Value::Local(var.to_string()),
            rhs: Value::Local(range.end_local.clone()),
        });
        out.push(Instruction::Branch {
            cond: Value::Temp(cond_tmp),
            then_label: body_lbl.clone(),
            else_label: end_lbl.clone(),
        });

        out.push(Instruction::Label(body_lbl));
        self.push_defer_scope();
        for s in body {
            self.lower_stmt(s, out, tmp)?;
        }
        self.emit_local_defers(out);
        self.pop_defer_scope();

        let inc_tmp = format!("_t{}", {
            let t = *tmp;
            *tmp += 1;
            t
        });
        out.push(Instruction::Binary {
            dst: inc_tmp.clone(),
            op: BinaryOp::Add,
            lhs: Value::Local(var.to_string()),
            rhs: Value::Int(range.step),
        });
        out.push(Instruction::Let {
            name: var.to_string(),
            value: Value::Temp(inc_tmp),
        });

        out.push(Instruction::Jump(cond_lbl));
        out.push(Instruction::Label(end_lbl));
        self.loop_stack.pop();
        Ok(())
    }

    fn lower_match_stmt(
        &mut self,
        stmt: &Stmt,
        out: &mut Vec<Instruction>,
        tmp: &mut usize,
    ) -> Result<(), String> {
        let Stmt::Match { expr, arms, .. } = stmt else {
            return Err("error[E110]: internal match lowering error".into());
        };
        self.lower_match(expr, arms, None, out, tmp)?;
        Ok(())
    }

    fn lower_match_expr(
        &mut self,
        expr: &Expr,
        out: &mut Vec<Instruction>,
        tmp: &mut usize,
    ) -> Result<Value, String> {
        let Expr::Match { expr, arms } = expr else {
            return Err("error[E110]: internal match lowering error".into());
        };
        let result_name = format!("_match{}", {
            let t = *tmp;
            *tmp += 1;
            t
        });
        self.lower_match(expr, arms, Some(&result_name), out, tmp)?;
        Ok(Value::Local(result_name))
    }

    fn lower_match(
        &mut self,
        expr: &Expr,
        arms: &[MatchArm],
        result: Option<&str>,
        out: &mut Vec<Instruction>,
        tmp: &mut usize,
    ) -> Result<(), String> {
        if !self.match_is_exhaustive(arms) {
            return Err("error[E110]: match must be exhaustive; add a wildcard arm".into());
        }

        let scrutinee = self.lower_expr(expr, out, tmp)?;
        let match_name = format!("_match_value{}", {
            let t = *tmp;
            *tmp += 1;
            t
        });
        out.push(Instruction::Let {
            name: match_name.clone(),
            value: scrutinee,
        });
        if let Ok(ty) = self.infer_expr_type(expr) {
            self.local_types.insert(match_name.clone(), ty);
        }

        let end_lbl = self.new_label("match_end");
        let mut next_lbl = self.new_label("match_next");

        for (idx, arm) in arms.iter().enumerate() {
            let arm_lbl = self.new_label("match_arm");
            let last = idx + 1 == arms.len();
            let fallback_lbl = if last { end_lbl.clone() } else { next_lbl.clone() };

            let cond = self.lower_match_condition(&arm.pattern, &match_name, out, tmp)?;
            let cond = if let Some(guard) = &arm.guard {
                let guard_val = self.lower_truthy(guard, out, tmp)?;
                self.lower_and(cond, guard_val, out, tmp)?
            } else {
                cond
            };

            out.push(Instruction::Branch {
                cond,
                then_label: arm_lbl.clone(),
                else_label: fallback_lbl.clone(),
            });

            out.push(Instruction::Label(arm_lbl));
            self.lower_match_bindings(&arm.pattern, &match_name, out)?;

            if let Some(dest) = result {
                let value = self.lower_match_body_value(&arm.body, out, tmp)?;
                out.push(Instruction::Let {
                    name: dest.to_string(),
                    value,
                });
            } else {
                self.lower_match_body_stmt(&arm.body, out, tmp)?;
            }

            out.push(Instruction::Jump(end_lbl.clone()));
            if !last {
                out.push(Instruction::Label(next_lbl.clone()));
                next_lbl = self.new_label("match_next");
            }
        }

        out.push(Instruction::Label(end_lbl));
        Ok(())
    }

    fn lower_match_body_stmt(
        &mut self,
        body: &MatchBody,
        out: &mut Vec<Instruction>,
        tmp: &mut usize,
    ) -> Result<(), String> {
        match body {
            MatchBody::Expr(e) => {
                self.lower_expr(e, out, tmp)?;
            }
            MatchBody::Stmt(s) => {
                self.lower_stmt(s, out, tmp)?;
            }
            MatchBody::Block(stmts) => {
                for s in stmts {
                    self.lower_stmt(s, out, tmp)?;
                }
            }
        }
        Ok(())
    }

    fn lower_match_body_value(
        &mut self,
        body: &MatchBody,
        out: &mut Vec<Instruction>,
        tmp: &mut usize,
    ) -> Result<Value, String> {
        match body {
            MatchBody::Expr(e) => self.lower_expr(e, out, tmp),
            MatchBody::Stmt(s) => {
                self.lower_stmt(s, out, tmp)?;
                Ok(Value::Null)
            }
            MatchBody::Block(stmts) => {
                for s in stmts {
                    self.lower_stmt(s, out, tmp)?;
                }
                Ok(Value::Null)
            }
        }
    }

    fn lower_match_condition(
        &mut self,
        pattern: &MatchPattern,
        match_name: &str,
        out: &mut Vec<Instruction>,
        tmp: &mut usize,
    ) -> Result<Value, String> {
        self.lower_pattern_condition(pattern, match_name, out, tmp)
    }

    fn lower_match_bindings(
        &mut self,
        pattern: &MatchPattern,
        match_name: &str,
        out: &mut Vec<Instruction>,
    ) -> Result<(), String> {
        self.lower_match_bindings_on_base(pattern, match_name, out)
    }

    fn lower_pattern_condition(
        &mut self,
        pattern: &MatchPattern,
        base: &str,
        out: &mut Vec<Instruction>,
        tmp: &mut usize,
    ) -> Result<Value, String> {
        match pattern {
            MatchPattern::Wildcard
            | MatchPattern::Identifier(_)
            | MatchPattern::Rest
            | MatchPattern::Binding(_, _) => Ok(Value::Bool(true)),
            MatchPattern::Literal(e) => {
                let lit = self.lower_literal_expr(e)?;
                Ok(self.lower_compare(Value::Local(base.to_string()), lit, out, tmp)?)
            }
            MatchPattern::Or(parts) => {
                let mut cond = None;
                for p in parts {
                    let next = self.lower_pattern_condition(p, base, out, tmp)?;
                    cond = Some(if let Some(prev) = cond {
                        self.lower_or(prev, next, out, tmp)?
                    } else {
                        next
                    });
                }
                cond.ok_or_else(|| "error[E110]: empty or-pattern in match".to_string())
            }
            MatchPattern::Tuple(parts) => {
                let tuple_ty = self
                    .local_types
                    .get(base)
                    .cloned()
                    .ok_or_else(|| format!("error[E110]: unknown tuple '{base}'"))?;
                let IrType::Struct(tuple_name) = tuple_ty else {
                    return Err("error[E110]: tuple pattern on non-tuple value".into());
                };
                let mut cond = None;
                for (idx, p) in parts.iter().enumerate() {
                    if matches!(p, MatchPattern::Rest) {
                        continue;
                    }
                    let field_ty = self.struct_field_type(&tuple_name, &idx.to_string())?;
                    let elem_tmp = format!("_t{}", {
                        let t = *tmp;
                        *tmp += 1;
                        t
                    });
                    out.push(Instruction::StructGet {
                        dst: elem_tmp.clone(),
                        struct_name: tuple_name.clone(),
                        base: base.to_string(),
                        field: idx.to_string(),
                    });
                    self.local_types.insert(elem_tmp.clone(), field_ty);
                    let next = self.lower_pattern_condition(p, &elem_tmp, out, tmp)?;
                    cond = Some(if let Some(prev) = cond {
                        self.lower_and(prev, next, out, tmp)?
                    } else {
                        next
                    });
                }
                Ok(cond.unwrap_or(Value::Bool(true)))
            }
            _ => Err("error[E110]: unsupported match pattern in IR lowering".to_string()),
        }
    }

    fn lower_match_bindings_on_base(
        &mut self,
        pattern: &MatchPattern,
        base: &str,
        out: &mut Vec<Instruction>,
    ) -> Result<(), String> {
        match pattern {
            MatchPattern::Identifier(name) => {
                out.push(Instruction::Let {
                    name: name.clone(),
                    value: Value::Local(base.to_string()),
                });
            }
            MatchPattern::Binding(name, inner) => {
                out.push(Instruction::Let {
                    name: name.clone(),
                    value: Value::Local(base.to_string()),
                });
                self.lower_match_bindings_on_base(inner, base, out)?;
            }
            MatchPattern::Tuple(parts) => {
                let tuple_ty = self
                    .local_types
                    .get(base)
                    .cloned()
                    .ok_or_else(|| format!("error[E110]: unknown tuple '{base}'"))?;
                let IrType::Struct(tuple_name) = tuple_ty else {
                    return Err("error[E110]: tuple pattern on non-tuple value".into());
                };
                for (idx, p) in parts.iter().enumerate() {
                    if matches!(p, MatchPattern::Rest) {
                        continue;
                    }
                    let field_ty = self.struct_field_type(&tuple_name, &idx.to_string())?;
                    let elem_tmp = format!("_tbind{}_{}", base, idx);
                    out.push(Instruction::StructGet {
                        dst: elem_tmp.clone(),
                        struct_name: tuple_name.clone(),
                        base: base.to_string(),
                        field: idx.to_string(),
                    });
                    self.local_types.insert(elem_tmp.clone(), field_ty);
                    self.lower_match_bindings_on_base(p, &elem_tmp, out)?;
                }
            }
            MatchPattern::Or(parts) => {
                for p in parts {
                    self.lower_match_bindings_on_base(p, base, out)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn lower_literal_expr(&self, expr: &Expr) -> Result<Value, String> {
        match expr {
            Expr::IntLiteral(n) => Ok(Value::Int(*n)),
            Expr::BigIntLiteral(_) => Err("error[E110]: big integer match literals not supported".into()),
            Expr::FloatLiteral(_) => Err("error[E110]: float match literals not yet supported".into()),
            Expr::StringLiteral(_) => {
                Err("error[E110]: string match literals not yet supported".into())
            }
            Expr::BoolLiteral(b) => Ok(Value::Bool(*b)),
            Expr::CharLiteral(c) => Ok(Value::Int(*c as i64)),
            Expr::NullLiteral => Ok(Value::Null),
            _ => Err("error[E110]: match literal must be a simple literal".into()),
        }
    }

    fn lower_compare(
        &mut self,
        lhs: Value,
        rhs: Value,
        out: &mut Vec<Instruction>,
        tmp: &mut usize,
    ) -> Result<Value, String> {
        let dst = format!("_t{}", {
            let t = *tmp;
            *tmp += 1;
            t
        });
        out.push(Instruction::Binary {
            dst: dst.clone(),
            op: BinaryOp::Eq,
            lhs,
            rhs,
        });
        Ok(Value::Temp(dst))
    }

    fn lower_truthy(
        &mut self,
        expr: &Expr,
        out: &mut Vec<Instruction>,
        tmp: &mut usize,
    ) -> Result<Value, String> {
        let value = self.lower_expr(expr, out, tmp)?;
        let dst = format!("_t{}", {
            let t = *tmp;
            *tmp += 1;
            t
        });
        out.push(Instruction::Binary {
            dst: dst.clone(),
            op: BinaryOp::Ne,
            lhs: value,
            rhs: Value::Int(0),
        });
        Ok(Value::Temp(dst))
    }

    fn lower_and(
        &mut self,
        lhs: Value,
        rhs: Value,
        out: &mut Vec<Instruction>,
        tmp: &mut usize,
    ) -> Result<Value, String> {
        let dst = format!("_t{}", {
            let t = *tmp;
            *tmp += 1;
            t
        });
        out.push(Instruction::Binary {
            dst: dst.clone(),
            op: BinaryOp::And,
            lhs,
            rhs,
        });
        Ok(Value::Temp(dst))
    }

    fn lower_or(
        &mut self,
        lhs: Value,
        rhs: Value,
        out: &mut Vec<Instruction>,
        tmp: &mut usize,
    ) -> Result<Value, String> {
        let dst = format!("_t{}", {
            let t = *tmp;
            *tmp += 1;
            t
        });
        out.push(Instruction::Binary {
            dst: dst.clone(),
            op: BinaryOp::Or,
            lhs,
            rhs,
        });
        Ok(Value::Temp(dst))
    }

    fn match_is_exhaustive(&self, arms: &[MatchArm]) -> bool {
        arms.iter().any(|arm| {
            arm.guard.is_none()
                && matches!(arm.pattern, MatchPattern::Wildcard | MatchPattern::Identifier(_))
        })
    }

    fn lower_range_iter(
        &mut self,
        iter: &Expr,
        out: &mut Vec<Instruction>,
        tmp: &mut usize,
    ) -> Result<RangeIter, String> {
        match iter {
            Expr::Range {
                start,
                end,
                inclusive,
            } => {
                let Some(start) = start else {
                    return Err("error[E110]: range start is required in for-in".into());
                };
                let Some(end) = end else {
                    return Err("error[E110]: range end is required in for-in".into());
                };
                let start_v = self.lower_expr(start, out, tmp)?;
                let end_v = self.lower_expr(end, out, tmp)?;
                let compare_op = if *inclusive {
                    BinaryOp::Le
                } else {
                    BinaryOp::Lt
                };
                Ok(RangeIter {
                    start: start_v,
                    end: end_v,
                    end_local: format!("_range_end{}", {
                        let t = *tmp;
                        *tmp += 1;
                        t
                    }),
                    step: 1,
                    compare_op,
                })
            }
            Expr::Call { callee, args } => {
                let name = match callee.as_ref() {
                    Expr::Identifier(n) => n.as_str(),
                    Expr::Path(p) => p.last().map(|s| s.as_str()).unwrap_or(""),
                    _ => "",
                };
                if name != "range" {
                    return Err(
                        "error[E110]: for-in only supports range(...) or a..b ranges".into(),
                    );
                }
                if args.len() < 2 || args.len() > 3 {
                    return Err("error[E110]: range(...) expects 2 or 3 arguments".into());
                }
                let start_v = self.lower_expr(&args[0], out, tmp)?;
                let end_v = self.lower_expr(&args[1], out, tmp)?;
                let step = if args.len() == 3 {
                    match args[2] {
                        Expr::IntLiteral(n) if n != 0 => n,
                        Expr::IntLiteral(_) => {
                            return Err("error[E110]: range step cannot be zero".into())
                        }
                        Expr::BigIntLiteral(_) => {
                            return Err(
                                "error[E110]: range step must be a small integer literal".into(),
                            )
                        }
                        _ => {
                            return Err(
                                "error[E110]: range step must be an integer literal".into(),
                            )
                        }
                    }
                } else {
                    1
                };
                let compare_op = if step > 0 {
                    BinaryOp::Lt
                } else {
                    BinaryOp::Gt
                };
                Ok(RangeIter {
                    start: start_v,
                    end: end_v,
                    end_local: format!("_range_end{}", {
                        let t = *tmp;
                        *tmp += 1;
                        t
                    }),
                    step,
                    compare_op,
                })
            }
            _ => Err("error[E110]: for-in only supports range iteration".into()),
        }
    }

    fn collect_structs(&mut self, program: &Program) -> Result<Vec<StructDef>, String> {
        let mut structs = Vec::new();
        self.struct_defs.clear();
        for item in &program.items {
            if let ItemKind::Struct(s) = &item.kind {
                let mut fields = Vec::new();
                for f in &s.fields {
                    fields.push(StructFieldDef {
                        name: f.name.clone(),
                        ty: self.ast_type_to_ir(&f.field_type),
                    });
                }
                let def = StructDef {
                    name: s.name.clone(),
                    fields,
                };
                self.struct_defs.insert(s.name.clone(), def.clone());
                structs.push(def);
            }
        }
        Ok(structs)
    }

    fn collect_generic_functions(&mut self, program: &Program) {
        self.generic_fns.clear();
        for item in &program.items {
            if let ItemKind::Function(f) = &item.kind {
                if !f.generics.is_empty() {
                    self.generic_fns.insert(f.name.clone(), f.clone());
                }
            }
        }
    }

    fn collect_impl_methods(&mut self, program: &Program) -> Result<(), String> {
        self.impl_methods.clear();
        for item in &program.items {
            if let ItemKind::Impl(ib) = &item.kind {
                let self_name = self.type_path_name(&ib.self_type)?;
                let trait_name = ib.trait_name.as_ref().map(|t| t.last_name().to_string());
                for it in &ib.items {
                    if let ImplItem::Method(m) = it {
                        let key = (self_name.clone(), m.name.clone(), trait_name.clone());
                        let mangled =
                            self.method_mangled_name(&self_name, trait_name.as_deref(), &m.name);
                        self.impl_methods.insert(key, mangled);
                    }
                }
            }
        }
        Ok(())
    }

    fn type_path_name(&self, ty: &Type) -> Result<String, String> {
        match ty {
            Type::Named(path) => Ok(path.last_name().to_string()),
            _ => Err("error[E110]: impl self type must be a named type".into()),
        }
    }

    fn method_mangled_name(
        &self,
        self_name: &str,
        trait_name: Option<&str>,
        method: &str,
    ) -> String {
        if let Some(tr) = trait_name {
            format!("{tr}::{self_name}::{method}")
        } else {
            format!("{self_name}::{method}")
        }
    }

    fn resolve_method_callee(&self, recv_ty: &IrType, method: &str) -> Option<String> {
        let recv_name = match recv_ty {
            IrType::Struct(n) => n,
            _ => return None,
        };
        let key = (recv_name.clone(), method.to_string(), None);
        if let Some(name) = self.impl_methods.get(&key) {
            return Some(name.clone());
        }
        let mut found: Option<String> = None;
        for ((ty, meth, trait_name), name) in &self.impl_methods {
            if ty == recv_name && meth == method && trait_name.is_some() {
                if found.is_some() {
                    return None;
                }
                found = Some(name.clone());
            }
        }
        found
    }

    fn monomorphize_call_if_needed(
        &mut self,
        callee: &str,
        args: &[Expr],
    ) -> Result<String, String> {
        let Some(decl) = self.generic_fns.get(callee).cloned() else {
            return Ok(callee.to_string());
        };
        let subst = self.infer_call_subst(&decl, args)?;
        let mut parts = Vec::new();
        for gp in &decl.generics {
            let ty = subst
                .get(&gp.name)
                .ok_or_else(|| format!("error[E110]: cannot infer generic '{}'", gp.name))?;
            parts.push(self.ir_type_sig(ty));
        }
        if parts.is_empty() {
            return Ok(callee.to_string());
        }
        let sig = parts.join("_");
        let mono_name = format!("{callee}${sig}");
        if !self.seen_monomorphized.contains_key(&mono_name) {
            self.seen_monomorphized
                .insert(mono_name.clone(), callee.to_string());
            self.pending_monomorphized.push(MonoRequest {
                name: mono_name.clone(),
                decl,
                subst,
            });
        }
        Ok(mono_name)
    }

    fn infer_call_subst(
        &mut self,
        decl: &FunctionDecl,
        args: &[Expr],
    ) -> Result<HashMap<String, IrType>, String> {
        if args.len() < decl.params.len() {
            return Err("error[E110]: insufficient arguments to infer generics".into());
        }
        let mut subst: HashMap<String, IrType> = HashMap::new();
        let generics: std::collections::HashSet<String> =
            decl.generics.iter().map(|g| g.name.clone()).collect();
        for (param, arg) in decl.params.iter().zip(args.iter()) {
            let arg_ty = self.infer_expr_type(arg)?;
            self.unify_type(&param.param_type, &arg_ty, &mut subst, &generics)?;
        }
        Ok(subst)
    }

    fn unify_type(
        &self,
        param_ty: &Type,
        arg_ty: &IrType,
        subst: &mut HashMap<String, IrType>,
        generics: &std::collections::HashSet<String>,
    ) -> Result<(), String> {
        match param_ty {
            Type::Named(path) => {
                let name = path.last_name();
                if generics.contains(name) {
                    if let Some(g) = subst.get(name) {
                        if g != arg_ty {
                            return Err("error[E110]: generic type mismatch".into());
                        }
                    } else {
                        subst.insert(name.to_string(), arg_ty.clone());
                    }
                }
                Ok(())
            }
            Type::Array(inner) => {
                if let IrType::Array(arg_inner) = arg_ty {
                    self.unify_type(inner, arg_inner, subst, generics)
                } else {
                    Err("error[E110]: generic array type mismatch".into())
                }
            }
            Type::Tuple(inner) => {
                let IrType::Struct(name) = arg_ty else {
                    return Err("error[E110]: generic tuple type mismatch".into());
                };
                let def = self
                    .struct_defs
                    .get(name)
                    .or_else(|| self.synthetic_structs.get(name))
                    .ok_or_else(|| "error[E110]: unknown tuple type".to_string())?;
                if def.fields.len() != inner.len() {
                    return Err("error[E110]: tuple arity mismatch".into());
                }
                for (field, ty) in def.fields.iter().zip(inner.iter()) {
                    self.unify_type(ty, &field.ty, subst, generics)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn ensure_tuple_struct(&mut self, elem_tys: &[IrType]) -> String {
        let name = self.tuple_struct_name(elem_tys);
        if self.struct_defs.contains_key(&name) || self.synthetic_structs.contains_key(&name) {
            return name;
        }
        let mut fields = Vec::new();
        for (idx, ty) in elem_tys.iter().enumerate() {
            fields.push(StructFieldDef {
                name: idx.to_string(),
                ty: ty.clone(),
            });
        }
        let def = StructDef {
            name: name.clone(),
            fields,
        };
        self.synthetic_structs.insert(name.clone(), def);
        name
    }

    fn tuple_struct_name(&self, elem_tys: &[IrType]) -> String {
        if elem_tys.is_empty() {
            return "__tuple_unit".to_string();
        }
        let mut parts = Vec::new();
        for ty in elem_tys {
            parts.push(self.ir_type_sig(ty));
        }
        format!("__tuple_{}", parts.join("_"))
    }

    fn ir_type_sig(&self, ty: &IrType) -> String {
        match ty {
            IrType::I64 => "i64".to_string(),
            IrType::F64 => "f64".to_string(),
            IrType::Ptr => "ptr".to_string(),
            IrType::Struct(n) => format!("s{}", n.replace("::", "_")),
            IrType::Array(inner) => format!("arr{}", self.ir_type_sig(inner)),
        }
    }

    fn propagate_closure_local(&mut self, expr: &Expr, name: &str) {
        let src = match expr {
            Expr::Identifier(n) => Some(n.clone()),
            Expr::Path(parts) => Some(parts.join("::")),
            _ => None,
        };
        if let Some(src) = src {
            if let Some(cl) = self.closure_locals.get(&src).cloned() {
                self.local_types
                    .insert(name.to_string(), IrType::Struct(cl.env_struct.clone()));
                self.closure_locals.insert(name.to_string(), cl);
            }
        }
    }

    fn propagate_closure_value(&mut self, value: &Value, name: &str) {
        let src = match value {
            Value::Local(n) | Value::Temp(n) => Some(n.clone()),
            _ => None,
        };
        if let Some(src) = src {
            if let Some(cl) = self.closure_locals.get(&src).cloned() {
                self.closure_locals.insert(name.to_string(), cl);
            }
        }
    }

    fn collect_closure_captures(
        &self,
        body: &Expr,
        params: &[ClosureParam],
    ) -> Result<Vec<String>, String> {
        let mut locals: Vec<std::collections::HashSet<String>> = Vec::new();
        locals.push(params.iter().map(|p| p.name.clone()).collect());
        let mut captures: std::collections::HashSet<String> = std::collections::HashSet::new();
        self.walk_expr_for_captures(body, &mut locals, &mut captures);
        let mut out: Vec<String> = captures.into_iter().collect();
        out.sort();
        Ok(out)
    }

    fn walk_expr_for_captures(
        &self,
        expr: &Expr,
        locals: &mut Vec<std::collections::HashSet<String>>,
        captures: &mut std::collections::HashSet<String>,
    ) {
        match expr {
            Expr::Identifier(name) => {
                if !locals.iter().any(|s| s.contains(name))
                    && self.local_types.contains_key(name)
                {
                    captures.insert(name.clone());
                }
            }
            Expr::Block(stmts, tail) => {
                locals.push(std::collections::HashSet::new());
                for s in stmts {
                    self.walk_stmt_for_captures(s, locals, captures);
                }
                if let Some(t) = tail {
                    self.walk_expr_for_captures(t, locals, captures);
                }
                locals.pop();
            }
            Expr::Call { callee, args } => {
                self.walk_expr_for_captures(callee, locals, captures);
                for a in args {
                    self.walk_expr_for_captures(a, locals, captures);
                }
            }
            Expr::MethodCall { receiver, args, .. } => {
                self.walk_expr_for_captures(receiver, locals, captures);
                for a in args {
                    self.walk_expr_for_captures(a, locals, captures);
                }
            }
            Expr::Binary { left, right, .. } => {
                self.walk_expr_for_captures(left, locals, captures);
                self.walk_expr_for_captures(right, locals, captures);
            }
            Expr::Unary { right, .. }
            | Expr::Await(right)
            | Expr::Move(right)
            | Expr::Deref(right)
            | Expr::TryOp(right) => {
                self.walk_expr_for_captures(right, locals, captures);
            }
            Expr::FieldAccess { object, .. } => {
                self.walk_expr_for_captures(object, locals, captures);
            }
            Expr::Index { object, index } => {
                self.walk_expr_for_captures(object, locals, captures);
                self.walk_expr_for_captures(index, locals, captures);
            }
            Expr::Slice { object, start, end } => {
                self.walk_expr_for_captures(object, locals, captures);
                if let Some(s) = start {
                    self.walk_expr_for_captures(s, locals, captures);
                }
                if let Some(e) = end {
                    self.walk_expr_for_captures(e, locals, captures);
                }
            }
            Expr::StructLiteral { fields, .. } => {
                for f in fields {
                    self.walk_expr_for_captures(&f.value, locals, captures);
                }
            }
            Expr::ArrayLiteral(items) | Expr::TupleLiteral(items) => {
                for it in items {
                    self.walk_expr_for_captures(it, locals, captures);
                }
            }
            Expr::ArrayRepeat { value, len } => {
                self.walk_expr_for_captures(value, locals, captures);
                self.walk_expr_for_captures(len, locals, captures);
            }
            Expr::IfExpr { branches, else_body } => {
                for b in branches {
                    self.walk_expr_for_captures(&b.condition, locals, captures);
                    locals.push(std::collections::HashSet::new());
                    for s in &b.body {
                        self.walk_stmt_for_captures(s, locals, captures);
                    }
                    locals.pop();
                }
                if let Some(e) = else_body {
                    self.walk_expr_for_captures(e, locals, captures);
                }
            }
            Expr::Ternary {
                condition,
                then_expr,
                else_expr,
            } => {
                self.walk_expr_for_captures(condition, locals, captures);
                self.walk_expr_for_captures(then_expr, locals, captures);
                self.walk_expr_for_captures(else_expr, locals, captures);
            }
            Expr::Match { expr, arms } => {
                self.walk_expr_for_captures(expr, locals, captures);
                for arm in arms {
                    locals.push(std::collections::HashSet::new());
                    match &arm.body {
                        MatchBody::Expr(e) => self.walk_expr_for_captures(e, locals, captures),
                        MatchBody::Stmt(s) => self.walk_stmt_for_captures(s, locals, captures),
                        MatchBody::Block(stmts) => {
                            for s in stmts {
                                self.walk_stmt_for_captures(s, locals, captures);
                            }
                        }
                    }
                    locals.pop();
                }
            }
            Expr::Closure { .. } => {}
            _ => {}
        }
    }

    fn walk_stmt_for_captures(
        &self,
        stmt: &Stmt,
        locals: &mut Vec<std::collections::HashSet<String>>,
        captures: &mut std::collections::HashSet<String>,
    ) {
        match stmt {
            Stmt::Let { name, expr, .. } => {
                self.walk_expr_for_captures(expr, locals, captures);
                if let Some(scope) = locals.last_mut() {
                    scope.insert(name.clone());
                }
            }
            Stmt::Assign { target, value, .. } => {
                self.walk_expr_for_captures(target, locals, captures);
                self.walk_expr_for_captures(value, locals, captures);
            }
            Stmt::CompoundAssign { target, value, .. } => {
                self.walk_expr_for_captures(target, locals, captures);
                self.walk_expr_for_captures(value, locals, captures);
            }
            Stmt::Expr(e) | Stmt::Print { expr: e } => {
                self.walk_expr_for_captures(e, locals, captures);
            }
            Stmt::Return { expr, .. } => {
                if let Some(e) = expr {
                    self.walk_expr_for_captures(e, locals, captures);
                }
            }
            Stmt::If { branches, else_body, .. } => {
                for b in branches {
                    self.walk_expr_for_captures(&b.condition, locals, captures);
                    locals.push(std::collections::HashSet::new());
                    for s in &b.body {
                        self.walk_stmt_for_captures(s, locals, captures);
                    }
                    locals.pop();
                }
                if let Some(eb) = else_body {
                    locals.push(std::collections::HashSet::new());
                    for s in eb {
                        self.walk_stmt_for_captures(s, locals, captures);
                    }
                    locals.pop();
                }
            }
            Stmt::While { condition, body, .. } => {
                self.walk_expr_for_captures(condition, locals, captures);
                locals.push(std::collections::HashSet::new());
                for s in body {
                    self.walk_stmt_for_captures(s, locals, captures);
                }
                locals.pop();
            }
            Stmt::Loop { body, .. } | Stmt::Unsafe { body, .. } => {
                locals.push(std::collections::HashSet::new());
                for s in body {
                    self.walk_stmt_for_captures(s, locals, captures);
                }
                locals.pop();
            }
            Stmt::ForIn { var, iter, body, .. } => {
                self.walk_expr_for_captures(iter, locals, captures);
                locals.push(std::collections::HashSet::new());
                if let Some(scope) = locals.last_mut() {
                    scope.insert(var.clone());
                }
                for s in body {
                    self.walk_stmt_for_captures(s, locals, captures);
                }
                locals.pop();
            }
            Stmt::Match { expr, arms, .. } => {
                self.walk_expr_for_captures(expr, locals, captures);
                for arm in arms {
                    locals.push(std::collections::HashSet::new());
                    match &arm.body {
                        MatchBody::Expr(e) => self.walk_expr_for_captures(e, locals, captures),
                        MatchBody::Stmt(s) => self.walk_stmt_for_captures(s, locals, captures),
                        MatchBody::Block(stmts) => {
                            for s in stmts {
                                self.walk_stmt_for_captures(s, locals, captures);
                            }
                        }
                    }
                    locals.pop();
                }
            }
            Stmt::InlineAsm { outputs, inputs, .. } => {
                for o in outputs {
                    self.walk_expr_for_captures(&o.expr, locals, captures);
                }
                for i in inputs {
                    self.walk_expr_for_captures(&i.expr, locals, captures);
                }
            }
            _ => {}
        }
    }

    fn lower_closure_fn(
        &mut self,
        fn_name: &str,
        captures: &[String],
        capture_tys: &[IrType],
        params: &[ClosureParam],
        body: &Expr,
    ) -> Result<IrFunction, String> {
        for p in params {
            if captures.iter().any(|c| c == &p.name) {
                return Err(format!(
                    "error[E110]: closure param '{}' shadows captured name",
                    p.name
                ));
            }
        }
        let mut instrs = Vec::new();
        let mut tmp = 0usize;
        let prev_locals = std::mem::take(&mut self.local_types);
        let prev_closures = std::mem::take(&mut self.closure_locals);

        let mut params_out = Vec::new();
        for (cap, ty) in captures.iter().zip(capture_tys.iter()) {
            self.local_types.insert(cap.clone(), ty.clone());
            params_out.push(IrParam {
                name: cap.clone(),
                ty: ty.clone(),
            });
        }
        for p in params {
            let ty = p
                .ty
                .as_ref()
                .map(|t| self.ast_type_to_ir(t))
                .unwrap_or(IrType::I64);
            self.local_types.insert(p.name.clone(), ty.clone());
            params_out.push(IrParam {
                name: p.name.clone(),
                ty,
            });
        }

        let value = self.lower_expr(body, &mut instrs, &mut tmp)?;
        instrs.push(Instruction::Return { value: Some(value) });

        let func = IrFunction {
            name: fn_name.to_string(),
            params: params_out,
            instructions: instrs,
        };

        self.local_types = prev_locals;
        self.closure_locals = prev_closures;
        Ok(func)
    }

    fn ast_type_to_ir(&mut self, ty: &Type) -> IrType {
        match ty {
            Type::Named(path) => {
                let name = path.last_name().to_string();
                if let Some(subst) = self.type_subst.as_ref().and_then(|s| s.get(&name)) {
                    return subst.clone();
                }
                match name.as_str() {
                    "i64" | "int" | "i32" | "i16" | "i8" | "u64" | "u32" | "u16" | "u8"
                    | "bool" | "char" => IrType::I64,
                    "f64" | "float" | "f32" => IrType::F64,
                    "string" | "String" | "str" => IrType::Ptr,
                    _ => IrType::Struct(name),
                }
            }
            Type::Reference(_) | Type::MutReference(_) | Type::Pointer { .. } => IrType::Ptr,
            Type::Array(inner) => IrType::Array(Box::new(self.ast_type_to_ir(inner))),
            Type::Tuple(types) => {
                let mut elem_tys = Vec::new();
                for t in types {
                    elem_tys.push(self.ast_type_to_ir(t));
                }
                let name = self.ensure_tuple_struct(&elem_tys);
                IrType::Struct(name)
            }
            Type::Literal(lit) => match lit {
                TypeLiteral::Int(_) | TypeLiteral::Bool(_) => IrType::I64,
                TypeLiteral::Float(_) => IrType::F64,
                TypeLiteral::String(_) => IrType::Ptr,
            },
            _ => IrType::I64,
        }
    }

    fn struct_field_type(&self, struct_name: &str, field: &str) -> Result<IrType, String> {
        let def = self
            .struct_defs
            .get(struct_name)
            .or_else(|| self.synthetic_structs.get(struct_name))
            .ok_or_else(|| format!("error[E110]: unknown struct '{struct_name}'"))?;
        def.fields
            .iter()
            .find(|f| f.name == field)
            .map(|f| f.ty.clone())
            .ok_or_else(|| {
                format!(
                    "error[E110]: unknown field '{}' in struct '{}'",
                    field, struct_name
                )
            })
    }

    fn infer_expr_type(&mut self, expr: &Expr) -> Result<IrType, String> {
        Ok(match expr {
            Expr::IntLiteral(_) | Expr::BoolLiteral(_) | Expr::CharLiteral(_) => IrType::I64,
            Expr::BigIntLiteral(_) => {
                return Err("error[E110]: big integer literals are not supported in IR types".into())
            }
            Expr::FloatLiteral(_) => IrType::F64,
            Expr::StringLiteral(_) => IrType::Ptr,
            Expr::StructLiteral { name, .. } => IrType::Struct(name.clone()),
            Expr::ArrayLiteral(items) => {
                let elem = self.infer_array_elem_type(items)?;
                IrType::Array(Box::new(elem))
            }
            Expr::TupleLiteral(items) => {
                let mut elem_tys = Vec::new();
                for it in items {
                    elem_tys.push(self.infer_expr_type(it)?);
                }
                let name = self.ensure_tuple_struct(&elem_tys);
                IrType::Struct(name)
            }
            Expr::Identifier(name) => self
                .local_types
                .get(name)
                .cloned()
                .unwrap_or(IrType::I64),
            Expr::Ternary {
                then_expr,
                else_expr,
                ..
            } => {
                let then_ty = self.infer_expr_type(then_expr)?;
                let else_ty = self.infer_expr_type(else_expr)?;
                if then_ty == else_ty {
                    then_ty
                } else {
                    IrType::I64
                }
            }
            _ => IrType::I64,
        })
    }

    fn infer_array_elem_type(&mut self, items: &[Expr]) -> Result<IrType, String> {
        if items.is_empty() {
            return Ok(IrType::I64);
        }
        let first = self.infer_expr_type(&items[0])?;
        for it in &items[1..] {
            let ty = self.infer_expr_type(it)?;
            if ty != first {
                return Err("error[E110]: array literal must have uniform element types".into());
            }
        }
        Ok(first)
    }
}

impl IrBuilder {
    fn resolve_loop_target(&self, label: Option<&str>) -> Result<LoopLabels, String> {
        if self.loop_stack.is_empty() {
            return Err("error[E110]: 'break' or 'continue' used outside of a loop".into());
        }
        if let Some(name) = label {
            for entry in self.loop_stack.iter().rev() {
                if entry.label_name.as_deref() == Some(name) {
                    return Ok(entry.clone());
                }
            }
            return Err(format!("error[E110]: unknown loop label '{name}'"));
        }
        Ok(self.loop_stack.last().cloned().unwrap())
    }
}

struct RangeIter {
    start: Value,
    end: Value,
    end_local: String,
    step: i64,
    compare_op: BinaryOp,
}

fn expr_to_local(e: &Expr) -> Result<String, String> {
    match e {
        Expr::Identifier(n) => Ok(n.clone()),
        Expr::Path(parts) => Ok(parts.join("::")),
        Expr::FieldAccess { object, field } => {
            Ok(format!("{}.{field}", expr_to_local(object)?))
        }
        _ => Err("error[E110]: assignment target must be an identifier or field".into()),
    }
}
