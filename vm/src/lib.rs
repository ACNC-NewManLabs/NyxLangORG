//! Nyx Bytecode Virtual Machine
//! 
//! This module provides a stack-based bytecode VM for executing Nyx programs.
//! The VM executes bytecode generated.
// from Nyx IR!
//! Execution pipeline:
//! Nyx Source → Lexer → Parser → AST → Semantic → Nyx IR → Bytecode → Nyx VM

pub mod bytecode;
pub mod emitter;
pub mod runtime;
pub mod loader;
pub mod jit;

pub use bytecode::{BytecodeModule, Function, BytecodeInstr as Instruction, OpCode, Value};
pub use emitter::BytecodeEmitter;
pub use runtime::NyxVm;
pub use loader::BytecodeLoader;

/// VM version
pub const VM_VERSION: &str = "1.0.0";

/// Maximum stack size
pub const MAX_STACK_SIZE: usize = 1024;

/// Maximum call frame depth
pub const MAX_CALL_DEPTH: usize = 256;

/// Default heap size (16MB)
pub const DEFAULT_HEAP_SIZE: usize = 16 * 1024 * 1024;

/// Default instruction budget (0 = unlimited)
pub const DEFAULT_MAX_INSTRUCTIONS: u64 = 0;

/// Default max execution time in milliseconds (0 = unlimited)
pub const DEFAULT_MAX_EXEC_TIME_MS: u64 = 0;

/// VM configuration
pub struct VmConfig {
    /// Maximum stack size
    pub max_stack_size: usize,
    /// Maximum call depth
    pub max_call_depth: usize,
    /// Heap size in bytes
    pub heap_size: usize,
    /// Maximum instructions per run (0 = unlimited)
    pub max_instructions: u64,
    /// Maximum execution time in milliseconds (0 = unlimited)
    pub max_exec_time_ms: u64,
    /// Enable JIT compilation
    pub enable_jit: bool,
    /// Enable garbage collection
    pub enable_gc: bool,
    /// Enable debug mode
    pub debug: bool,
    /// Optional hook to call before executing each instruction
    #[allow(clippy::type_complexity)]
    pub on_step: Option<Box<dyn FnMut(&crate::runtime::NyxVm, &crate::bytecode::BytecodeInstr, usize) -> Result<(), crate::VmError>>>,
}

impl std::fmt::Debug for VmConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VmConfig")
            .field("max_stack_size", &self.max_stack_size)
            .field("max_call_depth", &self.max_call_depth)
            .field("heap_size", &self.heap_size)
            .field("max_instructions", &self.max_instructions)
            .field("max_exec_time_ms", &self.max_exec_time_ms)
            .field("enable_jit", &self.enable_jit)
            .field("enable_gc", &self.enable_gc)
            .field("debug", &self.debug)
            .field("on_step", &self.on_step.is_some())
            .finish()
    }
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            max_stack_size: MAX_STACK_SIZE,
            max_call_depth: MAX_CALL_DEPTH,
            heap_size: DEFAULT_HEAP_SIZE,
            max_instructions: DEFAULT_MAX_INSTRUCTIONS,
            max_exec_time_ms: DEFAULT_MAX_EXEC_TIME_MS,
            enable_jit: false,
            enable_gc: true,
            debug: false,
            on_step: None,
        }
    }
}

/// VM error types
#[derive(Debug)]
pub enum VmError {
    /// Out of memory
    OutOfMemory,
    /// Stack overflow
    StackOverflow,
    /// Stack underflow
    StackUnderflow,
    /// Invalid opcode
    InvalidOpCode(u8),
    /// Invalid operand
    InvalidOperand(String),
    /// Division by zero
    DivisionByZero,
    /// Undefined variable
    UndefinedVariable(String),
    /// Undefined function
    UndefinedFunction(String),
    /// Type error
    TypeError(String),
    /// Runtime error
    RuntimeError(String),
    /// Instruction limit exceeded
    InstructionLimitExceeded(u64),
    /// Execution time limit exceeded
    TimeLimitExceeded(u64),
    /// I/O error
    IoError(String),
    /// Breakpoint hit
    Breakpoint(u32),
}

impl std::fmt::Display for VmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VmError::OutOfMemory => write!(f, "Out of memory"),
            VmError::StackOverflow => write!(f, "Stack overflow"),
            VmError::StackUnderflow => write!(f, "Stack underflow"),
            VmError::InvalidOpCode(op) => write!(f, "Invalid opcode: {}", op),
            VmError::InvalidOperand(msg) => write!(f, "Invalid operand: {}", msg),
            VmError::DivisionByZero => write!(f, "Division by zero"),
            VmError::UndefinedVariable(name) => write!(f, "Undefined variable: {}", name),
            VmError::UndefinedFunction(name) => write!(f, "Undefined function: {}", name),
            VmError::TypeError(msg) => write!(f, "Type error: {}", msg),
            VmError::RuntimeError(msg) => write!(f, "Runtime error: {}", msg),
            VmError::InstructionLimitExceeded(limit) => write!(f, "Instruction limit exceeded: {}", limit),
            VmError::TimeLimitExceeded(limit) => write!(f, "Execution time limit exceeded: {}ms", limit),
            VmError::IoError(msg) => write!(f, "I/O error: {}", msg),
            VmError::Breakpoint(line) => write!(f, "Breakpoint at line {}", line),
        }
    }
}

impl std::error::Error for VmError {}

/// VM result type
pub type VmResult<T> = Result<T, VmError>;

/// Initialize the VM subsystem
pub fn init() -> Result<(), VmError> {
    log::info!("Nyx VM v{} initialized", VM_VERSION);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_config() {
        let config = VmConfig::default();
        assert_eq!(config.max_stack_size, MAX_STACK_SIZE);
    }

    #[test]
    fn test_vm_init() {
        assert!(init().is_ok());
    }
}
