//! Debug Runtime for Nyx Debugger
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;

/// Runtime state during debugging
#[derive(Debug, Clone)]
pub struct DebugRuntime {
    /// Current execution state
    pub state: RuntimeState,
    /// Current line
    pub current_line: usize,
    /// Current function
    pub current_function: Option<String>,
    /// Call stack
    pub call_stack: Vec<CallFrame>,
    /// Execution history for stepping back
    pub history: Vec<ExecutionPoint>,
    /// Breakpoint hits
    pub breakpoint_hits: usize,
}

/// Runtime execution state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RuntimeState {
    /// Not started
    NotStarted,
    /// Running
    Running,
    /// Paused at breakpoint
    Paused,
    /// Stepping
    Stepping,
    /// Finished
    Finished,
    /// Error
    Error,
}

/// A call frame in the stack
#[derive(Debug, Clone)]
pub struct CallFrame {
    pub function: String,
    pub file: String,
    pub line: usize,
    pub return_address: Option<usize>,
}

/// A point in execution history
#[derive(Debug, Clone)]
pub struct ExecutionPoint {
    pub file: String,
    pub line: usize,
    pub function: String,
}

impl DebugRuntime {
    /// Create a new debug runtime
    pub fn new() -> Self {
        Self {
            state: RuntimeState::NotStarted,
            current_line: 0,
            current_function: None,
            call_stack: Vec::new(),
            history: Vec::new(),
            breakpoint_hits: 0,
        }
    }

    /// Start execution
    pub fn start(&mut self, entry: &str) {
        self.state = RuntimeState::Running;
        self.current_function = Some(entry.to_string());
        self.call_stack.push(CallFrame {
            function: entry.to_string(),
            file: String::new(),
            line: 1,
            return_address: None,
        });
    }

    /// Pause execution
    pub fn pause(&mut self) {
        if self.state == RuntimeState::Running || self.state == RuntimeState::Stepping {
            self.state = RuntimeState::Paused;
        }
    }

    /// Resume execution
    pub fn resume(&mut self) {
        if self.state == RuntimeState::Paused {
            self.state = RuntimeState::Running;
        }
    }

    /// Step to next line
    pub fn step(&mut self) {
        self.state = RuntimeState::Stepping;
    }

    /// Step over function call
    pub fn next(&mut self) {
        self.state = RuntimeState::Stepping;
    }

    /// Step out of function
    pub fn step_out(&mut self) {
        if !self.call_stack.is_empty() {
            self.call_stack.pop();
        }
        self.state = RuntimeState::Running;
    }

    /// Stop execution
    pub fn stop(&mut self) {
        self.state = RuntimeState::Finished;
        self.call_stack.clear();
    }

    /// Record current position in history
    pub fn record_position(&mut self, file: &str, line: usize, function: &str) {
        self.history.push(ExecutionPoint {
            file: file.to_string(),
            line,
            function: function.to_string(),
        });

        // Limit history size
        if self.history.len() > 1000 {
            self.history.remove(0);
        }
    }

    /// Get backtrace
    pub fn backtrace(&self) -> Vec<String> {
        self.call_stack
            .iter()
            .enumerate()
            .map(|(i, frame)| format!("#{} {} at {}:{}", i, frame.function, frame.file, frame.line))
            .collect()
    }

    /// Check if should stop (breakpoint or step complete)
    pub fn should_stop(&self) -> bool {
        matches!(
            self.state,
            RuntimeState::Paused | RuntimeState::Finished | RuntimeState::Error
        )
    }

    /// Set current line
    pub fn set_line(&mut self, line: usize) {
        self.current_line = line;
        if let Some(frame) = self.call_stack.last_mut() {
            frame.line = line;
        }
    }

    /// Set error state
    pub fn set_error(&mut self) {
        self.state = RuntimeState::Error;
    }

    /// Get state as string
    pub fn state_string(&self) -> &'static str {
        match self.state {
            RuntimeState::NotStarted => "not started",
            RuntimeState::Running => "running",
            RuntimeState::Paused => "paused",
            RuntimeState::Stepping => "stepping",
            RuntimeState::Finished => "finished",
            RuntimeState::Error => "error",
        }
    }

    /// Increment breakpoint hits
    pub fn hit_breakpoint(&mut self) {
        self.breakpoint_hits += 1;
        self.state = RuntimeState::Paused;
    }
}

impl Default for DebugRuntime {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for the debugger
#[derive(Debug, Clone)]
pub struct DebugConfig {
    /// Source files to debug
    pub sources: Vec<PathBuf>,
    /// Working directory
    pub work_dir: PathBuf,
    /// Environment variables
    pub env: HashMap<String, String>,
    /// Arguments
    pub args: Vec<String>,
    /// Timeout in seconds
    pub timeout: Option<u64>,
    /// Enable verbose output
    pub verbose: bool,
}

impl DebugConfig {
    /// Create a new config
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
            work_dir: std::env::current_dir().unwrap_or_default(),
            env: HashMap::new(),
            args: Vec::new(),
            timeout: None,
            verbose: false,
        }
    }

    /// Add a source file
    pub fn with_source(mut self, path: PathBuf) -> Self {
        self.sources.push(path);
        self
    }

    /// Set working directory
    pub fn with_work_dir(mut self, dir: PathBuf) -> Self {
        self.work_dir = dir;
        self
    }

    /// Add environment variable
    pub fn with_env(mut self, key: &str, value: &str) -> Self {
        self.env.insert(key.to_string(), value.to_string());
        self
    }

    /// Add argument
    pub fn with_arg(mut self, arg: &str) -> Self {
        self.args.push(arg.to_string());
        self
    }

    /// Set timeout
    pub fn with_timeout(mut self, seconds: u64) -> Self {
        self.timeout = Some(seconds);
        self
    }

    /// Enable verbose output
    pub fn verbose(mut self) -> Self {
        self.verbose = true;
        self
    }
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_state() {
        let mut runtime = DebugRuntime::new();
        assert_eq!(runtime.state, RuntimeState::NotStarted);

        runtime.start("main");
        assert_eq!(runtime.state, RuntimeState::Running);

        runtime.pause();
        assert_eq!(runtime.state, RuntimeState::Paused);

        runtime.resume();
        assert_eq!(runtime.state, RuntimeState::Running);

        runtime.step();
        assert_eq!(runtime.state, RuntimeState::Stepping);

        runtime.stop();
        assert_eq!(runtime.state, RuntimeState::Finished);
    }

    #[test]
    fn test_backtrace() {
        let mut runtime = DebugRuntime::new();
        runtime.start("main");

        let bt = runtime.backtrace();
        assert!(!bt.is_empty());
    }
}
