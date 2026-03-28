use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Module {
    pub structs: Vec<StructDef>,
    pub functions: Vec<IrFunction>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<StructFieldDef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructFieldDef {
    pub name: String,
    pub ty: IrType,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IrType {
    I64,
    F64,
    Ptr,
    Struct(String),
    Array(Box<IrType>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrFunction {
    pub name: String,
    pub params: Vec<IrParam>,
    pub instructions: Vec<Instruction>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrParam {
    pub name: String,
    pub ty: IrType,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InlineAsmOutput {
    pub name: String,
    pub reg: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InlineAsmInput {
    pub value: Value,
    pub reg: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Instruction {
    Let {
        name: String,
        value: Value,
    },
    Print {
        value: Value,
    },
    Return {
        value: Option<Value>,
    },
    Binary {
        dst: String,
        op: BinaryOp,
        lhs: Value,
        rhs: Value,
    },
    Call {
        dst: String,
        callee: String,
        args: Vec<Value>,
    },
    Label(String),
    Jump(String),
    Branch {
        cond: Value,
        then_label: String,
        else_label: String,
    },
    StructInit {
        dst: String,
        struct_name: String,
        fields: Vec<(String, Value)>,
    },
    StructGet {
        dst: String,
        struct_name: String,
        base: String,
        field: String,
    },
    ArrayInit {
        dst: String,
        elem_ty: IrType,
        len: Value,
    },
    ArraySet {
        base: String,
        elem_ty: IrType,
        index: Value,
        value: Value,
    },
    ArrayGet {
        dst: String,
        elem_ty: IrType,
        base: String,
        index: Value,
    },
    InlineAsm {
        code: String,
        outputs: Vec<InlineAsmOutput>,
        inputs: Vec<InlineAsmInput>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Null,
    Local(String),
    Temp(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Not,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}
