//! Nyx AST Node Definitions
//!
//! Strongly-typed, immutable-after-construction AST nodes for every Nyx
//! language construct.  The root type is [`Program`].

use serde::Serialize;
use crate::core::lexer::token::Span;

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
    Defer {
        stmt: Box<Stmt>,
        span: Span,
    },
    /// `print(…)` — special built-in kept for backward compat
    Print { expr: Expr },
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
    IntLiteral(i64),
    FloatLiteral(f64),
    StringLiteral(String),
    CharLiteral(char),
    BoolLiteral(bool),
    NullLiteral,

    // Composite literals
    ArrayLiteral(Vec<Expr>),
    ArrayRepeat {
        value: Box<Expr>,
        len: Box<Expr>,
    },
    TupleLiteral(Vec<Expr>),
    BigIntLiteral(String),

    // Named reference
    Identifier(String),
    Path(Vec<String>), // a::b::c

    // Operators
    Binary {
        left: Box<Expr>,
        op: String,
        right: Box<Expr>,
    },
    Unary {
        op: String,
        right: Box<Expr>,
    },

    // Postfix
    FieldAccess {
        object: Box<Expr>,
        field: String,
    },
    MethodCall {
        receiver: Box<Expr>,
        method: String,
        args: Vec<Expr>,
    },
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
    },
    Slice {
        object: Box<Expr>,
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
    },
    TryOp(Box<Expr>), // `expr?`

    // Calls
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },

    // Struct literal  `Name { field: val, … }`
    StructLiteral {
        name: String,
        fields: Vec<FieldInit>,
    },

    // Block struct literal  `{ field: val, … }` (anonymous)
    BlockLiteral(Vec<BlockItem>),

    // Type cast  `expr as Type`
    Cast {
        expr: Box<Expr>,
        ty: Type,
    },

    // Async  `expr.await`
    Await(Box<Expr>),

    // Range  `a..b` / `a..=b`
    Range {
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
        inclusive: bool,
    },

    // Closure  `|params| [-> T] body`
    Closure {
        params: Vec<ClosureParam>,
        return_ty: Option<Type>,
        body: Box<Expr>,
    },

    // Reference  `&expr` / `&mut expr`
    Reference {
        mutable: bool,
        expr: Box<Expr>,
    },

    // Dereference  `*expr`
    Deref(Box<Expr>),

    // Move  `move expr`
    Move(Box<Expr>),

    // Block expression  `{ stmts… [tail_expr] }`
    Block(Vec<Stmt>, Option<Box<Expr>>),

    // If-else as expression
    IfExpr {
        branches: Vec<IfBranch>,
        else_body: Option<Box<Expr>>,
    },
    // Ternary  `cond ? then_expr : else_expr`
    Ternary {
        condition: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Box<Expr>,
    },

    // Match expression
    Match {
        expr: Box<Expr>,
        arms: Vec<MatchArm>,
    },

    // Async block  `async { … }`
    AsyncBlock(Vec<Stmt>),

    /// `css\`...\`` template literal. The payload is the raw CSS text
    /// (with any ${...} interpolation segments kept verbatim).
    /// Evaluates to `Map<string, string>` at runtime.
    CssLiteral(String),

    /// `loop expr` as expression
    Loop(Box<Expr>),
}

// Keep old names for backward compat with IR builder / tests
impl Expr {
    pub fn int(n: i64) -> Self {
        Expr::IntLiteral(n)
    }
    pub fn float(f: f64) -> Self {
        Expr::FloatLiteral(f)
    }
    pub fn string(s: impl Into<String>) -> Self {
        Expr::StringLiteral(s.into())
    }
    pub fn bool(b: bool) -> Self {
        Expr::BoolLiteral(b)
    }
    pub fn ident(s: impl Into<String>) -> Self {
        Expr::Identifier(s.into())
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
