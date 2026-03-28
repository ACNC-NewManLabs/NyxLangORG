use std::collections::HashMap;
// use crate::core::ast::ast_nodes::*;
// use crate::core::lexer::token::Span;

// ─── Type system ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum NyxType {
    // Primitives
    I8,
    I16,
    I32,
    I64,
    I128,
    U8,
    U16,
    U32,
    U64,
    U128,
    F32,
    F64,
    Bool,
    Char,
    Str,
    String,
    Unit,
    Void,
    // Named (user-defined or external)
    Named(String),
    // Generics (simplified)
    Generic(String, Vec<NyxType>),
    // Protocol-aware types
    ProtocolRole {
        protocol: String,
        role: String,
        state: i32,
    },
    // Unknown — permissive mode for cross-module refs
    Unknown,
}

impl NyxType {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "i8" => NyxType::I8,
            "i16" => NyxType::I16,
            "i32" | "int" => NyxType::I32,
            "i64" => NyxType::I64,
            "i128" => NyxType::I128,
            "u8" => NyxType::U8,
            "u16" => NyxType::U16,
            "u32" => NyxType::U32,
            "u64" => NyxType::U64,
            "u128" => NyxType::U128,
            "f32" => NyxType::F32,
            "f64" | "float" => NyxType::F64,
            "bool" => NyxType::Bool,
            "char" => NyxType::Char,
            "str" => NyxType::Str,
            "string" | "String" => NyxType::String,
            "()" | "void" => NyxType::Void,
            _ => NyxType::Named(s.to_string()),
        }
    }
}

// ─── Primitive type (old compat shim) ────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum PrimitiveType {
    Int,
    Float,
    Bool,
    String,
    Char,
    Void,
    Unknown,
}

// ─── Symbol table ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct SymbolTable {
    scopes: Vec<HashMap<String, NyxType>>,
}

impl SymbolTable {
    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }
    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    pub fn define(&mut self, name: String, ty: NyxType) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, ty);
        }
    }

    pub fn lookup(&self, name: &str) -> Option<&NyxType> {
        for scope in self.scopes.iter().rev() {
            if let Some(t) = scope.get(name) {
                return Some(t);
            }
        }
        None
    }

    pub fn is_defined(&self, name: &str) -> bool {
        self.lookup(name).is_some()
    }
}
