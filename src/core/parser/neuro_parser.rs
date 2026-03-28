//! Nyx Language Parser — deterministic recursive descent.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::core::ast::ast_nodes::*;
use crate::core::diagnostics::{codes, Diagnostic};
use crate::core::lexer::token::{Span, Token, TokenKind};
use crate::core::parser::grammar_engine::GrammarEngine;

// ─── Backward-compat Program wrapper ─────────────────────────────────────────
// Old code accessed program.functions / program.structs directly.
// We expose those via the helpers defined on Program in ast_nodes.

/// Incremental parsing cache.
#[derive(Debug, Clone, Default)]
pub struct IncrementalState {
    last_hash: u64,
    last_ast: Option<Program>,
}

#[derive(Debug, Clone)]
pub struct ParserErrors {
    pub errors: Vec<Diagnostic>,
}

impl ParserErrors {
    fn new(errors: Vec<Diagnostic>) -> Self {
        Self { errors }
    }
}

impl std::fmt::Display for ParserErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, err) in self.errors.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            writeln!(f, "{err}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ParserErrors {}

// ─── Parser ───────────────────────────────────────────────────────────────────

pub struct NeuroParser {
    #[allow(dead_code)]
    grammar: GrammarEngine,
    pos: usize,
    tokens: Vec<Token>,
    errors: Vec<Diagnostic>,
    disallow_struct_literal: bool,
}

impl std::fmt::Debug for NeuroParser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NeuroParser")
            .field("pos", &self.pos)
            .finish()
    }
}

impl NeuroParser {
    pub fn new(grammar: GrammarEngine) -> Self {
        Self {
            grammar,
            pos: 0,
            tokens: vec![],
            errors: vec![],
            disallow_struct_literal: false,
        }
    }

    pub fn grammar_size(&self) -> usize {
        0
    }

    // ── Public entry points ──────────────────────────────────────────────────

    pub fn parse(&mut self, tokens: &[Token]) -> Result<Program, ParserErrors> {
        self.tokens = tokens.to_vec();
        self.pos = 0;
        self.errors.clear();
        let program = match self.parse_program() {
            Ok(p) => p,
            Err(e) => {
                self.errors.push(e);
                return Err(ParserErrors::new(self.errors.clone()));
            }
        };
        if self.errors.is_empty() {
            Ok(program)
        } else {
            Err(ParserErrors::new(self.errors.clone()))
        }
    }

    pub fn parse_incremental(
        &mut self,
        state: &mut IncrementalState,
        source: &str,
        tokens: &[Token],
    ) -> Result<Program, ParserErrors> {
        let mut h = DefaultHasher::new();
        source.hash(&mut h);
        let hash = h.finish();
        if hash == state.last_hash {
            if let Some(ast) = &state.last_ast {
                return Ok(ast.clone());
            }
        }
        let ast = self.parse(tokens)?;
        state.last_hash = hash;
        state.last_ast = Some(ast.clone());
        Ok(ast)
    }

    // ── Token helpers ────────────────────────────────────────────────────────

    fn cur(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }
    fn is_eof(&self) -> bool {
        self.cur().kind == TokenKind::Eof
    }
    fn at(&self, k: &TokenKind) -> bool {
        &self.cur().kind == k
    }

    fn at_kw(&self, kw: &str) -> bool {
        let t = self.cur();
        t.lexeme == kw && t.kind.is_keyword()
    }

    fn at_contextual_kw(&self, kw: &str) -> bool {
        let t = self.cur();
        t.lexeme == kw && (t.kind == TokenKind::Identifier || t.kind.is_keyword())
    }

    fn eat_contextual_kw(&mut self, kw: &str) -> bool {
        if self.at_contextual_kw(kw) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn _peek_kind(&self) -> &TokenKind {
        &self.cur().kind
    }

    fn advance(&mut self) -> &Token {
        let t = &self.tokens[self.pos.min(self.tokens.len() - 1)];
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn expect(&mut self, k: TokenKind) -> Result<String, Diagnostic> {
        if self.cur().kind == k {
            Ok(self.advance().lexeme.clone())
        } else {
            Err(self.missing_token(&k.display()))
        }
    }

    fn expect_ident(&mut self) -> Result<String, Diagnostic> {
        if matches!(
            self.cur().kind,
            TokenKind::Identifier | TokenKind::KwSelf | TokenKind::Null
        ) {
            Ok(self.advance().lexeme.clone())
        } else if self.cur().kind.is_keyword() {
            // allow keyword identifiers in some positions (e.g. field names)
            Ok(self.advance().lexeme.clone())
        } else {
            Err(
                self.error_here(
                    codes::PARSER_UNEXPECTED_TOKEN,
                    format!("expected identifier, found '{}'", self.cur().lexeme),
                )
                .with_suggestion("use a valid identifier name"),
            )
        }
    }

    fn is_expr_start_kind(&self, kind: &TokenKind) -> bool {
        matches!(
            kind,
            TokenKind::Identifier
                | TokenKind::Integer
                | TokenKind::Float
                | TokenKind::String
                | TokenKind::Boolean
                | TokenKind::Char
                | TokenKind::Null
                | TokenKind::KwNull
                | TokenKind::KwSelf
                | TokenKind::KwSelfType
                | TokenKind::LParen
                | TokenKind::LBracket
                | TokenKind::LBrace
                | TokenKind::KwIf
                | TokenKind::KwMatch
                | TokenKind::KwAsync
                | TokenKind::KwFn
                | TokenKind::KwLoop
                | TokenKind::KwAwait
                | TokenKind::KwStatic
                | TokenKind::KwMove
                | TokenKind::Bang
                | TokenKind::KwNot
                | TokenKind::Minus
                | TokenKind::Tilde
                | TokenKind::Star
                | TokenKind::Ampersand
        )
    }

    fn parse_control_expr(&mut self) -> Result<Expr, Diagnostic> {
        let prev = self.disallow_struct_literal;
        self.disallow_struct_literal = true;
        let expr = self.parse_expr_with_stop(true)?;
        self.disallow_struct_literal = prev;
        Ok(expr)
    }

    fn parse_field_name(&mut self) -> Result<String, Diagnostic> {
        if self.at(&TokenKind::String)
            || self.at(&TokenKind::Integer)
            || self.at(&TokenKind::Float)
        {
            return Ok(self.advance().lexeme.clone());
        }
        self.expect_ident()
    }

    fn skip_newlines(&mut self) {
        while matches!(
            self.cur().kind,
            TokenKind::Newline
                | TokenKind::Semicolon
                | TokenKind::Comment
                | TokenKind::MultiLineComment
                | TokenKind::Whitespace
        ) && !self.is_eof()
        {
            self.pos += 1;
        }
    }

    fn span_at(&self) -> Span {
        self.cur().span
    }

    fn eat_kw(&mut self, kw: &str) -> bool {
        if self.at_kw(kw) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn record_error(&mut self, err: Diagnostic) {
        self.errors.push(err);
    }

    fn error_at(&self, code: &'static str, span: Span, msg: impl Into<String>) -> Diagnostic {
        Diagnostic::error(code, msg).with_span(span)
    }

    fn error_here(&self, code: &'static str, msg: impl Into<String>) -> Diagnostic {
        self.error_at(code, self.span_at(), msg)
    }

    fn missing_token(&self, expected: &str) -> Diagnostic {
        if self.is_eof() {
            self.error_here(
                codes::PARSER_UNEXPECTED_EOF,
                format!("expected {expected}, found end of file"),
            )
            .with_suggestion(format!("insert {expected} before end of file"))
        } else {
            self.error_here(
                codes::PARSER_MISSING_TOKEN,
                format!("expected {expected}, found '{}'", self.cur().lexeme),
            )
            .with_suggestion(format!("insert {expected} here"))
        }
    }

    // ── Visibility ───────────────────────────────────────────────────────────

    fn parse_visibility(&mut self) -> Visibility {
        if !self.at_kw("pub") {
            return Visibility::Inherited;
        }
        self.advance();
        if self.at(&TokenKind::LParen) {
            self.advance();
            let s = self.cur().lexeme.clone();
            self.advance();
            let _ = self.expect(TokenKind::RParen);
            return match s.as_str() {
                "crate" => Visibility::PublicCrate,
                "super" => Visibility::PublicSuper,
                _ => Visibility::Public,
            };
        }
        Visibility::Public
    }

    // ── Program ──────────────────────────────────────────────────────────────

    fn parse_program(&mut self) -> Result<Program, Diagnostic> {
        let mut items = Vec::new();
        loop {
            self.skip_newlines();
            if self.is_eof() {
                break;
            }
            match self.parse_item() {
                Ok(item) => items.push(item),
                Err(e) => {
                    self.record_error(e);
                    self.recover_to_item_boundary();
                }
            }
        }
        Ok(Program { items })
    }

    fn recover_to_item_boundary(&mut self) {
        // skip tokens until we see something that can start a top-level item
        while !self.is_eof() {
            match self.cur().kind {
                TokenKind::KwFn
                | TokenKind::KwStruct
                | TokenKind::KwUnion
                | TokenKind::KwEnum
                | TokenKind::KwTrait
                | TokenKind::KwImpl
                | TokenKind::KwMod
                | TokenKind::KwUse
                | TokenKind::KwExport
                | TokenKind::KwPub
                | TokenKind::KwConst
                | TokenKind::KwStatic
                | TokenKind::KwType => break,
                _ => {
                    self.advance();
                }
            }
        }
    }

    // ── Attributes ───────────────────────────────────────────────────────────

    fn parse_attributes(&mut self) -> Result<Vec<Attribute>, Diagnostic> {
        let mut attrs = Vec::new();
        while self.at(&TokenKind::At) {
            let start = self.span_at();
            self.advance(); // consume @

            let name = self.expect_ident()?;
            let mut args = None;

            if self.at(&TokenKind::LParen) {
                self.advance();
                let mut arg_str = String::new();
                while !self.at(&TokenKind::RParen) && !self.is_eof() {
                    arg_str.push_str(&self.advance().lexeme);
                }
                self.expect(TokenKind::RParen)?;
                args = Some(arg_str);
            }

            attrs.push(Attribute {
                name,
                args,
                span: Span::new(start.start, self.span_at().start),
            });
            self.skip_newlines();
        }
        Ok(attrs)
    }

    // ── Items ────────────────────────────────────────────────────────────────

    fn parse_item(&mut self) -> Result<Item, Diagnostic> {
        self.skip_newlines();
        let start = self.span_at();
        let attributes = self.parse_attributes()?;
        let vis = self.parse_visibility();
        self.skip_newlines();

        let kind = match &self.cur().kind {
            TokenKind::KwFn | TokenKind::KwAsync | TokenKind::KwExtern => {
                ItemKind::Function(self.parse_fn_decl()?)
            }
            TokenKind::Identifier if self.cur().lexeme == "config" => {
                self.advance(); // config
                ItemKind::Struct(self.parse_record_decl(start)?)
            }
            TokenKind::KwStruct => ItemKind::Struct(self.parse_struct()?),
            TokenKind::KwUnion => ItemKind::Struct(self.parse_union()?),
            TokenKind::KwEnum => ItemKind::Enum(self.parse_enum()?),
            TokenKind::KwTrait => ItemKind::Trait(self.parse_trait()?),
            TokenKind::KwImpl => ItemKind::Impl(self.parse_impl()?),
            TokenKind::KwType => ItemKind::TypeAlias(self.parse_type_alias()?),
            TokenKind::KwConst => ItemKind::Const(self.parse_const()?),
            TokenKind::KwStatic | TokenKind::KwLet => {
                // Map 'let' at top level to Static for now
                if self.at(&TokenKind::KwLet) {
                    self.advance();
                } else {
                    self.expect(TokenKind::KwStatic)?;
                }
                ItemKind::Static(self.parse_static_after_kw(start)?)
            }
            TokenKind::KwMod => self.parse_mod_item()?,
            TokenKind::KwUse => ItemKind::Use(self.parse_use()?),
            TokenKind::Identifier if self.cur().lexeme == "import" => {
                ItemKind::Use(self.parse_use()?)
            }
            TokenKind::KwExport => ItemKind::Export(self.parse_export()?),
            TokenKind::KwSecure => ItemKind::Protocol(self.parse_protocol_decl()?),
            _ => {
                return Err(
                    self.error_here(
                        codes::PARSER_UNEXPECTED_TOKEN,
                        format!(
                            "unexpected token '{}' at top level",
                            self.cur().lexeme
                        ),
                    )
                    .with_suggestion("remove this token or start a valid item"),
                )
            }
        };

        Ok(Item {
            attributes,
            vis,
            kind,
            span: Span::new(start.start, self.cur().span.start),
        })
    }

    fn parse_fn_decl(&mut self) -> Result<FunctionDecl, Diagnostic> {
        let start = self.span_at();
        let is_extern = self.eat_kw("extern");
        let extern_abi = if is_extern && self.at(&TokenKind::String) {
            Some(self.advance().lexeme.clone())
        } else {
            None
        };
        let is_async = self.eat_kw("async");
        self.expect(TokenKind::KwFn)?;
        let name = self.parse_fn_name()?;
        let generics = self.parse_generics()?;
        self.expect(TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.expect(TokenKind::RParen)?;
        let mut return_type = None;
        let mut expr_body = None;

        if self.at(&TokenKind::Arrow) {
            self.advance();
            let saved_pos = self.pos;
            if let Ok(ty) = self.parse_type() {
                let next_kind = self.cur().kind.clone();
                if matches!(
                    next_kind,
                    TokenKind::LBrace | TokenKind::Semicolon | TokenKind::KwWhere
                ) {
                    return_type = Some(ty);
                } else {
                    self.pos = saved_pos;
                }
            } else {
                self.pos = saved_pos;
            }

            if return_type.is_none() {
                expr_body = Some(self.parse_expr()?);
            }
        }

        let where_clauses = if expr_body.is_none() {
            self.parse_where_clauses()?
        } else {
            Vec::new()
        };

        let body = if let Some(expr) = expr_body {
            vec![Stmt::Return {
                expr: Some(expr),
                span: Span::new(start.start, self.span_at().start),
            }]
        } else if self.at(&TokenKind::Semicolon) {
            self.advance();
            Vec::new()
        } else {
            self.parse_block()?
        };
        Ok(FunctionDecl {
            name,
            is_async,
            is_extern,
            extern_abi,
            generics,
            params,
            return_type,
            where_clauses,
            body,
            span: Span::new(start.start, self.span_at().start),
        })
    }

    /// Handles `fn Name::method(...)` syntax for impl-style free fns
    fn parse_fn_name(&mut self) -> Result<String, Diagnostic> {
        let mut name = self.expect_ident()?;
        while self.at(&TokenKind::ColonColon) {
            self.advance();
            name.push_str("::");
            name.push_str(&self.expect_ident()?);
        }
        Ok(name)
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, Diagnostic> {
        let mut params = Vec::new();
        while !self.at(&TokenKind::RParen) && !self.is_eof() {
            if self.at_kw("self")
                || (self.at(&TokenKind::Ampersand) && {
                    let lexeme = self
                        .tokens
                        .get(self.pos + 1)
                        .map(|t| t.lexeme.as_str())
                        .unwrap_or("");
                    lexeme == "self" || lexeme == "mut"
                })
            {
                // &self / &mut self / self
                if self.at(&TokenKind::Ampersand) {
                    self.advance();
                }
                let mutable = self.eat_kw("mut");
                self.advance(); // self
                params.push(Param {
                    name: "self".into(),
                    mutable,
                    param_type: Type::simple("Self"),
                    default_value: None,
                });
            } else {
                let mutable = self.eat_kw("mut");
                let name = self.expect_ident()?;
                let mut is_optional = false;
                if self.at(&TokenKind::Question) {
                    self.advance();
                    is_optional = true;
                }
                let param_type = if self.at(&TokenKind::Colon) {
                    self.advance();
                    self.parse_type()?
                } else {
                    Type::Infer
                };
                let mut default_value = None;
                if self.at(&TokenKind::Equal) {
                    self.advance();
                    default_value = Some(self.parse_expr()?);
                }
                if is_optional && default_value.is_none() {
                    default_value = Some(Expr::NullLiteral);
                }
                params.push(Param {
                    name,
                    mutable,
                    param_type,
                    default_value,
                });
            }
            if !self.at(&TokenKind::Comma) {
                break;
            }
            self.advance();
        }
        Ok(params)
    }

    fn parse_struct(&mut self) -> Result<StructDecl, Diagnostic> {
        let start = self.span_at();
        self.advance(); // struct
        self.parse_record_decl(start)
    }

    fn parse_union(&mut self) -> Result<StructDecl, Diagnostic> {
        let start = self.span_at();
        self.advance(); // union
        self.parse_record_decl(start)
    }

    fn parse_record_decl(&mut self, start: Span) -> Result<StructDecl, Diagnostic> {
        let name = self.expect_ident()?;
        let generics = self.parse_generics()?;
        let where_clauses = self.parse_where_clauses()?;
        self.expect(TokenKind::LBrace)?;
        let mut fields = Vec::new();
        loop {
            self.skip_newlines();
            if self.at(&TokenKind::RBrace) || self.is_eof() {
                break;
            }
            let vis = self.parse_visibility();
            let fname = self.expect_ident()?;
            self.expect(TokenKind::Colon)?;
            let ftype = self.parse_type()?;
            let default = if self.at(&TokenKind::Equal) {
                self.advance();
                Some(self.parse_expr()?)
            } else {
                None
            };
            fields.push(StructField {
                vis,
                name: fname,
                field_type: ftype,
                default,
            });
            if self.at(&TokenKind::Comma) {
                self.advance();
            }
        }
        self.expect(TokenKind::RBrace)?;
        Ok(StructDecl {
            name,
            generics,
            where_clauses,
            fields,
            span: Span::new(start.start, self.span_at().start),
        })
    }

    fn parse_enum(&mut self) -> Result<EnumDecl, Diagnostic> {
        let start = self.span_at();
        self.advance(); // enum
        let name = self.expect_ident()?;
        let generics = self.parse_generics()?;
        let where_clauses = self.parse_where_clauses()?;
        self.expect(TokenKind::LBrace)?;
        let mut variants = Vec::new();
        loop {
            self.skip_newlines();
            if self.at(&TokenKind::RBrace) || self.is_eof() {
                break;
            }
            let vname = self.expect_ident()?;
            let variant = if self.at(&TokenKind::LParen) {
                self.advance();
                let mut types = Vec::new();
                while !self.at(&TokenKind::RParen) && !self.is_eof() {
                    types.push(self.parse_type()?);
                    if !self.at(&TokenKind::Comma) {
                        break;
                    }
                    self.advance();
                }
                self.expect(TokenKind::RParen)?;
                EnumVariant::Tuple(vname, types)
            } else if self.at(&TokenKind::LBrace) {
                self.advance();
                let mut fields = Vec::new();
                loop {
                    self.skip_newlines();
                    if self.at(&TokenKind::RBrace) || self.is_eof() {
                        break;
                    }
                    let fn_ = self.expect_ident()?;
                    self.expect(TokenKind::Colon)?;
                    let ft = self.parse_type()?;
                    fields.push(StructField {
                        vis: Visibility::Inherited,
                        name: fn_,
                        field_type: ft,
                        default: None,
                    });
                    if self.at(&TokenKind::Comma) {
                        self.advance();
                    }
                }
                self.expect(TokenKind::RBrace)?;
                EnumVariant::Struct(vname, fields)
            } else {
                EnumVariant::Unit(vname)
            };
            if self.at(&TokenKind::Equal) {
                self.advance();
                let _ = self.parse_expr()?;
            }
            variants.push(variant);
            if self.at(&TokenKind::Comma) {
                self.advance();
            }
        }
        self.expect(TokenKind::RBrace)?;
        Ok(EnumDecl {
            name,
            generics,
            where_clauses,
            variants,
            span: Span::new(start.start, self.span_at().start),
        })
    }

    fn parse_trait(&mut self) -> Result<TraitDecl, Diagnostic> {
        let start = self.span_at();
        self.advance(); // trait
        let name = self.expect_ident()?;
        let generics = self.parse_generics()?;
        let super_traits = if self.at(&TokenKind::Colon) {
            self.advance();
            let mut bounds = Vec::new();
            loop {
                bounds.push(TypeBound {
                    path: self.parse_type_path()?,
                });
                if !self.at(&TokenKind::Plus) {
                    break;
                }
                self.advance();
            }
            bounds
        } else {
            vec![]
        };
        let where_clauses = self.parse_where_clauses()?;
        self.expect(TokenKind::LBrace)?;
        let mut items = Vec::new();
        loop {
            self.skip_newlines();
            if self.at(&TokenKind::RBrace) || self.is_eof() {
                break;
            }
            if matches!(self.cur().kind, TokenKind::KwFn | TokenKind::KwAsync) {
                items.push(TraitItem::Method(self.parse_fn_decl()?));
            } else if self.at_kw("type") {
                self.advance();
                let n = self.expect_ident()?;
                self.skip_newlines();
                items.push(TraitItem::Type(n));
            } else if self.at_kw("const") {
                items.push(TraitItem::Const(self.parse_const()?));
            } else {
                self.advance();
            }
        }
        self.expect(TokenKind::RBrace)?;
        Ok(TraitDecl {
            name,
            generics,
            super_traits,
            where_clauses,
            items,
            span: Span::new(start.start, self.span_at().start),
        })
    }

    fn parse_impl(&mut self) -> Result<ImplBlock, Diagnostic> {
        let start = self.span_at();
        self.advance(); // impl
        let generics = self.parse_generics()?;
        let first_type = self.parse_type()?;
        let (trait_name, self_type) = if self.at_kw("for") {
            self.advance();
            let st = self.parse_type()?;
            if let Type::Named(path) = first_type {
                (Some(path), st)
            } else {
                (None, st)
            }
        } else {
            (None, first_type)
        };
        let _ = self.parse_where_clauses()?;
        self.expect(TokenKind::LBrace)?;
        let mut items = Vec::new();
        loop {
            self.skip_newlines();
            if self.at(&TokenKind::RBrace) || self.is_eof() {
                break;
            }
            let _vis = self.parse_visibility();
            if matches!(self.cur().kind, TokenKind::KwFn | TokenKind::KwAsync) {
                items.push(ImplItem::Method(self.parse_fn_decl()?));
            } else if self.at_kw("type") {
                items.push(ImplItem::TypeAlias(self.parse_type_alias()?));
            } else if self.at_kw("const") {
                items.push(ImplItem::Const(self.parse_const()?));
            } else {
                self.advance();
            }
        }
        self.expect(TokenKind::RBrace)?;
        Ok(ImplBlock {
            generics,
            trait_name,
            self_type,
            items,
            span: Span::new(start.start, self.span_at().start),
        })
    }

    fn parse_type_alias(&mut self) -> Result<TypeAlias, Diagnostic> {
        let start = self.span_at();
        self.advance(); // type
        let name = self.expect_ident()?;
        let generics = self.parse_generics()?;
        let where_clauses = self.parse_where_clauses()?;
        self.expect(TokenKind::Equal)?;
        let ty = self.parse_type()?;
        self.eat_semi();
        Ok(TypeAlias {
            name,
            generics,
            where_clauses,
            ty,
            span: Span::new(start.start, self.span_at().start),
        })
    }

    fn parse_const(&mut self) -> Result<ConstDecl, Diagnostic> {
        let start = self.span_at();
        self.advance(); // const
        let name = self.expect_ident()?;
        let ty = if self.at(&TokenKind::Colon) {
            self.advance();
            self.parse_type()?
        } else {
            Type::Infer
        };
        self.expect(TokenKind::Equal)?;
        let value = self.parse_expr()?;
        self.eat_semi();
        Ok(ConstDecl {
            name,
            ty,
            value,
            span: Span::new(start.start, self.span_at().start),
        })
    }

    fn _parse_static(&mut self) -> Result<StaticDecl, Diagnostic> {
        let start = self.span_at();
        self.advance(); // static
        self.parse_static_after_kw(start)
    }

    fn parse_static_after_kw(&mut self, start: Span) -> Result<StaticDecl, Diagnostic> {
        let mutable = self.eat_kw("mut");
        let name = self.expect_ident()?;
        let ty = if self.at(&TokenKind::Colon) {
            self.advance();
            self.parse_type()?
        } else {
            Type::Infer
        };
        self.expect(TokenKind::Equal)?;
        let value = self.parse_expr()?;
        self.eat_semi();
        Ok(StaticDecl {
            name,
            mutable,
            ty,
            value,
            span: Span::new(start.start, self.span_at().start),
        })
    }

    fn parse_mod_item(&mut self) -> Result<ItemKind, Diagnostic> {
        let start = self.span_at();
        self.advance(); // mod
        let name = self.parse_module_name()?;

        if self.at(&TokenKind::Equal) {
            self.advance();
            let value = self.parse_expr()?;
            self.eat_semi();
            return Ok(ItemKind::ModuleValue(ModuleValueDecl {
                name,
                value,
                span: Span::new(start.start, self.span_at().start),
            }));
        }

        if !self.disallow_struct_literal && self.at(&TokenKind::LBrace) {
            self.advance();
            let mut items = Vec::new();
            loop {
                self.skip_newlines();
                if self.at(&TokenKind::RBrace) || self.is_eof() {
                    break;
                }
                match self.parse_item() {
                    Ok(i) => items.push(i),
                    Err(e) => {
                        self.record_error(e);
                        self.recover_to_item_boundary();
                    }
                }
            }
            self.expect(TokenKind::RBrace)?;
            Ok(ItemKind::Module(ModuleDecl::Inline { name, items }))
        } else {
            self.eat_semi();
            Ok(ItemKind::Module(ModuleDecl::External(name)))
        }
    }

    fn parse_module_name(&mut self) -> Result<String, Diagnostic> {
        let mut name = self.expect_ident()?;
        while self.at(&TokenKind::Slash) || self.at(&TokenKind::ColonColon) {
            let sep = self.advance().lexeme.clone();
            name.push_str(&sep);
            name.push_str(&self.expect_ident()?);
        }
        Ok(name)
    }

    fn parse_use(&mut self) -> Result<UseDecl, Diagnostic> {
        let start = self.span_at();
        self.advance(); // use
        let tree = self.parse_use_tree()?;
        self.eat_semi();
        Ok(UseDecl {
            tree,
            span: Span::new(start.start, self.span_at().start),
        })
    }

    fn parse_use_tree(&mut self) -> Result<UseTree, Diagnostic> {
        let seg = self.expect_ident()?;
        if self.at(&TokenKind::ColonColon) || self.at(&TokenKind::Dot) {
            self.advance();
            if self.at(&TokenKind::LBrace) {
                self.advance();
                let mut trees = Vec::new();
                while !self.at(&TokenKind::RBrace) && !self.is_eof() {
                    trees.push(self.parse_use_tree()?);
                    if !self.at(&TokenKind::Comma) {
                        break;
                    }
                    self.advance();
                }
                self.expect(TokenKind::RBrace)?;
                return Ok(UseTree::Path {
                    segment: seg,
                    child: Box::new(UseTree::Group(trees)),
                });
            }
            if self.at(&TokenKind::Star) {
                self.advance();
                return Ok(UseTree::Path {
                    segment: seg,
                    child: Box::new(UseTree::Glob),
                });
            }
            let child = self.parse_use_tree()?;
            return Ok(UseTree::Path {
                segment: seg,
                child: Box::new(child),
            });
        }
        let alias = if self.at_kw("as") {
            self.advance();
            Some(self.expect_ident()?)
        } else {
            None
        };
        Ok(UseTree::Name { name: seg, alias })
    }

    fn parse_export(&mut self) -> Result<ExportDecl, Diagnostic> {
        let start = self.span_at();
        self.advance(); // export
        let mut items = Vec::new();
        let mut brace_mode = false;
        if self.at(&TokenKind::LBrace) {
            self.advance();
            brace_mode = true;
        }
        loop {
            self.skip_newlines();
            if brace_mode && self.at(&TokenKind::RBrace) {
                self.advance();
                break;
            }
            if !brace_mode
                && (self.is_eof()
                    || matches!(
                        self.cur().kind,
                        TokenKind::KwFn
                            | TokenKind::KwStruct
                            | TokenKind::KwEnum
                            | TokenKind::KwTrait
                            | TokenKind::KwImpl
                            | TokenKind::KwMod
                            | TokenKind::KwUse
                            | TokenKind::KwExport
                            | TokenKind::KwPub
                    ))
            {
                break;
            }
            if !matches!(
                self.cur().kind,
                TokenKind::Identifier | TokenKind::KwSelfType
            ) && !self.cur().kind.is_keyword()
            {
                break;
            }
            let name = self.advance().lexeme.clone();
            let alias = if self.at_kw("as") {
                self.advance();
                Some(self.expect_ident()?)
            } else {
                None
            };
            items.push(ExportItem { name, alias });
            if self.at(&TokenKind::Comma) {
                self.advance();
                continue;
            }
            if brace_mode {
                self.skip_newlines();
                if self.at(&TokenKind::RBrace) {
                    self.advance();
                }
            } else {
                break;
            }
        }
        Ok(ExportDecl {
            items,
            span: Span::new(start.start, self.span_at().start),
        })
    }

    // ── Generics ─────────────────────────────────────────────────────────────

    fn parse_generics(&mut self) -> Result<Vec<GenericParam>, Diagnostic> {
        if !self.at(&TokenKind::Less) {
            return Ok(vec![]);
        }
        self.advance();
        let mut params = Vec::new();
        while !self.at(&TokenKind::Greater) && !self.is_eof() {
            let name = self.expect_ident()?;
            let bounds = if self.at(&TokenKind::Colon) {
                self.advance();
                let mut b = Vec::new();
                loop {
                    b.push(TypeBound {
                        path: self.parse_type_path()?,
                    });
                    if !self.at(&TokenKind::Plus) {
                        break;
                    }
                    self.advance();
                }
                b
            } else {
                vec![]
            };
            let default = if self.at(&TokenKind::Equal) {
                self.advance();
                Some(self.parse_type()?)
            } else {
                None
            };
            params.push(GenericParam {
                name,
                bounds,
                default,
            });
            if !self.at(&TokenKind::Comma) {
                break;
            }
            self.advance();
        }
        self.expect(TokenKind::Greater)?;
        Ok(params)
    }

    fn parse_where_clauses(&mut self) -> Result<Vec<WherePredicate>, Diagnostic> {
        if !self.at_kw("where") {
            return Ok(vec![]);
        }
        self.advance();
        let mut preds = Vec::new();
        loop {
            let path = self.parse_type_path()?;
            self.expect(TokenKind::Colon)?;
            let mut bounds = Vec::new();
            loop {
                bounds.push(TypeBound {
                    path: self.parse_type_path()?,
                });
                if !self.at(&TokenKind::Plus) {
                    break;
                }
                self.advance();
            }
            preds.push(WherePredicate { ty: path, bounds });
            if !self.at(&TokenKind::Comma) {
                break;
            }
            self.advance();
        }
        Ok(preds)
    }

    // ── Types ─────────────────────────────────────────────────────────────────

    fn parse_type(&mut self) -> Result<Type, Diagnostic> {
        if self.at(&TokenKind::Pipe) {
            let mut types = Vec::new();
            while self.at(&TokenKind::Pipe) && !self.is_eof() {
                self.advance();
                types.push(self.parse_type()?);
                self.skip_newlines();
            }
            return Ok(Type::Union(types));
        }
        // record type: { field: Type, ... }
        if self.at(&TokenKind::LBrace) {
            self.advance();
            let mut fields = Vec::new();
            loop {
                self.skip_newlines();
                if self.at(&TokenKind::RBrace) || self.is_eof() {
                    break;
                }
                let name = self.expect_ident()?;
                if self.at(&TokenKind::Question) {
                    self.advance();
                }
                self.expect(TokenKind::Colon)?;
                let ty = self.parse_type()?;
                fields.push(RecordField { name, ty });
                if self.at(&TokenKind::Comma) {
                    self.advance();
                }
            }
            self.expect(TokenKind::RBrace)?;
            return Ok(Type::Record(fields));
        }
        // &T  /  &mut T
        if self.at(&TokenKind::Ampersand) {
            self.advance();
            if self.eat_kw("mut") {
                return Ok(Type::MutReference(Box::new(self.parse_type()?)));
            }
            return Ok(Type::Reference(Box::new(self.parse_type()?)));
        }
        // *T / *const T / *mut T
        if self.at(&TokenKind::Star) {
            self.advance();
            let mutable = self.eat_kw("mut");
            if !mutable {
                self.eat_kw("const");
            }
            return Ok(Type::Pointer {
                mutable,
                to: Box::new(self.parse_type()?),
            });
        }
        // [T]
        if self.at(&TokenKind::LBracket) {
            self.advance();
            let inner = self.parse_type()?;
            if self.at(&TokenKind::Semicolon) {
                self.advance();
                while !self.at(&TokenKind::RBracket) && !self.is_eof() {
                    self.advance();
                }
            }
            self.expect(TokenKind::RBracket)?;
            return Ok(Type::Array(Box::new(inner)));
        }
        // (A, B) tuple
        if self.at(&TokenKind::LParen) {
            self.advance();
            let mut types = Vec::new();
            while !self.at(&TokenKind::RParen) && !self.is_eof() {
                types.push(self.parse_type()?);
                if !self.at(&TokenKind::Comma) {
                    break;
                }
                self.advance();
            }
            self.expect(TokenKind::RParen)?;
            let mut ty = if types.len() == 1 {
                types.remove(0)
            } else {
                Type::Tuple(types)
            };

            if self.at(&TokenKind::Question) {
                self.advance();
                ty = Type::Nullable(Box::new(ty));
            }
            return Ok(ty);
        }
        // fn(A, B) -> C
        if self.at(&TokenKind::KwFn) {
            self.advance();
            self.expect(TokenKind::LParen)?;
            let mut args = Vec::new();
            while !self.at(&TokenKind::RParen) && !self.is_eof() {
                if self.looks_like_named_fn_type_param() {
                    self.advance();
                    self.expect(TokenKind::Colon)?;
                }
                args.push(self.parse_type()?);
                if !self.at(&TokenKind::Comma) {
                    break;
                }
                self.advance();
            }
            self.expect(TokenKind::RParen)?;
            let ret = if self.at(&TokenKind::Arrow) {
                self.advance();
                self.parse_type()?
            } else {
                Type::Tuple(vec![])
            };
            let mut ty = Type::Function(args, Box::new(ret));
            if self.at(&TokenKind::Question) {
                self.advance();
                ty = Type::Nullable(Box::new(ty));
            }
            return Ok(ty);
        }
        // literal types: "foo", 123, 1.2, true/false
        if self.at(&TokenKind::String) {
            let v = self.advance().lexeme.clone();
            return Ok(Type::Literal(TypeLiteral::String(v)));
        }
        if self.at(&TokenKind::Integer) {
            let v = self.advance().lexeme.clone();
            return Ok(Type::Literal(TypeLiteral::Int(v)));
        }
        if self.at(&TokenKind::Float) {
            let v = self.advance().lexeme.clone();
            return Ok(Type::Literal(TypeLiteral::Float(v)));
        }
        if matches!(self.cur().kind, TokenKind::Boolean | TokenKind::KwTrue | TokenKind::KwFalse)
        {
            let v = self.cur().lexeme == "true";
            self.advance();
            return Ok(Type::Literal(TypeLiteral::Bool(v)));
        }
        // _ infer
        let mut ty = if self.at(&TokenKind::Identifier) && self.cur().lexeme == "_" {
            self.advance();
            Type::Infer
        } else {
            Type::Named(self.parse_type_path()?)
        };

        if self.at(&TokenKind::Pipe) {
            let mut types = vec![ty];
            while self.at(&TokenKind::Pipe) && !self.is_eof() {
                self.advance();
                types.push(self.parse_type()?);
                self.skip_newlines();
            }
            ty = Type::Union(types);
        }

        if self.at(&TokenKind::Question) {
            self.advance();
            ty = Type::Nullable(Box::new(ty));
        }
        Ok(ty)
    }

    fn parse_type_path(&mut self) -> Result<TypePath, Diagnostic> {
        let mut segs = Vec::new();
        loop {
            if !matches!(
                self.cur().kind,
                TokenKind::Identifier
                    | TokenKind::KwSelfType
                    | TokenKind::KwSelf
                    | TokenKind::KwCrate
                    | TokenKind::KwSuper
            ) && !self.cur().kind.is_keyword()
            {
                break;
            }
            let name = self.advance().lexeme.clone();
            let args = if self.at(&TokenKind::Less) {
                self.advance();
                let mut a = Vec::new();
                while !self.at_type_arg_end() && !self.is_eof() {
                    a.push(self.parse_type()?);
                    if !self.at(&TokenKind::Comma) {
                        break;
                    }
                    self.advance();
                }
                self.consume_type_arg_end()?;
                a
            } else {
                vec![]
            };
            segs.push(TypeSegment { name, args });
            if !self.at(&TokenKind::ColonColon) && !self.at(&TokenKind::Dot) {
                break;
            }
            self.advance();
        }
        if segs.is_empty() {
            return Err(
                self.error_here(
                    codes::PARSER_SYNTAX_ERROR,
                    format!("expected type, found '{}'", self.cur().lexeme),
                )
                .with_suggestion("provide a valid type name here"),
            );
        }
        Ok(TypePath { segments: segs })
    }

    // ── Block & statements ────────────────────────────────────────────────────

    fn parse_block(&mut self) -> Result<Vec<Stmt>, Diagnostic> {
        self.expect(TokenKind::LBrace)?;
        let mut stmts = Vec::new();
        loop {
            self.skip_newlines();
            if self.at(&TokenKind::RBrace) || self.is_eof() {
                break;
            }
            match self.parse_stmt() {
                Ok(s) => stmts.push(s),
                Err(e) => {
                    self.record_error(e);
                    // skip to next brace or newline
                    while !self.is_eof()
                        && !matches!(
                            self.cur().kind,
                            TokenKind::RBrace | TokenKind::Newline | TokenKind::Semicolon
                        )
                    {
                        self.advance();
                    }
                }
            }
        }
        self.expect_r_brace_or_tail(&mut stmts)?;
        Ok(stmts)
    }

    fn expect_r_brace_or_tail(&mut self, _stmts: &mut Vec<Stmt>) -> Result<(), Diagnostic> {
        if self.at(&TokenKind::RBrace) {
            self.advance();
            Ok(())
        } else {
            Err(
                self.missing_token("'}'")
                    .with_suggestion("add a closing '}' to finish this block"),
            )
        }
    }

    fn eat_semi(&mut self) {
        while matches!(self.cur().kind, TokenKind::Semicolon | TokenKind::Newline) {
            self.advance();
        }
    }

    fn parse_asm_operand(&mut self) -> Result<AsmOperand, Diagnostic> {
        self.expect(TokenKind::LBracket)?;
        let expr = self.parse_expr()?;
        self.expect(TokenKind::RBracket)?;
        let reg = if self.at(&TokenKind::LParen) {
            self.advance();
            let r = self.expect_ident()?;
            self.expect(TokenKind::RParen)?;
            Some(r)
        } else {
            None
        };
        Ok(AsmOperand { expr, reg })
    }

    fn parse_stmt(&mut self) -> Result<Stmt, Diagnostic> {
        let span = self.span_at();
        // let / mut let
        let is_mut_let = self.at_kw("mut") && {
            let nxt = self
                .tokens
                .get(self.pos + 1)
                .map(|t| t.kind == TokenKind::KwLet)
                .unwrap_or(false);
            nxt
        };
        let is_var_let = self.at(&TokenKind::Identifier) && self.cur().lexeme == "var";
        if self.at(&TokenKind::KwLet) || is_mut_let || is_var_let {
            let mutable = if is_mut_let || is_var_let {
                self.advance();
                true
            } else {
                false
            };
            if !is_var_let {
                self.advance(); // let
            }
            let mut is_mut2 = mutable;
            if self.eat_kw("mut") {
                is_mut2 = true;
            }
            let name = self.expect_ident()?;
            let var_type = if self.at(&TokenKind::Colon) {
                self.advance();
                Some(self.parse_type()?)
            } else {
                None
            };
            let expr = if self.at(&TokenKind::Equal) {
                self.advance();
                self.parse_expr()?
            } else {
                Expr::NullLiteral
            };
            self.eat_semi();
            return Ok(Stmt::Let {
                mutable: is_mut2,
                name,
                var_type,
                expr,
                span,
            });
        }
        // return
        if self.at(&TokenKind::KwReturn) {
            self.advance();
            let expr = if !matches!(
                self.cur().kind,
                TokenKind::RBrace | TokenKind::Newline | TokenKind::Semicolon | TokenKind::Eof
            ) {
                Some(self.parse_expr()?)
            } else {
                None
            };
            self.eat_semi();
            return Ok(Stmt::Return { expr, span });
        }
        // if
        if self.at(&TokenKind::KwIf) {
            return self.parse_if_stmt();
        }
        // while
        if self.at(&TokenKind::KwWhile) {
            self.advance();
            let cond = self.parse_control_expr()?;
            let body = self.parse_block()?;
            return Ok(Stmt::While {
                condition: Box::new(cond),
                body,
                span,
            });
        }
        // for … in …
        if self.at(&TokenKind::KwFor) {
            self.advance();
            let var = self.expect_ident()?;
            self.expect(TokenKind::KwIn)?;
            let iter = self.parse_control_expr()?;
            let body = self.parse_block()?;
            return Ok(Stmt::ForIn {
                var,
                iter: Box::new(iter),
                body,
                span,
            });
        }
        // loop
        if self.at(&TokenKind::KwLoop) {
            let next_pos = self.next_non_trivia_pos(self.pos + 1);
            let next_kind = self.tokens.get(next_pos).map(|t| t.kind.clone());
            if !matches!(
                next_kind,
                Some(TokenKind::Dot)
                    | Some(TokenKind::ColonColon)
                    | Some(TokenKind::LBracket)
                    | Some(TokenKind::LParen)
                    | Some(TokenKind::Less)
            ) {
                self.advance();
                let body = self.parse_block()?;
                return Ok(Stmt::Loop { body, span });
            }
        }
        // match
        if self.at(&TokenKind::KwMatch) {
            return self.parse_match_stmt();
        }
        // break
        if self.at(&TokenKind::KwBreak) {
            self.advance();
            let label = if self.at(&TokenKind::Identifier) {
                Some(self.advance().lexeme.clone())
            } else {
                None
            };
            self.eat_semi();
            return Ok(Stmt::Break { label, span });
        }
        // continue
        if self.at(&TokenKind::KwContinue) {
            self.advance();
            let label = if self.at(&TokenKind::Identifier) {
                Some(self.advance().lexeme.clone())
            } else {
                None
            };
            self.eat_semi();
            return Ok(Stmt::Continue { label, span });
        }
        // defer
        if self.at_kw("defer") {
            self.advance();
            let stmt = self.parse_stmt()?;
            return Ok(Stmt::Defer {
                stmt: Box::new(stmt),
                span: Span::new(span.start, self.cur().span.start),
            });
        }
        // unsafe
        if self.at(&TokenKind::KwUnsafe) {
            self.advance();
            let body = self.parse_block()?;
            return Ok(Stmt::Unsafe { body, span });
        }
        // asm
        if self.at(&TokenKind::KwAsm) {
            self.advance();
            self.expect(TokenKind::LParen)?;
            let code = if self.at(&TokenKind::String) {
                self.advance().lexeme.clone()
            } else {
                "".into()
            };
            self.expect(TokenKind::RParen)?;
            let mut outputs = Vec::new();
            if self.at(&TokenKind::Colon) {
                self.advance();
                while !self.at(&TokenKind::Colon)
                    && !self.at(&TokenKind::Semicolon)
                    && !self.is_eof()
                {
                    outputs.push(self.parse_asm_operand()?);
                    if !self.at(&TokenKind::Comma) {
                        break;
                    }
                    self.advance();
                }
            }
            let mut inputs = Vec::new();
            if self.at(&TokenKind::Colon) {
                self.advance();
                while !self.at(&TokenKind::Semicolon) && !self.is_eof() {
                    inputs.push(self.parse_asm_operand()?);
                    if !self.at(&TokenKind::Comma) {
                        break;
                    }
                    self.advance();
                }
            }
            self.eat_semi();
            return Ok(Stmt::InlineAsm {
                code,
                outputs,
                inputs,
                span,
            });
        }

        // expression or assignment
        let expr = self.parse_expr()?;
        // check for assignment or compound-assign
        if self.at(&TokenKind::Equal) {
            self.advance();
            let val = self.parse_expr()?;
            self.eat_semi();
            return Ok(Stmt::Assign {
                target: expr,
                value: val,
                span,
            });
        }
        if let Some(op) = self.compound_assign_op() {
            self.advance();
            let val = self.parse_expr()?;
            self.eat_semi();
            return Ok(Stmt::CompoundAssign {
                target: expr,
                op,
                value: val,
                span,
            });
        }
        // print built-in shim
        if let Expr::Call {
            ref callee,
            ref args,
        } = expr
        {
            if let Expr::Identifier(ref name) = callee.as_ref() {
                if name == "print" && args.len() == 1 {
                    self.eat_semi();
                    return Ok(Stmt::Print {
                        expr: args[0].clone(),
                    });
                }
            }
        }
        self.eat_semi();
        Ok(Stmt::Expr(expr))
    }

    fn compound_assign_op(&self) -> Option<String> {
        match self.cur().kind {
            TokenKind::PlusEqual => Some("+=".into()),
            TokenKind::MinusEqual => Some("-=".into()),
            TokenKind::StarEqual => Some("*=".into()),
            TokenKind::SlashEqual => Some("/=".into()),
            TokenKind::PercentEqual => Some("%=".into()),
            TokenKind::AmpersandEqual => Some("&=".into()),
            TokenKind::PipeEqual => Some("|=".into()),
            TokenKind::CaretEqual => Some("^=".into()),
            TokenKind::LessLessEqual => Some("<<=".into()),
            TokenKind::GreaterGreaterEqual => Some(">>=".into()),
            _ => None,
        }
    }

    fn parse_if_stmt(&mut self) -> Result<Stmt, Diagnostic> {
        let span = self.span_at();
        let mut branches = Vec::new();
        let mut else_body = None;
        loop {
            self.advance(); // if
            let cond = self.parse_control_expr()?;
            let body = self.parse_block()?;
            branches.push(IfBranch {
                condition: cond,
                body,
            });
            if self.at(&TokenKind::KwElse) {
                self.advance();
                if self.at(&TokenKind::KwIf) {
                    continue;
                }
                else_body = Some(self.parse_block()?);
            }
            break;
        }
        Ok(Stmt::If {
            branches,
            else_body,
            span,
        })
    }

    fn parse_match_stmt(&mut self) -> Result<Stmt, Diagnostic> {
        let span = self.span_at();
        self.advance(); // match
        let expr = self.parse_control_expr()?;
        self.expect(TokenKind::LBrace)?;
        let arms = self.parse_match_arms()?;
        self.expect(TokenKind::RBrace)?;
        Ok(Stmt::Match {
            expr: Box::new(expr),
            arms,
            span,
        })
    }

    fn parse_match_arms(&mut self) -> Result<Vec<MatchArm>, Diagnostic> {
        let mut arms = Vec::new();
        loop {
            self.skip_newlines();
            if self.at(&TokenKind::RBrace) || self.is_eof() {
                break;
            }
            let pattern = self.parse_pattern()?;
            let guard = if self.at_kw("if") {
                self.advance();
                Some(self.parse_expr()?)
            } else {
                None
            };
            self.expect(TokenKind::FatArrow)?;
            let body = if self.at(&TokenKind::LBrace) {
                MatchBody::Block(self.parse_block()?)
            } else {
                let s = self.parse_stmt()?;
                if let Stmt::Expr(e) = s {
                    MatchBody::Expr(e)
                } else {
                    MatchBody::Stmt(s)
                }
            };
            arms.push(MatchArm {
                pattern,
                guard,
                body,
            });
            if self.at(&TokenKind::Comma) {
                self.advance();
            }
        }
        Ok(arms)
    }

    fn parse_pattern(&mut self) -> Result<MatchPattern, Diagnostic> {
        let p = self.parse_single_pattern()?;
        if self.at(&TokenKind::Pipe) {
            let mut pats = vec![p];
            while self.at(&TokenKind::Pipe) {
                self.advance();
                pats.push(self.parse_single_pattern()?);
            }
            return Ok(MatchPattern::Or(pats));
        }
        Ok(p)
    }

    fn parse_single_pattern(&mut self) -> Result<MatchPattern, Diagnostic> {
        if self.at(&TokenKind::Identifier) && self.cur().lexeme == "_" {
            self.advance();
            return Ok(MatchPattern::Wildcard);
        }
        if self.at(&TokenKind::DotDot) {
            self.advance();
            return Ok(MatchPattern::Rest);
        }
        if self.at(&TokenKind::LParen) {
            self.advance();
            let mut pats = Vec::new();
            while !self.at(&TokenKind::RParen) && !self.is_eof() {
                pats.push(self.parse_pattern()?);
                if !self.at(&TokenKind::Comma) {
                    break;
                }
                self.advance();
            }
            self.expect(TokenKind::RParen)?;
            return Ok(MatchPattern::Tuple(pats));
        }
        if self.at(&TokenKind::LBracket) {
            self.advance();
            let mut pats = Vec::new();
            while !self.at(&TokenKind::RBracket) && !self.is_eof() {
                pats.push(self.parse_pattern()?);
                if !self.at(&TokenKind::Comma) {
                    break;
                }
                self.advance();
            }
            self.expect(TokenKind::RBracket)?;
            return Ok(MatchPattern::Tuple(pats));
        }
        if matches!(
            self.cur().kind,
            TokenKind::Integer
                | TokenKind::Float
                | TokenKind::String
                | TokenKind::Boolean
                | TokenKind::Char
        ) {
            let e = self.parse_primary()?;
            return Ok(MatchPattern::Literal(e));
        }
        if self.at(&TokenKind::Minus) {
            self.advance();
            let e = self.parse_primary()?;
            return Ok(MatchPattern::Literal(Expr::Unary {
                op: "-".into(),
                right: Box::new(e),
            }));
        }
        let mut name = self.expect_ident()?;
        while self.at(&TokenKind::Dot) || self.at(&TokenKind::ColonColon) {
            let sep = self.advance().lexeme.clone();
            name.push_str(&sep);
            name.push_str(&self.expect_ident()?);
        }

        // binding pattern: name @ pat
        if self.at(&TokenKind::At) {
            self.advance();
            let inner = self.parse_single_pattern()?;
            return Ok(MatchPattern::Binding(name, Box::new(inner)));
        }
        // tuple variant: Name(p1, p2)
        if self.at(&TokenKind::LParen) {
            self.advance();
            let mut pats = Vec::new();
            while !self.at(&TokenKind::RParen) && !self.is_eof() {
                pats.push(self.parse_pattern()?);
                if !self.at(&TokenKind::Comma) {
                    break;
                }
                self.advance();
            }
            self.expect(TokenKind::RParen)?;
            return Ok(MatchPattern::TupleVariant(name, pats));
        }
        // struct variant: Name { field: pat }
        if self.at(&TokenKind::LBrace) {
            self.advance();
            let mut fields = Vec::new();
            while !self.at(&TokenKind::RBrace) && !self.is_eof() {
                let fn_ = self.expect_ident()?;
                let fp = if self.at(&TokenKind::Colon) {
                    self.advance();
                    self.parse_pattern()?
                } else {
                    MatchPattern::Identifier(fn_.clone())
                };
                fields.push((fn_, fp));
                if self.at(&TokenKind::Comma) {
                    self.advance();
                }
            }
            self.expect(TokenKind::RBrace)?;
            return Ok(MatchPattern::StructVariant(name, fields));
        }
        Ok(MatchPattern::Identifier(name))
    }

    // ── Expressions (Pratt precedence climbing) ───────────────────────────────

    fn parse_expr(&mut self) -> Result<Expr, Diagnostic> {
        self.parse_expr_with_stop(false)
    }

    fn parse_expr_with_stop(&mut self, stop_on_lbrace: bool) -> Result<Expr, Diagnostic> {
        let mut expr = self.parse_expr_prec_with_stop(0, false, stop_on_lbrace)?;
        loop {
            if !self.at(&TokenKind::Question) {
                break;
            }
            let next_kind = self.tokens.get(self.pos + 1).map(|t| t.kind.clone());
            let is_ternary = next_kind
                .as_ref()
                .map(|k| self.is_expr_start_kind(k))
                .unwrap_or(false);
            if is_ternary {
                self.advance();
                let then_expr = self.parse_expr_prec_with_stop(0, true, stop_on_lbrace)?;
                self.expect(TokenKind::Colon)?;
                let else_expr = self.parse_expr()?;
                expr = Expr::Ternary {
                    condition: Box::new(expr),
                    then_expr: Box::new(then_expr),
                    else_expr: Box::new(else_expr),
                };
                break;
            } else {
                self.advance();
                expr = Expr::TryOp(Box::new(expr));
                continue;
            }
        }
        Ok(expr)
    }

    fn parse_expr_prec_with_stop(
        &mut self,
        min_prec: u8,
        stop_on_colon: bool,
        stop_on_lbrace: bool,
    ) -> Result<Expr, Diagnostic> {
        let mut left = self.parse_unary()?;
        loop {
            if (stop_on_colon && self.at(&TokenKind::Colon))
                || (stop_on_lbrace && self.at(&TokenKind::LBrace))
            {
                break;
            }
            // range: .. or ..=
            if self.at(&TokenKind::DotDot) || self.at(&TokenKind::DotDotEq) {
                let inclusive = self.cur().kind == TokenKind::DotDotEq;
                self.advance();
                let end = if !matches!(
                    self.cur().kind,
                    TokenKind::RBrace
                        | TokenKind::RBracket
                        | TokenKind::Comma
                        | TokenKind::Semicolon
                        | TokenKind::Newline
                        | TokenKind::Eof
                ) {
                    Some(Box::new(self.parse_unary()?))
                } else {
                    None
                };
                left = Expr::Range {
                    start: Some(Box::new(left)),
                    end,
                    inclusive,
                };
                continue;
            }
            let Some((op, prec, right_assoc)) = self.binary_op_prec() else {
                break;
            };
            if prec < min_prec {
                break;
            }
            self.advance();
            let next_prec = if right_assoc { prec } else { prec + 1 };
            let right = self.parse_expr_prec_with_stop(next_prec, stop_on_colon, stop_on_lbrace)?;
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn binary_op_prec(&self) -> Option<(String, u8, bool)> {
        let (op, prec) = match &self.cur().kind {
            TokenKind::PipePipe | TokenKind::KwOr => ("||".into(), 1u8),
            TokenKind::QuestionQuestion => ("??".into(), 1u8),
            TokenKind::AmpersandAmpersand | TokenKind::KwAnd => ("&&".into(), 2),
            TokenKind::Pipe => ("|".into(), 3),
            TokenKind::Caret => ("^".into(), 4),
            TokenKind::Ampersand => ("&".into(), 5),
            TokenKind::EqEq => ("==".into(), 6),
            TokenKind::BangEqual => ("!=".into(), 6),
            TokenKind::Less => ("<".into(), 7),
            TokenKind::Greater => (">".into(), 7),
            TokenKind::LessEqual => ("<=".into(), 7),
            TokenKind::GreaterEqual => (">=".into(), 7),
            TokenKind::KwIn => ("in".into(), 7),
            TokenKind::LessLess => ("<<".into(), 9),
            TokenKind::GreaterGreater => (">>".into(), 9),
            TokenKind::Plus => ("+".into(), 10),
            TokenKind::Minus => ("-".into(), 10),
            TokenKind::Star => ("*".into(), 11),
            TokenKind::Slash => ("/".into(), 11),
            TokenKind::Percent => ("%".into(), 11),
            _ => return None,
        };
        Some((op, prec, false))
    }

    fn parse_unary(&mut self) -> Result<Expr, Diagnostic> {
        if self.at(&TokenKind::Bang) || self.at(&TokenKind::KwNot) {
            self.advance();
            let r = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: "!".into(),
                right: Box::new(r),
            });
        }
        if self.at(&TokenKind::Minus) {
            self.advance();
            let r = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: "-".into(),
                right: Box::new(r),
            });
        }
        if self.at(&TokenKind::Tilde) {
            self.advance();
            let r = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: "~".into(),
                right: Box::new(r),
            });
        }
        if self.at(&TokenKind::Star) {
            self.advance();
            let r = self.parse_unary()?;
            return Ok(Expr::Deref(Box::new(r)));
        }
        if self.at(&TokenKind::Ampersand) {
            self.advance();
            let mutable = self.eat_kw("mut");
            let r = self.parse_unary()?;
            return Ok(Expr::Reference {
                mutable,
                expr: Box::new(r),
            });
        }
        if self.at_kw("move") {
            self.advance();
            let r = self.parse_unary()?;
            return Ok(Expr::Move(Box::new(r)));
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr, Diagnostic> {
        let mut e = self.parse_primary()?;
        loop {
            // `.await`
            if self.at(&TokenKind::Dot)
                && self
                    .tokens
                    .get(self.pos + 1)
                    .map(|t| t.lexeme == "await")
                    .unwrap_or(false)
            {
                self.advance();
                self.advance();
                e = Expr::Await(Box::new(e));
                continue;
            }
            // field/method access
            let is_field_access = self.at(&TokenKind::Dot)
                || (self.at(&TokenKind::ColonColon)
                    && self
                        .tokens
                        .get(self.pos + 1)
                        .map(|t| t.kind != TokenKind::Less)
                        .unwrap_or(true));

            if is_field_access {
                self.advance();
                let field = self.expect_ident()?;
                if self.at(&TokenKind::ColonColon)
                    && self
                        .tokens
                        .get(self.pos + 1)
                        .map(|t| t.kind == TokenKind::Less)
                        .unwrap_or(false)
                {
                    self.advance();
                    self.advance();
                    while !self.at_type_arg_end() && !self.is_eof() {
                        let _ = self.parse_type()?;
                        if !self.at(&TokenKind::Comma) {
                            break;
                        }
                        self.advance();
                    }
                    self.consume_type_arg_end()?;
                }
                if self.at(&TokenKind::LParen) {
                    self.advance();
                    let args = self.parse_call_args()?;
                    self.expect(TokenKind::RParen)?;
                    e = Expr::MethodCall {
                        receiver: Box::new(e),
                        method: field,
                        args,
                    };
                } else {
                    e = Expr::FieldAccess {
                        object: Box::new(e),
                        field,
                    };
                }
                continue;
            }
            // index
            if self.at(&TokenKind::LBracket) {
                self.advance();
                if self.at(&TokenKind::Colon) {
                    self.advance();
                    let end = if !self.at(&TokenKind::RBracket) {
                        Some(Box::new(self.parse_expr()?))
                    } else {
                        None
                    };
                    self.expect(TokenKind::RBracket)?;
                    e = Expr::Slice {
                        object: Box::new(e),
                        start: None,
                        end,
                    };
                } else {
                    let start_expr = self.parse_expr()?;
                    if self.at(&TokenKind::Colon) {
                        self.advance();
                        let end = if !self.at(&TokenKind::RBracket) {
                            Some(Box::new(self.parse_expr()?))
                        } else {
                            None
                        };
                        self.expect(TokenKind::RBracket)?;
                        e = Expr::Slice {
                            object: Box::new(e),
                            start: Some(Box::new(start_expr)),
                            end,
                        };
                    } else {
                        self.expect(TokenKind::RBracket)?;
                        e = Expr::Index {
                            object: Box::new(e),
                            index: Box::new(start_expr),
                        };
                    }
                }
                continue;
            }
            // call
            if self.at(&TokenKind::ColonColon)
                && self
                    .tokens
                    .get(self.pos + 1)
                    .map(|t| t.kind == TokenKind::Less)
                    .unwrap_or(false)
            {
                self.advance();
                self.advance();
                while !self.at_type_arg_end() && !self.is_eof() {
                    let _ = self.parse_type()?;
                    if !self.at(&TokenKind::Comma) {
                        break;
                    }
                    self.advance();
                }
                self.consume_type_arg_end()?;
                continue;
            }
            if self.at(&TokenKind::LParen) {
                self.advance();
                let args = self.parse_call_args()?;
                self.expect(TokenKind::RParen)?;
                e = Expr::Call {
                    callee: Box::new(e),
                    args,
                };
                continue;
            }
            // as cast
            if self.at(&TokenKind::KwAs) {
                self.advance();
                let ty = self.parse_type()?;
                e = Expr::Cast {
                    expr: Box::new(e),
                    ty,
                };
                continue;
            }
            break;
        }
        Ok(e)
    }

    fn parse_call_args(&mut self) -> Result<Vec<Expr>, Diagnostic> {
        let mut args = Vec::new();
        while !self.at(&TokenKind::RParen) && !self.is_eof() {
            args.push(self.parse_expr()?);
            if !self.at(&TokenKind::Comma) {
                break;
            }
            self.advance();
        }
        Ok(args)
    }

    fn parse_primary(&mut self) -> Result<Expr, Diagnostic> {
        match self.cur().kind.clone() {
            TokenKind::Integer => {
                let _span = self.cur().span;
                let v = self.cur().lexeme.clone();
                self.advance();
                let parsed = if v.starts_with("0x") || v.starts_with("0X") {
                    i64::from_str_radix(&v[2..], 16)
                } else if let Some(stripped) = v.strip_prefix("0b") {
                    i64::from_str_radix(stripped, 2)
                } else if let Some(stripped) = v.strip_prefix("0o") {
                    i64::from_str_radix(stripped, 8)
                } else {
                    v.parse()
                };
                match parsed {
                    Ok(n) => Ok(Expr::IntLiteral(n)),
                    Err(_) => Ok(Expr::BigIntLiteral(v)),
                }
            }
            TokenKind::Float => {
                let span = self.cur().span;
                let v = self.cur().lexeme.clone();
                self.advance();
                match v.parse::<f64>() {
                    Ok(n) => Ok(Expr::FloatLiteral(n)),
                    Err(_) => Err(
                        self.error_at(
                            codes::INVALID_LITERAL,
                            span,
                            format!("invalid float literal '{v}'"),
                        )
                        .with_suggestion("use a valid float literal for this expression"),
                    ),
                }
            }
            TokenKind::String => {
                let v = self.cur().lexeme.clone();
                self.advance();
                Ok(Expr::StringLiteral(v))
            }
            TokenKind::CssLiteral => {
                let raw = self.cur().lexeme.clone();
                self.advance();
                Ok(Expr::CssLiteral(raw))
            }
            TokenKind::Char => {
                let v = self.cur().lexeme.clone();
                self.advance();
                Ok(Expr::CharLiteral(v.chars().next().unwrap_or('\0')))
            }
            TokenKind::Boolean | TokenKind::KwTrue => {
                let v = self.cur().lexeme == "true";
                self.advance();
                Ok(Expr::BoolLiteral(v))
            }
            TokenKind::KwFalse => {
                self.advance();
                Ok(Expr::BoolLiteral(false))
            }
            TokenKind::Null | TokenKind::KwNull => {
                self.advance();
                Ok(Expr::NullLiteral)
            }

            // array literal
            TokenKind::LBracket => {
                self.advance();
                if self.at(&TokenKind::RBracket) {
                    self.advance();
                    return Ok(Expr::ArrayLiteral(vec![]));
                }
                let first = self.parse_expr()?;
                if self.at(&TokenKind::Semicolon) {
                    self.advance();
                    let len = self.parse_expr()?;
                    self.expect(TokenKind::RBracket)?;
                    return Ok(Expr::ArrayRepeat {
                        value: Box::new(first),
                        len: Box::new(len),
                    });
                }
                let mut elems = Vec::new();
                elems.push(first);
                while !self.at(&TokenKind::RBracket) && !self.is_eof() {
                    self.skip_newlines();
                    if self.at(&TokenKind::RBracket) {
                        break;
                    }
                    if self.at(&TokenKind::Comma) {
                        self.advance();
                        self.skip_newlines();
                        if self.at(&TokenKind::RBracket) {
                            break;
                        }
                        elems.push(self.parse_expr()?);
                        continue;
                    }
                    elems.push(self.parse_expr()?);
                }
                self.expect(TokenKind::RBracket)?;
                Ok(Expr::ArrayLiteral(elems))
            }

            // grouped, tuple, or closure `(…)`
            TokenKind::LParen => {
                self.advance();
                if self.at(&TokenKind::RParen) {
                    self.advance();
                    return Ok(Expr::TupleLiteral(vec![]));
                }
                let first = self.parse_expr()?;
                if self.at(&TokenKind::RParen) {
                    self.advance();
                    return Ok(first);
                }
                let mut elems = vec![first];
                while self.at(&TokenKind::Comma) && !self.is_eof() {
                    self.advance();
                    if self.at(&TokenKind::RParen) {
                        break;
                    }
                    elems.push(self.parse_expr()?);
                }
                self.expect(TokenKind::RParen)?;
                Ok(Expr::TupleLiteral(elems))
            }

            // block expression / struct-literal-without-name `{ field: val }`
            TokenKind::LBrace => {
                // peek: if next is field name followed by colon (skipping trivia), it's a block literal
                let next = self.next_non_trivia_pos(self.pos + 1);
                let next2 = self.next_non_trivia_pos(next + 1);
                let is_block_lit = self.is_field_name_token(next)
                    && self
                        .tokens
                        .get(next2)
                        .map(|t| t.kind == TokenKind::Colon)
                        .unwrap_or(false)
                    || self
                        .tokens
                        .get(next)
                        .map(|t| t.kind == TokenKind::DotDotDot)
                        .unwrap_or(false);
                if is_block_lit {
                    self.advance();
                    let mut items = Vec::new();
                    while !self.at(&TokenKind::RBrace) && !self.is_eof() {
                        self.skip_newlines();
                        if self.at(&TokenKind::RBrace) {
                            break;
                        }
                        if self.at(&TokenKind::DotDotDot) {
                            self.advance();
                            let spread = self.parse_expr()?;
                            items.push(BlockItem::Spread(spread));
                        } else {
                            let fname = self.parse_field_name()?;
                            self.expect(TokenKind::Colon)?;
                            let fval = self.parse_expr()?;
                            items.push(BlockItem::Field(FieldInit {
                                name: fname,
                                value: fval,
                            }));
                        }
                        if self.at(&TokenKind::Comma) {
                            self.advance();
                        }
                        self.skip_newlines();
                    }
                    self.expect(TokenKind::RBrace)?;
                    return Ok(Expr::BlockLiteral(items));
                }
                // otherwise a block expression
                let stmts = self.parse_block()?;
                Ok(Expr::Block(stmts, None))
            }

            // anonymous function `fn(params) -> ret { body }` or `fn(params) -> expr`
            TokenKind::KwFn => {
                self.advance();
                self.expect(TokenKind::LParen)?;
                let params = self.parse_params()?;
                self.expect(TokenKind::RParen)?;
                let mut return_ty = None;
                if self.at(&TokenKind::Arrow) {
                    self.advance();
                    let type_pos = self.pos;
                    if let Ok(ty) = self.parse_type() {
                        if self.at(&TokenKind::LBrace) {
                            return_ty = Some(ty);
                            let body_stmts = self.parse_block()?;
                            let mut cparams = Vec::new();
                            for p in params {
                                cparams.push(ClosureParam {
                                    name: p.name,
                                    mutable: p.mutable,
                                    ty: Some(p.param_type),
                                });
                            }
                            return Ok(Expr::Closure {
                                params: cparams,
                                return_ty,
                                body: Box::new(Expr::Block(body_stmts, None)),
                            });
                        }
                    }
                    self.pos = type_pos;
                    let body_expr = self.parse_expr()?;
                    let mut cparams = Vec::new();
                    for p in params {
                        cparams.push(ClosureParam {
                            name: p.name,
                            mutable: p.mutable,
                            ty: Some(p.param_type),
                        });
                    }
                    return Ok(Expr::Closure {
                        params: cparams,
                        return_ty: None,
                        body: Box::new(body_expr),
                    });
                }
                let body_stmts = self.parse_block()?;
                let mut cparams = Vec::new();
                for p in params {
                    cparams.push(ClosureParam {
                        name: p.name,
                        mutable: p.mutable,
                        ty: Some(p.param_type),
                    });
                }
                Ok(Expr::Closure {
                    params: cparams,
                    return_ty,
                    body: Box::new(Expr::Block(body_stmts, None)),
                })
            }

            // closure `|params| body`
            TokenKind::Pipe => {
                self.advance();
                let mut params = Vec::new();
                while !self.at(&TokenKind::Pipe) && !self.is_eof() {
                    let mutable = self.eat_kw("mut");
                    let name = self.expect_ident()?;
                    let ty = if self.at(&TokenKind::Colon) {
                        self.advance();
                        Some(self.parse_type()?)
                    } else {
                        None
                    };
                    params.push(ClosureParam { name, mutable, ty });
                    if !self.at(&TokenKind::Comma) {
                        break;
                    }
                    self.advance();
                }
                self.expect(TokenKind::Pipe)?;
                let ret_ty = if self.at(&TokenKind::Arrow) {
                    self.advance();
                    Some(self.parse_type()?)
                } else {
                    None
                };
                let body = self.parse_expr()?;
                Ok(Expr::Closure {
                    params,
                    return_ty: ret_ty,
                    body: Box::new(body),
                })
            }

            // if expression
            TokenKind::KwIf => {
                self.advance();
                let cond = self.parse_control_expr()?;
                let body = self.parse_block()?;
                let else_e = if self.at(&TokenKind::KwElse) {
                    self.advance();
                    if self.at(&TokenKind::KwIf) {
                        Some(Box::new(self.parse_primary()?))
                    } else {
                        let block = self.parse_block()?;
                        Some(Box::new(Expr::Block(block, None)))
                    }
                } else {
                    None
                };
                Ok(Expr::IfExpr {
                    branches: vec![IfBranch {
                        condition: cond,
                        body,
                    }],
                    else_body: else_e,
                })
            }

            // loop expression
            TokenKind::KwLoop => {
                let next_pos = self.next_non_trivia_pos(self.pos + 1);
                let next_kind = self.tokens.get(next_pos).map(|t| t.kind.clone());
                if !matches!(next_kind, Some(TokenKind::LBrace)) {
                    let first = self.advance().lexeme.clone();
                    return self.parse_identifier_like_expr(first);
                }
                self.advance();
                let e = self.parse_expr()?;
                Ok(Expr::Loop(Box::new(e)))
            }

            // match expression
            TokenKind::KwMatch => {
                self.advance();
                let e = self.parse_control_expr()?;
                self.expect(TokenKind::LBrace)?;
                let arms = self.parse_match_arms()?;
                self.expect(TokenKind::RBrace)?;
                Ok(Expr::Match {
                    expr: Box::new(e),
                    arms,
                })
            }

            // async block
            TokenKind::KwAsync => {
                self.advance();
                let _ = self.eat_kw("move");
                let block = self.parse_block()?;
                Ok(Expr::AsyncBlock(block))
            }

            // identifier / path / struct literal
            TokenKind::Identifier
            | TokenKind::KwType
            | TokenKind::KwMod
            | TokenKind::KwSelf
            | TokenKind::KwSelfType
            | TokenKind::KwExport
            | TokenKind::KwCrate
            | TokenKind::KwSuper
            | TokenKind::KwWhere
            | TokenKind::KwAwait
            | TokenKind::KwStatic => {
                let first = self.advance().lexeme.clone();
                self.parse_identifier_like_expr(first)
            }

            _ => Err(
                self.error_here(
                    codes::PARSER_INVALID_EXPRESSION,
                    format!("unexpected token '{}' in expression", self.cur().lexeme),
                )
                .with_suggestion("start an expression with a literal, identifier, or '('"),
            ),
        }
    }
}

impl NeuroParser {
    fn next_non_trivia_pos(&self, mut pos: usize) -> usize {
        while let Some(tok) = self.tokens.get(pos) {
            if !matches!(
                tok.kind,
                TokenKind::Newline
                    | TokenKind::Semicolon
                    | TokenKind::Comment
                    | TokenKind::MultiLineComment
                    | TokenKind::Whitespace
            ) {
                break;
            }
            pos += 1;
        }
        pos
    }

    fn parse_identifier_like_expr(&mut self, first: String) -> Result<Expr, Diagnostic> {
        let mut parts = vec![first];
        if self.at(&TokenKind::Less) && self.can_parse_type_args_in_expr() {
            self.advance();
            while !self.at_type_arg_end() && !self.is_eof() {
                let _ = self.parse_type()?;
                if !self.at(&TokenKind::Comma) {
                    break;
                }
                self.advance();
            }
            self.consume_type_arg_end()?;
        }
        // path segments
        while self.at(&TokenKind::ColonColon) {
            self.advance();
            if self.at(&TokenKind::Less) && self.can_parse_type_args_in_expr() {
                self.advance();
                while !self.at_type_arg_end() && !self.is_eof() {
                    let _ = self.parse_type()?;
                    if !self.at(&TokenKind::Comma) {
                        break;
                    }
                    self.advance();
                }
                self.consume_type_arg_end()?;
                continue;
            }
            parts.push(self.expect_ident()?);
            if self.at(&TokenKind::Less) && self.can_parse_type_args_in_expr() {
                self.advance();
                while !self.at_type_arg_end() && !self.is_eof() {
                    let _ = self.parse_type()?;
                    if !self.at(&TokenKind::Comma) {
                        break;
                    }
                    self.advance();
                }
                self.consume_type_arg_end()?;
            }
        }
        let name = parts.last().cloned().unwrap_or_default();
        // struct literal: Name { field: … }
        // (only if next non-whitespace is `{` and then ident then `:`)
        // Also, skip if we are inside a control expression (if/while condition).
        if self.at(&TokenKind::LBrace) && !self.disallow_struct_literal {
            let is_struct = self
                .tokens
                .get(self.pos + 1)
                .map(|t| t.kind == TokenKind::RBrace)
                .unwrap_or(false)
                || (self.is_field_name_token(self.pos + 1)
                    && self
                        .tokens
                        .get(self.pos + 2)
                        .map(|t| {
                            matches!(t.kind, TokenKind::Colon | TokenKind::Comma | TokenKind::RBrace)
                        })
                        .unwrap_or(false));
            if is_struct {
                self.advance();
                let mut fields = Vec::new();
                loop {
                    self.skip_newlines();
                    if self.at(&TokenKind::RBrace) || self.is_eof() {
                        break;
                    }
                    let fn_ = self.parse_field_name()?;
                    // shorthand: `name` expands to `name: name`
                    let fv = if self.at(&TokenKind::Colon) {
                        self.advance();
                        self.parse_expr()?
                    } else {
                        Expr::Identifier(fn_.clone())
                    };
                    fields.push(FieldInit {
                        name: fn_,
                        value: fv,
                    });
                    if self.at(&TokenKind::Comma) {
                        self.advance();
                    }
                    self.skip_newlines();
                }
                self.expect(TokenKind::RBrace)?;
                return Ok(Expr::StructLiteral { name, fields });
            }
        }
        if parts.len() == 1 {
            Ok(Expr::Identifier(parts.remove(0)))
        } else {
            Ok(Expr::Path(parts))
        }
    }

    fn looks_like_named_fn_type_param(&self) -> bool {
        matches!(self.cur().kind, TokenKind::Identifier)
            && self
                .tokens
                .get(self.pos + 1)
                .map(|t| t.kind == TokenKind::Colon)
                .unwrap_or(false)
    }

    fn at_type_arg_end(&self) -> bool {
        matches!(
            self.cur().kind,
            TokenKind::Greater | TokenKind::GreaterGreater | TokenKind::GreaterGreaterEqual
        )
    }

    fn consume_type_arg_end(&mut self) -> Result<(), Diagnostic> {
        match self.cur().kind {
            TokenKind::Greater => {
                self.advance();
                Ok(())
            }
            TokenKind::GreaterGreater => {
                self.split_angle_close(TokenKind::Greater, TokenKind::Greater);
                self.advance();
                Ok(())
            }
            TokenKind::GreaterGreaterEqual => {
                self.split_angle_close(TokenKind::Greater, TokenKind::GreaterEqual);
                self.advance();
                Ok(())
            }
            _ => self.expect(TokenKind::Greater).map(|_| ()),
        }
    }

    fn can_parse_type_args_in_expr(&self) -> bool {
        if !self.at(&TokenKind::Less) {
            return false;
        }
        let mut depth: i32 = 0;
        let mut i = self.pos;
        while let Some(tok) = self.tokens.get(i) {
            match tok.kind {
                TokenKind::Less => depth += 1,
                TokenKind::Greater => {
                    depth -= 1;
                    if depth == 0 {
                        return self.type_args_followed_by_call_or_path(i + 1);
                    }
                }
                TokenKind::GreaterGreater => {
                    depth -= 2;
                    if depth <= 0 {
                        return depth == 0 && self.type_args_followed_by_call_or_path(i + 1);
                    }
                }
                TokenKind::GreaterGreaterEqual => {
                    depth -= 2;
                    if depth <= 0 {
                        return depth == 0 && self.type_args_followed_by_call_or_path(i + 1);
                    }
                }
                TokenKind::Eof
                | TokenKind::Semicolon
                | TokenKind::Newline
                | TokenKind::LBrace
                | TokenKind::RBrace => return false,
                _ => {}
            }
            i += 1;
        }
        false
    }

    fn type_args_followed_by_call_or_path(&self, pos: usize) -> bool {
        matches!(
            self.tokens.get(pos).map(|t| t.kind.clone()),
            Some(TokenKind::ColonColon)
                | Some(TokenKind::LParen)
                | Some(TokenKind::LBrace)
                | Some(TokenKind::Dot)
                | Some(TokenKind::LBracket)
        )
    }

    fn split_angle_close(&mut self, first: TokenKind, second: TokenKind) {
        let span = self.cur().span;
        self.tokens[self.pos].kind = first.clone();
        self.tokens[self.pos].lexeme = first.display().to_string();
        self.tokens.insert(
            self.pos + 1,
            Token::new(second.clone(), second.display().to_string(), span),
        );
    }

    fn is_field_name_token(&self, pos: usize) -> bool {
        self.tokens
            .get(pos)
            .map(|t| {
                t.kind == TokenKind::Identifier
                    || t.kind.is_keyword()
                    || t.kind == TokenKind::String
                    || t.kind == TokenKind::Integer
                    || t.kind == TokenKind::Float
            })
            .unwrap_or(false)
    }

    // ── Protocol DSL Parsing ─────────────────────────────────────────────────

    fn parse_protocol_decl(&mut self) -> Result<ProtocolDecl, Diagnostic> {
        let start = self.span_at();
        self.expect(TokenKind::KwSecure)?;
        self.expect(TokenKind::KwProtocol)?;
        let name = self.expect_ident()?;
        let version = if self.at(&TokenKind::Identifier) && self.cur().lexeme.starts_with('v') {
             Some(self.advance().lexeme.clone())
        } else {
             None
        };
        self.expect(TokenKind::LBrace)?;
        
        let mut roles = Vec::new();
        let mut primitives = Vec::new();
        let mut properties = Vec::new();
        let mut transport = ProtocolTransport { framing: None, versioning: None };
        let mut handshake = None;
        let mut session = None;
        let mut policies = Vec::new();

        while !self.at(&TokenKind::RBrace) && !self.is_eof() {
            self.skip_newlines();
            if self.at(&TokenKind::RBrace) { break; }
            
            let ident = self.expect_ident()?;
            match ident.as_str() {
                "roles" => {
                    self.expect(TokenKind::Colon)?;
                    while !self.at(&TokenKind::Newline) && !self.at(&TokenKind::Semicolon) && !self.at(&TokenKind::RBrace) {
                        roles.push(self.expect_ident()?);
                        if self.at(&TokenKind::Comma) { self.advance(); } else { break; }
                    }
                }
                "primitives" => {
                    self.expect(TokenKind::LBrace)?;
                    while !self.at(&TokenKind::RBrace) && !self.is_eof() {
                        self.skip_newlines();
                        if self.at(&TokenKind::RBrace) { break; }
                        let pkind = self.expect_ident()?;
                        self.expect(TokenKind::Colon)?;
                        let algo = self.expect_ident()?;
                        primitives.push(ProtocolPrimitive { kind: pkind, algo });
                        if self.at(&TokenKind::Comma) { self.advance(); }
                        self.skip_newlines();
                    }
                    self.expect(TokenKind::RBrace)?;
                }
                "properties" => {
                    self.expect(TokenKind::LBrace)?;
                    while !self.at(&TokenKind::RBrace) && !self.is_eof() {
                        self.skip_newlines();
                        if self.at(&TokenKind::RBrace) { break; }
                        let pname = self.expect_ident()?;
                        self.expect(TokenKind::Equal)?;
                        let pval = self.parse_expr()?;
                        properties.push(ProtocolProperty { name: pname, value: pval });
                        if self.at(&TokenKind::Comma) { self.advance(); }
                        self.skip_newlines();
                    }
                    self.expect(TokenKind::RBrace)?;
                }
                "transport" => {
                    self.expect(TokenKind::LBrace)?;
                    while !self.at(&TokenKind::RBrace) && !self.is_eof() {
                        self.skip_newlines();
                        if self.at(&TokenKind::RBrace) { break; }
                        let tkind = self.expect_ident()?;
                        self.expect(TokenKind::Equal)?;
                        let tval = self.expect_ident()?;
                        match tkind.as_str() {
                            "framing" => transport.framing = Some(tval),
                            "versioning" => transport.versioning = Some(tval),
                            _ => {}
                        }
                        if self.at(&TokenKind::Comma) { self.advance(); }
                        self.skip_newlines();
                    }
                    self.expect(TokenKind::RBrace)?;
                }
                "handshake" => {
                    handshake = Some(self.parse_handshake_def()?);
                }
                "session" => {
                    session = Some(self.parse_session_def()?);
                }
                "policy" => {
                    self.expect(TokenKind::LBrace)?;
                    while !self.at(&TokenKind::RBrace) && !self.is_eof() {
                        self.skip_newlines();
                        if self.at(&TokenKind::RBrace) { break; }
                        let pname = self.expect_ident()?;
                        self.expect(TokenKind::Equal)?;
                        let pval = self.parse_expr()?;
                        policies.push(ProtocolPolicy { name: pname, value: pval });
                        if self.at(&TokenKind::Comma) { self.advance(); }
                        self.skip_newlines();
                    }
                    self.expect(TokenKind::RBrace)?;
                }
                _ => {
                    return Err(self.error_here(codes::PARSER_UNEXPECTED_TOKEN, format!("unexpected protocol item '{}'", ident)));
                }
            }
            self.skip_newlines();
        }
        self.expect(TokenKind::RBrace)?;

        Ok(ProtocolDecl {
            name,
            version,
            roles,
            primitives,
            properties,
            transport,
            handshake,
            session,
            policies,
            span: Span::new(start.start, self.span_at().start),
        })
    }

    fn parse_handshake_def(&mut self) -> Result<HandshakeDef, Diagnostic> {
        self.expect(TokenKind::LBrace)?;
        let mut steps = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.is_eof() {
            self.skip_newlines();
            if self.at(&TokenKind::RBrace) { break; }

            if self.eat_contextual_kw("derive") {
                self.expect(TokenKind::LBrace)?;
                let mut assignments = Vec::new();
                while !self.at(&TokenKind::RBrace) && !self.is_eof() {
                    self.skip_newlines();
                    if self.at(&TokenKind::RBrace) { break; }
                    let name = self.expect_ident()?;
                    self.expect(TokenKind::Equal)?;
                    let value = self.parse_expr()?;
                    assignments.push(HandshakeAssignment { name, value });
                    self.skip_newlines();
                }
                self.expect(TokenKind::RBrace)?;
                steps.push(HandshakeStep::Derive { assignments });
            } else if self.eat_contextual_kw("finish") {
                self.expect(TokenKind::LBrace)?;
                let mut actions = Vec::new();
                while !self.at(&TokenKind::RBrace) && !self.is_eof() {
                    self.skip_newlines();
                    if self.at(&TokenKind::RBrace) { break; }
                    actions.push(self.expect_ident()?);
                    self.skip_newlines();
                }
                self.expect(TokenKind::RBrace)?;
                steps.push(HandshakeStep::Finish { actions });
            } else if self.at_kw("derive") {
                 // handle KwSecure/KwProtocol style if they were keywords, 
                 // but here they are contextual or keywords.
                 // The earlier eat_contextual_kw("derive") handled it.
                 // Let's make sure we handle the case where it might be a keyword.
                 self.advance();
                 self.expect(TokenKind::LBrace)?;
                 let mut assignments = Vec::new();
                 while !self.at(&TokenKind::RBrace) && !self.is_eof() {
                     self.skip_newlines();
                     if self.at(&TokenKind::RBrace) { break; }
                     let name = self.expect_ident()?;
                     self.expect(TokenKind::Equal)?;
                     let value = self.parse_expr()?;
                     assignments.push(HandshakeAssignment { name, value });
                     self.skip_newlines();
                 }
                 self.expect(TokenKind::RBrace)?;
                 steps.push(HandshakeStep::Derive { assignments });
            } else {
                // message: Client -> Server: name { ... }
                let from = self.expect_ident()?;
                self.expect(TokenKind::Arrow)?;
                let to = self.expect_ident()?;
                self.expect(TokenKind::Colon)?;
                let name = self.expect_ident()?;
                self.expect(TokenKind::LBrace)?;
                let mut fields = Vec::new();
                while !self.at(&TokenKind::RBrace) && !self.is_eof() {
                    self.skip_newlines();
                    if self.at(&TokenKind::RBrace) { break; }
                    let fname = self.expect_ident()?;
                    self.expect(TokenKind::Equal)?;
                    let fval = self.parse_expr()?;
                    fields.push(HandshakeField { name: fname, value: fval });
                    self.skip_newlines();
                }
                self.expect(TokenKind::RBrace)?;
                steps.push(HandshakeStep::Message { from, to, name, fields });
            }
            self.skip_newlines();
        }
        self.expect(TokenKind::RBrace)?;
        Ok(HandshakeDef { steps })
    }

    fn parse_session_def(&mut self) -> Result<SessionDef, Diagnostic> {
        self.expect(TokenKind::LBrace)?;
        let mut body = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.is_eof() {
             self.skip_newlines();
             if self.at(&TokenKind::RBrace) { break; }
             if self.at(&TokenKind::KwFn) {
                 // treat fn inside session as a special statement or just parse it
                 let f = self.parse_fn_decl()?;
                 body.push(Stmt::Expr(Expr::Identifier(format!("__session_method_{}", f.name)))); 
                 // For now, I'll just push a dummy expression or extend SessionDef
                 // Actually, let's keep it simple: SessionDef has Vec<Stmt>.
                 // I'll wrap the function in a Block or similar if needed.
             } else {
                 body.push(self.parse_stmt()?);
             }
             self.skip_newlines();
        }
        self.expect(TokenKind::RBrace)?;
        Ok(SessionDef { body })
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::diagnostics::codes;
    use crate::core::lexer::lexer::Lexer;
    use crate::core::parser::grammar_engine::GrammarEngine;
    use crate::core::registry::language_registry::LanguageRegistry;

    fn parse_with_errors(source: &str) -> ParserErrors {
        let registry = LanguageRegistry::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/registry/language.json"
        ))
        .unwrap_or_default();
        let grammar = GrammarEngine::from_registry(&registry);
        let mut lexer = Lexer::from_source(source.to_string());
        let tokens = lexer.tokenize().expect("lexer must succeed for tests");
        let mut parser = NeuroParser::new(grammar);
        match parser.parse(&tokens) {
            Ok(_) => panic!("expected parser errors"),
            Err(errs) => errs,
        }
    }

    #[test]
    fn test_parser_multi_error_aggregation() {
        let source = "fn a( { let x = 1; }\nfn b() { let y = ; }\n";
        let errs = parse_with_errors(source);
        assert!(
            errs.errors.len() >= 2,
            "expected multiple errors, got {}",
            errs.errors.len()
        );
    }

    #[test]
    fn test_parser_error_span_accuracy() {
        let source = "fn main() {\n    let x = ;\n}\n";
        let errs = parse_with_errors(source);
        let diag = errs
            .errors
            .iter()
            .find(|d| d.code == codes::PARSER_INVALID_EXPRESSION)
            .expect("expected invalid expression diagnostic");
        let span = diag.span.expect("expected span for diagnostic");
        assert_eq!(span.start.line, 2);
        assert_eq!(span.start.column, 13);
    }
}

