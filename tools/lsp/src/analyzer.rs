//! Nyx Source Code Analyzer for LSP

use lsp_types::{Hover, HoverContents, Location, MarkedString, Position, Range, Url};
use std::collections::HashMap;

use crate::index::{GlobalIndex, Symbol, SymbolKind as LspSymbolKind};
use nyx::core::ast::ast_nodes::{Expr, ItemKind, Stmt};
use nyx::core::lexer::lexer::Lexer;
use nyx::core::lexer::token::{Token, TokenKind};
use nyx::core::parser::grammar_engine::GrammarEngine;
use nyx::core::parser::neuro_parser::NeuroParser;
use nyx::core::registry::language_registry::LanguageRegistry;

/// Document analyzer for Nyx source files
pub struct DocumentAnalyzer {
    uri: Url,
    pub source: String,
    tokens: Vec<Token>,
    functions: HashMap<String, Range>,
    variables: HashMap<String, Range>,
    types: HashMap<String, Range>,
    /// Maps struct name to its fields (name, type)
    pub struct_fields: HashMap<String, Vec<(String, String)>>,
    /// Maps variable name to its type name (if known)
    pub variable_types: HashMap<String, String>,
    ast: Option<nyx::core::ast::ast_nodes::Program>,
    diagnostics: Vec<lsp_types::Diagnostic>,
}

impl DocumentAnalyzer {
    /// Create a new analyzer for the given source
    pub fn new(uri: Url, source: &str) -> Self {
        let source_str = source.to_string();

        let mut lexer = Lexer::from_source(source_str.clone());
        let tokens = lexer.tokenize().unwrap_or_default();

        let registry = LanguageRegistry::default();
        let grammar = GrammarEngine::from_registry(&registry);
        let mut parser = NeuroParser::new(grammar);

        let (functions, variables, types, struct_fields, variable_types, ast_cached) = match parser
            .parse(&tokens)
        {
            Ok(ast) => {
                let mut funcs = HashMap::new();
                let mut vars = HashMap::new();
                let mut typs = HashMap::new();
                let mut s_fields = HashMap::new();
                let mut v_types = HashMap::new();

                for item in &ast.items {
                    match &item.kind {
                        ItemKind::Function(func) => {
                            let r = Range {
                                start: Position {
                                    line: item.span.start.line as u32 - 1,
                                    character: item.span.start.column as u32 - 1,
                                },
                                end: Position {
                                    line: item.span.end.line as u32 - 1,
                                    character: item.span.end.column as u32 - 1,
                                },
                            };
                            funcs.insert(func.name.clone(), r);

                            // extract local vars and types
                            extract_vars_and_types(&func.body, &mut vars, &mut v_types);
                        }
                        ItemKind::Struct(strct) => {
                            let r = Range {
                                start: Position {
                                    line: item.span.start.line as u32 - 1,
                                    character: item.span.start.column as u32 - 1,
                                },
                                end: Position {
                                    line: item.span.end.line as u32 - 1,
                                    character: item.span.end.column as u32 - 1,
                                },
                            };
                            typs.insert(strct.name.clone(), r);

                            let mut fields = Vec::new();
                            for f in &strct.fields {
                                if let nyx::core::ast::ast_nodes::Type::Named(path) = &f.field_type
                                {
                                    fields.push((f.name.clone(), path.last_name().to_string()));
                                }
                            }
                            s_fields.insert(strct.name.clone(), fields);
                        }
                        ItemKind::Enum(enm) => {
                            let r = Range {
                                start: Position {
                                    line: item.span.start.line as u32 - 1,
                                    character: item.span.start.column as u32 - 1,
                                },
                                end: Position {
                                    line: item.span.end.line as u32 - 1,
                                    character: item.span.end.column as u32 - 1,
                                },
                            };
                            typs.insert(enm.name.clone(), r);
                        }
                        ItemKind::Trait(trt) => {
                            let r = Range {
                                start: Position {
                                    line: item.span.start.line as u32 - 1,
                                    character: item.span.start.column as u32 - 1,
                                },
                                end: Position {
                                    line: item.span.end.line as u32 - 1,
                                    character: item.span.end.column as u32 - 1,
                                },
                            };
                            typs.insert(trt.name.clone(), r);
                        }
                        _ => {}
                    }
                }

                (funcs, vars, typs, s_fields, v_types, Some(ast))
            }
            Err(_) => (
                HashMap::new(),
                HashMap::new(),
                HashMap::new(),
                HashMap::new(),
                HashMap::new(),
                None,
            ),
        };

        // Very basic error collection using lexer fails or parser errors (would use parser.get_errors() but not exposed so leaving diagnostics empty here, semantic errors will be handled in full compiler).
        let diagnostics = Vec::new();

        Self {
            uri,
            source: source_str,
            tokens,
            functions,
            variables,
            types,
            struct_fields,
            variable_types,
            ast: ast_cached,
            diagnostics,
        }
    }

    /// Extract symbols for global indexing
    pub fn extract_symbols(&self) -> Vec<Symbol> {
        let mut symbols = Vec::new();

        if let Some(ast) = &self.ast {
            for item in &ast.items {
                let (name, kind, fields) = match &item.kind {
                    ItemKind::Function(f) => (f.name.clone(), LspSymbolKind::FUNCTION, None),
                    ItemKind::Struct(s) => {
                        let mut flds = Vec::new();
                        for f in &s.fields {
                            if let nyx::core::ast::ast_nodes::Type::Named(path) = &f.field_type {
                                flds.push((f.name.clone(), path.last_name().to_string()));
                            }
                        }
                        (s.name.clone(), LspSymbolKind::STRUCT, Some(flds))
                    }
                    ItemKind::Enum(e) => (e.name.clone(), LspSymbolKind::ENUM, None),
                    ItemKind::Trait(t) => (t.name.clone(), LspSymbolKind::INTERFACE, None),
                    _ => continue,
                };

                symbols.push(Symbol {
                    name,
                    kind,
                    location: Location {
                        uri: self.uri.clone(),
                        range: Range {
                            start: Position {
                                line: item.span.start.line as u32 - 1,
                                character: item.span.start.column as u32 - 1,
                            },
                            end: Position {
                                line: item.span.end.line as u32 - 1,
                                character: item.span.end.column as u32 - 1,
                            },
                        },
                    },
                    description: None,
                    fields,
                });
            }
        }
        symbols
    }

    /// Update the document content
    pub fn update(&mut self, new_source: &str) {
        let uri = self.uri.clone();
        *self = Self::new(uri, new_source);
    }

    /// Get the current source
    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    /// Get all diagnostics for this document
    pub fn get_diagnostics(&self) -> &[lsp_types::Diagnostic] {
        &self.diagnostics
    }

    /// Find definition at position
    pub fn find_definition(&self, position: Position, index: &GlobalIndex) -> Option<Location> {
        let line_idx = position.line as usize;
        let lines: Vec<&str> = self.source.lines().collect();
        if line_idx >= lines.len() {
            return None;
        }

        let line = lines[line_idx];
        let line_str = line.to_string();

        // Extract identifier at position
        if let Some(identifier) = self.get_identifier_at(&line_str, position.character as usize) {
            // Check functions
            if let Some(range) = self.functions.get(&identifier) {
                return Some(Location {
                    uri: self.uri.clone(),
                    range: *range,
                });
            }
            // Check variables
            if let Some(range) = self.variables.get(&identifier) {
                return Some(Location {
                    uri: self.uri.clone(),
                    range: *range,
                });
            }
            // Check types
            if let Some(range) = self.types.get(&identifier) {
                return Some(Location {
                    uri: self.uri.clone(),
                    range: *range,
                });
            }

            // Check global index for cross-file definition
            let globals = index.find(&identifier);
            if let Some(symbol) = globals.first() {
                return Some(symbol.location.clone());
            }
        }

        None
    }

    /// Find all references to identifier at position
    pub fn find_references(&self, position: Position, _index: &GlobalIndex) -> Vec<Location> {
        let mut references = Vec::new();

        let line_idx = position.line as usize;
        let lines: Vec<&str> = self.source.lines().collect();
        if line_idx >= lines.len() {
            return references;
        }

        let line = lines[line_idx];
        let line_str = line.to_string();

        if let Some(identifier) = self.get_identifier_at(&line_str, position.character as usize) {
            // Very naive exact scan through tokens
            for tok in &self.tokens {
                if tok.lexeme == identifier {
                    references.push(Location {
                        uri: self.uri.clone(),
                        range: Range {
                            start: Position {
                                line: tok.span.start.line as u32 - 1,
                                character: tok.span.start.column as u32 - 1,
                            },
                            end: Position {
                                line: tok.span.end.line as u32 - 1,
                                character: tok.span.end.column as u32 - 1,
                            },
                        },
                    });
                }
            }
        }

        references
    }

    /// Get member suggestions after a dot
    pub fn get_members_at(&self, position: Position, index: &GlobalIndex) -> Vec<String> {
        let line_idx = position.line as usize;
        let lines: Vec<&str> = self.source.lines().collect();
        if line_idx >= lines.len() {
            return Vec::new();
        }

        let line = lines[line_idx];
        let col = position.character as usize;

        // Find if we are after a dot (e.g. "obj.|" or "obj.f|")
        let mut dot_pos = None;
        for i in (0..col).rev() {
            let ch = line.as_bytes().get(i).copied().unwrap_or(0);
            if ch == b'.' {
                dot_pos = Some(i);
                break;
            }
            if !ch.is_ascii_alphanumeric() && ch != b'_' {
                break;
            }
        }

        let Some(dot_idx) = dot_pos else {
            return Vec::new();
        };

        // Receiver is the identifier before the dot
        if let Some(receiver) =
            self.get_identifier_at(line, if dot_idx > 0 { dot_idx - 1 } else { 0 })
        {
            // 1. Check local variables for type
            if let Some(type_name) = self.variable_types.get(&receiver) {
                return self.get_fields_of_type(type_name, index);
            }

            // 2. Check if the receiver itself is a known type (Static access)
            if self.types.contains_key(&receiver) {
                return self.get_fields_of_type(&receiver, index);
            }

            // 3. Check GlobalIndex for the receiver's type
            if !index.find(&receiver).is_empty() {
                return self.get_fields_of_type(&receiver, index);
            }
        }

        Vec::new()
    }

    fn get_fields_of_type(&self, type_name: &str, index: &GlobalIndex) -> Vec<String> {
        let mut members = Vec::new();
        // Check local struct definitions
        if let Some(fields) = self.struct_fields.get(type_name) {
            for (f_name, _) in fields {
                members.push(f_name.clone());
            }
        } else {
            // Check GlobalIndex for cross-file struct definitions
            for symbol in index.find(type_name) {
                if let Some(fields) = &symbol.fields {
                    for (f_name, _) in fields {
                        if !members.contains(f_name) {
                            members.push(f_name.clone());
                        }
                    }
                }
            }
        }
        members
    }

    /// Get hover information at position
    pub fn get_hover(&self, position: Position, index: &GlobalIndex) -> Option<Hover> {
        let line_idx = position.line as usize;
        let lines: Vec<&str> = self.source.lines().collect();
        if line_idx >= lines.len() {
            return None;
        }

        let line = lines[line_idx];
        let line_str = line.to_string();

        if let Some(identifier) = self.get_identifier_at(&line_str, position.character as usize) {
            let mut contents = Vec::new();

            if self.functions.contains_key(&identifier) {
                contents.push(MarkedString::String(format!("function `{}`", identifier)));
            }
            if self.variables.contains_key(&identifier) {
                contents.push(MarkedString::String(format!("variable `{}`", identifier)));
            }
            if self.types.contains_key(&identifier) {
                contents.push(MarkedString::String(format!("type `{}`", identifier)));
            }

            // Check global index for hover info
            for symbol in index.find(&identifier) {
                let kind_str = match symbol.kind {
                    LspSymbolKind::FUNCTION => "function",
                    LspSymbolKind::STRUCT => "struct",
                    LspSymbolKind::ENUM => "enum",
                    LspSymbolKind::INTERFACE => "trait",
                    _ => "symbol",
                };
                contents.push(MarkedString::String(format!(
                    "{} `{}` (global)",
                    kind_str, identifier
                )));
                if let Some(desc) = &symbol.description {
                    contents.push(MarkedString::String(desc.clone()));
                }
            }

            if !contents.is_empty() {
                return Some(Hover {
                    contents: HoverContents::Array(contents),
                    range: None,
                });
            }
        }

        None
    }

    /// Get code actions at range
    pub fn get_code_actions(&self, _range: Range) -> Vec<lsp_types::CodeAction> {
        Vec::new() // Not implemented yet without proper semantic analysis
    }

    /// Get semantic tokens for syntax highlighting
    pub fn get_semantic_tokens(&self) -> lsp_types::SemanticTokens {
        let mut semantic_tokens: Vec<lsp_types::SemanticToken> = Vec::new();

        let mut last_line: u32 = 0;
        let mut last_col: u32 = 0;
        for tok in self.tokens() {
            let token_type = match tok.kind {
                TokenKind::Identifier => Some(2),
                TokenKind::String => Some(4),
                TokenKind::Integer | TokenKind::Float => Some(5),
                TokenKind::Comment | TokenKind::MultiLineComment => Some(7),
                _ if tok.kind.is_keyword() => Some(0),
                _ => None, // operators/punct not needed for this basic test
            };
            let Some(tt) = token_type else { continue };

            let line = tok.span.start.line as u32 - 1; // 1 to 0 index
            let col = tok.span.start.column as u32 - 1; // 1 to 0 index

            let (delta_line, delta_start) = if semantic_tokens.is_empty() {
                (line, col)
            } else if line == last_line {
                (0, col.saturating_sub(last_col))
            } else {
                (line.saturating_sub(last_line), col)
            };

            semantic_tokens.push(lsp_types::SemanticToken {
                delta_line,
                delta_start,
                length: tok.lexeme.len() as u32,
                token_type: tt,
                token_modifiers_bitset: 0,
            });
            last_line = line;
            last_col = col;
        }

        lsp_types::SemanticTokens {
            data: semantic_tokens,
            ..Default::default()
        }
    }

    /// Extract identifier at position
    fn get_identifier_at(&self, line: &str, col: usize) -> Option<String> {
        let mut start = col;
        let mut end = col;

        let chars: Vec<char> = line.chars().collect();
        if start >= chars.len() {
            return None;
        }

        // Find start of word
        while start > 0 && (chars[start].is_alphanumeric() || chars[start] == '_') {
            start -= 1;
        }
        if start < chars.len() && !chars[start].is_alphanumeric() && chars[start] != '_' {
            start += 1;
        }

        // Find end of word
        while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
            end += 1;
        }

        if end > start {
            Some(chars[start..end].iter().collect())
        } else {
            None
        }
    }
}

fn extract_vars_and_types(
    stmts: &[Stmt],
    vars: &mut HashMap<String, Range>,
    v_types: &mut HashMap<String, String>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::Let {
                name,
                var_type,
                span,
                ..
            } => {
                vars.insert(
                    name.clone(),
                    Range {
                        start: Position {
                            line: span.start.line as u32 - 1,
                            character: span.start.column as u32 - 1,
                        },
                        end: Position {
                            line: span.end.line as u32 - 1,
                            character: span.end.column as u32 - 1,
                        },
                    },
                );

                if let Some(vt) = var_type {
                    if let nyx::core::ast::ast_nodes::Type::Named(path) = vt {
                        v_types.insert(name.clone(), path.last_name().to_string());
                    }
                }
            }
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for branch in branches {
                    extract_vars_and_types(&branch.body, vars, v_types);
                }
                if let Some(eb) = else_body {
                    extract_vars_and_types(eb, vars, v_types);
                }
            }
            Stmt::While { body, .. } | Stmt::Loop { body, .. } => {
                extract_vars_and_types(body, vars, v_types)
            }
            Stmt::Expr(Expr::Block { stmts: body, .. }) => {
                extract_vars_and_types(body, vars, v_types)
            }
            _ => {}
        }
    }
}
