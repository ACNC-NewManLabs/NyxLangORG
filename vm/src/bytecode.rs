//! Bytecode Definitions
//!
//! This module defines the bytecode instruction set for the Nyx VM.
//! The VM uses a stack-based architecture with 71 opcodes.

use std::collections::HashMap;

/// Bytecode magic number
pub const BYTECODE_MAGIC: &[u8; 4] = b"NYXB";

/// Bytecode version
pub const BYTECODE_VERSION: u16 = 6;

/// Number of opcodes
pub const NUM_OPCODES: usize = 75;

/// Opcode enum for bytecode instructions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OpCode {
    // Control Flow (0-7)
    HALT = 0,
    NOP = 1,
    CALL = 2,
    RET = 3,
    JMP = 4,
    JZ = 5,      // Jump if zero
    JNZ = 6,     // Jump if not zero
    CallExt = 7, // Call external function

    // Stack Operations (8-15)
    PUSH = 8,
    POP = 9,
    DUP = 10,
    DUP2 = 11,
    SWAP = 12,
    ROT = 13,
    PICK = 14, // Pick nth item from stack
    PUT = 15,  // Put item at nth position

    // Arithmetic (16-31)
    ADD = 16,
    SUB = 17,
    MUL = 18,
    DIV = 19,
    MOD = 20,
    NEG = 21,
    INC = 22,
    DEC = 23,
    POW = 24,
    DivRem = 25,
    ABS = 26,
    MIN = 27,
    MAX = 28,

    // Comparison (32-39)
    CMP = 32,
    EQ = 33,
    NE = 34,
    LT = 35,
    GT = 36,
    LE = 37,
    GE = 38,
    IsNull = 39,

    // Logical (40-47)
    AND = 40,
    OR = 41,
    NOT = 42,
    XOR = 43,

    // Bitwise (44-51)
    BAND = 44,
    BOR = 45,
    BNOT = 46,
    BXOR = 47,
    SHL = 48,
    SHR = 49,
    USHR = 50, // Unsigned shift right

    // Memory (51-55)
    LOAD = 51,  // Load from local
    STORE = 52, // Store to local
    ALLOC = 53, // Allocate heap
    FREE = 54,  // Free heap
    GetGlobal = 55,
    SetGlobal = 56,

    // Object/Array (57-64)
    NewArray = 57,
    NewObj = 58,
    GetField = 59,
    SetField = 60,
    GetIndex = 61,
    SetIndex = 62,
    LEN = 63,
    SLICE = 64,

    // Closure (65-70)
    CLOSURE = 65,
    PushM = 66,           // Push module constant
    ClosureRef = 67,      // Closure with by-reference upvalues
    GetGlobalM = 68,      // Get global by module constant
    SetGlobalM = 69,      // Set global by module constant
    ClosureRefStack = 70, // Closure with by-reference upvalues from stack

    // String/Array Primitives (71-74)
    CONTAINS = 71,
    SPLIT = 72,
    CHARS = 73,
    SHIFT = 74,
}

impl OpCode {
    /// Get opcode from u8
    pub fn from_u8(n: u8) -> Option<Self> {
        if n < NUM_OPCODES as u8 {
            Some(unsafe { std::mem::transmute(n) })
        } else {
            None
        }
    }

    /// Get opcode name
    pub fn name(&self) -> &'static str {
        match self {
            OpCode::HALT => "HALT",
            OpCode::NOP => "NOP",
            OpCode::CALL => "CALL",
            OpCode::RET => "RET",
            OpCode::JMP => "JMP",
            OpCode::JZ => "JZ",
            OpCode::JNZ => "JNZ",
            OpCode::CallExt => "CallExt",
            OpCode::PUSH => "PUSH",
            OpCode::POP => "POP",
            OpCode::DUP => "DUP",
            OpCode::DUP2 => "DUP2",
            OpCode::SWAP => "SWAP",
            OpCode::ROT => "ROT",
            OpCode::PICK => "PICK",
            OpCode::PUT => "PUT",
            OpCode::ADD => "ADD",
            OpCode::SUB => "SUB",
            OpCode::MUL => "MUL",
            OpCode::DIV => "DIV",
            OpCode::MOD => "MOD",
            OpCode::NEG => "NEG",
            OpCode::INC => "INC",
            OpCode::DEC => "DEC",
            OpCode::POW => "POW",
            OpCode::DivRem => "DivRem",
            OpCode::ABS => "ABS",
            OpCode::MIN => "MIN",
            OpCode::MAX => "MAX",
            OpCode::CMP => "CMP",
            OpCode::EQ => "EQ",
            OpCode::NE => "NE",
            OpCode::LT => "LT",
            OpCode::GT => "GT",
            OpCode::LE => "LE",
            OpCode::GE => "GE",
            OpCode::IsNull => "IsNull",
            OpCode::AND => "AND",
            OpCode::OR => "OR",
            OpCode::NOT => "NOT",
            OpCode::XOR => "XOR",
            OpCode::BAND => "BAND",
            OpCode::BOR => "BOR",
            OpCode::BNOT => "BNOT",
            OpCode::BXOR => "BXOR",
            OpCode::SHL => "SHL",
            OpCode::SHR => "SHR",
            OpCode::USHR => "USHR",
            OpCode::LOAD => "LOAD",
            OpCode::STORE => "STORE",
            OpCode::ALLOC => "ALLOC",
            OpCode::FREE => "FREE",
            OpCode::GetGlobal => "GetGlobal",
            OpCode::SetGlobal => "SetGlobal",
            OpCode::NewArray => "NewArray",
            OpCode::NewObj => "NewObj",
            OpCode::GetField => "GetField",
            OpCode::SetField => "SetField",
            OpCode::GetIndex => "GetIndex",
            OpCode::SetIndex => "SetIndex",
            OpCode::LEN => "LEN",
            OpCode::SLICE => "SLICE",
            OpCode::CLOSURE => "CLOSURE",
            OpCode::PushM => "PUSHM",
            OpCode::ClosureRef => "CLOSURE_REF",
            OpCode::GetGlobalM => "GetGlobalM",
            OpCode::SetGlobalM => "SetGlobalM",
            OpCode::ClosureRefStack => "CLOSURE_REF_STACK",
            OpCode::CONTAINS => "CONTAINS",
            OpCode::SPLIT => "SPLIT",
            OpCode::CHARS => "CHARS",
            OpCode::SHIFT => "SHIFT",
        }
    }
}

/// Value types in the VM
#[repr(C, u64)]
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Null/undefined
    Null,
    /// Boolean
    Bool(bool),
    /// Integer (64-bit signed)
    Int(i64),
    /// Float (64-bit)
    Float(f64),
    /// String (heap-allocated)
    String(String),
    /// Array (heap-allocated)
    Array(Vec<Value>),
    /// Object (heap-allocated)
    Object(HashMap<String, Value>),
    /// Function reference
    Function(usize),
    /// Native function reference
    NativeFunc(NativeFunction),
    /// Closure
    Closure(Closure),
    /// Pointer
    Pointer(usize),
    /// Unit (empty tuple)
    Unit,
}

impl Value {
    /// Get type name
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
            Value::Function(_) => "function",
            Value::NativeFunc(_) => "native",
            Value::Closure(_) => "closure",
            Value::Pointer(_) => "pointer",
            Value::Unit => "unit",
        }
    }

    /// Check if truthy
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Int(i) => *i != 0,
            Value::Float(f) => *f != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::Array(arr) => !arr.is_empty(),
            Value::Object(obj) => !obj.is_empty(),
            Value::Function(_) | Value::NativeFunc(_) | Value::Closure(_) => true,
            Value::Pointer(p) => *p != 0,
            Value::Unit => false,
        }
    }

    /// Convert to string
    pub fn to_string(&self) -> String {
        match self {
            Value::Null => "null".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Int(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
            Value::String(s) => s.clone(),
            Value::Array(arr) => {
                let items: Vec<String> = arr.iter().map(|v| v.to_string()).collect();
                format!("[{}]", items.join(", "))
            }
            Value::Object(obj) => {
                let items: Vec<String> = obj
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v.to_string()))
                    .collect();
                format!("{{{}}}", items.join(", "))
            }
            Value::Function(idx) => format!("<function {}>", idx),
            Value::NativeFunc(f) => format!("<native {}>", f.name),
            Value::Closure(c) => format!("<closure {}>", c.function_idx),
            Value::Pointer(p) => format!("<pointer {:#x}>", p),
            Value::Unit => "()".to_string(),
        }
    }
}

/// Native function type
#[derive(Clone)]
pub struct NativeFunction {
    pub name: String,
    pub arity: usize,
    pub func: fn(&[Value]) -> Result<Value, String>,
}

impl std::fmt::Debug for NativeFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeFunction")
            .field("name", &self.name)
            .field("arity", &self.arity)
            .finish()
    }
}

impl PartialEq for NativeFunction {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.arity == other.arity
            && self.func as usize == other.func as usize
    }
}

/// Closure definition
#[derive(Clone, Debug, PartialEq)]
pub struct Closure {
    pub function_idx: usize,
    pub upvalues: Vec<Value>,
    pub upvalue_names: Vec<String>,
    pub upvalue_captures: Vec<UpvalueCapture>,
}

/// How an upvalue was captured (debug metadata).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpvalueCapture {
    /// Captured by value (copied at closure creation time).
    ByValue,
    /// Captured by reference from a local slot.
    ByRefLocal { local_idx: usize },
    /// Captured by reference from a stack value.
    ByRefStack { stack_index: usize },
}

/// Function definition in bytecode
#[derive(Debug, Clone)]
pub struct Function {
    /// Function name
    pub name: String,
    /// Number of parameters
    pub arity: usize,
    /// Number of local variables
    pub num_locals: usize,
    /// Bytecode instructions
    pub instructions: Vec<BytecodeInstr>,
    /// Constant pool
    pub constants: Vec<Value>,
    /// Upvalue names
    pub upvalues: Vec<String>,
    /// Source line mapping
    pub line_info: Vec<(usize, usize)>, // (instruction_index, line_number)
}

/// Single bytecode instruction
#[derive(Debug, Clone)]
pub struct BytecodeInstr {
    /// Opcode
    pub opcode: OpCode,
    /// Operands (varies by opcode)
    pub operands: Vec<i32>,
    /// Source line number
    pub line: usize,
}

impl BytecodeInstr {
    /// Create new instruction
    pub fn new(opcode: OpCode, operands: Vec<i32>, line: usize) -> Self {
        Self {
            opcode,
            operands,
            line,
        }
    }

    /// Create instruction with single operand
    pub fn with_operand(opcode: OpCode, operand: i32, line: usize) -> Self {
        Self {
            opcode,
            operands: vec![operand],
            line,
        }
    }

    /// Get operand at index
    pub fn operand(&self, index: usize) -> Option<i32> {
        self.operands.get(index).copied()
    }
}

/// Bytecode module container
#[derive(Debug, Clone)]
pub struct BytecodeModule {
    /// Module name
    pub name: String,
    /// Functions in the module
    pub functions: Vec<Function>,
    /// Global variables
    pub globals: Vec<String>,
    /// Module constants
    pub constants: Vec<Value>,
    /// Source file path
    pub source_path: Option<String>,
    /// Dependencies
    pub dependencies: Vec<String>,
}

impl BytecodeModule {
    /// Create new bytecode module
    pub fn new(name: String) -> Self {
        Self {
            name,
            functions: Vec::new(),
            globals: Vec::new(),
            constants: Vec::new(),
            source_path: None,
            dependencies: Vec::new(),
        }
    }

    /// Add function to module
    pub fn add_function(&mut self, func: Function) -> usize {
        let idx = self.functions.len();
        self.functions.push(func);
        idx
    }

    /// Add constant and return its index
    pub fn add_constant(&mut self, value: Value) -> usize {
        let idx = self.constants.len();
        self.constants.push(value);
        idx
    }

    /// Get function by index
    pub fn get_function(&self, idx: usize) -> Option<&Function> {
        self.functions.get(idx)
    }

    /// Get constant by index
    pub fn get_constant(&self, idx: usize) -> Option<&Value> {
        self.constants.get(idx)
    }
}

fn write_u8(out: &mut Vec<u8>, value: u8) {
    out.push(value);
}

fn write_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn write_i32(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn write_i64(out: &mut Vec<u8>, value: i64) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn write_f64(out: &mut Vec<u8>, value: f64) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn write_string(out: &mut Vec<u8>, value: &str) {
    write_u32(out, value.len() as u32);
    out.extend_from_slice(value.as_bytes());
}

fn write_value(out: &mut Vec<u8>, value: &Value) -> Result<(), String> {
    match value {
        Value::Null => write_u8(out, 0),
        Value::Bool(b) => {
            write_u8(out, 1);
            write_u8(out, if *b { 1 } else { 0 });
        }
        Value::Int(i) => {
            write_u8(out, 2);
            write_i64(out, *i);
        }
        Value::Float(f) => {
            write_u8(out, 3);
            write_f64(out, *f);
        }
        Value::String(s) => {
            write_u8(out, 4);
            write_string(out, s);
        }
        Value::Array(arr) => {
            write_u8(out, 5);
            write_u32(out, arr.len() as u32);
            for item in arr {
                write_value(out, item)?;
            }
        }
        Value::Object(map) => {
            write_u8(out, 6);
            write_u32(out, map.len() as u32);
            for (k, v) in map {
                write_string(out, k);
                write_value(out, v)?;
            }
        }
        Value::Function(idx) => {
            write_u8(out, 7);
            write_u32(out, *idx as u32);
        }
        Value::NativeFunc(_) => {
            return Err("NativeFunc cannot be serialized".to_string());
        }
        Value::Closure(c) => {
            write_u8(out, 8);
            write_u32(out, c.function_idx as u32);
            write_u32(out, c.upvalues.len() as u32);
            for item in &c.upvalues {
                write_value(out, item)?;
            }
            write_u32(out, c.upvalue_names.len() as u32);
            for name in &c.upvalue_names {
                write_string(out, name);
            }
            write_u32(out, c.upvalue_captures.len() as u32);
            for cap in &c.upvalue_captures {
                match cap {
                    UpvalueCapture::ByValue => {
                        write_u8(out, 0);
                    }
                    UpvalueCapture::ByRefLocal { local_idx } => {
                        write_u8(out, 1);
                        write_u32(out, *local_idx as u32);
                    }
                    UpvalueCapture::ByRefStack { stack_index } => {
                        write_u8(out, 2);
                        write_u32(out, *stack_index as u32);
                    }
                }
            }
        }
        Value::Pointer(_) => {
            return Err("Pointer cannot be serialized".to_string());
        }
        Value::Unit => {
            write_u8(out, 9);
        }
    }
    Ok(())
}

fn read_u8(bytes: &[u8], pos: &mut usize) -> Result<u8, String> {
    if *pos + 1 > bytes.len() {
        return Err("Unexpected EOF".to_string());
    }
    let value = bytes[*pos];
    *pos += 1;
    Ok(value)
}

fn read_u32(bytes: &[u8], pos: &mut usize) -> Result<u32, String> {
    if *pos + 4 > bytes.len() {
        return Err("Unexpected EOF".to_string());
    }
    let value = u32::from_be_bytes([
        bytes[*pos],
        bytes[*pos + 1],
        bytes[*pos + 2],
        bytes[*pos + 3],
    ]);
    *pos += 4;
    Ok(value)
}

fn read_i32(bytes: &[u8], pos: &mut usize) -> Result<i32, String> {
    if *pos + 4 > bytes.len() {
        return Err("Unexpected EOF".to_string());
    }
    let value = i32::from_be_bytes([
        bytes[*pos],
        bytes[*pos + 1],
        bytes[*pos + 2],
        bytes[*pos + 3],
    ]);
    *pos += 4;
    Ok(value)
}

fn read_i64(bytes: &[u8], pos: &mut usize) -> Result<i64, String> {
    if *pos + 8 > bytes.len() {
        return Err("Unexpected EOF".to_string());
    }
    let value = i64::from_be_bytes([
        bytes[*pos],
        bytes[*pos + 1],
        bytes[*pos + 2],
        bytes[*pos + 3],
        bytes[*pos + 4],
        bytes[*pos + 5],
        bytes[*pos + 6],
        bytes[*pos + 7],
    ]);
    *pos += 8;
    Ok(value)
}

fn read_f64(bytes: &[u8], pos: &mut usize) -> Result<f64, String> {
    if *pos + 8 > bytes.len() {
        return Err("Unexpected EOF".to_string());
    }
    let value = f64::from_be_bytes([
        bytes[*pos],
        bytes[*pos + 1],
        bytes[*pos + 2],
        bytes[*pos + 3],
        bytes[*pos + 4],
        bytes[*pos + 5],
        bytes[*pos + 6],
        bytes[*pos + 7],
    ]);
    *pos += 8;
    Ok(value)
}

fn read_string(bytes: &[u8], pos: &mut usize) -> Result<String, String> {
    let len = read_u32(bytes, pos)? as usize;
    if *pos + len > bytes.len() {
        return Err("Unexpected EOF".to_string());
    }
    let slice = &bytes[*pos..*pos + len];
    *pos += len;
    String::from_utf8(slice.to_vec()).map_err(|_| "Invalid UTF-8 string".to_string())
}

fn read_value(bytes: &[u8], pos: &mut usize) -> Result<Value, String> {
    let tag = read_u8(bytes, pos)?;
    match tag {
        0 => Ok(Value::Null),
        1 => {
            let b = read_u8(bytes, pos)?;
            Ok(Value::Bool(b != 0))
        }
        2 => Ok(Value::Int(read_i64(bytes, pos)?)),
        3 => Ok(Value::Float(read_f64(bytes, pos)?)),
        4 => Ok(Value::String(read_string(bytes, pos)?)),
        5 => {
            let len = read_u32(bytes, pos)? as usize;
            let mut items = Vec::with_capacity(len);
            for _ in 0..len {
                items.push(read_value(bytes, pos)?);
            }
            Ok(Value::Array(items))
        }
        6 => {
            let len = read_u32(bytes, pos)? as usize;
            let mut map = HashMap::with_capacity(len);
            for _ in 0..len {
                let key = read_string(bytes, pos)?;
                let value = read_value(bytes, pos)?;
                map.insert(key, value);
            }
            Ok(Value::Object(map))
        }
        7 => {
            let idx = read_u32(bytes, pos)? as usize;
            Ok(Value::Function(idx))
        }
        8 => {
            let func_idx = read_u32(bytes, pos)? as usize;
            let count = read_u32(bytes, pos)? as usize;
            let mut upvalues = Vec::with_capacity(count);
            for _ in 0..count {
                upvalues.push(read_value(bytes, pos)?);
            }
            let name_count = read_u32(bytes, pos)? as usize;
            let mut upvalue_names = Vec::with_capacity(name_count);
            for _ in 0..name_count {
                upvalue_names.push(read_string(bytes, pos)?);
            }
            let cap_count = read_u32(bytes, pos)? as usize;
            let mut upvalue_captures = Vec::with_capacity(cap_count);
            for _ in 0..cap_count {
                let tag = read_u8(bytes, pos)?;
                match tag {
                    0 => upvalue_captures.push(UpvalueCapture::ByValue),
                    1 => {
                        let local_idx = read_u32(bytes, pos)? as usize;
                        upvalue_captures.push(UpvalueCapture::ByRefLocal { local_idx });
                    }
                    2 => {
                        let stack_index = read_u32(bytes, pos)? as usize;
                        upvalue_captures.push(UpvalueCapture::ByRefStack { stack_index });
                    }
                    _ => return Err("Invalid upvalue capture tag".to_string()),
                }
            }
            Ok(Value::Closure(Closure {
                function_idx: func_idx,
                upvalues,
                upvalue_names,
                upvalue_captures,
            }))
        }
        9 => Ok(Value::Unit),
        _ => Err("Invalid value tag".to_string()),
    }
}

/// Serialize module to bytes
pub fn serialize_module(module: &BytecodeModule) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();

    // Magic number
    bytes.extend_from_slice(BYTECODE_MAGIC);

    // Version
    bytes.extend_from_slice(&BYTECODE_VERSION.to_be_bytes());

    // Module name
    write_string(&mut bytes, &module.name);

    // Source path
    match &module.source_path {
        Some(path) => {
            write_u8(&mut bytes, 1);
            write_string(&mut bytes, path);
        }
        None => write_u8(&mut bytes, 0),
    }

    // Dependencies
    write_u32(&mut bytes, module.dependencies.len() as u32);
    for dep in &module.dependencies {
        write_string(&mut bytes, dep);
    }

    // Counts
    write_u32(&mut bytes, module.functions.len() as u32);
    write_u32(&mut bytes, module.constants.len() as u32);
    write_u32(&mut bytes, module.globals.len() as u32);

    // Functions
    for func in &module.functions {
        write_string(&mut bytes, &func.name);
        write_u32(&mut bytes, func.arity as u32);
        write_u32(&mut bytes, func.num_locals as u32);

        write_u32(&mut bytes, func.upvalues.len() as u32);
        for up in &func.upvalues {
            write_string(&mut bytes, up);
        }

        write_u32(&mut bytes, func.line_info.len() as u32);
        for (idx, line) in &func.line_info {
            write_u32(&mut bytes, *idx as u32);
            write_u32(&mut bytes, *line as u32);
        }

        write_u32(&mut bytes, func.instructions.len() as u32);
        for instr in &func.instructions {
            write_u8(&mut bytes, instr.opcode as u8);
            write_u8(&mut bytes, instr.operands.len() as u8);
            for op in &instr.operands {
                write_i32(&mut bytes, *op);
            }
            write_u32(&mut bytes, instr.line as u32);
        }

        write_u32(&mut bytes, func.constants.len() as u32);
        for value in &func.constants {
            write_value(&mut bytes, value)?;
        }
    }

    // Globals
    for global in &module.globals {
        write_string(&mut bytes, global);
    }

    // Module constants
    for value in &module.constants {
        write_value(&mut bytes, value)?;
    }

    Ok(bytes)
}

/// Deserialize module from bytes
pub fn deserialize_module(bytes: &[u8]) -> Result<BytecodeModule, String> {
    let mut pos = 0;

    if bytes.len() < 4 {
        return Err("Invalid bytecode file".to_string());
    }
    let magic = &bytes[0..4];
    if magic != BYTECODE_MAGIC {
        return Err("Invalid bytecode magic".to_string());
    }
    pos += 4;

    let version = read_u16(bytes, &mut pos)?;
    if version != BYTECODE_VERSION {
        return Err(format!("Unsupported bytecode version: {}", version));
    }

    let name = read_string(bytes, &mut pos)?;
    let mut module = BytecodeModule::new(name);

    let has_source = read_u8(bytes, &mut pos)?;
    if has_source != 0 {
        module.source_path = Some(read_string(bytes, &mut pos)?);
    }

    let dep_count = read_u32(bytes, &mut pos)? as usize;
    for _ in 0..dep_count {
        module.dependencies.push(read_string(bytes, &mut pos)?);
    }

    let num_funcs = read_u32(bytes, &mut pos)? as usize;
    let num_consts = read_u32(bytes, &mut pos)? as usize;
    let num_globals = read_u32(bytes, &mut pos)? as usize;

    let mut functions = Vec::with_capacity(num_funcs);
    for _ in 0..num_funcs {
        let func_name = read_string(bytes, &mut pos)?;
        let arity = read_u32(bytes, &mut pos)? as usize;
        let num_locals = read_u32(bytes, &mut pos)? as usize;

        let up_count = read_u32(bytes, &mut pos)? as usize;
        let mut upvalues = Vec::with_capacity(up_count);
        for _ in 0..up_count {
            upvalues.push(read_string(bytes, &mut pos)?);
        }

        let line_count = read_u32(bytes, &mut pos)? as usize;
        let mut line_info = Vec::with_capacity(line_count);
        for _ in 0..line_count {
            let idx = read_u32(bytes, &mut pos)? as usize;
            let line = read_u32(bytes, &mut pos)? as usize;
            line_info.push((idx, line));
        }

        let instr_count = read_u32(bytes, &mut pos)? as usize;
        let mut instructions = Vec::with_capacity(instr_count);
        for _ in 0..instr_count {
            let opcode = read_u8(bytes, &mut pos)?;
            let opcode = OpCode::from_u8(opcode).ok_or_else(|| "Invalid opcode".to_string())?;
            let op_count = read_u8(bytes, &mut pos)? as usize;
            let mut operands = Vec::with_capacity(op_count);
            for _ in 0..op_count {
                operands.push(read_i32(bytes, &mut pos)?);
            }
            let line = read_u32(bytes, &mut pos)? as usize;
            instructions.push(BytecodeInstr::new(opcode, operands, line));
        }

        let const_count = read_u32(bytes, &mut pos)? as usize;
        let mut constants = Vec::with_capacity(const_count);
        for _ in 0..const_count {
            constants.push(read_value(bytes, &mut pos)?);
        }

        functions.push(Function {
            name: func_name,
            arity,
            num_locals,
            instructions,
            constants,
            upvalues,
            line_info,
        });
    }
    module.functions = functions;

    for _ in 0..num_globals {
        module.globals.push(read_string(bytes, &mut pos)?);
    }

    for _ in 0..num_consts {
        module.constants.push(read_value(bytes, &mut pos)?);
    }

    Ok(module)
}

fn read_u16(bytes: &[u8], pos: &mut usize) -> Result<u16, String> {
    if *pos + 2 > bytes.len() {
        return Err("Unexpected EOF".to_string());
    }
    let value = u16::from_be_bytes([bytes[*pos], bytes[*pos + 1]]);
    *pos += 2;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opcode_names() {
        assert_eq!(OpCode::HALT.name(), "HALT");
        assert_eq!(OpCode::ADD.name(), "ADD");
    }

    #[test]
    fn test_value_type_name() {
        assert_eq!(Value::Int(42).type_name(), "int");
        assert_eq!(Value::String("hello".to_string()).type_name(), "string");
    }

    #[test]
    fn test_value_truthy() {
        assert!(!Value::Null.is_truthy());
        assert!(Value::Bool(true).is_truthy());
        assert!(!Value::Bool(false).is_truthy());
        assert!(Value::Int(1).is_truthy());
        assert!(!Value::Int(0).is_truthy());
    }

    #[test]
    fn test_module_serialize() {
        let mut module = BytecodeModule::new("test".to_string());
        module.globals.push("x".to_string());

        let bytes = serialize_module(&module).expect("serialize");
        assert!(!bytes.is_empty());

        let decoded = deserialize_module(&bytes).expect("deserialize");
        assert_eq!(decoded.name, "test");
        assert_eq!(decoded.globals, vec!["x".to_string()]);
    }
}
