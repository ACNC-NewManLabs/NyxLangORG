//! Nyx AST VM (interpreter)
//!
//! This is the current authoritative execution runtime used by Nyx tooling.
//! It intentionally interprets Nyx AST to support fast dev workflows (hot reload).
//! A future milestone is swapping the backend to Nyx bytecode without changing the
//! public execution API surface.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::core::ast::ast_nodes::{
    BlockItem, Expr, FieldInit, FunctionDecl, ImplItem, ItemKind, MatchBody, MatchPattern,
    ModuleDecl, Program, Stmt, Type,
};
use crate::core::lexer::lexer::Lexer;
use crate::core::lexer::token::Span;
use crate::core::parser::grammar_engine::GrammarEngine;
use crate::core::parser::neuro_parser::NeuroParser;
use crate::core::registry::language_registry::LanguageRegistry;
// use serde::{Deserialize, Serialize}; // Removed duplicate

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
    pub function: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvalError {
    pub message: String,
    pub stack: Vec<String>,
}

impl EvalError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            stack: Vec::new(),
        }
    }

    pub fn with_frame(&self, name: &str, line: usize, col: usize) -> Self {
        let mut new_stack = self.stack.clone();
        new_stack.push(format!("at {name} (line {line}, col {col})"));
        Self {
            message: self.message.clone(),
            stack: new_stack,
        }
    }
}

#[derive(Debug, Clone)]
pub enum TensorStorage {
    Gpu(std::sync::Arc<crate::runtime::execution::gpu_bridge::NyxBuffer>),
    Cpu(std::sync::Arc<std::sync::RwLock<Vec<f32>>>),
    Tiered {
        buffer: std::sync::Arc<crate::runtime::execution::gpu_bridge::NyxBuffer>,
        disk_path: std::path::PathBuf,
        evicted: bool,
    },
}

impl serde::Serialize for TensorStorage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeSeq;
        match self {
            TensorStorage::Cpu(rc) => {
                let data = rc.read().unwrap_or_else(|e| e.into_inner());
                let mut seq = serializer.serialize_seq(Some(data.len()))?;
                for &f in data.iter() {
                    seq.serialize_element(&f)?;
                }
                seq.end()
            }
            TensorStorage::Gpu(buf) => {
                // Pull from GPU for serialization
                let mut data = vec![0.0f32; (buf.bucket_size / 4) as usize];
                if crate::runtime::execution::gpu_bridge::gpu_read_buffer(buf, &mut data) {
                    let mut seq = serializer.serialize_seq(Some(data.len()))?;
                    for &f in data.iter() {
                        seq.serialize_element(&f)?;
                    }
                    seq.end()
                } else {
                    serializer.serialize_seq(Some(0))?.end()
                }
            }
            _ => serializer.serialize_seq(Some(0))?.end(),
        }
    }
}

impl<'de> serde::Deserialize<'de> for TensorStorage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data: Vec<f32> = serde::Deserialize::deserialize(deserializer)?;
        Ok(TensorStorage::Cpu(std::sync::Arc::new(
            std::sync::RwLock::new(data),
        )))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Value {
    Null,
    Int(i64),
    Float(f64),
    Bool(bool),
    BigInt(String),
    Str(String),
    #[serde(
        serialize_with = "serialize_array",
        deserialize_with = "deserialize_array"
    )]
    Array(std::sync::Arc<std::sync::RwLock<Vec<Value>>>),
    #[serde(skip)]
    FloatArray(std::sync::Arc<std::sync::RwLock<Vec<f32>>>),
    #[serde(skip)]
    DoubleArray(std::sync::Arc<std::sync::RwLock<Vec<f64>>>),
    #[serde(
        serialize_with = "serialize_object",
        deserialize_with = "deserialize_object"
    )]
    Object(std::sync::Arc<std::sync::RwLock<std::collections::HashMap<String, Value>>>),
    #[serde(skip)]
    Closure(Box<ClosureValue>),
    Node(VNode),
    #[serde(skip)]
    Bytes(std::sync::Arc<std::sync::RwLock<Vec<u8>>>),
    Pointer(u64),
    #[serde(skip)]
    Promise(std::sync::Arc<std::sync::RwLock<PromiseState>>),
    Tensor(TensorStorage, Vec<usize>),
}

#[derive(Debug, Clone)]
pub struct PromiseState {
    pub resolved: bool,
    pub value: Value,
}

impl Value {
    pub fn array(v: Vec<Value>) -> Value {
        Value::Array(std::sync::Arc::new(std::sync::RwLock::new(v)))
    }
    pub fn object(m: HashMap<String, Value>) -> Value {
        Value::Object(std::sync::Arc::new(std::sync::RwLock::new(m)))
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Int(i) => *i != 0,
            Value::Float(f) => *f != 0.0,
            Value::BigInt(n) => n != "0",
            Value::Str(s) => !s.is_empty(),
            Value::Array(a_rc) => !a_rc.read().unwrap_or_else(|e| e.into_inner()).is_empty(),
            Value::FloatArray(f_rc) => !f_rc.read().unwrap_or_else(|e| e.into_inner()).is_empty(),
            Value::DoubleArray(d_rc) => !d_rc.read().unwrap_or_else(|e| e.into_inner()).is_empty(),
            Value::Object(o_rc) => !o_rc.read().unwrap_or_else(|e| e.into_inner()).is_empty(),
            Value::Bytes(b_rc) => !b_rc.read().unwrap_or_else(|e| e.into_inner()).is_empty(),
            Value::Promise(p_rc) => p_rc.read().unwrap_or_else(|e| e.into_inner()).resolved,
            Value::Tensor(_, shape) => !shape.is_empty() && shape.iter().all(|&s| s > 0),
            Value::Pointer(_) => true,
            Value::Closure(_) | Value::Node(_) => true,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            Value::Int(i) => Some(*i as f64),
            Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            Value::Float(f) => Some(*f as i64),
            Value::Bool(b) => Some(if *b { 1 } else { 0 }),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            Value::Int(i) => Some(*i != 0),
            Value::Float(f) => Some(*f != 0.0),
            Value::Str(s) => s.parse().ok(),
            _ => None,
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Int(i) => write!(f, "{}", i),
            Value::Float(fv) => write!(f, "{}", fv),
            Value::Bool(b) => write!(f, "{}", b),
            Value::BigInt(n) => write!(f, "{}", n),
            Value::Str(s) => write!(f, "{}", s),
            Value::Array(_) => write!(f, "[...]"),
            Value::FloatArray(fr) => write!(
                f,
                "<f32[{}]>",
                fr.read().unwrap_or_else(|e| e.into_inner()).len()
            ),
            Value::DoubleArray(dr) => write!(
                f,
                "<f64[{}]>",
                dr.read().unwrap_or_else(|e| e.into_inner()).len()
            ),
            Value::Object(_) => write!(f, "{{...}}"),
            Value::Closure(_) => write!(f, "<closure>"),
            Value::Node(_) => write!(f, "<vnode>"),
            Value::Bytes(_) => write!(f, "<bytes>"),
            Value::Pointer(p) => write!(f, "0x{:x}", p),
            Value::Promise(_) => write!(f, "<promise>"),
            Value::Tensor(_, shape) => write!(f, "<Tensor{:?}>", shape),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ClosureValue {
    pub params: Vec<String>,
    pub body: Expr,
    pub captured: HashMap<String, Value>,
    pub module_prefix: String,
}

#[derive(Debug, Clone)]
pub(crate) enum Control {
    Continue,
    Break,
    ContinueLoop,
    Return(Value),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Child {
    Text(String),
    Node(VNode),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VNode {
    pub tag: String,
    pub attrs: HashMap<String, String>,
    pub children: Vec<Child>,
}

pub type NativeFn = fn(&mut NyxVm, &[Value]) -> Result<Value, EvalError>;

#[derive(Debug, Clone, Serialize)]
pub struct VmFunction {
    pub decl: FunctionDecl,
    pub module_prefix: String,
}

#[derive(Debug, Clone, Serialize)]
pub enum TraceEvent {
    Call {
        name: String,
        args: Vec<Value>,
        module: String,
    },
    Return {
        name: String,
        value: Value,
    },
    NativeCall {
        name: String,
        args: Vec<Value>,
        result: Value,
    },
    Error {
        message: String,
        context: String,
    },
}

use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

pub struct NyxVm {
    pub globals: HashMap<String, Value>,
    pub functions: HashMap<String, VmFunction>,
    pub natives: HashMap<String, NativeFn>,
    pub routes: HashMap<String, Box<ClosureValue>>,
    pub current_module_prefix: String,
    pub standard_streams: bool,
    pub stdlib_path: Option<PathBuf>,
    pub imports: HashMap<String, String>,
    pub loaded_files: HashSet<PathBuf>,
    pub record_grad: bool,
    pub record_traces: bool,
    pub buffer_cache: std::sync::Arc<std::sync::Mutex<Vec<Vec<Value>>>>,
    pub gas: u64,
    pub max_gas: u64,
    pub memory_limit: u64,
    pub memory_used: u64,
    pub source_cache: HashMap<String, String>,
    pub last_span: Option<Span>,
    pub last_hint: Option<String>,
    pub traces: Vec<TraceEvent>,
    pub actor_mailbox: Option<Arc<Mutex<Receiver<Value>>>>,
}

/// Configuration passed when constructing a `NyxVm`.
#[derive(Debug, Clone, Default)]
pub struct VmConfig {
    pub max_gas: Option<u64>,
    pub memory_limit_mb: Option<u64>,
    pub record_traces: bool,
}

/// Evaluate a single REPL line and return the result as a string.
/// This is a convenience wrapper used by the REPL and session layer.
pub fn eval_repl_line(vm: &mut NyxVm, line: &str) -> Result<Value, EvalError> {
    use crate::core::lexer::lexer::Lexer;
    use crate::core::parser::grammar_engine::GrammarEngine;
    use crate::core::parser::neuro_parser::NeuroParser;
    use crate::core::registry::language_registry::LanguageRegistry;
    let registry_path = concat!(env!("CARGO_MANIFEST_DIR"), "/registry/language.json");
    let registry = LanguageRegistry::load(registry_path).unwrap_or_default();
    let grammar = GrammarEngine::from_registry(&registry);
    let mut lexer = Lexer::from_source(line.to_string());
    let tokens = lexer.tokenize().map_err(|e| EvalError {
        message: e.to_string(),
        stack: vec![],
    })?;
    let mut parser = NeuroParser::new(grammar);
    let program = parser.parse(&tokens).map_err(|e| EvalError {
        message: e.to_string(),
        stack: vec![],
    })?;
    vm.load_program("", program.clone(), None)?;
    Ok(Value::Null)
}

impl NyxVm {
    pub fn new(config: VmConfig) -> Self {
        Self {
            globals: HashMap::new(),
            functions: HashMap::new(),
            natives: HashMap::new(),
            routes: HashMap::new(),
            current_module_prefix: String::new(),
            standard_streams: true,
            stdlib_path: None,
            imports: HashMap::new(),
            loaded_files: HashSet::new(),
            record_grad: true,
            record_traces: config.record_traces,
            buffer_cache: std::sync::Arc::new(std::sync::Mutex::new(Vec::with_capacity(32))),
            gas: config.max_gas.unwrap_or(u64::MAX),
            max_gas: config.max_gas.unwrap_or(u64::MAX),
            memory_limit: config
                .memory_limit_mb
                .map(|m| m * 1024 * 1024)
                .unwrap_or(u64::MAX),
            memory_used: 0,
            source_cache: HashMap::new(),
            last_span: None,
            last_hint: None,
            traces: Vec::new(),
            actor_mailbox: None,
        }
    }

    pub fn new_default() -> Self {
        Self::new(VmConfig::default())
    }

    /// Creates a lightweight isolate for an Actor.
    /// Preserves functions, natives, and configuration, but starts with fresh globals/stack.
    pub fn clone_for_actor(&self) -> Self {
        Self {
            globals: HashMap::new(),
            functions: self.functions.clone(),
            natives: self.natives.clone(),
            routes: self.routes.clone(),
            current_module_prefix: self.current_module_prefix.clone(),
            standard_streams: self.standard_streams,
            stdlib_path: self.stdlib_path.clone(),
            imports: self.imports.clone(),
            loaded_files: self.loaded_files.clone(),
            record_grad: self.record_grad,
            record_traces: self.record_traces,
            buffer_cache: self.buffer_cache.clone(),
            gas: self.max_gas,
            max_gas: self.max_gas,
            memory_limit: self.memory_limit,
            memory_used: 0,
            source_cache: self.source_cache.clone(),
            last_span: None,
            last_hint: None,
            traces: Vec::new(),
            actor_mailbox: self.actor_mailbox.clone(),
        }
    }

    pub fn set_stdlib_path(&mut self, path: PathBuf) {
        self.stdlib_path = Some(path);
    }

    pub fn set_limits(&mut self, gas: u64, memory_mb: u64) {
        self.gas = gas;
        self.max_gas = gas;
        self.memory_limit = memory_mb * 1024 * 1024;
        self.memory_used = 0;
    }

    pub fn trace_call(&mut self, name: &str, args: &[Value], module: &str) {
        if !self.record_traces {
            return;
        }
        if self.traces.len() < 1000 {
            self.traces.push(TraceEvent::Call {
                name: name.to_string(),
                args: args.to_vec(),
                module: module.to_string(),
            });
        }
    }

    pub fn trace_return(&mut self, name: &str, value: &Value) {
        if !self.record_traces {
            return;
        }
        if self.traces.len() < 1000 {
            self.traces.push(TraceEvent::Return {
                name: name.to_string(),
                value: value.clone(),
            });
        }
    }

    pub fn trace_native_call(&mut self, name: &str, args: &[Value], result: &Value) {
        if !self.record_traces {
            return;
        }
        if self.traces.len() < 1000 {
            self.traces.push(TraceEvent::NativeCall {
                name: name.to_string(),
                args: args.to_vec(),
                result: result.clone(),
            });
        }
    }

    pub fn trace_error(&mut self, error: &EvalError) {
        if !self.record_traces {
            return;
        }
        if self.traces.len() < 1000 {
            self.traces.push(TraceEvent::Error {
                message: error.message.clone(),
                context: error.stack.join("\n"),
            });
        }
    }

    pub fn get_traces(&self) -> Vec<TraceEvent> {
        self.traces.clone()
    }

    pub fn clear_traces(&mut self) {
        self.traces.clear();
    }

    pub fn dump_trace(&self, path: &Path) -> Result<(), String> {
        let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
        serde_json::to_writer_pretty(file, &self.traces).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn track_memory(&mut self, bytes: u64) -> Result<(), EvalError> {
        if self.memory_limit == u64::MAX {
            return Ok(());
        }
        self.memory_used += bytes;
        if self.memory_used > self.memory_limit {
            return Err(EvalError {
                message: format!(
                    "Memory limit exceeded: {} bytes used, limit is {} bytes",
                    self.memory_used, self.memory_limit
                ),
                stack: vec![],
            });
        }
        Ok(())
    }

    pub fn register_function(&mut self, prefix: &str, decl: FunctionDecl) {
        let name = qualify(prefix, &decl.name);
        self.functions.insert(
            name,
            VmFunction {
                decl,
                module_prefix: prefix.to_string(),
            },
        );
    }

    pub fn register_native(&mut self, name: &str, f: NativeFn) {
        self.natives.insert(name.to_string(), f);
    }

    pub fn register_route(&mut self, path: String, handler: Box<ClosureValue>) {
        self.routes.insert(path, handler);
    }

    pub fn routes(&self) -> &HashMap<String, Box<ClosureValue>> {
        &self.routes
    }

    pub fn load_program(
        &mut self,
        module_prefix: impl Into<String>,
        mut program: Program,
        base_path: Option<&Path>,
    ) -> Result<(), EvalError> {
        let module_prefix = module_prefix.into();

        // --- Aero-AST-Optimizer (AAO) Pass ---
        crate::runtime::execution::loop_optimizer::AeroOptimizer::optimize_program(&mut program);

        for item in &program.items {
            if let ItemKind::Function(f) = &item.kind {
                let full = qualify(&module_prefix, &f.name);
                self.functions.insert(
                    full,
                    VmFunction {
                        decl: f.clone(),
                        module_prefix: module_prefix.clone(),
                    },
                );
            } else if let ItemKind::Enum(e) = &item.kind {
                let mut map = HashMap::new();
                for variant in &e.variants {
                    let vname = variant.name().to_string();
                    map.insert(vname.clone(), Value::Str(vname));
                }
                let full = qualify(&module_prefix, &e.name);
                self.globals.insert(
                    full,
                    Value::Object(std::sync::Arc::new(std::sync::RwLock::new(map))),
                );
            } else if let ItemKind::ModuleValue(m) = &item.kind {
                let full = qualify(&module_prefix, &m.name);
                self.globals.insert(full, Value::Null); // Forward-declare
            } else if let ItemKind::Impl(ib) = &item.kind {
                let self_name = match &ib.self_type {
                    Type::Named(tp) => tp.last_name().to_string(),
                    _ => continue,
                };
                for it in &ib.items {
                    if let ImplItem::Method(m) = it {
                        let full_name = format!("{}::{}", self_name, m.name);
                        let full = qualify(&module_prefix, &full_name);
                        self.functions.insert(
                            full,
                            VmFunction {
                                decl: m.clone(),
                                module_prefix: module_prefix.clone(),
                            },
                        );
                    }
                }
            }
        }

        // Handle modules (recursive)
        for item in &program.items {
            if let ItemKind::Module(m) = &item.kind {
                match m {
                    ModuleDecl::External(name) => {
                        let mut resolved = false;
                        if let Some(base) = base_path {
                            let parent = base.parent().unwrap_or_else(|| Path::new("."));
                            let mod_path = parent.join(format!("{}.nyx", name));
                            if mod_path.exists() {
                                self.load_file(qualify(&module_prefix, name), &mod_path)?;
                                resolved = true;
                            } else {
                                let mod_path = parent.join(name).join("mod.nyx");
                                if mod_path.exists() {
                                    self.load_file(qualify(&module_prefix, name), &mod_path)?;
                                    resolved = true;
                                }
                            }
                        }

                        if !resolved {
                            if let Some(std) = &self.stdlib_path {
                                let mod_path = std.join(format!("{}.nyx", name));
                                if mod_path.exists() {
                                    self.load_file(name, &mod_path)?;
                                } else {
                                    let mod_path = std.join(name).join("mod.nyx");
                                    if mod_path.exists() {
                                        self.load_file(name, &mod_path)?;
                                    }
                                }
                            }
                        }
                    }
                    ModuleDecl::Inline { name, items } => {
                        let inner_program = Program {
                            items: items.clone(),
                        };
                        self.load_program(qualify(&module_prefix, name), inner_program, base_path)?;
                    }
                }
            }
        }

        for item in &program.items {
            if let ItemKind::Use(u) = &item.kind {
                self.process_use_tree(&u.tree, &module_prefix);
            }
        }

        // Execute statics
        for item in &program.items {
            match &item.kind {
                ItemKind::Static(s) => {
                    let v = self.eval_expr_with_module(
                        &s.value,
                        &module_prefix,
                        &mut HashMap::new(),
                        0,
                    )?;
                    let full = qualify(&module_prefix, &s.name);
                    self.globals.insert(full, v);
                }
                ItemKind::Const(c) => {
                    let v = self.eval_expr_with_module(
                        &c.value,
                        &module_prefix,
                        &mut HashMap::new(),
                        0,
                    )?;
                    let full = qualify(&module_prefix, &c.name);
                    self.globals.insert(full, v);
                }
                ItemKind::ModuleValue(m) => {
                    let v = self.eval_expr_with_module(
                        &m.value,
                        &module_prefix,
                        &mut HashMap::new(),
                        0,
                    )?;
                    let full = qualify(&module_prefix, &m.name);
                    self.globals.insert(full, v);
                }
                _ => {}
            }
        }

        Ok(())
    }

    pub fn load_file(
        &mut self,
        module_prefix: impl Into<String>,
        path: &Path,
    ) -> Result<(), EvalError> {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if self.loaded_files.contains(&canonical) {
            return Ok(());
        }
        self.loaded_files.insert(canonical.clone());

        let prefix = module_prefix.into();
        let program = parse_program(path).map_err(|e| EvalError {
            message: e,
            stack: vec![],
        })?;
        self.load_program(prefix, program, Some(path))
    }

    pub fn load_engine_from_manifest(
        &mut self,
        engine_root: &Path,
        manifest_rel: &str,
    ) -> Result<(), EvalError> {
        let manifest_path = engine_root.join(manifest_rel);
        let raw = std::fs::read_to_string(&manifest_path).map_err(|e| EvalError {
            message: format!(
                "failed to read engine manifest {}: {e}",
                manifest_path.display()
            ),
            stack: vec![],
        })?;

        #[derive(serde::Deserialize)]
        struct EngineManifest {
            name: Option<String>,
            entry: Option<String>,
            modules: Vec<String>,
        }

        let manifest: EngineManifest = serde_json::from_str(&raw).map_err(|e| EvalError {
            message: format!(
                "invalid engine manifest JSON {}: {e}",
                manifest_path.display()
            ),
            stack: vec![],
        })?;

        let base_prefix = manifest.name.as_deref().unwrap_or_else(|| {
            engine_root
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
        });

        for rel in manifest.modules {
            let module_path = engine_root.join(rel.trim_start_matches("./"));
            let rel_mod = rel.trim_start_matches("./");
            let mut parts: Vec<&str> = rel_mod.split('/').collect();

            // Skip "src" or "source" at the start of the module path
            if !parts.is_empty() && (parts[0] == "src" || parts[0] == "source") {
                parts.remove(0);
            }

            if !parts.is_empty() {
                let last_idx = parts.len() - 1;
                let stem = if parts[last_idx].ends_with(".nyx") {
                    parts[last_idx]
                        .strip_suffix(".nyx")
                        .unwrap_or(&parts[last_idx])
                } else {
                    parts[last_idx]
                };

                if stem == "mod" {
                    parts.pop();
                } else if parts.len() > 1 && parts[parts.len() - 2] == stem {
                    parts.pop();
                } else if parts[last_idx].ends_with(".nyx") {
                    parts[last_idx] = stem;
                }
            }

            let sub_prefix = parts.join("::");
            let full_prefix = if sub_prefix.is_empty() {
                base_prefix.to_string()
            } else {
                format!("{base_prefix}::{sub_prefix}")
            };

            self.load_file(full_prefix, &module_path)?;
        }

        if let Some(entry) = manifest.entry {
            let entry_path = engine_root.join(entry.trim_start_matches("./"));
            self.load_file(base_prefix.to_string(), &entry_path)?;

            // Attempt to alias base_prefix::base_prefix OR base_prefix::main::base_prefix to base_prefix
            let main_obj_direct = format!("{base_prefix}::{base_prefix}");
            let main_obj_main = format!("{base_prefix}::main::{base_prefix}");

            if let Some(val) = self.globals.get(&main_obj_direct).cloned() {
                self.globals.insert(base_prefix.to_string(), val);
            } else if let Some(val) = self.globals.get(&main_obj_main).cloned() {
                self.globals.insert(base_prefix.to_string(), val);
            }
        }

        Ok(())
    }

    pub fn execute_main(&mut self) -> Result<Value, EvalError> {
        if self.has_function("main") {
            self.call_function("main", vec![])
        } else {
            Ok(Value::Null)
        }
    }

    pub fn call_function(&mut self, name: &str, args: Vec<Value>) -> Result<Value, EvalError> {
        // First try natives
        if let Some(n) = self.natives.get(name).copied() {
            let res = n(self, &args);
            match &res {
                Ok(v) => self.trace_native_call(name, &args, v),
                Err(e) => self.trace_error(e),
            }
            return res;
        }

        let (decl, prefix) = match self.resolve_function(name, "") {
            Some(v) => v,
            None => {
                let err = EvalError::new(format!("Unknown function '{}'", name));
                self.trace_error(&err);
                return Err(err);
            }
        };

        let rs_res = self.eval_fn(&decl, &prefix, args);
        if let Err(ref e) = rs_res {
            self.trace_error(e);
        }
        let rs = rs_res?;

        // Tag objects with their origin module for smarter UFCS resolution
        if let Value::Object(map_rc) = &rs {
            let mut map = map_rc.write().unwrap_or_else(|e| e.into_inner());
            if !map.contains_key("__origin__") {
                map.insert("__origin__".to_string(), Value::Str(prefix.to_string()));
            }
        }

        Ok(rs)
    }

    pub fn render_http_via_routes_or_app(&mut self, path: &str) -> Result<Value, EvalError> {
        let path = match path {
            "/index.html" => "/",
            other => other,
        };

        let req = Value::object(HashMap::from([
            ("path".to_string(), Value::Str(path.to_string())),
            ("method".to_string(), Value::Str("GET".to_string())),
        ]));

        if let Some(handler) = self.routes.get(path).cloned() {
            return self.call_closure(&handler, vec![req]);
        }

        if path != "/" {
            // Check if App handles routing internally instead of simply returning 404
            if self.has_function("App") {
                return self
                    .call_function("App", vec![req.clone()])
                    .or_else(|_| self.call_function("App", vec![]));
            } else if self.has_function("app") {
                return self
                    .call_function("app", vec![req.clone()])
                    .or_else(|_| self.call_function("app", vec![]));
            }
            return Ok(Value::Null);
        }

        if self.has_function("App") {
            self.call_function("App", vec![req.clone()])
        } else if self.has_function("app") {
            self.call_function("app", vec![req.clone()])
        } else {
            Err(EvalError {
                message: "Unknown function 'App'".to_string(),
                stack: vec![],
            })
        }
    }

    pub fn render_app_fragment(&mut self) -> Result<String, EvalError> {
        let req = Value::object(HashMap::from([
            ("path".to_string(), Value::Str("/".to_string())),
            ("method".to_string(), Value::Str("GET".to_string())),
        ]));

        let app = if self.has_function("App") {
            self.call_function("App", vec![req.clone()])?
        } else if self.has_function("app") {
            self.call_function("app", vec![req.clone()])?
        } else {
            return Err(EvalError {
                message: "Unknown function 'App'".to_string(),
                stack: vec![],
            });
        };

        let Value::Node(root) = app else {
            return Err(EvalError {
                message: "App() did not return a ui::VNode".to_string(),
                stack: vec![],
            });
        };
        Ok(render_node(&root))
    }

    pub fn has_function(&self, name: &str) -> bool {
        self.resolve_function(name, "").is_some()
    }

    fn process_use_tree(
        &mut self,
        tree: &crate::core::ast::ast_nodes::UseTree,
        current_prefix: &str,
    ) {
        use crate::core::ast::ast_nodes::UseTree;
        match tree {
            UseTree::Name { name, alias } => {
                let local_name = alias.clone().unwrap_or_else(|| name.clone());
                let qualified = if current_prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{}::{}", current_prefix, name)
                };
                self.imports.insert(local_name, qualified);
            }
            UseTree::Path { segment, child } => {
                let next_prefix = if current_prefix.is_empty() {
                    segment.clone()
                } else {
                    format!("{}::{}", current_prefix, segment)
                };
                self.process_use_tree(child, &next_prefix);
            }
            UseTree::Group(children) => {
                for child in children {
                    self.process_use_tree(child, current_prefix);
                }
            }
            UseTree::Glob => {
                // Not supported for simple interpreter mapping yet
            }
        }
    }

    fn resolve_function(&self, name: &str, current_prefix: &str) -> Option<(FunctionDecl, String)> {
        // 1. Try absolute/direct match
        if let Some(f) = self.functions.get(name) {
            return Some((f.decl.clone(), f.module_prefix.clone()));
        }

        // 2. Try prefix-relative match
        if !current_prefix.is_empty() {
            let qualified = format!("{}::{}", current_prefix, name);
            if let Some(f) = self.functions.get(&qualified) {
                return Some((f.decl.clone(), f.module_prefix.clone()));
            }
        }

        // 3. Try map through imports
        if !name.contains("::") {
            if let Some(qualified) = self.imports.get(name) {
                // Try the qualified name directly
                if let Some(f) = self.functions.get(qualified) {
                    return Some((f.decl.clone(), f.module_prefix.clone()));
                }

                // If it's a module, it might be qualified further
                if !current_prefix.is_empty() {
                    let double_qualified = format!("{}::{}", current_prefix, qualified);
                    if let Some(f) = self.functions.get(&double_qualified) {
                        return Some((f.decl.clone(), f.module_prefix.clone()));
                    }
                }
            }
        }

        // Handle ui:: or ui_engine:: alias for main::
        if name.starts_with("ui::") || name.starts_with("ui_engine::") {
            let prefix_len = if name.starts_with("ui::") { 4 } else { 11 };
            let inner = &name[prefix_len..];
            if let Some(res) = self.resolve_function(&format!("main::{inner}"), "") {
                return Some(res);
            }
        }

        if let Some(inner) = name.strip_prefix("ui_runtime::event_bus::") {
            if let Some(res) = self.resolve_function(&format!("ui_runtime::event_bus::{inner}"), "")
            {
                return Some(res);
            }
        }

        if let Some(inner) = name.strip_prefix("ui_runtime::input::") {
            if let Some(res) = self.resolve_function(&format!("ui_runtime::input::{inner}"), "") {
                return Some(res);
            }
        }

        if let Some(inner) = name.strip_prefix("ui_runtime::platform::") {
            if let Some(res) = self.resolve_function(&format!("ui_runtime::platform::{inner}"), "")
            {
                return Some(res);
            }
        }

        if let Some(inner) = name.strip_prefix("platform::") {
            if let Some(res) = self.resolve_function(&format!("ui_runtime::platform::{inner}"), "")
            {
                return Some(res);
            }
        }

        // Qualified match with current module
        let qualified = qualify(&self.current_module_prefix, name);
        if let Some(f) = self.functions.get(&qualified) {
            return Some((f.decl.clone(), f.module_prefix.clone()));
        }

        if name.starts_with("std::") {
            return None;
        }

        let parts: Vec<&str> = name.split("::").collect();
        if parts.len() >= 2 {
            let mut candidates = Vec::new();
            let suffix = format!("::{}", parts.last().unwrap_or(&""));
            let mod_segment = parts[parts.len() - 2];
            let mod_pattern1 = format!("::{}::", mod_segment);
            let mod_pattern2 = format!("{}::", mod_segment);

            for (full_name, f) in &self.functions {
                if full_name.ends_with(&suffix)
                    && (full_name.contains(&mod_pattern1) || full_name.starts_with(&mod_pattern2))
                {
                    candidates.push((full_name, f));
                }
            }

            if candidates.len() == 1 {
                let (_full_name, f) = candidates[0];
                return Some((f.decl.clone(), f.module_prefix.clone()));
            } else if candidates.len() > 1 {
                let (_full_name, f) = candidates[0];
                return Some((f.decl.clone(), f.module_prefix.clone()));
            }
        }

        // Search in same engine root (e.g. web_engine::*)
        if !self.current_module_prefix.is_empty() && !name.contains("::") {
            let root = self.current_module_prefix.split("::").next().unwrap_or("");
            if !root.is_empty() {
                let suffix = format!("::{}", name);
                let root_prefix = format!("{}::", root);
                let mut candidates = Vec::new();
                for (full_name, f) in &self.functions {
                    if full_name.starts_with(&root_prefix) && full_name.ends_with(&suffix) {
                        candidates.push((full_name, f));
                    }
                }
                if candidates.len() == 1 {
                    let (_full_name, f) = candidates[0];
                    return Some((f.decl.clone(), f.module_prefix.clone()));
                }
            }
        }

        None
    }

    pub fn eval_fn(
        &mut self,
        decl: &FunctionDecl,
        module_prefix: &str,
        args: Vec<Value>,
    ) -> Result<Value, EvalError> {
        let mut locals = HashMap::new();
        for (i, param) in decl.params.iter().enumerate() {
            if let Some(val) = args.get(i) {
                locals.insert(param.name.clone(), val.clone());
            }
        }

        self.trace_call(&decl.name, &args, module_prefix);

        let out = (|| {
            for stmt in &decl.body {
                match self.eval_stmt_raw(decl, module_prefix, stmt, &mut locals)? {
                    Control::Return(v) => return Ok(v),
                    Control::Break => return Err(EvalError::new("Break outside loop")),
                    Control::ContinueLoop => return Err(EvalError::new("Continue outside loop")),
                    Control::Continue => {}
                }
            }
            Ok(Value::Null)
        })();

        if let Ok(ref res) = out {
            self.trace_return(&decl.name, res);
        }
        out
    }

    #[allow(dead_code)]
    pub(crate) fn eval_stmt(
        &mut self,
        stmt: &Stmt,
        locals: &mut HashMap<String, Value>,
    ) -> Result<Control, EvalError> {
        let dummy_fn = FunctionDecl {
            name: "eval_stmt_dummy".to_string(),
            is_async: false,
            is_extern: false,
            extern_abi: None,
            generics: vec![],
            params: vec![],
            return_type: None,
            where_clauses: vec![],
            body: vec![],
            span: crate::core::diagnostics::Span::default(),
        };
        self.eval_stmt_raw(&dummy_fn, "", stmt, locals)
    }

    fn eval_stmt_raw(
        &mut self,
        current_fn: &FunctionDecl,
        module_prefix: &str,
        stmt: &Stmt,
        locals: &mut HashMap<String, Value>,
    ) -> Result<Control, EvalError> {
        self.last_span = Some(*stmt.span());
        self.gas = self.gas.saturating_sub(1);
        if self.gas == 0 && self.max_gas != u64::MAX {
            return Err(EvalError::new(
                "Gas limit exceeded (infinite loop or resource exhaustion)",
            ));
        }

        match stmt {
            Stmt::Let {
                name, expr, span, ..
            } => {
                let v = self
                    .eval_expr_with_module(expr, module_prefix, locals, 0)
                    .map_err(|e| {
                        e.with_frame(&current_fn.name, span.start.line, span.start.column)
                    })?;
                locals.insert(name.clone(), v);
                Ok(Control::Continue)
            }
            Stmt::Assign {
                target,
                value,
                span,
            } => {
                let v = self
                    .eval_expr_with_module(value, module_prefix, locals, 0)
                    .map_err(|e| {
                        e.with_frame(&current_fn.name, span.start.line, span.start.column)
                    })?;
                self.assign_target(locals, target, v)?;
                Ok(Control::Continue)
            }
            Stmt::CompoundAssign {
                target,
                op,
                value,
                span,
            } => {
                let rhs = self
                    .eval_expr_with_module(value, module_prefix, locals, 0)
                    .map_err(|e| {
                        e.with_frame(&current_fn.name, span.start.line, span.start.column)
                    })?;
                let lhs = self
                    .eval_expr_with_module(target, module_prefix, locals, 0)
                    .unwrap_or(Value::Null);
                let base_op = if op.ends_with('=') {
                    &op[..op.len() - 1]
                } else {
                    op.as_str()
                };
                let out = eval_binary(&lhs, base_op, &rhs)?;
                self.assign_target(locals, target, out)?;
                Ok(Control::Continue)
            }
            Stmt::Return { expr, span } => {
                let v = if let Some(e) = expr {
                    self.eval_expr_with_module(e, module_prefix, locals, 0)
                        .map_err(|er| {
                            er.with_frame(&current_fn.name, span.start.line, span.start.column)
                        })?
                } else {
                    Value::Null
                };
                Ok(Control::Return(v))
            }
            Stmt::If {
                branches,
                else_body,
                span,
            } => {
                for br in branches {
                    let cond = self
                        .eval_expr_with_module(&br.condition, module_prefix, locals, 0)
                        .map_err(|er| {
                            er.with_frame(&current_fn.name, span.start.line, span.start.column)
                        })?;
                    if cond.is_truthy() {
                        for st in &br.body {
                            match self.eval_stmt_raw(current_fn, module_prefix, st, locals)? {
                                Control::Continue => {}
                                other => return Ok(other),
                            }
                        }
                        return Ok(Control::Continue);
                    }
                }
                if let Some(body) = else_body {
                    for st in body {
                        match self.eval_stmt_raw(current_fn, module_prefix, st, locals)? {
                            Control::Continue => {}
                            other => return Ok(other),
                        }
                    }
                }
                Ok(Control::Continue)
            }
            Stmt::While {
                condition,
                body,
                span,
            } => {
                // Aero-Loops Optimization Tier 1 & 2
                use crate::runtime::execution::aero_jit::AeroJit;
                use crate::runtime::execution::loop_optimizer::{AeroOptimizer, LoopAnalysis};

                match AeroOptimizer::analyze_while(condition, body, locals) {
                    LoopAnalysis::Deterministic(updates) => {
                        for (name, val) in updates {
                            locals.insert(name, val);
                        }
                        return Ok(Control::Continue);
                    }
                    LoopAnalysis::JitReady => {
                        println!("Aero-JIT Tier 2: Compiling hot-loop...");
                        AeroJit::execute_loop(condition, body, locals)
                            .map_err(|e| EvalError::new(format!("Aero-JIT Error: {}", e)))?;
                        return Ok(Control::Continue);
                    }
                    LoopAnalysis::Standard => {}
                }

                loop {
                    let cond = self
                        .eval_expr_with_module(condition, module_prefix, locals, 0)
                        .map_err(|er| {
                            er.with_frame(&current_fn.name, span.start.line, span.start.column)
                        })?;
                    if !cond.is_truthy() {
                        break;
                    }
                    for st in body {
                        match self.eval_stmt_raw(current_fn, module_prefix, st, locals)? {
                            Control::Continue => {}
                            Control::Break => return Ok(Control::Continue),
                            Control::ContinueLoop => break,
                            Control::Return(v) => return Ok(Control::Return(v)),
                        }
                    }
                }
                Ok(Control::Continue)
            }
            Stmt::ForIn {
                var,
                iter,
                body,
                span: _,
            } => {
                let iterable = self.eval_expr_with_module(iter, module_prefix, locals, 0)?;
                if let Value::Array(items_rc) = iterable {
                    let items = items_rc.read().unwrap_or_else(|e| e.into_inner()).clone();
                    for item in items {
                        locals.insert(var.clone(), item.clone());
                        for st in body {
                            match self.eval_stmt_raw(current_fn, module_prefix, st, locals)? {
                                Control::Continue => {}
                                Control::Break => return Ok(Control::Continue),
                                Control::ContinueLoop => break,
                                Control::Return(v) => return Ok(Control::Return(v)),
                            }
                        }
                    }
                }
                Ok(Control::Continue)
            }
            Stmt::Break { .. } => Ok(Control::Break),
            Stmt::Continue { .. } => Ok(Control::ContinueLoop),
            Stmt::Expr(e) | Stmt::Print { expr: e } => {
                let value = self
                    .eval_expr_with_module(e, module_prefix, locals, 0)
                    .map_err(|er| {
                        er.with_frame(
                            &current_fn.name,
                            current_fn.span.start.line,
                            current_fn.span.start.column,
                        )
                    })?;
                if matches!(stmt, Stmt::Print { .. }) {
                    println!("{}", to_stringish(&value));
                }
                Ok(Control::Continue)
            }
            Stmt::Unsafe { body, .. } => {
                // Unsafe blocks just execute their body in the current context.
                for st in body {
                    match self.eval_stmt_raw(current_fn, module_prefix, st, locals)? {
                        Control::Continue => {}
                        other => return Ok(other),
                    }
                }
                Ok(Control::Continue)
            }
            Stmt::InlineAsm { code, .. } => {
                // Extreme Level Zero: Emulated assembly execution.
                // In a real JIT this would emit machine code.
                // Here we bridge it to an emulated hypercall or log it.
                println!("[nyx-vm] inline asm: {}", code);
                Ok(Control::Continue)
            }
            _ => Ok(Control::Continue),
        }
    }

    fn eval_expr_with_module(
        &mut self,
        expr: &Expr,
        module_prefix: &str,
        locals: &mut HashMap<String, Value>,
        _depth: usize,
    ) -> Result<Value, EvalError> {
        let prev_prefix =
            std::mem::replace(&mut self.current_module_prefix, module_prefix.to_string());
        let out = self.eval_expr(expr, locals);
        self.current_module_prefix = prev_prefix;
        out
    }

    fn eval_expr(
        &mut self,
        expr: &Expr,
        locals: &mut HashMap<String, Value>,
    ) -> Result<Value, EvalError> {
        match expr {
            Expr::NullLiteral { .. } => Ok(Value::Null),
            Expr::IntLiteral { value: i, .. } => Ok(Value::Int(*i)),
            Expr::FloatLiteral { value: f, .. } => Ok(Value::Float(*f)),
            Expr::BigIntLiteral { value: v, .. } => Ok(Value::BigInt(v.clone())),
            Expr::BoolLiteral { value: b, .. } => Ok(Value::Bool(*b)),
            Expr::StringLiteral { value: s, .. } => Ok(Value::Str(s.clone())),
            Expr::CssLiteral { value: raw, .. } => {
                // 1. Resolve ${ expr } interpolations at runtime.
                //    For now, we perform a simple variable-name substitution
                //    by scanning for ${name} patterns and looking up locals/globals.
                let resolved = resolve_css_interpolations(raw, locals, &self.globals);
                // 2. Parse the resolved CSS text into (property, value) pairs.
                let declarations = match crate::core::css_parser::parse_css(&resolved) {
                    Ok(d) => d,
                    Err(e) => {
                        return Err(EvalError {
                            message: format!("css literal error: {e}"),
                            stack: vec![],
                        });
                    }
                };
                // 3. Build a Value::Object (Map<string, string>)
                let mut map = std::collections::HashMap::new();
                for decl in declarations {
                    map.insert(decl.property, Value::Str(decl.value));
                }
                Ok(Value::Object(std::sync::Arc::new(std::sync::RwLock::new(
                    map,
                ))))
            }
            Expr::CharLiteral { value: c, .. } => Ok(Value::Str(c.to_string())),
            Expr::Identifier { name, .. } => Ok(locals
                .get(name)
                .cloned()
                .or_else(|| self.globals.get(name).cloned())
                .or_else(|| {
                    let qualified = qualify(&self.current_module_prefix, name);
                    self.globals.get(&qualified).cloned()
                })
                .unwrap_or_else(|| {
                    let cp = self.current_module_prefix.clone();
                    if let Some((decl, prefix)) = self.resolve_function(name, &cp) {
                        Value::Closure(Box::new(ClosureValue {
                            params: decl.params.iter().map(|p| p.name.clone()).collect(),
                            body: Expr::Block {
                                stmts: decl.body.clone(),
                                tail_expr: None,
                                span: Span::default(),
                            },
                            captured: HashMap::new(),
                            module_prefix: prefix,
                        }))
                    } else {
                        Value::Null
                    }
                })),
            Expr::Path {
                segments: parts, ..
            } => {
                let mut parts = parts.clone();
                if !parts.is_empty() {
                    if let Some(imported) = self.imports.get(&parts[0]) {
                        parts[0] = imported.replace('/', "::");
                    }
                }

                let full = parts.join("::");
                if let Some(v) = self.globals.get(&full) {
                    return Ok(v.clone());
                }

                let mut current_val = None;
                let mut full_path = String::new();
                for (i, part) in parts.iter().enumerate() {
                    if i > 0 {
                        full_path.push_str("::");
                    }
                    full_path.push_str(part);

                    if i == 0 {
                        current_val = self.globals.get(&full_path).cloned();
                    } else if let Some(Value::Object(map_rc)) = current_val {
                        current_val = map_rc
                            .read()
                            .unwrap_or_else(|e| e.into_inner())
                            .get(part)
                            .cloned();
                    } else {
                        current_val = None;
                        break;
                    }
                }

                if let Some(v) = current_val {
                    Ok(v)
                } else {
                    let cp = self.current_module_prefix.clone();
                    if let Some((decl, prefix)) = self.resolve_function(&full, &cp) {
                        Ok(Value::Closure(Box::new(ClosureValue {
                            params: decl.params.iter().map(|p| p.name.clone()).collect(),
                            body: Expr::Block {
                                stmts: decl.body.clone(),
                                tail_expr: None,
                                span: Span::default(),
                            },
                            captured: HashMap::new(),
                            module_prefix: prefix,
                        })))
                    } else {
                        Ok(Value::Str(full))
                    }
                }
            }
            Expr::TupleLiteral {
                elements: items, ..
            }
            | Expr::ArrayLiteral {
                elements: items, ..
            } => {
                let mut out = Vec::with_capacity(items.len());
                for it in items {
                    out.push(self.eval_expr(it, locals)?);
                }
                Ok(Value::Array(std::sync::Arc::new(std::sync::RwLock::new(
                    out,
                ))))
            }
            Expr::ArrayRepeat { value, len, .. } => {
                let repeated = self.eval_expr(value, locals)?;
                let len_value = self.eval_expr(len, locals)?;
                let count = match len_value {
                    Value::Int(i) if i > 0 => i as usize,
                    _ => 0,
                };
                Ok(Value::Array(std::sync::Arc::new(std::sync::RwLock::new(
                    vec![repeated; count],
                ))))
            }
            Expr::BlockLiteral { items, .. } => Ok(Value::Object(std::sync::Arc::new(
                std::sync::RwLock::new(eval_fields(self, items, locals)?),
            ))),
            Expr::StructLiteral { name, fields, .. } => {
                let mut obj = eval_field_inits(self, fields, locals)?;
                obj.insert("__type".to_string(), Value::Str(name.clone()));
                Ok(Value::Object(std::sync::Arc::new(std::sync::RwLock::new(
                    obj,
                ))))
            }
            Expr::Binary {
                left, op, right, ..
            } => {
                let l = self.eval_expr(left, locals)?;
                let r = self.eval_expr(right, locals)?;
                eval_binary(&l, op, &r)
            }
            Expr::Unary { op, right, .. } => {
                let v = self.eval_expr(right, locals)?;
                match op.as_str() {
                    "!" => Ok(Value::Bool(!v.is_truthy())),
                    "-" => match v {
                        Value::Int(i) => Ok(Value::Int(-i)),
                        Value::Float(f) => Ok(Value::Float(-f)),
                        _ => Ok(Value::Null),
                    },
                    "~" => match v {
                        Value::Int(i) => Ok(Value::Int(!i)),
                        _ => Ok(Value::Null),
                    },
                    "*" => Ok(v), // Deref transparency for VM values
                    _ => Ok(Value::Null),
                }
            }
            Expr::Reference {
                mutable: _, expr, ..
            } => {
                // Address-of optimization: if it's a Bytes buffer, return its actual pointer.
                let val = self.eval_expr(expr, locals)?;
                match val {
                    Value::Bytes(b_rc) => {
                        let addr = b_rc.read().unwrap_or_else(|e| e.into_inner()).as_ptr() as u64;
                        Ok(Value::Pointer(addr))
                    }
                    Value::Pointer(p) => Ok(Value::Pointer(p)),
                    _ => {
                        // In a real JIT/Compiler we'd get the stack/heap address.
                        // In the interpreter, we can't safely return a pointer to the HashMap entry.
                        Ok(Value::Pointer(0))
                    }
                }
            }
            Expr::Deref { expr, .. } => {
                let val = self.eval_expr(expr, locals)?;
                if let Value::Pointer(addr) = val {
                    unsafe {
                        // Safety: This is "Level Zero", we assume the pointer is valid.
                        let ptr = addr as *const u8;
                        Ok(Value::Int(*ptr as i64))
                    }
                } else {
                    Ok(Value::Null)
                }
            }
            Expr::Cast { expr, ty: _, .. } => {
                let val = self.eval_expr(expr, locals)?;
                match val {
                    Value::Int(i) => Ok(Value::Pointer(i as u64)),
                    Value::Pointer(p) => Ok(Value::Int(p as i64)),
                    _ => Ok(val),
                }
            }
            Expr::TryOp { expr, .. } => self.eval_expr(expr, locals),
            Expr::Ternary {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
                let cond = self.eval_expr(condition, locals)?;
                if cond.is_truthy() {
                    self.eval_expr(then_expr, locals)
                } else {
                    self.eval_expr(else_expr, locals)
                }
            }
            Expr::IfExpr {
                branches,
                else_body,
                ..
            } => {
                for br in branches {
                    let cond = self.eval_expr(&br.condition, locals)?;
                    if cond.is_truthy() {
                        return self.eval_stmt_block_expr(&br.body, locals);
                    }
                }
                if let Some(body) = else_body {
                    return self.eval_expr(body, locals);
                }
                Ok(Value::Null)
            }
            Expr::Match { expr, arms, .. } => {
                let val = self.eval_expr(expr, locals)?;
                for arm in arms {
                    if matches_pattern(&val, &arm.pattern, locals) {
                        if let Some(guard) = &arm.guard {
                            if !self.eval_expr(guard, locals)?.is_truthy() {
                                continue;
                            }
                        }
                        match &arm.body {
                            MatchBody::Expr(e) => return self.eval_expr(e, locals),
                            MatchBody::Stmt(s) => {
                                match self.eval_stmt_raw(
                                    &FunctionDecl {
                                        name: "match_arm".to_string(),
                                        is_async: false,
                                        is_extern: false,
                                        extern_abi: None,
                                        generics: vec![],
                                        params: vec![],
                                        return_type: None,
                                        where_clauses: vec![],
                                        body: vec![],
                                        span: Span::default(),
                                    },
                                    &self.current_module_prefix.clone(),
                                    s,
                                    locals,
                                )? {
                                    Control::Continue => return Ok(Value::Null),
                                    Control::Return(v) => return Ok(v),
                                    _ => return Ok(Value::Null),
                                }
                            }
                            MatchBody::Block(stmts) => {
                                let mut inner = locals.clone();
                                return self.eval_stmt_block_expr(stmts, &mut inner);
                            }
                        }
                    }
                }
                Ok(Value::Null)
            }
            Expr::Call { callee, args, .. } => {
                let callee_val = self.eval_expr(callee, locals)?;
                let mut av = Vec::with_capacity(args.len());
                for a in args {
                    av.push(self.eval_expr(a, locals)?);
                }
                match callee_val {
                    Value::Closure(clo) => self.call_closure(&clo, av),
                    Value::Str(name) => self.eval_call(&name, &av),
                    _ => {
                        let callee_name = expr_callee_name(callee);
                        self.eval_call(&callee_name, &av)
                    }
                }
            }
            Expr::MethodCall {
                receiver,
                method,
                args,
                ..
            } => {
                let rv = self.eval_expr(receiver, locals)?;
                let mut av = Vec::with_capacity(args.len());
                for a in args {
                    av.push(self.eval_expr(a, locals)?);
                }
                eval_method(self, rv, method, &av)
            }
            Expr::Closure { params, body, .. } => {
                let captured = capture_env(&self.globals, locals);
                Ok(Value::Closure(Box::new(ClosureValue {
                    params: params.iter().map(|p| p.name.clone()).collect(),
                    body: (**body).clone(),
                    captured,
                    module_prefix: self.current_module_prefix.clone(),
                })))
            }
            Expr::Block {
                stmts,
                tail_expr: tail,
                ..
            } => {
                // Parser ambiguity: `{}` is sometimes used as an empty attribute map.
                if stmts.is_empty() && tail.is_none() {
                    return Ok(Value::object(HashMap::new()));
                }
                let mut inner = locals.clone();
                let dummy_fn = FunctionDecl {
                    name: "<block>".to_string(),
                    is_async: false,
                    is_extern: false,
                    extern_abi: None,
                    generics: vec![],
                    params: vec![],
                    return_type: None,
                    where_clauses: vec![],
                    body: vec![],
                    span: crate::core::diagnostics::Span::default(),
                };
                let prefix = self.current_module_prefix.clone();
                for st in stmts {
                    if let Control::Return(v) =
                        self.eval_stmt_raw(&dummy_fn, &prefix, st, &mut inner)?
                    {
                        return Ok(v);
                    }
                }
                if let Some(t) = tail {
                    return self.eval_expr(t, &mut inner);
                }
                Ok(Value::Null)
            }
            Expr::AsyncBlock { body, .. } => {
                let mut inner = locals.clone();
                let dummy_fn = FunctionDecl {
                    name: "<async_block>".to_string(),
                    is_async: true,
                    is_extern: false,
                    extern_abi: None,
                    generics: vec![],
                    params: vec![],
                    return_type: None,
                    where_clauses: vec![],
                    body: vec![],
                    span: crate::core::diagnostics::Span::default(),
                };
                let prefix = self.current_module_prefix.clone();
                for st in body {
                    if let Control::Return(v) =
                        self.eval_stmt_raw(&dummy_fn, &prefix, st, &mut inner)?
                    {
                        return Ok(v);
                    }
                }
                Ok(Value::Null)
            }
            Expr::Loop { expr, .. } => {
                loop {
                    match self.eval_expr(expr, locals)? {
                        // In Nyx, loop expressions can 'break' with a value.
                        // For now we just loop.
                        _ => {}
                    }
                }
            }
            Expr::FieldAccess { object, field, .. } => {
                let o = self.eval_expr(object, locals)?;
                if let Value::Object(map_rc) = o {
                    let map = map_rc.read().unwrap_or_else(|e| e.into_inner());
                    let mut val = Value::Null;
                    let mut _found = false;
                    for (k, v) in map.iter() {
                        if k == field {
                            val = v.clone();
                            _found = true;
                            break;
                        }
                    }
                    Ok(val)
                } else {
                    Ok(Value::Null)
                }
            }
            Expr::Index { object, index, .. } => {
                let o = self.eval_expr(object, locals)?;
                let i = self.eval_expr(index, locals)?;
                match (&o, &i) {
                    (Value::Array(v_rc), Value::Int(idx)) => Ok(v_rc
                        .read()
                        .unwrap_or_else(|e| e.into_inner())
                        .get(*idx as usize)
                        .cloned()
                        .unwrap_or(Value::Null)),
                    (Value::FloatArray(f_rc), Value::Int(idx)) => Ok(f_rc
                        .read()
                        .unwrap_or_else(|e| e.into_inner())
                        .get(*idx as usize)
                        .cloned()
                        .map(|v| Value::Float(v as f64))
                        .unwrap_or(Value::Null)),
                    (Value::DoubleArray(d_rc), Value::Int(idx)) => Ok(d_rc
                        .read()
                        .unwrap_or_else(|e| e.into_inner())
                        .get(*idx as usize)
                        .cloned()
                        .map(Value::Float)
                        .unwrap_or(Value::Null)),
                    (Value::Object(m_rc), Value::Int(idx)) => Ok(m_rc
                        .read()
                        .unwrap_or_else(|e| e.into_inner())
                        .get(&idx.to_string())
                        .cloned()
                        .unwrap_or(Value::Null)),
                    (Value::Object(m_rc), Value::Str(s)) => Ok(m_rc
                        .read()
                        .unwrap_or_else(|e| e.into_inner())
                        .get(s)
                        .cloned()
                        .unwrap_or(Value::Null)),
                    _ => Ok(Value::Null),
                }
            }
            Expr::Slice {
                object, start, end, ..
            } => {
                let o = self.eval_expr(object, locals)?;
                let start_v = start
                    .as_ref()
                    .map(|e| self.eval_expr(e, locals))
                    .transpose()?;
                let end_v = end
                    .as_ref()
                    .map(|e| self.eval_expr(e, locals))
                    .transpose()?;
                let start_idx = match start_v {
                    Some(Value::Int(i)) if i >= 0 => i as usize,
                    _ => 0,
                };
                let end_idx = match end_v {
                    Some(Value::Int(i)) if i >= 0 => Some(i as usize),
                    _ => None,
                };
                match o {
                    Value::Array(v_rc) => {
                        let v = v_rc.read().unwrap_or_else(|e| e.into_inner());
                        let end_idx = end_idx.unwrap_or(v.len()).min(v.len());
                        let start_idx = start_idx.min(end_idx);
                        Ok(Value::Array(std::sync::Arc::new(std::sync::RwLock::new(
                            v[start_idx..end_idx].to_vec(),
                        ))))
                    }
                    Value::Str(s) => {
                        let chars: Vec<char> = s.chars().collect();
                        let end_idx = end_idx.unwrap_or(chars.len()).min(chars.len());
                        let start_idx = start_idx.min(end_idx);
                        let out: String = chars[start_idx..end_idx].iter().collect();
                        Ok(Value::Str(out))
                    }
                    _ => Ok(Value::Null),
                }
            }
            _ => Ok(Value::Null),
        }
    }

    fn eval_call(&mut self, callee: &str, args: &[Value]) -> Result<Value, EvalError> {
        // Strip generic type parameters from callee names.
        let stripped_owned: String;
        let callee = if callee.contains('<') {
            stripped_owned = strip_generics(callee);
            &stripped_owned
        } else {
            callee
        };

        // Native/builtin functions.
        let mut target = callee;
        if !self.natives.contains_key(target) && target.starts_with("main::") {
            target = &target[6..];
        }

        if let Some(n) = self.natives.get(target).copied() {
            let res = n(self, args);
            match &res {
                Ok(v) => self.trace_native_call(target, args, v),
                Err(e) => self.trace_error(e),
            }
            return res;
        }

        // Try std:: prefix for natives
        if !target.starts_with("std::") {
            let std_target = format!("std::{}", target);
            if let Some(n) = self.natives.get(&std_target).copied() {
                let res = n(self, args);
                match &res {
                    Ok(v) => self.trace_native_call(&std_target, args, v),
                    Err(e) => self.trace_error(e),
                }
                return res;
            }
        }

        // Component calls (user-defined)
        if self.resolve_function(callee, "").is_some() {
            return self.call_function(callee, args.to_vec());
        }

        // ui::render_to_string (VNode -> HTML) must be handled before generic ui::* tags.
        if callee == "render_to_string" || callee == "ui::render_to_string" {
            if let Some(Value::Node(n)) = args.first().cloned() {
                return Ok(Value::Str(render_node(&n)));
            }
            return Err(EvalError {
                message: "ui::render_to_string expects a VNode".to_string(),
                stack: vec![],
            });
        }

        // ui helpers - only for simple tags, qualified names should fall through
        if let Some(tag) = callee.strip_prefix("ui::") {
            if !tag.contains("::") && !tag.contains('.') {
                return eval_ui_call(tag, args);
            }
        }

        Err(EvalError {
            message: format!("Unknown function '{}'", callee),
            stack: vec![],
        })
    }

    pub fn call_closure(
        &mut self,
        clo: &ClosureValue,
        args: Vec<Value>,
    ) -> Result<Value, EvalError> {
        let prev_prefix =
            std::mem::replace(&mut self.current_module_prefix, clo.module_prefix.clone());
        let mut locals = clo.captured.clone();
        for (i, name) in clo.params.iter().enumerate() {
            locals.insert(name.clone(), args.get(i).cloned().unwrap_or(Value::Null));
        }
        let out = self.eval_expr(&clo.body, &mut locals);
        self.current_module_prefix = prev_prefix;
        out
    }

    fn assign_target(
        &mut self,
        locals: &mut HashMap<String, Value>,
        target: &Expr,
        value: Value,
    ) -> Result<(), EvalError> {
        match target {
            Expr::Identifier { name, .. } => {
                locals.insert(name.clone(), value);
            }
            Expr::FieldAccess { object, field, .. } => {
                let obj = self.eval_expr(object, locals)?;
                if let Value::Object(map_rc) = obj {
                    map_rc
                        .write()
                        .unwrap_or_else(|e| e.into_inner())
                        .insert(field.clone(), value);
                }
            }
            Expr::Index { object, index, .. } => {
                let obj = self.eval_expr(object, locals)?;
                let idx_val = self.eval_expr(index, locals)?;
                match (&obj, &idx_val) {
                    (Value::Array(arr_rc), Value::Int(i)) => {
                        let mut arr = arr_rc.write().unwrap_or_else(|e| e.into_inner());
                        if *i >= 0 && (*i as usize) < arr.len() {
                            arr[*i as usize] = value;
                        }
                    }
                    (Value::FloatArray(f_rc), Value::Int(i)) => {
                        let mut arr = f_rc.write().unwrap_or_else(|e| e.into_inner());
                        if *i >= 0 && (*i as usize) < arr.len() {
                            if let Some(f) = value.as_f64() {
                                arr[*i as usize] = f as f32;
                            }
                        }
                    }
                    (Value::DoubleArray(d_rc), Value::Int(i)) => {
                        let mut arr = d_rc.write().unwrap_or_else(|e| e.into_inner());
                        if *i >= 0 && (*i as usize) < arr.len() {
                            if let Some(f) = value.as_f64() {
                                arr[*i as usize] = f;
                            }
                        }
                    }
                    (Value::Object(map_rc), Value::Int(i)) => {
                        map_rc
                            .write()
                            .unwrap_or_else(|e| e.into_inner())
                            .insert(i.to_string(), value);
                    }
                    (Value::Object(map_rc), Value::Str(s)) => {
                        map_rc
                            .write()
                            .unwrap_or_else(|e| e.into_inner())
                            .insert(s.clone(), value);
                    }
                    _ => {}
                }
            }
            Expr::Deref { expr, .. } => {
                let ptr_val = self.eval_expr(expr, locals)?;
                if let (Value::Pointer(addr), Value::Int(b)) = (ptr_val, value) {
                    unsafe {
                        let ptr = addr as *mut u8;
                        *ptr = b as u8;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn eval_stmt_block_expr(
        &mut self,
        stmts: &[Stmt],
        locals: &mut HashMap<String, Value>,
    ) -> Result<Value, EvalError> {
        let dummy_fn = FunctionDecl {
            name: "<block>".to_string(),
            is_async: false,
            is_extern: false,
            extern_abi: None,
            generics: vec![],
            params: vec![],
            return_type: None,
            where_clauses: vec![],
            body: vec![],
            span: crate::core::diagnostics::Span::default(),
        };
        let prefix = self.current_module_prefix.clone();
        for st in stmts {
            if let Control::Return(v) = self.eval_stmt_raw(&dummy_fn, &prefix, st, locals)? {
                return Ok(v);
            }
        }
        Ok(Value::Null)
    }
}

impl Default for NyxVm {
    fn default() -> Self {
        Self::new(crate::runtime::execution::VmConfig::default())
    }
}

fn eval_fields(
    vm: &mut NyxVm,
    items: &[BlockItem],
    locals: &mut HashMap<String, Value>,
) -> Result<HashMap<String, Value>, EvalError> {
    let mut map = HashMap::new();
    for item in items {
        match item {
            BlockItem::Field(f) => {
                map.insert(f.name.clone(), vm.eval_expr(&f.value, locals)?);
            }
            BlockItem::Spread(expr) => {
                if let Value::Object(obj_rc) = vm.eval_expr(expr, locals)? {
                    map.extend(obj_rc.read().unwrap_or_else(|e| e.into_inner()).clone());
                }
            }
        }
    }
    Ok(map)
}

fn eval_field_inits(
    vm: &mut NyxVm,
    fields: &[FieldInit],
    locals: &mut HashMap<String, Value>,
) -> Result<HashMap<String, Value>, EvalError> {
    let mut map = HashMap::new();
    for f in fields {
        let val = vm.eval_expr(&f.value, locals)?;
        map.insert(f.name.clone(), val);
    }
    Ok(map)
}

fn capture_env(
    globals: &HashMap<String, Value>,
    locals: &HashMap<String, Value>,
) -> HashMap<String, Value> {
    let mut out = globals.clone();
    for (k, v) in locals {
        out.insert(k.clone(), v.clone());
    }
    out
}

fn qualify(prefix: &str, name: &str) -> String {
    let prefix = prefix.replace('/', "::");
    let name = name.replace('/', "::");
    if prefix.is_empty() {
        name
    } else {
        format!("{prefix}::{name}")
    }
}

#[allow(dead_code)]
fn expr_ident(e: &Expr) -> Option<String> {
    match e {
        Expr::Identifier { name: n, .. } => Some(n.clone()),
        Expr::Path {
            segments: parts, ..
        } => Some(parts.join("::")),
        _ => None,
    }
}

fn expr_callee_name(e: &Expr) -> String {
    match e {
        Expr::Identifier { name: n, .. } => n.clone(),
        Expr::Path {
            segments: parts, ..
        } => parts.join("::"),
        _ => "unknown".to_string(),
    }
}

/// Strip angle-bracket generic parameters from a Nyx callee name.
/// e.g. "Map<string, u64>::new" → "Map::new"
///      "Option<fn() -> any>::None" → "Option::None"
fn strip_generics(name: &str) -> String {
    let mut result = String::with_capacity(name.len());
    let mut depth = 0usize;
    for ch in name.chars() {
        match ch {
            '<' => depth += 1,
            '>' => {
                depth = depth.saturating_sub(1);
            }
            _ if depth == 0 => result.push(ch),
            _ => {}
        }
    }
    result
}

fn eval_binary(left: &Value, op: &str, right: &Value) -> Result<Value, EvalError> {
    match op {
        "+" => match (left, right) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Float(*a + *b as f64)),
            (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 + *b)),
            (Value::Array(a), Value::Array(b)) => {
                let mut left_vec = a.read().unwrap_or_else(|e| e.into_inner()).clone();
                let right_vec = b.read().unwrap_or_else(|e| e.into_inner());
                left_vec.extend(right_vec.iter().cloned());
                Ok(Value::Array(std::sync::Arc::new(std::sync::RwLock::new(
                    left_vec,
                ))))
            }
            _ => Ok(Value::Str(format!(
                "{}{}",
                to_stringish(left),
                to_stringish(right)
            ))),
        },
        "-" => match (left, right) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a - b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a - *b as f64)),
            (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 - b)),
            _ => Ok(Value::Null),
        },
        "*" => match (left, right) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a * b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
            (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a * (*b as f64))),
            (Value::Int(a), Value::Float(b)) => Ok(Value::Float((*a as f64) * b)),
            _ => Ok(Value::Null),
        },
        "/" => match (left, right) {
            (Value::Int(a), Value::Int(b)) => {
                if *b == 0 {
                    return Err(EvalError {
                        message: "division by zero".to_string(),
                        stack: vec![],
                    });
                }
                Ok(Value::Int(a / b))
            }
            (Value::Float(a), Value::Float(b)) => {
                if *b == 0.0 {
                    return Err(EvalError {
                        message: "division by zero".to_string(),
                        stack: vec![],
                    });
                }
                Ok(Value::Float(a / b))
            }
            (Value::Float(a), Value::Int(b)) => {
                if *b == 0 {
                    return Err(EvalError::new("division by zero".to_string()));
                }
                Ok(Value::Float(a / *b as f64))
            }
            (Value::Int(a), Value::Float(b)) => {
                if *b == 0.0 {
                    return Err(EvalError::new("division by zero".to_string()));
                }
                Ok(Value::Float(*a as f64 / b))
            }
            _ => Ok(Value::Null),
        },
        "==" => Ok(Value::Bool(values_equal(left, right))),
        "!=" => Ok(Value::Bool(!values_equal(left, right))),
        "&&" => Ok(Value::Bool(left.is_truthy() && right.is_truthy())),
        "||" => Ok(Value::Bool(left.is_truthy() || right.is_truthy())),
        ">" | "<" | ">=" | "<=" => eval_compare(left, op, right),
        "??" => {
            if matches!(left, Value::Null) {
                Ok(right.clone())
            } else {
                Ok(left.clone())
            }
        }
        "in" => match right {
            Value::Array(items_rc) => Ok(Value::Bool(
                items_rc
                    .read()
                    .unwrap_or_else(|e| e.into_inner())
                    .iter()
                    .any(|v| values_equal(v, left)),
            )),
            Value::Str(s) => match left {
                Value::Str(needle) => Ok(Value::Bool(s.contains(needle))),
                _ => Ok(Value::Bool(false)),
            },
            Value::Object(map_rc) => match left {
                Value::Str(key) => Ok(Value::Bool(
                    map_rc
                        .read()
                        .unwrap_or_else(|e| e.into_inner())
                        .contains_key(key),
                )),
                _ => Ok(Value::Bool(false)),
            },
            _ => Ok(Value::Bool(false)),
        },
        "&" => match (left, right) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a & b)),
            _ => Ok(Value::Null),
        },
        "|" => match (left, right) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a | b)),
            _ => Ok(Value::Null),
        },
        "^" => match (left, right) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a ^ b)),
            _ => Ok(Value::Null),
        },
        "<<" => match (left, right) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a << b)),
            _ => Ok(Value::Null),
        },
        ">>" => match (left, right) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a >> b)),
            _ => Ok(Value::Null),
        },
        _ => Ok(Value::Null),
    }
}

fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Float(x), Value::Float(y)) => x == y,
        (Value::Int(x), Value::Float(y)) => *x as f64 == *y,
        (Value::Float(x), Value::Int(y)) => *x == *y as f64,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::Array(x), Value::Array(y)) => std::sync::Arc::ptr_eq(x, y),
        (Value::Object(x), Value::Object(y)) => std::sync::Arc::ptr_eq(x, y),
        (Value::Bytes(x), Value::Bytes(y)) => std::sync::Arc::ptr_eq(x, y),
        (Value::Pointer(x), Value::Pointer(y)) => x == y,
        (Value::Promise(a), Value::Promise(b)) => std::sync::Arc::ptr_eq(a, b),
        (Value::Tensor(TensorStorage::Gpu(a), _), Value::Tensor(TensorStorage::Gpu(b), _)) => {
            std::sync::Arc::ptr_eq(a, b)
        }
        (Value::Tensor(TensorStorage::Cpu(a), _), Value::Tensor(TensorStorage::Cpu(b), _)) => {
            std::sync::Arc::ptr_eq(a, b)
        }
        (Value::FloatArray(a), Value::FloatArray(b)) => std::sync::Arc::ptr_eq(a, b),
        (Value::DoubleArray(a), Value::DoubleArray(b)) => std::sync::Arc::ptr_eq(a, b),
        _ => false,
    }
}

fn eval_compare(left: &Value, op: &str, right: &Value) -> Result<Value, EvalError> {
    let as_f64 = |v: &Value| match v {
        Value::Int(i) => Some(*i as f64),
        Value::Float(f) => Some(*f),
        _ => None,
    };
    if let (Some(l), Some(r)) = (as_f64(left), as_f64(right)) {
        let out = match op {
            ">" => l > r,
            "<" => l < r,
            ">=" => l >= r,
            "<=" => l <= r,
            _ => false,
        };
        return Ok(Value::Bool(out));
    }
    Ok(Value::Bool(false))
}

fn eval_method(
    vm: &mut NyxVm,
    receiver: Value,
    method: &str,
    args: &[Value],
) -> Result<Value, EvalError> {
    match method {
        "to_string" if args.is_empty() => return Ok(Value::Str(to_stringish(&receiver))),
        "unwrap" => {
            return Ok(match receiver {
                Value::Object(map_rc) => {
                    let map = map_rc.read().unwrap_or_else(|e| e.into_inner());
                    map.values().next().cloned().unwrap_or(Value::Null)
                }
                Value::Null => {
                    return Err(EvalError {
                        message: "called unwrap() on a Null value".to_string(),
                        stack: vec![],
                    })
                }
                _ => receiver,
            });
        }
        "unwrap_or" => {
            let default = args.first().cloned().unwrap_or(Value::Null);
            return Ok(match receiver {
                Value::Null => default,
                other => other,
            });
        }
        _ => {}
    }

    // List/Array helpers
    match receiver {
        Value::Array(arr_rc) => match method {
            "push" => {
                if let Some(v) = args.first().cloned() {
                    arr_rc.write().unwrap_or_else(|e| e.into_inner()).push(v);
                }
                Ok(Value::Array(arr_rc.clone()))
            }
            "pop" => {
                let mut arr = arr_rc.write().unwrap_or_else(|e| e.into_inner());
                Ok(arr.pop().unwrap_or(Value::Null))
            }
            "shift" => {
                let mut arr = arr_rc.write().unwrap_or_else(|e| e.into_inner());
                if !arr.is_empty() {
                    Ok(arr.remove(0))
                } else {
                    Ok(Value::Null)
                }
            }
            "remove" => {
                let idx = args.first().and_then(|v| {
                    if let Value::Int(i) = v {
                        Some(*i as usize)
                    } else {
                        None
                    }
                });
                let mut arr = arr_rc.write().unwrap_or_else(|e| e.into_inner());
                if let Some(i) = idx {
                    if i < arr.len() {
                        return Ok(arr.remove(i));
                    }
                }
                Ok(Value::Null)
            }
            "contains" => {
                if let Some(v) = args.first() {
                    let arr = arr_rc.read().unwrap_or_else(|e| e.into_inner());
                    Ok(Value::Bool(arr.iter().any(|x| values_equal(x, v))))
                } else {
                    Ok(Value::Bool(false))
                }
            }
            "index_of" => {
                if let Some(v) = args.first() {
                    let arr = arr_rc.read().unwrap_or_else(|e| e.into_inner());
                    for (i, x) in arr.iter().enumerate() {
                        if values_equal(x, v) {
                            return Ok(Value::Int(i as i64));
                        }
                    }
                }
                Ok(Value::Int(-1))
            }
            "last" => {
                let arr = arr_rc.read().unwrap_or_else(|e| e.into_inner());
                Ok(arr.last().cloned().unwrap_or(Value::Null))
            }
            "concat" => {
                if let Some(Value::Array(other_rc)) = args.first() {
                    let other = other_rc.read().unwrap_or_else(|e| e.into_inner());
                    let mut arr = arr_rc.write().unwrap_or_else(|e| e.into_inner());
                    arr.extend(other.iter().cloned());
                }
                Ok(Value::Array(arr_rc.clone()))
            }
            "join" => {
                let sep = args
                    .first()
                    .and_then(|v| {
                        if let Value::Str(s) = v {
                            Some(s.as_str())
                        } else {
                            None
                        }
                    })
                    .unwrap_or("");
                let arr = arr_rc.read().unwrap_or_else(|e| e.into_inner());
                let parts: Vec<String> = arr.iter().map(to_stringish).collect();
                Ok(Value::Str(parts.join(sep)))
            }
            "reverse" => {
                let mut arr = arr_rc.write().unwrap_or_else(|e| e.into_inner());
                arr.reverse();
                Ok(Value::Array(arr_rc.clone()))
            }
            "slice" => {
                let start = args
                    .first()
                    .and_then(|v| {
                        if let Value::Int(i) = v {
                            Some(*i as isize)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);
                let end = args.get(1).and_then(|v| {
                    if let Value::Int(i) = v {
                        Some(*i as isize)
                    } else {
                        None
                    }
                });
                let arr = arr_rc.read().unwrap_or_else(|e| e.into_inner());
                let len = arr.len() as isize;
                let mut s_idx = start.max(0).min(len) as usize;
                let mut e_idx = end.unwrap_or(len).max(0).min(len) as usize;
                if e_idx < s_idx {
                    std::mem::swap(&mut s_idx, &mut e_idx);
                }
                let sliced = arr[s_idx..e_idx].to_vec();
                Ok(Value::Array(std::sync::Arc::new(std::sync::RwLock::new(
                    sliced,
                ))))
            }
            "clear" => {
                arr_rc.write().unwrap_or_else(|e| e.into_inner()).clear();
                Ok(Value::Array(arr_rc.clone()))
            }
            "len" | "length" => Ok(Value::Int(
                arr_rc.read().unwrap_or_else(|e| e.into_inner()).len() as i64,
            )),
            "get" | "nth" | "at" => {
                let idx = args.first().and_then(|v| {
                    if let Value::Int(i) = v {
                        Some(*i as usize)
                    } else {
                        None
                    }
                });
                Ok(idx
                    .and_then(|i| {
                        arr_rc
                            .read()
                            .unwrap_or_else(|e| e.into_inner())
                            .get(i)
                            .cloned()
                    })
                    .unwrap_or(Value::Null))
            }
            "at_put" | "set" => {
                let idx = args.first().and_then(|v| {
                    if let Value::Int(i) = v {
                        Some(*i as usize)
                    } else {
                        None
                    }
                });
                if let (Some(i), Some(v)) = (idx, args.get(1)) {
                    let mut arr = arr_rc.write().unwrap_or_else(|e| e.into_inner());
                    if i < arr.len() {
                        arr[i] = v.clone();
                    }
                }
                Ok(Value::Null)
            }
            "map" => {
                if let Some(Value::Closure(clo)) = args.first() {
                    let arr = arr_rc.read().unwrap_or_else(|e| e.into_inner());
                    let mut res = Vec::with_capacity(arr.len());
                    for x in arr.iter() {
                        res.push(vm.call_closure(clo, vec![x.clone()])?);
                    }
                    Ok(Value::Array(std::sync::Arc::new(std::sync::RwLock::new(
                        res,
                    ))))
                } else {
                    Ok(Value::Array(arr_rc.clone()))
                }
            }
            "filter" => {
                if let Some(Value::Closure(clo)) = args.first() {
                    let arr = arr_rc.read().unwrap_or_else(|e| e.into_inner());
                    let mut res = Vec::new();
                    for x in arr.iter() {
                        if vm.call_closure(clo, vec![x.clone()])?.is_truthy() {
                            res.push(x.clone());
                        }
                    }
                    Ok(Value::Array(std::sync::Arc::new(std::sync::RwLock::new(
                        res,
                    ))))
                } else {
                    Ok(Value::Array(arr_rc.clone()))
                }
            }
            "reduce" => {
                if let (Some(initial), Some(Value::Closure(clo))) = (args.first(), args.get(1)) {
                    let arr = arr_rc.read().unwrap_or_else(|e| e.into_inner());
                    let mut acc = initial.clone();
                    for x in arr.iter() {
                        acc = vm.call_closure(clo, vec![acc, x.clone()])?;
                    }
                    Ok(acc)
                } else {
                    Ok(Value::Null)
                }
            }
            _ => eval_ufcs(vm, Value::Array(arr_rc), method, args),
        },
        Value::Object(map_rc) => {
            let clo = {
                let map = map_rc.read().unwrap_or_else(|e| e.into_inner());
                map.get(method).cloned()
            };
            if let Some(Value::Closure(c)) = clo {
                // If it's a framework method (belongs to a module), don't prepend self
                // Standard Nyx objects expect self, but our aggregate framework objects don't.
                if c.module_prefix.is_empty() || c.params.first() == Some(&"self".to_string()) {
                    let mut call_args = vec![Value::Object(map_rc.clone())];
                    call_args.extend_from_slice(args);
                    return vm.call_closure(&c, call_args);
                } else {
                    return vm.call_closure(&c, args.to_vec());
                }
            }

            match method {
                "get" => {
                    let key = args.first().and_then(|v| {
                        if let Value::Str(s) = v {
                            Some(s.clone())
                        } else {
                            None
                        }
                    });
                    Ok(key
                        .and_then(|k| {
                            map_rc
                                .read()
                                .unwrap_or_else(|e| e.into_inner())
                                .get(&k)
                                .cloned()
                        })
                        .unwrap_or(Value::Null))
                }
                "set" | "insert" => {
                    if let (Some(Value::Str(k)), Some(v)) = (args.first(), args.get(1)) {
                        map_rc
                            .write()
                            .unwrap_or_else(|e| e.into_inner())
                            .insert(k.clone(), v.clone());
                    }
                    Ok(Value::Null)
                }
                "remove" => {
                    if let Some(Value::Str(k)) = args.first() {
                        Ok(map_rc
                            .write()
                            .unwrap_or_else(|e| e.into_inner())
                            .remove(k)
                            .unwrap_or(Value::Null))
                    } else {
                        Ok(Value::Null)
                    }
                }
                "contains" | "has" => {
                    if let Some(Value::Str(k)) = args.first() {
                        Ok(Value::Bool(
                            map_rc
                                .read()
                                .unwrap_or_else(|e| e.into_inner())
                                .contains_key(k),
                        ))
                    } else {
                        Ok(Value::Bool(false))
                    }
                }
                "keys" => Ok(Value::Array(std::sync::Arc::new(std::sync::RwLock::new(
                    map_rc
                        .read()
                        .unwrap_or_else(|e| e.into_inner())
                        .keys()
                        .map(|k| Value::Str(k.clone()))
                        .collect(),
                )))),
                "values" => Ok(Value::Array(std::sync::Arc::new(std::sync::RwLock::new(
                    map_rc
                        .read()
                        .unwrap_or_else(|e| e.into_inner())
                        .values()
                        .cloned()
                        .collect(),
                )))),
                "length" | "len" => Ok(Value::Int(
                    map_rc.read().unwrap_or_else(|e| e.into_inner()).len() as i64,
                )),
                "clear" => {
                    map_rc.write().unwrap_or_else(|e| e.into_inner()).clear();
                    Ok(Value::Object(map_rc.clone()))
                }
                "entries" => {
                    let map = map_rc.read().unwrap_or_else(|e| e.into_inner());
                    let mut entries = Vec::new();
                    for (k, v) in map.iter() {
                        let mut entry = Vec::new();
                        entry.push(Value::Str(k.clone()));
                        entry.push(v.clone());
                        entries.push(Value::Array(std::sync::Arc::new(std::sync::RwLock::new(
                            entry,
                        ))));
                    }
                    Ok(Value::Array(std::sync::Arc::new(std::sync::RwLock::new(
                        entries,
                    ))))
                }
                "typeof" => Ok(Value::Str("Object".to_string())),
                "fields" => {
                    let map = map_rc.read().unwrap_or_else(|e| e.into_inner());
                    let keys: Vec<Value> = map.keys().map(|k| Value::Str(k.clone())).collect();
                    Ok(Value::Array(std::sync::Arc::new(std::sync::RwLock::new(
                        keys,
                    ))))
                }
                _ => eval_ufcs(vm, Value::Object(map_rc), method, args),
            }
        }
        Value::Str(s) => match method {
            "len" | "length" => Ok(Value::Int(s.chars().count() as i64)),
            "chars" => Ok(Value::Array(std::sync::Arc::new(std::sync::RwLock::new(
                s.chars().map(|c| Value::Str(c.to_string())).collect(),
            )))),
            "get" | "nth" | "at" | "char_at" => {
                let idx = args.first().and_then(|v| {
                    if let Value::Int(i) = v {
                        Some(*i as usize)
                    } else {
                        None
                    }
                });
                Ok(idx
                    .and_then(|i| s.chars().nth(i).map(|c| Value::Str(c.to_string())))
                    .unwrap_or(Value::Null))
            }
            "contains" => {
                if let Some(Value::Str(needle)) = args.first() {
                    Ok(Value::Bool(s.contains(needle)))
                } else {
                    Ok(Value::Bool(false))
                }
            }
            "starts_with" => {
                if let Some(Value::Str(prefix)) = args.first() {
                    Ok(Value::Bool(s.starts_with(prefix)))
                } else {
                    Ok(Value::Bool(false))
                }
            }
            "ends_with" => {
                if let Some(Value::Str(suffix)) = args.first() {
                    Ok(Value::Bool(s.ends_with(suffix)))
                } else {
                    Ok(Value::Bool(false))
                }
            }
            "split" => {
                if let Some(Value::Str(delim)) = args.first() {
                    let parts: Vec<Value> =
                        s.split(delim).map(|p| Value::Str(p.to_string())).collect();
                    Ok(Value::Array(std::sync::Arc::new(std::sync::RwLock::new(
                        parts,
                    ))))
                } else {
                    Ok(Value::Array(std::sync::Arc::new(std::sync::RwLock::new(
                        Vec::new(),
                    ))))
                }
            }
            "substring" => {
                let start = args
                    .first()
                    .and_then(|v| {
                        if let Value::Int(i) = v {
                            Some(*i as isize)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);
                let end = args.get(1).and_then(|v| {
                    if let Value::Int(i) = v {
                        Some(*i as isize)
                    } else {
                        None
                    }
                });
                let chars: Vec<char> = s.chars().collect();
                let len = chars.len() as isize;
                let mut s_idx = start.max(0).min(len) as usize;
                let mut e_idx = end.unwrap_or(len).max(0).min(len) as usize;
                if e_idx < s_idx {
                    std::mem::swap(&mut s_idx, &mut e_idx);
                }
                let out: String = chars[s_idx..e_idx].iter().collect();
                Ok(Value::Str(out))
            }
            "index_of" => {
                if let Some(Value::Str(needle)) = args.first() {
                    if let Some(byte_idx) = s.find(needle) {
                        let char_idx = s[..byte_idx].chars().count() as i64;
                        Ok(Value::Int(char_idx))
                    } else {
                        Ok(Value::Int(-1))
                    }
                } else {
                    Ok(Value::Int(-1))
                }
            }
            "to_lower" => Ok(Value::Str(s.to_lowercase())),
            "to_upper" => Ok(Value::Str(s.to_uppercase())),
            "trim_start" => Ok(Value::Str(s.trim_start().to_string())),
            "trim_end" => Ok(Value::Str(s.trim_end().to_string())),
            "to_u8" => {
                let byte = s.chars().next().map(|c| c as u32 as i64).unwrap_or(0);
                Ok(Value::Int(byte))
            }
            "replace" => {
                let target = args
                    .first()
                    .and_then(|v| {
                        if let Value::Str(t) = v {
                            Some(t.as_str())
                        } else {
                            None
                        }
                    })
                    .unwrap_or("");
                let replacement = args
                    .get(1)
                    .and_then(|v| {
                        if let Value::Str(r) = v {
                            Some(r.as_str())
                        } else {
                            None
                        }
                    })
                    .unwrap_or("");
                Ok(Value::Str(s.replace(target, replacement)))
            }
            "repeat" => {
                let count = args
                    .first()
                    .and_then(|v| {
                        if let Value::Int(c) = v {
                            Some(*c as usize)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);
                Ok(Value::Str(s.repeat(count)))
            }
            "to_string" => Ok(Value::Str(s.clone())),
            "trim" => Ok(Value::Str(s.trim().to_string())),
            "trim_matches" => {
                let pat = args
                    .first()
                    .and_then(|v| {
                        if let Value::Str(p) = v {
                            Some(p.as_str())
                        } else {
                            None
                        }
                    })
                    .unwrap_or("");
                let pat_chars: Vec<char> = pat.chars().collect();
                Ok(Value::Str(
                    s.trim_matches(|c| pat_chars.contains(&c)).to_string(),
                ))
            }
            "is_alphabetic" => Ok(Value::Bool(
                s.chars().next().map(|c| c.is_alphabetic()).unwrap_or(false),
            )),
            "is_digit" => Ok(Value::Bool(
                s.chars()
                    .next()
                    .map(|c| c.is_ascii_digit())
                    .unwrap_or(false),
            )),
            "is_whitespace" => Ok(Value::Bool(
                s.chars().next().map(|c| c.is_whitespace()).unwrap_or(false),
            )),
            _ => {
                // If s looks like a module path (contains dots), try to resolve as a static call
                // net.http.HttpServer(...) -> net::http::HttpServer(...)
                let qualified = format!("{}.{}", s, method).replace(".", "::");
                if vm.functions.contains_key(&qualified) {
                    return vm.call_function(&qualified, args.to_vec());
                }
                eval_ufcs(vm, Value::Str(s), method, args)
            }
        },
        Value::FloatArray(rc) => match method {
            "len" | "count" => Ok(Value::Int(
                rc.read().unwrap_or_else(|e| e.into_inner()).len() as i64,
            )),
            _ => eval_ufcs(vm, Value::FloatArray(rc), method, args),
        },
        Value::DoubleArray(rc) => match method {
            "len" | "count" => Ok(Value::Int(
                rc.read().unwrap_or_else(|e| e.into_inner()).len() as i64,
            )),
            _ => eval_ufcs(vm, Value::DoubleArray(rc), method, args),
        },
        Value::Bytes(rc) => match method {
            "len" | "count" => Ok(Value::Int(
                rc.read().unwrap_or_else(|e| e.into_inner()).len() as i64,
            )),
            _ => eval_ufcs(vm, Value::Bytes(rc), method, args),
        },
        other => eval_ufcs(vm, other, method, args),
    }
}

fn eval_ufcs(
    vm: &mut NyxVm,
    receiver: Value,
    method: &str,
    args: &[Value],
) -> Result<Value, EvalError> {
    let mut resolved_name = None;

    // 1. Try bare method name
    if vm.resolve_function(method, "").is_some() {
        resolved_name = Some(method.to_string());
    }

    // 2. Try with current module prefix
    if resolved_name.is_none() && !vm.current_module_prefix.is_empty() {
        let qualified = format!("{}::{}", vm.current_module_prefix, method);
        if vm.resolve_function(&qualified, "").is_some() {
            resolved_name = Some(qualified);
        }
    }

    // 3. Search via origin tag if receiver is an object
    if resolved_name.is_none() {
        let origin = if let Value::Object(map_rc) = &receiver {
            let map = map_rc.read().unwrap_or_else(|e| e.into_inner());
            map.get("__origin__").and_then(|v| {
                if let Value::Str(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            })
        } else {
            None
        };

        if let Some(origin) = origin {
            // Try in the same module
            let qualified = qualify(&origin, method);
            if vm.resolve_function(&qualified, "").is_some() {
                resolved_name = Some(qualified);
            }

            if resolved_name.is_none() {
                // Try in parent modules
                let mut parts: Vec<&str> = origin.split("::").collect();
                while parts.len() > 1 {
                    parts.pop();
                    let parent = parts.join("::");
                    let qualified = qualify(&parent, method);
                    if vm.resolve_function(&qualified, "").is_some() {
                        resolved_name = Some(qualified);
                        break;
                    }
                }
            }
        }
    }

    // 4. Try with object's __type prefix
    if resolved_name.is_none() {
        if let Value::Object(map_rc) = &receiver {
            let type_name = {
                let map = map_rc.read().unwrap_or_else(|e| e.into_inner());
                map.get("__type").and_then(|v| {
                    if let Value::Str(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
            };
            if let Some(tn) = type_name {
                let qualified = qualify(&tn, method);
                if vm.resolve_function(&qualified, "").is_some() {
                    resolved_name = Some(qualified);
                } else {
                    // Also try with module prefix if tn is not fully qualified
                    let fully_qualified = qualify(&vm.current_module_prefix, &qualified);
                    if vm.resolve_function(&fully_qualified, "").is_some() {
                        resolved_name = Some(fully_qualified);
                    }
                }
            }
        }
    }

    // 5. Search all registered functions and natives for a match ending in ::method
    // (Only if not ambiguous)
    if resolved_name.is_none() {
        let mut candidates = Vec::new();
        let method_suffix = format!("::{}", method);
        for name in vm.functions.keys() {
            if name.ends_with(&method_suffix) {
                candidates.push(name.clone());
            }
        }
        for name in vm.natives.keys() {
            if name.ends_with(&method_suffix) {
                candidates.push(name.clone());
            }
        }

        if candidates.len() == 1 {
            resolved_name = Some(candidates[0].clone());
        } else if !candidates.is_empty() {
            // Log ambiguity but continue to error if no clear winner
        }
    }

    if let Some(name) = resolved_name {
        let mut ufcs_args = Vec::with_capacity(args.len() + 1);
        ufcs_args.push(receiver);
        ufcs_args.extend_from_slice(args);
        return vm.call_function(&name, ufcs_args);
    }

    // If still not found, report as unknown method error
    let type_name = match receiver {
        Value::Null => "Null",
        Value::Int(_) => "Int",
        Value::Float(_) => "Float",
        Value::Bool(_) => "Bool",
        Value::BigInt(_) => "BigInt",
        Value::Str(_) => "String",
        Value::Array(_) => "Array",
        Value::Object(_) => "Object",
        Value::Closure(_) => "Closure",
        Value::Node(_) => "VNode",
        Value::Bytes(_) => "Bytes",
        Value::Pointer(_) => "Pointer",
        Value::Promise(_) => "Promise",
        Value::FloatArray(_) => "FloatArray",
        Value::DoubleArray(_) => "DoubleArray",
        Value::Tensor(_, _) => "Tensor",
    };

    Err(EvalError {
        message: format!("Unknown method '{}' for {}", method, type_name),
        stack: vec![],
    })
}

pub fn to_stringish(v: &Value) -> String {
    match v {
        Value::Null => "null".to_string(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::BigInt(n) => n.clone(),
        Value::Str(s) => s.clone(),
        Value::Array(a_rc) => {
            let parts: Vec<String> = a_rc
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .iter()
                .map(to_stringish)
                .collect();
            format!("[{}]", parts.join(", "))
        }
        Value::Object(o_rc) => {
            let map = o_rc.read().unwrap_or_else(|e| e.into_inner());
            let mut parts: Vec<String> = map
                .iter()
                .map(|(k, v)| format!("{k}: {}", to_stringish(v)))
                .collect();
            parts.sort();
            format!("{{{}}}", parts.join(", "))
        }
        Value::Bytes(b) => format!(
            "<bytes len={}>",
            b.read().unwrap_or_else(|e| e.into_inner()).len()
        ),
        Value::Pointer(p) => format!("*0x{:016x}", p),
        Value::Promise(p) => {
            let state = p.read().unwrap_or_else(|e| e.into_inner());
            if state.resolved {
                format!("<promise: resolved({})>", to_stringish(&state.value))
            } else {
                "<promise: pending>".to_string()
            }
        }
        Value::Closure(_) => "<closure>".to_string(),
        Value::Node(n) => render_node(n),
        Value::FloatArray(rc) => format!(
            "[f32; {}]",
            rc.read().unwrap_or_else(|e| e.into_inner()).len()
        ),
        Value::DoubleArray(rc) => format!(
            "[f64; {}]",
            rc.read().unwrap_or_else(|e| e.into_inner()).len()
        ),
        Value::Tensor(_, shape) => format!("<Tensor{:?}>", shape),
    }
}

pub fn render_node(n: &VNode) -> String {
    let mut out = String::new();
    out.push('<');
    out.push_str(&n.tag);
    for (k, v) in &n.attrs {
        out.push(' ');
        out.push_str(k);
        out.push('=');
        out.push('"');
        out.push_str(&escape_attr(v));
        out.push('"');
    }
    out.push('>');
    for ch in &n.children {
        match ch {
            Child::Text(t) => out.push_str(&escape_text(t)),
            Child::Node(nn) => out.push_str(&render_node(nn)),
        }
    }
    out.push_str("</");
    out.push_str(&n.tag);
    out.push('>');
    out
}

fn escape_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_attr(s: &str) -> String {
    escape_text(s).replace('"', "&quot;")
}

fn eval_ui_call(tag: &str, args: &[Value]) -> Result<Value, EvalError> {
    let (attrs, children) = match args {
        [a, b] => (a.clone(), b.clone()),
        _ => {
            return Err(EvalError {
                message: format!("ui::{tag} expects (attrs, children)"),
                stack: vec![],
            })
        }
    };

    let raw_attrs = match attrs {
        Value::Object(o_rc) => o_rc.read().unwrap_or_else(|e| e.into_inner()).clone(),
        Value::Null => HashMap::new(),
        _ => {
            return Err(EvalError {
                message: format!("ui::{tag} attribute map must be an object literal"),
                stack: vec![],
            })
        }
    };
    let mut attrs = HashMap::new();
    for (k, v) in raw_attrs {
        let sv = match v {
            Value::Str(s) => s,
            Value::Int(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => "".to_string(),
            other => {
                return Err(EvalError {
                    message: format!(
                        "ui::{tag} attribute '{k}' has invalid value: {}",
                        to_stringish(&other)
                    ),
                    stack: vec![],
                });
            }
        };
        attrs.insert(k, sv);
    }

    let children_list = match children {
        Value::Array(items_rc) => items_rc.read().unwrap_or_else(|e| e.into_inner()).clone(),
        Value::Null => vec![],
        Value::Str(s) => vec![Value::Str(s)],
        other => vec![other.clone()],
    };

    let mut out_children = Vec::new();
    for c in children_list {
        match c {
            Value::Str(s) => out_children.push(Child::Text(s)),
            Value::Int(i) => out_children.push(Child::Text(i.to_string())),
            Value::Float(f) => out_children.push(Child::Text(f.to_string())),
            Value::Bool(b) => out_children.push(Child::Text(b.to_string())),
            Value::Node(n) => out_children.push(Child::Node(n)),
            Value::Null => {}
            other => out_children.push(Child::Text(to_stringish(&other))),
        }
    }

    Ok(Value::Node(VNode {
        tag: tag.to_string(),
        attrs,
        children: out_children,
    }))
}

pub fn parse_program(path: &Path) -> Result<Program, String> {
    let source = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let (_registry, grammar) = shared_registry_and_grammar()?;

    let mut lexer = Lexer::from_source(source);
    let tokens = lexer.tokenize().map_err(|e| e.to_string())?;
    let mut parser = NeuroParser::new(grammar.clone());
    let mut program = parser.parse(&tokens).map_err(|e| e.to_string())?;
    crate::core::lowering::protocol_lower::ProtocolLowerer::lower(&mut program);
    Ok(program)
}

fn shared_registry_and_grammar(
) -> Result<(&'static LanguageRegistry, &'static GrammarEngine), String> {
    static REGISTRY: OnceLock<LanguageRegistry> = OnceLock::new();
    static GRAMMAR: OnceLock<GrammarEngine> = OnceLock::new();

    let reg = REGISTRY.get_or_init(|| {
        LanguageRegistry::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/registry/language.json"
        ))
        .unwrap_or_default()
    });

    let gr = GRAMMAR.get_or_init(|| {
        let g = GrammarEngine::from_registry(reg);
        let _ = g.validate_determinism(reg);
        g
    });

    Ok((reg, gr))
}

pub fn format_eval_error(e: &EvalError) -> String {
    let mut out = e.message.clone();
    if !e.stack.is_empty() {
        out.push_str("\n\nstack:");
        for fr in e.stack.iter().rev() {
            out.push_str(&format!("\n  at {}", fr));
        }
    }
    out
}

pub fn engine_root_from_repo() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/engines/ui_engine"))
}

fn matches_pattern(
    val: &Value,
    pattern: &MatchPattern,
    locals: &mut HashMap<String, Value>,
) -> bool {
    match pattern {
        MatchPattern::Wildcard => true,
        MatchPattern::Literal(e) => {
            // Simple literal comparison
            match (val, e) {
                (Value::Int(a), Expr::IntLiteral { value: b, .. }) => *a == *b,
                (Value::Str(a), Expr::StringLiteral { value: b, .. }) => a == b,
                (Value::Bool(a), Expr::BoolLiteral { value: b, .. }) => *a == *b,
                (Value::Null, Expr::NullLiteral { .. }) => true,
                _ => false,
            }
        }
        MatchPattern::Identifier(name) => {
            locals.insert(name.clone(), val.clone());
            true
        }
        MatchPattern::TupleVariant(name, patterns) => {
            if let Value::Object(map_rc) = val {
                let map = map_rc.read().unwrap_or_else(|e| e.into_inner());
                let key = name.rsplit('.').next().unwrap_or(name);
                if map.contains_key(key) && patterns.len() == 1 {
                    return matches_pattern(&map[key], &patterns[0], locals);
                }
            }
            false
        }
        _ => false,
    }
}

fn serialize_array<S>(
    val: &std::sync::Arc<std::sync::RwLock<Vec<Value>>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::ser::SerializeSeq;
    let arr = val.read().map_err(serde::ser::Error::custom)?;
    let mut seq = serializer.serialize_seq(Some(arr.len()))?;
    for item in arr.iter() {
        seq.serialize_element(item)?;
    }
    seq.end()
}

fn deserialize_array<'de, D>(
    deserializer: D,
) -> Result<std::sync::Arc<std::sync::RwLock<Vec<Value>>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let items: Vec<Value> = serde::Deserialize::deserialize(deserializer)?;
    Ok(std::sync::Arc::new(std::sync::RwLock::new(items)))
}

fn serialize_object<S>(
    val: &std::sync::Arc<std::sync::RwLock<std::collections::HashMap<String, Value>>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::ser::SerializeMap;
    let obj = val.read().map_err(serde::ser::Error::custom)?;
    let mut map = serializer.serialize_map(Some(obj.len()))?;
    for (k, v) in obj.iter() {
        map.serialize_entry(k, v)?;
    }
    map.end()
}

fn deserialize_object<'de, D>(
    deserializer: D,
) -> Result<std::sync::Arc<std::sync::RwLock<std::collections::HashMap<String, Value>>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let items: std::collections::HashMap<String, Value> =
        serde::Deserialize::deserialize(deserializer)?;
    Ok(std::sync::Arc::new(std::sync::RwLock::new(items)))
}

/// Resolve `${...}` interpolations within a css`` raw string.
///
/// Simple variable name references (`${my_var}`) are looked up in `locals` then
/// `globals` and replaced with their string representation.  Complex expressions
/// (containing spaces, operators, colons, etc.) are left verbatim so the caller
/// can still parse the surrounding static declarations.
fn resolve_css_interpolations(
    raw: &str,
    locals: &std::collections::HashMap<String, Value>,
    globals: &std::collections::HashMap<String, Value>,
) -> String {
    let mut result = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(start) = rest.find("${") {
        result.push_str(&rest[..start]);
        let inner_start = start + 2;
        let inner = &rest[inner_start..];
        // find matching closing brace
        let mut depth = 1usize;
        let mut end = inner.len();
        for (i, ch) in inner.char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = i;
                        break;
                    }
                }
                _ => {}
            }
        }
        let expr_text = inner[..end].trim();
        // Only simple identifier lookups are resolved; anything complex is kept verbatim.
        let is_simple_ident = expr_text
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == ':');
        if is_simple_ident {
            let val = locals
                .get(expr_text)
                .or_else(|| globals.get(expr_text))
                .cloned()
                .unwrap_or(Value::Null);
            result.push_str(&to_stringish(&val));
        } else {
            result.push_str("${");
            result.push_str(expr_text);
            result.push('}');
        }
        rest = &rest[inner_start + end + 1..];
    }
    result.push_str(rest);
    result
}
