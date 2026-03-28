//! Extensions Layer: Plugin loading and domain-specific extensions.
//!
//! This module provides the extension points for the Nyx compiler,
//! allowing for dynamic loading of plugins and extensible compiler functionality.
//!
//! ## Extension Points
//!
//! - [`CompilerPass`] - Extension point for compiler passes
//! - [`RuntimeBackend`] - Extension point for runtime backends
//! - [`TargetBackend`] - Extension point for target backends
//!
//! ## Plugin System
//!
//! The plugin system allows loading external code at runtime to extend
//! the compiler's capabilities. See the [`plugin_loader`] module for details.

pub mod plugin_loader;
pub mod interfaces;

use std::collections::HashMap;
use std::sync::Arc;

use crate::core::diagnostics::{codes, ErrorCategory, NyxError};

/// Re-export plugin types for convenient access
pub use plugin_loader::{
    Plugin, PluginConfig, PluginManager, PluginMetadata, PluginConfigBuilder, Version,
    VersionRange,
};

/// Re-export interface types for convenient access
pub use interfaces::{
    Bytecode, Capabilities, CompilationUnit, CompilationWarning, EngineConfig, EngineHandle,
    EngineAPI, ModuleAPI, ModuleId, RuntimeAPI, Sandbox, SandboxConfig, SourceLocation,
    Symbol, SymbolKind, TypeCheckResult, TypeError, TypeInfo, TypeKind, Value, ValueData,
    Version as ApiVersion, CompilerAPI, ExtensionConfig, PluginConfig as InterfacePluginConfig,
    RuntimeCapabilities, RuntimeConfig,
};

/// Extension point for compiler passes
///
/// Compiler passes transform the AST at various stages of compilation.
/// This trait allows adding custom transformations to the compilation pipeline.
///
/// # Example
///
/// ```
/// # use nyx::extensions::{CompilerPass, AST};
/// # use nyx::core::diagnostics::NyxError;
/// struct MyPass;
///
/// impl CompilerPass for MyPass {
///     fn name(&self) -> &str {
///         "my_custom_pass"
///     }
///
///     fn run(&self, ast: &mut AST) -> Result<(), NyxError> {
///         // Transform the AST
///         Ok(())
///     }
/// }
/// ```
pub trait CompilerPass: Send + Sync {
    /// Returns the name of the pass
    fn name(&self) -> &str;

    /// Run the pass on the AST
    ///
    /// This method is called during compilation to transform
    /// or analyze the abstract syntax tree.
    fn run(&self, ast: &mut AST) -> Result<(), NyxError>;

    /// Get the pass description
    fn description(&self) -> Option<&str> {
        None
    }

    /// Get the pass dependencies
    ///
    /// Returns a list of pass names that must run before this pass.
    fn dependencies(&self) -> Vec<String> {
        Vec::new()
    }
}

/// Extension point for runtime backends
///
/// Runtime backends provide the execution environment for compiled code.
/// This trait allows adding custom runtime implementations.
///
/// # Example
///
/// ```
/// # use nyx::extensions::{RuntimeBackend, ExecutionContext, Value};
/// # use nyx::core::diagnostics::NyxError;
/// struct MyRuntimeBackend;
///
/// impl RuntimeBackend for MyRuntimeBackend {
///     fn name(&self) -> &str {
///         "my_runtime"
///     }
///
///     fn execute(&self, context: &ExecutionContext) -> Result<Value, NyxError> {
///         // Execute code in the runtime
/// #       unimplemented!()
///     }
/// }
/// ```
pub trait RuntimeBackend: Send + Sync {
    /// Returns the name of the runtime backend
    fn name(&self) -> &str;

    /// Execute code in this runtime backend
    fn execute(&self, context: &ExecutionContext) -> Result<Value, NyxError>;

    /// Initialize the runtime backend
    fn initialize(&mut self, config: RuntimeConfig) -> Result<(), NyxError> {
        let _ = config;
        Ok(())
    }

    /// Shutdown the runtime backend
    fn shutdown(&mut self) -> Result<(), NyxError> {
        Ok(())
    }

    /// Get supported bytecode versions
    fn supported_versions(&self) -> Vec<Version> {
        vec![Version::default()]
    }

    /// Check if a feature is supported
    fn supports_feature(&self, feature: &str) -> bool {
        let _ = feature;
        false
    }
}

/// Extension point for target backends
///
/// Target backends generate code for specific platforms and architectures.
/// This trait allows adding support for new compilation targets.
///
/// # Example
///
/// ```
/// # use nyx::extensions::{TargetBackend, IR};
/// # use nyx::core::diagnostics::NyxError;
/// struct MyTargetBackend;
///
/// impl TargetBackend for MyTargetBackend {
///     fn target_triple(&self) -> &str {
///         "mytarget-unknown-os"
///     }
///
///     fn emit(&self, ir: &IR) -> Result<Vec<u8>, NyxError> {
///         // Generate target code from IR
///         Ok(Vec::new())
///     }
/// }
/// ```
pub trait TargetBackend: Send + Sync {
    /// Returns the target triple (e.g., "x86_64-unknown-linux-gnu")
    fn target_triple(&self) -> &str;

    /// Emit target-specific code from IR
    fn emit(&self, ir: &IR) -> Result<Vec<u8>, NyxError>;

    /// Get the backend name
    fn name(&self) -> &str {
        "unknown"
    }

    /// Get supported optimization levels
    fn supported_optimization_levels(&self) -> Vec<u8> {
        vec![0, 1, 2, 3]
    }

    /// Get target-specific features
    fn features(&self) -> HashMap<String, bool> {
        HashMap::new()
    }
}

/// Execution context for runtime backends
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// The bytecode to execute
    pub bytecode: Arc<Bytecode>,
    /// Entry point function name
    pub entry_point: String,
    /// Arguments to the entry point
    pub arguments: Vec<Value>,
    /// Environment variables
    pub environment: HashMap<String, String>,
    /// Working directory
    pub working_directory: Option<String>,
    /// Timeout in milliseconds
    pub timeout_ms: Option<u64>,
    /// Memory limit in bytes
    pub memory_limit: Option<usize>,
}

impl ExecutionContext {
    /// Create a new execution context
    pub fn new(bytecode: Arc<Bytecode>, entry_point: impl Into<String>) -> Self {
        Self {
            bytecode,
            entry_point: entry_point.into(),
            arguments: Vec::new(),
            environment: HashMap::new(),
            working_directory: None,
            timeout_ms: None,
            memory_limit: None,
        }
    }

    /// Add an argument
    pub fn with_arg(mut self, arg: Value) -> Self {
        self.arguments.push(arg);
        self
    }

    /// Add an environment variable
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.environment.insert(key.into(), value.into());
        self
    }

    /// Set working directory
    pub fn with_cwd(mut self, cwd: impl Into<String>) -> Self {
        self.working_directory = Some(cwd.into());
        self
    }

    /// Set timeout
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    /// Set memory limit
    pub fn with_memory_limit(mut self, limit: usize) -> Self {
        self.memory_limit = Some(limit);
        self
    }
}

/// Intermediate Representation (IR)
///
/// This is a simplified representation used for code generation.
/// In a full implementation, this would be a proper IR with
/// multiple levels (e.g., HIR, MIR, LIR).
#[derive(Debug, Clone)]
pub struct IR {
    /// IR version
    pub version: u32,
    /// Target triple
    pub target: String,
    /// IR instructions
    pub instructions: Vec<IRInstruction>,
    /// Function definitions
    pub functions: Vec<IRFunction>,
    /// Global variables
    pub globals: Vec<IRGlobal>,
    /// Metadata
    pub metadata: HashMap<String, String>,
}

impl Default for IR {
    fn default() -> Self {
        Self {
            version: 1,
            target: String::new(),
            instructions: Vec::new(),
            functions: Vec::new(),
            globals: Vec::new(),
            metadata: HashMap::new(),
        }
    }
}

/// IR instruction
#[derive(Debug, Clone)]
pub struct IRInstruction {
    /// Opcode
    pub opcode: String,
    /// Operands
    pub operands: Vec<IRValue>,
    /// Result type
    pub result_type: Option<IRType>,
}

/// IR value
#[derive(Debug, Clone)]
pub enum IRValue {
    /// Integer constant
    Int(i64),
    /// Float constant
    Float(f64),
    /// String constant
    String(String),
    /// Boolean constant
    Bool(bool),
    /// Register reference
    Register(u32),
    /// Global reference
    Global(String),
}

/// IR type
#[derive(Debug, Clone)]
pub enum IRType {
    /// Integer type
    Int(u32),
    /// Float type
    Float(u32),
    /// Pointer type
    Pointer(Box<IRType>),
    /// Function type
    Function(Vec<IRType>, Box<IRType>),
    /// Void type
    Void,
    /// Label type
    Label,
}

/// IR function
#[derive(Debug, Clone)]
pub struct IRFunction {
    /// Function name
    pub name: String,
    /// Function parameters
    pub parameters: Vec<IRParameter>,
    /// Return type
    pub return_type: IRType,
    /// Basic blocks
    pub blocks: Vec<IRBlock>,
}

/// IR parameter
#[derive(Debug, Clone)]
pub struct IRParameter {
    /// Parameter name
    pub name: String,
    /// Parameter type
    pub param_type: IRType,
}

/// IR basic block
#[derive(Debug, Clone)]
pub struct IRBlock {
    /// Block label
    pub label: String,
    /// Block instructions
    pub instructions: Vec<IRInstruction>,
    /// Terminator instruction
    pub terminator: Option<IRInstruction>,
}

/// IR global variable
#[derive(Debug, Clone)]
pub struct IRGlobal {
    /// Global name
    pub name: String,
    /// Global type
    pub global_type: IRType,
    /// Initial value
    pub initializer: Option<IRValue>,
    /// Whether it's mutable
    pub mutable: bool,
}

/// AST placeholder for compiler passes
///
/// In a full implementation, this would be the actual AST type
/// from the core module. For now, we use a placeholder.
#[derive(Debug, Clone)]
pub struct AST {
    /// AST version
    pub version: u32,
    /// Root nodes
    pub nodes: Vec<ASTNode>,
    /// Source file
    pub source_file: Option<String>,
    /// Metadata
    pub metadata: HashMap<String, String>,
}

impl Default for AST {
    fn default() -> Self {
        Self {
            version: 1,
            nodes: Vec::new(),
            source_file: None,
            metadata: HashMap::new(),
        }
    }
}

/// AST node placeholder
#[derive(Debug, Clone)]
pub struct ASTNode {
    /// Node type
    pub node_type: String,
    /// Children
    pub children: Vec<ASTNode>,
    /// Attributes
    pub attributes: HashMap<String, String>,
}

/// Extension registry for managing extension points
pub struct ExtensionRegistry {
    /// Registered compiler passes
    compiler_passes: HashMap<String, Box<dyn CompilerPass>>,
    /// Registered runtime backends
    runtime_backends: HashMap<String, Box<dyn RuntimeBackend>>,
    /// Registered target backends
    target_backends: HashMap<String, Box<dyn TargetBackend>>,
}

impl Default for ExtensionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ExtensionRegistry {
    /// Create a new extension registry
    pub fn new() -> Self {
        Self {
            compiler_passes: HashMap::new(),
            runtime_backends: HashMap::new(),
            target_backends: HashMap::new(),
        }
    }

    /// Register a compiler pass
    pub fn register_compiler_pass(&mut self, pass: Box<dyn CompilerPass>) -> Result<(), NyxError> {
        let name = pass.name().to_string();
        if self.compiler_passes.contains_key(&name) {
            return Err(NyxError::new(
                codes::INTERNAL_UNKNOWN_ERROR,
                format!("Compiler pass already registered: {}", name),
                ErrorCategory::Internal,
            ));
        }
        self.compiler_passes.insert(name, pass);
        Ok(())
    }

    /// Get a compiler pass by name
    pub fn get_compiler_pass(&self, name: &str) -> Option<&dyn CompilerPass> {
        self.compiler_passes.get(name).map(|p| p.as_ref())
    }

    /// List all registered compiler passes
    pub fn list_compiler_passes(&self) -> Vec<String> {
        self.compiler_passes.keys().cloned().collect()
    }

    /// Register a runtime backend
    pub fn register_runtime_backend(&mut self, backend: Box<dyn RuntimeBackend>) -> Result<(), NyxError> {
        let name = backend.name().to_string();
        if self.runtime_backends.contains_key(&name) {
            return Err(NyxError::new(
                codes::INTERNAL_UNKNOWN_ERROR,
                format!("Runtime backend already registered: {}", name),
                ErrorCategory::Internal,
            ));
        }
        self.runtime_backends.insert(name, backend);
        Ok(())
    }

    /// Get a runtime backend by name
    pub fn get_runtime_backend(&self, name: &str) -> Option<&dyn RuntimeBackend> {
        self.runtime_backends.get(name).map(|b| b.as_ref())
    }

    /// List all registered runtime backends
    pub fn list_runtime_backends(&self) -> Vec<String> {
        self.runtime_backends.keys().cloned().collect()
    }

    /// Register a target backend
    pub fn register_target_backend(&mut self, backend: Box<dyn TargetBackend>) -> Result<(), NyxError> {
        let triple = backend.target_triple().to_string();
        if self.target_backends.contains_key(&triple) {
            return Err(NyxError::new(
                codes::INTERNAL_UNKNOWN_ERROR,
                format!("Target backend already registered for: {}", triple),
                ErrorCategory::Internal,
            ));
        }
        self.target_backends.insert(triple, backend);
        Ok(())
    }

    /// Get a target backend by triple
    pub fn get_target_backend(&self, triple: &str) -> Option<&dyn TargetBackend> {
        self.target_backends.get(triple).map(|b| b.as_ref())
    }

    /// List all registered target backends
    pub fn list_target_backends(&self) -> Vec<String> {
        self.target_backends.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use interfaces::PrimitiveType;

    #[test]
    fn test_execution_context() {
        let bytecode = Arc::new(Bytecode {
            data: vec![],
            version: ApiVersion::default(),
            target: String::new(),
        });
        
        let ctx = ExecutionContext::new(bytecode, "main")
            .with_arg(Value {
                type_info: TypeInfo {
                    name: "i32".to_string(),
                    kind: TypeKind::Primitive(PrimitiveType::I32),
                },
                data: ValueData::Integer(42),
            })
            .with_env("KEY", "value")
            .with_timeout(5000);

        assert_eq!(ctx.entry_point, "main");
        assert_eq!(ctx.arguments.len(), 1);
        assert_eq!(ctx.environment.get("KEY"), Some(&"value".to_string()));
        assert_eq!(ctx.timeout_ms, Some(5000));
    }

    #[test]
    fn test_extension_registry() {
        let registry = ExtensionRegistry::new();
        
        // Should be able to list empty registries
        assert!(registry.list_compiler_passes().is_empty());
        assert!(registry.list_runtime_backends().is_empty());
        assert!(registry.list_target_backends().is_empty());
    }

    #[test]
    fn test_ir_default() {
        let ir = IR::default();
        assert_eq!(ir.version, 1);
        assert!(ir.instructions.is_empty());
    }

    #[test]
    fn test_ast_default() {
        let ast = AST::default();
        assert_eq!(ast.version, 1);
        assert!(ast.nodes.is_empty());
    }
}
