//! AST Builder — thin facade over direct node construction.

use crate::core::ast::ast_nodes::*;
use crate::core::lexer::token::Span;

pub struct AstBuilder;

impl Default for AstBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl AstBuilder {
    pub fn new() -> Self {
        AstBuilder
    }

    pub fn program(items: Vec<Item>) -> Program {
        Program { items }
    }

    pub fn item(vis: Visibility, kind: ItemKind, span: Span) -> Item {
        Item { attributes: Vec::new(), vis, kind, span }
    }

    pub fn function_item(vis: Visibility, decl: FunctionDecl) -> Item {
        let span = decl.span;
        Item {
            attributes: Vec::new(),
            vis,
            kind: ItemKind::Function(decl),
            span,
        }
    }
}

fn _decl_span(kind: &ItemKind) -> Span {
    match kind {
        ItemKind::Function(f) => f.span,
        ItemKind::Struct(s) => s.span,
        ItemKind::Enum(e) => e.span,
        ItemKind::Trait(t) => t.span,
        ItemKind::Impl(i) => i.span,
        _ => Span::default(),
    }
}
