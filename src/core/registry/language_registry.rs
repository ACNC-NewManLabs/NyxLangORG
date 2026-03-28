use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::core::parser::syntax_rules::SyntaxPatterns;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageRegistry {
    pub language: String,
    pub version: String,
    pub keywords: Vec<String>,
    pub types: Vec<String>,
    pub operators: Vec<String>,
    pub syntax_patterns: SyntaxPatterns,
    pub grammar_structures: Vec<String>,
}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self {
            language: "Nyx".to_string(),
            version: "1.0".to_string(),
            keywords: vec!["fn", "let", "return", "module", "defer"]
                .into_iter()
                .map(String::from)
                .collect(),
            types: vec!["int", "float", "bool", "string"]
                .into_iter()
                .map(String::from)
                .collect(),
            operators: vec!["+", "-", "*", "/", "="]
                .into_iter()
                .map(String::from)
                .collect(),
            syntax_patterns: SyntaxPatterns {
                function: "fn <ident>() { <stmt>* }".to_string(),
                let_stmt: "let <ident> = <expr>".to_string(),
                return_stmt: "return <expr>?".to_string(),
                defer_stmt: "defer <stmt>".to_string(),
                call_expr: "<ident>(<expr>*)".to_string(),
            },
            grammar_structures: vec!["Program", "FunctionDecl", "Stmt", "Expr"]
                .into_iter()
                .map(String::from)
                .collect(),
        }
    }
}

impl LanguageRegistry {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
        Self::load_from_str(&text)
    }

    pub fn load_from_str(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|e| e.to_string())
    }
}
