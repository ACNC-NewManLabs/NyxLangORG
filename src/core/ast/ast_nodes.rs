//! Nyx AST Node Definitions
//!
//! Strongly-typed, immutable-after-construction AST nodes for every Nyx
//! language construct.  The root type is [`Program`].

use crate::core::lexer::token::Span;
use serde::Serialize;

// ─── Program ─────────────────────────────────────────────────────────────────

/// Root of the Nyx AST – the compiled unit of a single source file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Program {
    pub items: Vec<Item>,
}

// Backward-compat helpers (used by older tests and IR builder)
impl Program {
    /// Collect all top-level function declarations.
    pub fn functions(&self) -> Vec<&FunctionDecl> {
        self.items
            .iter()
            .filter_map(|i| {
                if let ItemKind::Function(f) = &i.kind {
                    Some(f)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Collect all top-level struct declarations.
    pub fn structs(&self) -> Vec<&StructDecl> {
        self.items
            .iter()
            .filter_map(|i| {
                if let ItemKind::Struct(s) = &i.kind {
                    Some(s)
                } else {
                    None
                }
            })
            .collect()
    }
}

// ─── Visibility ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Visibility {
    Inherited,   // no keyword
    Public,      // pub
    PublicCrate, // pub(crate)
    PublicSuper, // pub(super)
}

// ─── Attributes ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Attribute {
    pub name: String,
    pub args: Option<String>,
    pub span: Span,
}

// ─── Top-level items ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Item {
    pub attributes: Vec<Attribute>,
    pub vis: Visibility,
    pub kind: ItemKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ItemKind {
    Function(FunctionDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Trait(TraitDecl),
    Impl(ImplBlock),
    TypeAlias(TypeAlias),
    Const(ConstDecl),
    Static(StaticDecl),
    Module(ModuleDecl),
    ModuleValue(ModuleValueDecl),
    Use(UseDecl),
    Export(ExportDecl),
    Protocol(ProtocolDecl),
    Stmt(Stmt),
}

// ─── Function ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FunctionDecl {
    pub name: String,
    pub is_async: bool,
    pub is_extern: bool,
    pub extern_abi: Option<String>,
    pub generics: Vec<GenericParam>,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    pub where_clauses: Vec<WherePredicate>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Param {
    pub name: String,
    pub mutable: bool,
    pub param_type: Type,
    pub default_value: Option<Expr>,
    pub is_variadic: bool,
}

// ─── Struct ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StructDecl {
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub where_clauses: Vec<WherePredicate>,
    pub fields: Vec<StructField>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StructField {
    pub vis: Visibility,
    pub name: String,
    pub field_type: Type,
    pub default: Option<Expr>,
}

/// Keep the old name `Field` for backward compat.
pub type Field = StructField;

// ─── Enum ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EnumDecl {
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub where_clauses: Vec<WherePredicate>,
    pub variants: Vec<EnumVariant>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum EnumVariant {
    Unit(String),                     // Foo
    Tuple(String, Vec<Type>),         // Foo(T, U)
    Struct(String, Vec<StructField>), // Foo { x: T }
}

impl EnumVariant {
    pub fn name(&self) -> &str {
        match self {
            EnumVariant::Unit(n) | EnumVariant::Tuple(n, _) | EnumVariant::Struct(n, _) => n,
        }
    }
}

// ─── Trait ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TraitDecl {
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub super_traits: Vec<TypeBound>,
    pub where_clauses: Vec<WherePredicate>,
    pub items: Vec<TraitItem>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum TraitItem {
    Method(FunctionDecl),
    Type(String), // associated type
    Const(ConstDecl),
}

// ─── Impl block ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ImplBlock {
    pub generics: Vec<GenericParam>,
    pub trait_name: Option<TypePath>, // `for TraitName`
    pub self_type: Type,
    pub items: Vec<ImplItem>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ImplItem {
    Method(FunctionDecl),
    TypeAlias(TypeAlias),
    Const(ConstDecl),
}

// ─── Type alias ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TypeAlias {
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub where_clauses: Vec<WherePredicate>,
    pub ty: Type,
    pub span: Span,
}

// ─── Const / static ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConstDecl {
    pub name: String,
    pub ty: Type,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StaticDecl {
    pub name: String,
    pub mutable: bool,
    pub ty: Type,
    pub value: Expr,
    pub span: Span,
}

// ─── Module / use / export ────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ModuleDecl {
    /// `mod name;` — external module file
    External(String),
    /// `mod name { … }` — inline module
    Inline { name: String, items: Vec<Item> },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModuleValueDecl {
    pub name: String,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UseDecl {
    pub tree: UseTree,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum UseTree {
    Path {
        segment: String,
        child: Box<UseTree>,
    },
    Glob,
    Group(Vec<UseTree>),
    Name {
        name: String,
        alias: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExportDecl {
    pub items: Vec<ExportItem>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExportItem {
    pub name: String,
    pub alias: Option<String>,
}

// ─── Generics ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GenericParam {
    pub name: String,
    pub bounds: Vec<TypeBound>,
    pub default: Option<Type>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WherePredicate {
    pub ty: TypePath,
    pub bounds: Vec<TypeBound>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TypeBound {
    pub path: TypePath,
}

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Type {
    /// A path optionally with generic args: `Foo`, `List<i32>`, `a::b::C<T>`
    Named(TypePath),
    /// Shared reference: `&T`
    Reference(Box<Type>),
    /// Mutable reference: `&mut T`
    MutReference(Box<Type>),
    /// Raw pointer: `*const T` or `*mut T`
    Pointer { mutable: bool, to: Box<Type> },
    /// `[T]` — slice / array
    Array(Box<Type>),
    /// `(A, B, C)` — tuple
    Tuple(Vec<Type>),
    /// `fn(A, B) -> C`
    Function(Vec<Type>, Box<Type>),
    /// `T?` — nullable type
    Nullable(Box<Type>),
    /// `_` — inference placeholder
    Infer,
    /// `{ field: Type, ... }` — record type
    Record(Vec<RecordField>),
    /// `A | B | C` — union type
    Union(Vec<Type>),
    /// Literal types: `"foo"`, `123`, `true`
    Literal(TypeLiteral),
}

impl Type {
    /// Construct a simple named type with no generics.
    pub fn simple(name: impl Into<String>) -> Self {
        Type::Named(TypePath {
            segments: vec![TypeSegment {
                name: name.into(),
                args: vec![],
            }],
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TypePath {
    pub segments: Vec<TypeSegment>,
}

impl TypePath {
    pub fn single(name: impl Into<String>) -> Self {
        TypePath {
            segments: vec![TypeSegment {
                name: name.into(),
                args: vec![],
            }],
        }
    }

    pub fn last_name(&self) -> &str {
        self.segments.last().map(|s| s.name.as_str()).unwrap_or("")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TypeSegment {
    pub name: String,
    pub args: Vec<Type>, // generic arguments
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecordField {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum TypeLiteral {
    Int(String),
    Float(String),
    String(String),
    Bool(bool),
}

// ─── Statements ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Stmt {
    /// `let [mut] name [: Type] = expr`
    Let {
        mutable: bool,
        name: String,
        var_type: Option<Type>,
        expr: Expr,
        span: Span,
    },
    /// Assignment: `lhs = rhs`
    Assign {
        target: Expr,
        value: Expr,
        span: Span,
    },
    /// Compound assignment: `lhs op= rhs`
    CompoundAssign {
        target: Expr,
        op: String,
        value: Expr,
        span: Span,
    },
    /// `return [expr]`
    Return { expr: Option<Expr>, span: Span },
    /// `if expr { … } [else if … ] [else { … }]`
    If {
        branches: Vec<IfBranch>,
        else_body: Option<Vec<Stmt>>,
        span: Span,
    },
    /// `while expr { … }`
    While {
        condition: Box<Expr>,
        body: Vec<Stmt>,
        span: Span,
    },
    /// `for var in expr { … }`
    ForIn {
        var: String,
        iter: Box<Expr>,
        body: Vec<Stmt>,
        span: Span,
    },
    /// `loop { … }`
    Loop { body: Vec<Stmt>, span: Span },
    /// `match expr { … }`
    Match {
        expr: Box<Expr>,
        arms: Vec<MatchArm>,
        span: Span,
    },
    /// `break [label]`
    Break { label: Option<String>, span: Span },
    /// `continue [label]`
    Continue { label: Option<String>, span: Span },
    /// `unsafe { … }`
    Unsafe { body: Vec<Stmt>, span: Span },
    /// `asm("...") : [...] : [...]`
    InlineAsm {
        code: String,
        outputs: Vec<AsmOperand>,
        inputs: Vec<AsmOperand>,
        span: Span,
    },
    /// Expression used as statement
    Expr(Expr),
    /// `defer stmt`
    Defer { stmt: Box<Stmt>, span: Span },
    /// `print(…)` — special built-in kept for backward compat
    Print { expr: Expr },
    /// `yield expr`
    Yield { expr: Option<Expr>, span: Span },
}

impl Stmt {
    pub fn span(&self) -> &Span {
        match self {
            Stmt::Let { span, .. } => span,
            Stmt::Assign { span, .. } => span,
            Stmt::CompoundAssign { span, .. } => span,
            Stmt::Expr(expr) => expr.span(),
            Stmt::If { span, .. } => span,
            Stmt::While { span, .. } => span,
            Stmt::ForIn { span, .. } => span,
            Stmt::Loop { span, .. } => span,
            Stmt::Match { span, .. } => span,
            Stmt::Return { span, .. } => span,
            Stmt::Break { span, .. } => span,
            Stmt::Continue { span, .. } => span,
            Stmt::Unsafe { span, .. } => span,
            Stmt::InlineAsm { span, .. } => span,
            Stmt::Yield { span, .. } => span,
            Stmt::Defer { span, .. } => span,
            Stmt::Print { .. } => &Span {
                start: crate::core::lexer::token::Position {
                    line: 0,
                    column: 0,
                    offset: 0,
                },
                end: crate::core::lexer::token::Position {
                    line: 0,
                    column: 0,
                    offset: 0,
                },
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AsmOperand {
    pub expr: Expr,
    pub reg: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IfBranch {
    pub condition: Expr,
    pub body: Vec<Stmt>,
}

// ─── Expressions ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Expr {
    // Literals
    IntLiteral {
        value: i64,
        span: Span,
    },
    FloatLiteral {
        value: f64,
        span: Span,
    },
    StringLiteral {
        value: String,
        span: Span,
    },
    CharLiteral {
        value: char,
        span: Span,
    },
    BoolLiteral {
        value: bool,
        span: Span,
    },
    NullLiteral {
        span: Span,
    },

    // Composite literals
    ArrayLiteral {
        elements: Vec<Expr>,
        span: Span,
    },
    ArrayRepeat {
        value: Box<Expr>,
        len: Box<Expr>,
        span: Span,
    },
    TupleLiteral {
        elements: Vec<Expr>,
        span: Span,
    },
    BigIntLiteral {
        value: String,
        span: Span,
    },

    // Named reference
    Identifier {
        name: String,
        span: Span,
    },
    Path {
        segments: Vec<String>,
        span: Span,
    },

    // Operators
    Binary {
        left: Box<Expr>,
        op: String,
        right: Box<Expr>,
        span: Span,
    },
    Unary {
        op: String,
        right: Box<Expr>,
        span: Span,
    },

    // Postfix
    FieldAccess {
        object: Box<Expr>,
        field: String,
        span: Span,
    },
    MethodCall {
        receiver: Box<Expr>,
        method: String,
        args: Vec<Expr>,
        span: Span,
    },
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    Slice {
        object: Box<Expr>,
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
        span: Span,
    },
    TryOp {
        expr: Box<Expr>,
        span: Span,
    },

    // Calls
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },

    // Struct literal
    StructLiteral {
        name: String,
        fields: Vec<FieldInit>,
        span: Span,
    },

    // Block struct literal
    BlockLiteral {
        items: Vec<BlockItem>,
        span: Span,
    },

    // Type cast
    Cast {
        expr: Box<Expr>,
        ty: Type,
        span: Span,
    },

    // Async
    Await {
        expr: Box<Expr>,
        span: Span,
    },

    // Range
    Range {
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
        inclusive: bool,
        span: Span,
    },

    // Closure
    Closure {
        params: Vec<ClosureParam>,
        return_ty: Option<Type>,
        body: Box<Expr>,
        span: Span,
    },

    // Reference
    Reference {
        mutable: bool,
        expr: Box<Expr>,
        span: Span,
    },

    // Dereference
    Deref {
        expr: Box<Expr>,
        span: Span,
    },

    // Move
    Move {
        expr: Box<Expr>,
        span: Span,
    },

    // Block expression
    Block {
        stmts: Vec<Stmt>,
        tail_expr: Option<Box<Expr>>,
        span: Span,
    },

    // If-else
    IfExpr {
        branches: Vec<IfBranch>,
        else_body: Option<Box<Expr>>,
        span: Span,
    },

    // Ternary
    Ternary {
        condition: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Box<Expr>,
        span: Span,
    },

    // Match expression
    Match {
        expr: Box<Expr>,
        arms: Vec<MatchArm>,
        span: Span,
    },

    // Async block
    AsyncBlock {
        body: Vec<Stmt>,
        span: Span,
    },

    // Css template literal
    CssLiteral {
        value: String,
        span: Span,
    },

    /// `loop expr` as expression
    Loop {
        expr: Box<Expr>,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> &Span {
        match self {
            Expr::IntLiteral { span, .. } => span,
            Expr::FloatLiteral { span, .. } => span,
            Expr::StringLiteral { span, .. } => span,
            Expr::CharLiteral { span, .. } => span,
            Expr::BoolLiteral { span, .. } => span,
            Expr::NullLiteral { span } => span,
            Expr::ArrayLiteral { span, .. } => span,
            Expr::ArrayRepeat { span, .. } => span,
            Expr::TupleLiteral { span, .. } => span,
            Expr::BigIntLiteral { span, .. } => span,
            Expr::Identifier { span, .. } => span,
            Expr::Path { span, .. } => span,
            Expr::Binary { span, .. } => span,
            Expr::Unary { span, .. } => span,
            Expr::FieldAccess { span, .. } => span,
            Expr::MethodCall { span, .. } => span,
            Expr::Index { span, .. } => span,
            Expr::Slice { span, .. } => span,
            Expr::TryOp { span, .. } => span,
            Expr::Call { span, .. } => span,
            Expr::StructLiteral { span, .. } => span,
            Expr::BlockLiteral { span, .. } => span,
            Expr::Cast { span, .. } => span,
            Expr::Await { span, .. } => span,
            Expr::Range { span, .. } => span,
            Expr::Closure { span, .. } => span,
            Expr::Reference { span, .. } => span,
            Expr::Deref { span, .. } => span,
            Expr::Move { span, .. } => span,
            Expr::Block { span, .. } => span,
            Expr::IfExpr { span, .. } => span,
            Expr::Ternary { span, .. } => span,
            Expr::Match { span, .. } => span,
            Expr::AsyncBlock { span, .. } => span,
            Expr::CssLiteral { span, .. } => span,
            Expr::Loop { span, .. } => span,
        }
    }
}

// Keep old names for backward compat with IR builder / tests
impl Expr {
    pub fn int(n: i64) -> Self {
        Expr::IntLiteral {
            value: n,
            span: Span::default(),
        }
    }
    pub fn float(f: f64) -> Self {
        Expr::FloatLiteral {
            value: f,
            span: Span::default(),
        }
    }
    pub fn string(s: impl Into<String>) -> Self {
        Expr::StringLiteral {
            value: s.into(),
            span: Span::default(),
        }
    }
    pub fn bool(b: bool) -> Self {
        Expr::BoolLiteral {
            value: b,
            span: Span::default(),
        }
    }
    pub fn ident(s: impl Into<String>) -> Self {
        Expr::Identifier {
            name: s.into(),
            span: Span::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FieldInit {
    pub name: String,
    pub value: Expr,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum BlockItem {
    Field(FieldInit),
    Spread(Expr),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClosureParam {
    pub name: String,
    pub mutable: bool,
    pub ty: Option<Type>,
}

// ─── Match patterns ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MatchArm {
    pub pattern: MatchPattern,
    pub guard: Option<Expr>,
    pub body: MatchBody,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum MatchBody {
    Expr(Expr),
    Stmt(Stmt),
    Block(Vec<Stmt>),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum MatchPattern {
    Wildcard,
    Literal(Expr),
    Identifier(String),
    /// `(p1, p2, …)` — tuple pattern
    Tuple(Vec<MatchPattern>),
    /// `Name(p1, p2, …)` — enum tuple variant
    TupleVariant(String, Vec<MatchPattern>),
    /// `Name { field: p, … }` — enum struct variant
    StructVariant(String, Vec<(String, MatchPattern)>),
    /// `p1 | p2 | …` — or-pattern
    Or(Vec<MatchPattern>),
    /// `name @ pattern` — binding
    Binding(String, Box<MatchPattern>),
    /// `..` — rest / fill
    Rest,
}

// ─── Backward-compat type aliases ─────────────────────────────────────────────
// These allow old code in ir_builder / semantic_analyzer to keep compiling.

/// Old `Stmt` required by IR builder – maps to new `Stmt` directly.
/// Old Stmt::Print is still present in the enum above.
/// Old Stmt::Let is still present (mutable field added, default = false).
///
/// Old FunctionDecl shape: the `body: Vec<Stmt>` field is the same.
/// Old StructDecl shape: the `fields: Vec<Field>` field is the same.
pub type ParamCompat = Param;

// ─── Protocol DSL ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProtocolDecl {
    pub name: String,
    pub version: Option<String>,
    pub roles: Vec<String>,
    pub primitives: Vec<ProtocolPrimitive>,
    pub properties: Vec<ProtocolProperty>,
    pub transport: ProtocolTransport,
    pub handshake: Option<HandshakeDef>,
    pub session: Option<SessionDef>,
    pub policies: Vec<ProtocolPolicy>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProtocolPrimitive {
    pub kind: String, // kem, aead, kdf, hash, signature
    pub algo: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProtocolProperty {
    pub name: String,
    pub value: Expr,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProtocolTransport {
    pub framing: Option<String>,
    pub versioning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HandshakeDef {
    pub steps: Vec<HandshakeStep>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum HandshakeStep {
    Message {
        from: String,
        to: String,
        name: String,
        fields: Vec<HandshakeField>,
    },
    Derive {
        assignments: Vec<HandshakeAssignment>,
    },
    Finish {
        actions: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HandshakeField {
    pub name: String,
    pub value: Expr,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HandshakeAssignment {
    pub name: String,
    pub value: Expr,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SessionDef {
    pub body: Vec<Stmt>, // send/receive/on_rekey methods
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProtocolPolicy {
    pub name: String,
    pub value: Expr,
}
