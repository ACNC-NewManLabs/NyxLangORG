//! Nyx Language Token Definitions
//!
//! This module defines all token types for the Nyx lexer.

use serde::{Deserialize, Serialize};

pub use nyx_diagnostics::{Position, Span};

// ─── Token ───────────────────────────────────────────────────────────────────

/// A single token emitted by the lexer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Token {
    pub kind: TokenKind,
    /// The exact source text of this token.
    pub lexeme: String,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, lexeme: String, span: Span) -> Self {
        Self { kind, lexeme, span }
    }

    pub fn eof(pos: Position) -> Self {
        Self {
            kind: TokenKind::Eof,
            lexeme: String::new(),
            span: Span {
                start: pos,
                end: pos,
            },
        }
    }
}

// ─── TokenKind ───────────────────────────────────────────────────────────────

/// Every distinct kind of token in Nyx.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TokenKind {
    // ── Literals ──────────────────────────────────────────────────────────
    Identifier,
    Integer,
    Float,
    String,
    Char,
    Boolean,
    Null,
    /// css`...` template literal — lexeme is the raw CSS text between backticks.
    /// The token lexeme stores the raw CSS content; the VM/interpreter is
    /// responsible for parsing it into Map<string, string> at evaluation time.
    CssLiteral,

    // ── Keywords ──────────────────────────────────────────────────────────
    KwFn,
    KwLet,
    KwMut,
    KwReturn,
    KwIf,
    KwElse,
    KwWhile,
    KwFor,
    KwLoop,
    KwMatch,
    KwBreak,
    KwContinue,
    KwStruct,
    KwUnion,
    KwEnum,
    KwTrait,
    KwImpl,
    KwType,
    KwMod,
    KwUse,
    KwExport,
    KwPub,
    KwCrate,
    KwSuper,
    KwSelf,
    KwSelfType, // Self
    KwTrue,
    KwFalse,
    KwNull,
    KwAsync,
    KwAwait,
    KwMove,
    KwWhere,
    KwConst,
    KwStatic,
    KwUnsafe,
    KwExtern,
    KwAsm,
    KwIn,
    KwAs,
    KwAnd, // `and` — logical AND alias
    KwOr,  // `or`  — logical OR  alias
    KwNot, // `not` — logical NOT alias
    KwSecure,   // `secure`
    KwProtocol, // `protocol`
    KwYield,    // `yield`

    // ── Arithmetic operators ───────────────────────────────────────────────
    Plus,
    Minus,
    Star,
    Slash,
    Percent,

    // ── Comparison operators ───────────────────────────────────────────────
    Equal,        // =
    EqEq,         // ==
    Bang,         // !
    BangEqual,    // !=
    Less,         // <
    LessEqual,    // <=
    Greater,      // >
    GreaterEqual, // >=

    // ── Bitwise / logical ─────────────────────────────────────────────────
    Ampersand,          // &
    AmpersandAmpersand, // &&
    Pipe,               // |
    PipePipe,           // ||
    Caret,              // ^
    Tilde,              // ~
    LessLess,           // <<
    GreaterGreater,     // >>

    // ── Compound assignment ────────────────────────────────────────────────
    PlusEqual,
    MinusEqual,
    StarEqual,
    SlashEqual,
    PercentEqual,
    AmpersandEqual,
    PipeEqual,
    CaretEqual,
    LessLessEqual,
    GreaterGreaterEqual,

    // ── Delimiters ────────────────────────────────────────────────────────
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Colon,
    ColonColon, // ::
    Dot,
    DotDot,    // ..
    DotDotEq,  // ..=
    DotDotDot, // ... (variadic)
    Semicolon,
    Question,
    QuestionQuestion, // ??
    At, // @  (pattern binding  `name @ pattern`)

    // ── Compound tokens ───────────────────────────────────────────────────
    Arrow,     // ->
    FatArrow,  // =>
    ThinArrow, // <-

    // ── Trivia (skipped by parser) ────────────────────────────────────────
    Comment,
    MultiLineComment,
    Newline,
    Whitespace,

    // ── Sentinel ──────────────────────────────────────────────────────────
    Eof,
    Error,
}

impl TokenKind {
    /// Returns true for any keyword token.
    pub fn is_keyword(&self) -> bool {
        matches!(
            self,
            TokenKind::KwFn
                | TokenKind::KwLet
                | TokenKind::KwMut
                | TokenKind::KwReturn
                | TokenKind::KwIf
                | TokenKind::KwElse
                | TokenKind::KwWhile
                | TokenKind::KwFor
                | TokenKind::KwLoop
                | TokenKind::KwMatch
                | TokenKind::KwBreak
                | TokenKind::KwContinue
                | TokenKind::KwStruct
                | TokenKind::KwUnion
                | TokenKind::KwEnum
                | TokenKind::KwTrait
                | TokenKind::KwImpl
                | TokenKind::KwType
                | TokenKind::KwMod
                | TokenKind::KwUse
                | TokenKind::KwExport
                | TokenKind::KwPub
                | TokenKind::KwCrate
                | TokenKind::KwSuper
                | TokenKind::KwSelf
                | TokenKind::KwSelfType
                | TokenKind::KwTrue
                | TokenKind::KwFalse
                | TokenKind::KwNull
                | TokenKind::KwAsync
                | TokenKind::KwAwait
                | TokenKind::KwMove
                | TokenKind::KwWhere
                | TokenKind::KwConst
                | TokenKind::KwStatic
                | TokenKind::KwUnsafe
                | TokenKind::KwExtern
                | TokenKind::KwAsm
                | TokenKind::KwIn
                | TokenKind::KwAs
                | TokenKind::KwAnd
                | TokenKind::KwOr
                | TokenKind::KwNot
                | TokenKind::KwSecure
                | TokenKind::KwProtocol
                | TokenKind::KwYield
        )
    }

    /// Human-readable name for error messages.
    pub fn display(&self) -> &str {
        match self {
            TokenKind::Identifier => "identifier",
            TokenKind::Integer => "integer literal",
            TokenKind::Float => "float literal",
            TokenKind::String => "string literal",
            TokenKind::Char => "char literal",
            TokenKind::Boolean => "boolean",
            TokenKind::Null => "null",
            TokenKind::CssLiteral => "css literal",
            TokenKind::KwFn => "fn",
            TokenKind::KwLet => "let",
            TokenKind::KwMut => "mut",
            TokenKind::KwReturn => "return",
            TokenKind::KwIf => "if",
            TokenKind::KwElse => "else",
            TokenKind::KwWhile => "while",
            TokenKind::KwFor => "for",
            TokenKind::KwLoop => "loop",
            TokenKind::KwMatch => "match",
            TokenKind::KwBreak => "break",
            TokenKind::KwContinue => "continue",
            TokenKind::KwStruct => "struct",
            TokenKind::KwUnion => "union",
            TokenKind::KwEnum => "enum",
            TokenKind::KwTrait => "trait",
            TokenKind::KwImpl => "impl",
            TokenKind::KwType => "type",
            TokenKind::KwMod => "mod",
            TokenKind::KwUse => "use",
            TokenKind::KwExport => "export",
            TokenKind::KwPub => "pub",
            TokenKind::KwCrate => "crate",
            TokenKind::KwSuper => "super",
            TokenKind::KwSelf => "self",
            TokenKind::KwSelfType => "Self",
            TokenKind::KwTrue => "true",
            TokenKind::KwFalse => "false",
            TokenKind::KwNull => "null",
            TokenKind::KwAsync => "async",
            TokenKind::KwAwait => "await",
            TokenKind::KwMove => "move",
            TokenKind::KwWhere => "where",
            TokenKind::KwConst => "const",
            TokenKind::KwStatic => "static",
            TokenKind::KwUnsafe => "unsafe",
            TokenKind::KwExtern => "extern",
            TokenKind::KwAsm => "asm",
            TokenKind::KwIn => "in",
            TokenKind::KwAs => "as",
            TokenKind::KwAnd => "and",
            TokenKind::KwOr => "or",
            TokenKind::KwNot => "not",
            TokenKind::KwSecure => "secure",
            TokenKind::KwProtocol => "protocol",
            TokenKind::KwYield => "yield",
            TokenKind::Plus => "+",
            TokenKind::Minus => "-",
            TokenKind::Star => "*",
            TokenKind::Slash => "/",
            TokenKind::Percent => "%",
            TokenKind::Equal => "=",
            TokenKind::EqEq => "==",
            TokenKind::Bang => "!",
            TokenKind::BangEqual => "!=",
            TokenKind::Less => "<",
            TokenKind::LessEqual => "<=",
            TokenKind::Greater => ">",
            TokenKind::GreaterEqual => ">=",
            TokenKind::Ampersand => "&",
            TokenKind::AmpersandAmpersand => "&&",
            TokenKind::Pipe => "|",
            TokenKind::PipePipe => "||",
            TokenKind::Caret => "^",
            TokenKind::Tilde => "~",
            TokenKind::LessLess => "<<",
            TokenKind::GreaterGreater => ">>",
            TokenKind::PlusEqual => "+=",
            TokenKind::MinusEqual => "-=",
            TokenKind::StarEqual => "*=",
            TokenKind::SlashEqual => "/=",
            TokenKind::PercentEqual => "%=",
            TokenKind::AmpersandEqual => "&=",
            TokenKind::PipeEqual => "|=",
            TokenKind::CaretEqual => "^=",
            TokenKind::LessLessEqual => "<<=",
            TokenKind::GreaterGreaterEqual => ">>=",
            TokenKind::LParen => "(",
            TokenKind::RParen => ")",
            TokenKind::LBrace => "{",
            TokenKind::RBrace => "}",
            TokenKind::LBracket => "[",
            TokenKind::RBracket => "]",
            TokenKind::Comma => ",",
            TokenKind::Colon => ":",
            TokenKind::ColonColon => "::",
            TokenKind::Dot => ".",
            TokenKind::DotDot => "..",
            TokenKind::DotDotEq => "..=",
            TokenKind::DotDotDot => "...",
            TokenKind::Semicolon => ";",
            TokenKind::Question => "?",
            TokenKind::QuestionQuestion => "??",
            TokenKind::At => "@",
            TokenKind::Arrow => "->",
            TokenKind::FatArrow => "=>",
            TokenKind::ThinArrow => "<-",
            TokenKind::Comment => "comment",
            TokenKind::MultiLineComment => "block comment",
            TokenKind::Newline => "newline",
            TokenKind::Whitespace => "whitespace",
            TokenKind::Eof => "end of file",
            TokenKind::Error => "<error>",
        }
    }
}

// ─── Keyword table ────────────────────────────────────────────────────────────

/// Static keyword → TokenKind mapping.  Ordered by length (longer first) so that
/// `not` does not shadow `no`, etc.
pub static KEYWORDS: &[(&str, TokenKind)] = &[
    ("fn", TokenKind::KwFn),
    ("let", TokenKind::KwLet),
    ("mut", TokenKind::KwMut),
    ("return", TokenKind::KwReturn),
    ("if", TokenKind::KwIf),
    ("else", TokenKind::KwElse),
    ("while", TokenKind::KwWhile),
    ("for", TokenKind::KwFor),
    ("loop", TokenKind::KwLoop),
    ("match", TokenKind::KwMatch),
    ("break", TokenKind::KwBreak),
    ("continue", TokenKind::KwContinue),
    ("struct", TokenKind::KwStruct),
    ("union", TokenKind::KwUnion),
    ("enum", TokenKind::KwEnum),
    ("trait", TokenKind::KwTrait),
    ("impl", TokenKind::KwImpl),
    ("type", TokenKind::KwType),
    ("mod", TokenKind::KwMod),
    ("use", TokenKind::KwUse),
    ("export", TokenKind::KwExport),
    ("pub", TokenKind::KwPub),
    ("crate", TokenKind::KwCrate),
    ("super", TokenKind::KwSuper),
    ("self", TokenKind::KwSelf),
    ("Self", TokenKind::KwSelfType),
    ("true", TokenKind::KwTrue),
    ("false", TokenKind::KwFalse),
    ("null", TokenKind::KwNull),
    ("none", TokenKind::KwNull),
    ("async", TokenKind::KwAsync),
    ("await", TokenKind::KwAwait),
    ("move", TokenKind::KwMove),
    ("where", TokenKind::KwWhere),
    ("const", TokenKind::KwConst),
    ("static", TokenKind::KwStatic),
    ("unsafe", TokenKind::KwUnsafe),
    ("extern", TokenKind::KwExtern),
    ("asm", TokenKind::KwAsm),
    ("in", TokenKind::KwIn),
    ("as", TokenKind::KwAs),
    // English boolean-logic aliases
    ("and", TokenKind::KwAnd),
    ("or", TokenKind::KwOr),
    ("not", TokenKind::KwNot),
    ("secure", TokenKind::KwSecure),
    ("protocol", TokenKind::KwProtocol),
    ("yield", TokenKind::KwYield),
];

/// Look up a lexed identifier string and return its keyword kind, or
/// `TokenKind::Identifier` if it is not a reserved word.
pub fn lookup_keyword(ident: &str) -> TokenKind {
    KEYWORDS
        .iter()
        .find(|(kw, _)| *kw == ident)
        .map(|(_, kind)| kind.clone())
        .unwrap_or(TokenKind::Identifier)
}

// ─── Compatibility shims ─────────────────────────────────────────────────────

/// Kept for registry-layer compatibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxPatterns {
    pub function: String,
    pub let_stmt: String,
    pub return_stmt: String,
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
