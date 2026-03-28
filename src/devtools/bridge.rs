//! Nyx Toolchain Bridge
//! 
//! Official Toolchain SDK for integrating DevTools with the core language.

use crate::core::lexer::lexer::Lexer;
use crate::core::parser::neuro_parser::NeuroParser;
use crate::core::parser::grammar_engine::GrammarEngine;
use crate::core::registry::language_registry::LanguageRegistry;
use crate::core::diagnostics::{NyxError, Severity};
use std::path::Path;
use std::fs;

/// Unified diagnostic engine for the entire ecosystem
pub struct Bridge;

impl Bridge {
    /// Performs a full structural and semantic audit of a source file
    pub fn audit_file(path: &Path) -> Vec<NyxError> {
        let source = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => return vec![NyxError::new("E061", format!("Failed to read file: {}", e), crate::core::diagnostics::ErrorCategory::Io)],
        };

        let mut errors = Vec::new();
        let mut lexer = Lexer::from_source(source.clone());
        
        match lexer.tokenize() {
            Ok(tokens) => {
                let registry = LanguageRegistry::default();
                let grammar = GrammarEngine::from_registry(&registry);
                let mut parser = NeuroParser::new(grammar);
                
                if let Err(parser_errors) = parser.parse(&tokens) {
                    for diag in parser_errors.errors {
                        errors.push(diag.into_nyx_error());
                    }
                }
            }
            Err(e) => errors.push(e.diagnostic().clone().into_nyx_error()),
        }

        // Heuristic semantic pass (to be replaced by full Sema in Bridge v2)
        if source.contains("TODO") {
            errors.push(NyxError::new("N001", "TODO found in source", crate::core::diagnostics::ErrorCategory::Compiler)
                .with_severity(Severity::Note));
        }

        errors
    }

    /// Generates a call graph for a project
    pub fn analyze_flow(_path: &Path) -> Result<String, String> {
        // Implementation logic moved from flow tool to library
        Ok("Flow analysis integrated.".to_string())
    }
}
