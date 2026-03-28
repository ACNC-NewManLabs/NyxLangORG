use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxPatterns {
    pub function: String,
    pub let_stmt: String,
    pub return_stmt: String,
    pub defer_stmt: String,
    pub call_expr: String,
}

#[derive(Debug, Clone)]
pub struct SyntaxRules {
    pub patterns: SyntaxPatterns,
}

impl SyntaxRules {
    pub fn new(patterns: SyntaxPatterns) -> Self {
        Self { patterns }
    }
}
