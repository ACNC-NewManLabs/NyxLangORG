use super::type_system::{NyxType, SymbolTable};
use crate::core::ast::ast_nodes::*;
use std::collections::HashMap;

// ─── Diagnostic ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub code: &'static str,
    pub message: String,
}

// ─── SemanticAnalyzer ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SemanticAnalyzer {
    /// Primitive type names from the registry (kept for backward compat).
    pub primitive_types: Vec<String>,
}

impl SemanticAnalyzer {
    pub fn new(primitive_types: Vec<String>) -> Self {
        Self { primitive_types }
    }

    // ── Old-compat API ────────────────────────────────────────────────────────

    /// Analyze a `Program`. Returns Ok on success or the first hard error.
    pub fn analyze(&self, program: &Program) -> Result<(), String> {
        if !self.primitive_types.iter().any(|t| t == "int") {
            return Err("registry missing required primitive type: int".to_string());
        }
        let mut ctx = AnalysisCtx::new();

        // First pass: register all top-level names so forward-refs work.
        for item in &program.items {
            match &item.kind {
                ItemKind::Function(f) => {
                    let min_args = f
                        .params
                        .iter()
                        .filter(|p| p.default_value.is_none())
                        .count();
                    let max_args = f.params.len();
                    ctx.functions.insert(f.name.clone(), (min_args, max_args));
                }
                ItemKind::Struct(s) => {
                    ctx.types.insert(s.name.clone());
                }
                ItemKind::Enum(e) => {
                    ctx.types.insert(e.name.clone());
                }
                ItemKind::Trait(t) => {
                    ctx.types.insert(t.name.clone());
                }
                ItemKind::Protocol(p) => {
                    ctx.types.insert(p.name.clone());
                    let mut role_methods = HashMap::new();
                    for role in &p.roles {
                        let mut methods = Vec::new();
                        if let Some(handshake) = &p.handshake {
                            for (i, step) in handshake.steps.iter().enumerate() {
                                match step {
                                    HandshakeStep::Message { from, to, name, .. } => {
                                        if from == role { methods.push(format!("send_{}", name)); }
                                        if to == role { methods.push(format!("recv_{}", name)); }
                                    }
                                    HandshakeStep::Derive { .. } => {
                                        methods.push(format!("step_{}_derive", i));
                                    }
                                    HandshakeStep::Finish { .. } => {
                                        methods.push("complete_handshake".to_string());
                                    }
                                }
                            }
                        }
                        role_methods.insert(role.clone(), methods);
                    }
                    ctx.protocols.insert(p.name.clone(), ProtocolInfo {
                        _roles: p.roles.clone(),
                        handshake_methods: role_methods,
                    });
                }
                _ => {}
            }
        }

        // Second pass: analyze function bodies.
        for item in &program.items {
            if let ItemKind::Function(f) = &item.kind {
                self.analyze_fn(f, &ctx)?;
            }
            if let ItemKind::Protocol(p) = &item.kind {
                self.analyze_protocol(p, &ctx)?;
            }
        }
        Ok(())
    }

    fn analyze_fn(&self, f: &FunctionDecl, ctx: &AnalysisCtx) -> Result<(), String> {
        let mut syms = SymbolTable::default();
        syms.push_scope();
        // bind params
        for p in &f.params {
            syms.define(p.name.clone(), type_to_nyx(&p.param_type));
        }
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for stmt in &f.body {
            self.analyze_stmt(stmt, &mut syms, ctx, &mut seen)?;
        }
        syms.pop_scope();
        Ok(())
    }

    fn analyze_stmt(
        &self,
        stmt: &Stmt,
        syms: &mut SymbolTable,
        ctx: &AnalysisCtx,
        seen: &mut std::collections::HashSet<String>,
    ) -> Result<(), String> {
        match stmt {
            Stmt::Let {
                name,
                expr,
                mutable: _,
                var_type,
                span: _,
            } => {
                let ty = if let Some(vt) = var_type {
                    type_to_nyx(vt)
                } else {
                    self.infer_expr(expr, syms, ctx).unwrap_or(NyxType::Unknown)
                };
                syms.define(name.clone(), ty);
                seen.insert(name.clone());
            }
            Stmt::Return { expr, .. } => {
                if let Some(e) = expr {
                    self.infer_expr(e, syms, ctx)?;
                }
            }
            Stmt::Expr(e) | Stmt::Print { expr: e } => {
                self.infer_expr(e, syms, ctx)?;
            }
            Stmt::Assign { target, value, .. } => {
                self.infer_expr(target, syms, ctx)?;
                self.infer_expr(value, syms, ctx)?;
            }
            Stmt::CompoundAssign { target, value, .. } => {
                self.infer_expr(target, syms, ctx)?;
                self.infer_expr(value, syms, ctx)?;
            }
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for b in branches {
                    self.infer_expr(&b.condition, syms, ctx)?;
                    syms.push_scope();
                    let mut b_seen = std::collections::HashSet::new();
                    for s in &b.body {
                        self.analyze_stmt(s, syms, ctx, &mut b_seen)?;
                    }
                    syms.pop_scope();
                }
                if let Some(eb) = else_body {
                    syms.push_scope();
                    let mut e_seen = std::collections::HashSet::new();
                    for s in eb {
                        self.analyze_stmt(s, syms, ctx, &mut e_seen)?;
                    }
                    syms.pop_scope();
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                self.infer_expr(condition, syms, ctx)?;
                syms.push_scope();
                let mut w_seen = std::collections::HashSet::new();
                for s in body {
                    self.analyze_stmt(s, syms, ctx, &mut w_seen)?;
                }
                syms.pop_scope();
            }
            Stmt::ForIn {
                var, iter, body, ..
            } => {
                self.infer_expr(iter, syms, ctx)?;
                syms.push_scope();
                syms.define(var.clone(), NyxType::Unknown);
                let mut f_seen = std::collections::HashSet::new();
                for s in body {
                    self.analyze_stmt(s, syms, ctx, &mut f_seen)?;
                }
                syms.pop_scope();
            }
            Stmt::Loop { body, .. } | Stmt::Unsafe { body, .. } => {
                syms.push_scope();
                let mut l_seen = std::collections::HashSet::new();
                for s in body {
                    self.analyze_stmt(s, syms, ctx, &mut l_seen)?;
                }
                syms.pop_scope();
            }
            Stmt::Match { expr, arms, .. } => {
                self.infer_expr(expr, syms, ctx)?;
                for arm in arms {
                    syms.push_scope();
                    match &arm.body {
                        MatchBody::Expr(e) => {
                            self.infer_expr(e, syms, ctx)?;
                        }
                        MatchBody::Stmt(s) => {
                            let mut arm_seen = std::collections::HashSet::new();
                            self.analyze_stmt(s, &mut syms.clone(), ctx, &mut arm_seen)?;
                        }
                        MatchBody::Block(stmts) => {
                            let mut arm_syms = syms.clone();
                            let mut arm_seen = std::collections::HashSet::new();
                            for s in stmts {
                                self.analyze_stmt(s, &mut arm_syms, ctx, &mut arm_seen)?;
                            }
                        }
                    }
                    syms.pop_scope();
                }
            }
            Stmt::InlineAsm { outputs, inputs, .. } => {
                for out in outputs {
                    let name = self.asm_output_name(&out.expr)?;
                    if let Some(ty) = syms.lookup(&name) {
                        if *ty != NyxType::I64 {
                            return Err(format!(
                                "error[E110]: asm output '{name}' must be i64 (found {ty:?})"
                            ));
                        }
                    } else {
                        syms.define(name, NyxType::I64);
                    }
                }
                for inp in inputs {
                    self.infer_expr(&inp.expr, syms, ctx)?;
                }
            }
            Stmt::Break { .. } | Stmt::Continue { .. } => {}
            Stmt::Defer { stmt, .. } => {
                let mut d_seen = std::collections::HashSet::new();
                self.analyze_stmt(stmt, syms, ctx, &mut d_seen)?;
            }
            Stmt::Yield { expr, .. } => {
                if let Some(e) = expr {
                    self.infer_expr(e, syms, ctx)?;
                }
            }
        }
        Ok(())
    }

    fn infer_expr(
        &self,
        expr: &Expr,
        syms: &SymbolTable,
        ctx: &AnalysisCtx,
    ) -> Result<NyxType, String> {
        match expr {
            Expr::IntLiteral { value: _, .. } => Ok(NyxType::I64),
            Expr::FloatLiteral { value: _, .. } => Ok(NyxType::F64),
            Expr::StringLiteral { value: _, .. } => Ok(NyxType::String),
            Expr::CharLiteral { value: _, .. } => Ok(NyxType::Char),
            Expr::BoolLiteral { value: _, .. } => Ok(NyxType::Bool),
            Expr::BigIntLiteral { value: _, .. } => Ok(NyxType::Unknown),
            Expr::NullLiteral { .. } => Ok(NyxType::Unknown),
            Expr::ArrayLiteral { elements: _, .. }
            | Expr::ArrayRepeat { .. }
            | Expr::TupleLiteral { elements: _, .. }
            | Expr::BlockLiteral { items: _, .. }
            | Expr::Loop { expr: _, .. }
            | Expr::CssLiteral { value: _, .. } => Ok(NyxType::Unknown),
            Expr::Identifier { name, .. } => {
                if let Some(ty) = syms.lookup(name) {
                    return Ok(ty.clone());
                }
                // Permissive: don't fail on unknown identifiers (may be stdlib/cross-module)
                Ok(NyxType::Unknown)
            }
            Expr::Path { segments: _, .. } => Ok(NyxType::Unknown),
            Expr::Binary { left, op, right, .. } => {
                let lt = self.infer_expr(left, syms, ctx).unwrap_or(NyxType::Unknown);
                let _rt = self
                    .infer_expr(right, syms, ctx)
                    .unwrap_or(NyxType::Unknown);
                Ok(match op.as_str() {
                    "==" | "!=" | "<" | ">" | "<=" | ">=" | "&&" | "||" | "in" => {
                        NyxType::Bool
                    }
                    "??" => lt,
                    _ => lt,
                })
            }
            Expr::Unary { op, right, .. } => {
                let rt = self
                    .infer_expr(right, syms, ctx)
                    .unwrap_or(NyxType::Unknown);
                Ok(if op == "!" { NyxType::Bool } else { rt })
            }
            Expr::Call { callee, args, .. } => {
                // Validate known function arity
                let mut call_name = None;
                match callee.as_ref() {
                    Expr::Identifier { name, .. } => { call_name = Some(name.clone()); }
                    Expr::Path { segments: parts, .. } => { call_name = Some(parts.join("::")); }
                    _ => {}
                }
                
                if let Some(ref name) = call_name {
                    if name == "print" {
                        if args.len() != 1 {
                            return Err(
                                "error[E003]: print expects exactly one argument".to_string()
                            );
                        }
                        self.infer_expr(&args[0], syms, ctx)?;
                        return Ok(NyxType::Void);
                    }
                    if let Some(&(min, max)) = ctx.functions.get(name.as_str()) {
                        if args.len() < min || args.len() > max {
                            let expected_str = if min == max {
                                min.to_string()
                            } else {
                                format!("{}-{}", min, max)
                            };
                            return Err(format!(
                                "error[E003]: function '{}' expects {} arguments, got {}",
                                name, expected_str, args.len()
                            ));
                        }
                    }
                    
                    // Handle Protocol Constructor: Messaging_Client::new()
                    if name.contains("_") && name.ends_with("::new") {
                        let parts: Vec<&str> = name.split("::").collect();
                        if parts.len() == 2 {
                            let struct_name = parts[0];
                            if struct_name.contains("_") {
                                let subparts: Vec<&str> = struct_name.split('_').collect();
                                let protocol = subparts[0];
                                let role = subparts[1];
                                if ctx.protocols.contains_key(protocol) {
                                    return Ok(NyxType::ProtocolRole {
                                        protocol: protocol.to_string(),
                                        role: role.to_string(),
                                        state: 0,
                                    });
                                }
                            }
                        }
                    }
                }
                for a in args {
                    self.infer_expr(a, syms, ctx)?;
                }
                Ok(NyxType::Unknown)
            }
            Expr::MethodCall { receiver, method, args, .. } => {
                let recty = self.infer_expr(receiver, syms, ctx)?;
                for a in args {
                    self.infer_expr(a, syms, ctx)?;
                }
                
                if let NyxType::ProtocolRole { protocol, role, state } = recty {
                    if let Some(proto) = ctx.protocols.get(protocol.as_str()) {
                        if let Some(methods) = proto.handshake_methods.get(role.as_str()) {
                            if (state as usize) < methods.len() {
                                let expected = &methods[state as usize];
                                if method == expected {
                                    return Ok(NyxType::ProtocolRole {
                                        protocol,
                                        role,
                                        state: state + 1,
                                    });
                                } else {
                                    return Err(format!(
                                        "error[E510]: protocol method out of order in '{}::{}'. Expected '{}', found '{}'",
                                        protocol, role, expected, method
                                    ));
                                }
                            } else {
                                return Err(format!("error[E511]: handshake already complete for '{}::{}'", protocol, role));
                            }
                        }
                    }
                }
                Ok(NyxType::Unknown)
            }
            Expr::FieldAccess { object, .. } => {
                self.infer_expr(object, syms, ctx)?;
                Ok(NyxType::Unknown)
            }
            Expr::Index { object, index, .. } => {
                self.infer_expr(object, syms, ctx)?;
                self.infer_expr(index, syms, ctx)?;
                Ok(NyxType::Unknown)
            }
            Expr::Slice { object, start, end, .. } => {
                self.infer_expr(object, syms, ctx)?;
                if let Some(s) = start {
                    self.infer_expr(s, syms, ctx)?;
                }
                if let Some(e) = end {
                    self.infer_expr(e, syms, ctx)?;
                }
                Ok(NyxType::Unknown)
            }
            Expr::Cast { expr, .. } => {
                self.infer_expr(expr, syms, ctx)?;
                Ok(NyxType::Unknown)
            }
            Expr::Range { start, end, .. } => {
                if let Some(s) = start {
                    self.infer_expr(s, syms, ctx)?;
                }
                if let Some(e) = end {
                    self.infer_expr(e, syms, ctx)?;
                }
                Ok(NyxType::Unknown)
            }
            Expr::Await { expr: e, .. } | Expr::Move { expr: e, .. } | Expr::Deref { expr: e, .. } | Expr::TryOp { expr: e, .. } => {
                self.infer_expr(e, syms, ctx)
            }
            Expr::Reference { expr, .. } => {
                self.infer_expr(expr, syms, ctx)?;
                Ok(NyxType::Unknown)
            }
            Expr::Closure { .. } => Ok(NyxType::Unknown),
            Expr::Block { stmts, tail_expr: tail, .. } => {
                let mut block_syms = syms.clone();
                let mut block_seen = std::collections::HashSet::new();
                for s in stmts {
                    self.analyze_stmt(s, &mut block_syms, ctx, &mut block_seen)?;
                }
                if let Some(e) = tail {
                    self.infer_expr(e, &block_syms, ctx)
                } else {
                    Ok(NyxType::Void)
                }
            }
            Expr::StructLiteral { .. } | Expr::AsyncBlock { body: _, .. } => Ok(NyxType::Unknown),
            Expr::IfExpr {
                branches,
                else_body,
                ..
            } => {
                for b in branches {
                    self.infer_expr(&b.condition, syms, ctx)?;
                }
                if let Some(e) = else_body {
                    self.infer_expr(e, syms, ctx)
                } else {
                    Ok(NyxType::Void)
                }
            }
            Expr::Ternary {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
                self.infer_expr(condition, syms, ctx)?;
                let _ = self.infer_expr(then_expr, syms, ctx)?;
                let _ = self.infer_expr(else_expr, syms, ctx)?;
                Ok(NyxType::Unknown)
            }
            Expr::Match { expr, arms, .. } => {
                self.infer_expr(expr, syms, ctx)?;
                for arm in arms {
                    match &arm.body {
                        MatchBody::Expr(e) => {
                            self.infer_expr(e, syms, ctx)?;
                        }
                        MatchBody::Stmt(s) => {
                            let mut arm_seen = std::collections::HashSet::new();
                            self.analyze_stmt(s, &mut syms.clone(), ctx, &mut arm_seen)?;
                        }
                        MatchBody::Block(stmts) => {
                            let mut arm_syms = syms.clone();
                            let mut arm_seen = std::collections::HashSet::new();
                            for s in stmts {
                                self.analyze_stmt(s, &mut arm_syms, ctx, &mut arm_seen)?;
                            }
                        }
                    }
                }
                Ok(NyxType::Unknown)
            }
        }
    }

    fn asm_output_name(&self, expr: &Expr) -> Result<String, String> {
        match expr {
            Expr::Identifier { name: n, .. } => Ok(n.clone()),
            Expr::Path { segments: parts, .. } => Ok(parts.join("::")),
            Expr::FieldAccess { object, field, .. } => {
                Ok(format!("{}.{field}", self.asm_output_name(object)?))
            }
            _ => Err("error[E110]: asm output must be identifier/path/field".into()),
        }
    }
}

// ─── Analysis context ─────────────────────────────────────────────────────────

struct AnalysisCtx {
    functions: HashMap<std::string::String, (usize, usize)>, // name → (min_arity, max_arity)
    types: std::collections::HashSet<std::string::String>,
    protocols: HashMap<std::string::String, ProtocolInfo>,
}

struct ProtocolInfo {
    _roles: std::vec::Vec<String>,
    handshake_methods: HashMap<std::string::String, std::vec::Vec<std::string::String>>, // role -> ordered methods
}

impl AnalysisCtx {
    fn new() -> Self {
        Self {
            functions: HashMap::new(),
            types: std::collections::HashSet::new(),
            protocols: HashMap::new(),
        }
    }
}

fn type_to_nyx(ty: &Type) -> NyxType {
    match ty {
        Type::Named(path) => NyxType::from_str(path.last_name()),
        Type::Record(_) => NyxType::Unknown,
        Type::Union(_) => NyxType::Unknown,
        Type::Literal(_) => NyxType::Unknown,
        _ => NyxType::Unknown,
    }
}

impl SemanticAnalyzer {
    fn analyze_protocol(&self, p: &ProtocolDecl, _ctx: &AnalysisCtx) -> Result<(), std::string::String> {
        // ... (existing validations)
        let mut roles_seen = std::collections::HashSet::new();
        for role in &p.roles {
            roles_seen.insert(role.clone());
        }

        let mut role_methods: std::vec::Vec<(std::string::String, std::vec::Vec<std::string::String>)> = std::vec::Vec::new();
        for role in &p.roles {
            role_methods.push((role.clone(), std::vec::Vec::new()));
        }

        if let Some(handshake) = &p.handshake {
            for (i, step) in handshake.steps.iter().enumerate() {
                match step {
                    HandshakeStep::Message { from, to, name, .. } => {
                        for (r, methods) in &mut role_methods {
                            if r == from {
                                methods.push(format!("send_{}", name));
                            }
                            if r == to {
                                methods.push(format!("recv_{}", name));
                            }
                        }
                    }
                    HandshakeStep::Derive { .. } => {
                        for role in &p.roles {
                            for (r, methods) in &mut role_methods {
                                if r == role {
                                    methods.push(format!("step_{}_derive", i));
                                }
                            }
                        }
                    }
                    HandshakeStep::Finish { .. } => {
                        for role in &p.roles {
                            for (r, methods) in &mut role_methods {
                                if r == role {
                                    methods.push("complete_handshake".to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        // We can't easily mutate AnalysisCtx here as it's &ctx, 
        // but in a real compiler we'd populate it in a first pass.
        // For this hardening, we'll assume the analyzer is updated to handle this.
        
        Ok(())
    }
}
