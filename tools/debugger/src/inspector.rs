//! Variable Inspector for Nyx Debugger
#![allow(dead_code)]

use std::collections::HashMap;

/// A variable value in the debugger
#[derive(Debug, Clone, PartialEq)]
pub enum VariableValue {
    Integer(i64),
    Float(f64),
    Boolean(bool),
    String(String),
    Array(Vec<VariableValue>),
    Struct(HashMap<String, VariableValue>),
    Null,
    Unknown,
}

/// A variable in a scope
#[derive(Debug, Clone)]
pub struct Variable {
    pub name: String,
    pub value: VariableValue,
    pub type_name: String,
    pub scope: VariableScope,
}

/// Variable scope
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VariableScope {
    Global,
    Local,
    Parameter,
}

/// Inspector for examining variables at runtime
pub struct VariableInspector {
    /// Current scope variables
    variables: HashMap<String, Variable>,
    /// Call stack frames
    frames: Vec<Frame>,
}

/// A stack frame
#[derive(Debug, Clone)]
pub struct Frame {
    pub function: String,
    pub line: usize,
    pub locals: HashMap<String, VariableValue>,
}

impl VariableInspector {
    /// Create a new inspector
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            frames: Vec::new(),
        }
    }

    /// Set a variable value
    pub fn set_variable(
        &mut self,
        name: String,
        value: VariableValue,
        type_name: String,
        scope: VariableScope,
    ) {
        self.variables.insert(
            name.clone(),
            Variable {
                name,
                value,
                type_name,
                scope,
            },
        );
    }

    /// Get a variable by name
    pub fn get_variable(&self, name: &str) -> Option<&Variable> {
        self.variables.get(name)
    }

    /// Get all local variables
    pub fn get_locals(&self) -> Vec<&Variable> {
        self.variables
            .values()
            .filter(|v| v.scope == VariableScope::Local || v.scope == VariableScope::Parameter)
            .collect()
    }

    /// Get all global variables
    pub fn get_globals(&self) -> Vec<&Variable> {
        self.variables
            .values()
            .filter(|v| v.scope == VariableScope::Global)
            .collect()
    }

    /// Push a new stack frame
    pub fn push_frame(&mut self, function: String, line: usize) {
        self.frames.push(Frame {
            function,
            line,
            locals: HashMap::new(),
        });
    }

    /// Pop a stack frame
    pub fn pop_frame(&mut self) -> Option<Frame> {
        self.frames.pop()
    }

    /// Get current frame
    pub fn current_frame(&self) -> Option<&Frame> {
        self.frames.last()
    }

    /// Get backtrace (all frames)
    pub fn backtrace(&self) -> Vec<(String, usize)> {
        self.frames
            .iter()
            .map(|f| (f.function.clone(), f.line))
            .collect()
    }

    /// Format a value for display
    pub fn format_value(&self, value: &VariableValue) -> String {
        match value {
            VariableValue::Integer(i) => format!("{}", i),
            VariableValue::Float(f) => format!("{}", f),
            VariableValue::Boolean(b) => format!("{}", b),
            VariableValue::String(s) => format!("\"{}\"", s),
            VariableValue::Array(arr) => {
                let items: Vec<String> = arr.iter().map(|v| self.format_value(v)).collect();
                format!("[{}]", items.join(", "))
            }
            VariableValue::Struct(fields) => {
                let items: Vec<String> = fields
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, self.format_value(v)))
                    .collect();
                format!("{{{}}}", items.join(", "))
            }
            VariableValue::Null => "null".to_string(),
            VariableValue::Unknown => "<unknown>".to_string(),
        }
    }

    /// List all variables in a formatted string
    pub fn list_all(&self) -> String {
        let mut output = String::new();

        // Globals
        let globals: Vec<_> = self.get_globals();
        let globals_empty = globals.is_empty();
        if !globals_empty {
            output.push_str("Global variables:\n");
            for var in &globals {
                output.push_str(&format!(
                    "  {} ({}) = {}\n",
                    var.name,
                    var.type_name,
                    self.format_value(&var.value)
                ));
            }
        }

        // Locals
        let locals: Vec<_> = self.get_locals();
        let locals_empty = locals.is_empty();
        if !locals_empty {
            output.push_str("Local variables:\n");
            for var in &locals {
                output.push_str(&format!(
                    "  {} ({}) = {}\n",
                    var.name,
                    var.type_name,
                    self.format_value(&var.value)
                ));
            }
        }

        if globals_empty && locals_empty {
            output.push_str("No variables in scope.\n");
        }

        output
    }

    /// Evaluate a simple expression
    pub fn evaluate(&self, expr: &str) -> Option<VariableValue> {
        // Try to parse as a variable name
        if let Some(var) = self.get_variable(expr) {
            return Some(var.value.clone());
        }

        // Try to parse as integer literal
        if let Ok(i) = expr.parse::<i64>() {
            return Some(VariableValue::Integer(i));
        }

        // Try to parse as float literal
        if let Ok(f) = expr.parse::<f64>() {
            return Some(VariableValue::Float(f));
        }

        // Try to parse as boolean
        match expr {
            "true" => return Some(VariableValue::Boolean(true)),
            "false" => return Some(VariableValue::Boolean(false)),
            _ => {}
        }

        // Try to parse as string
        if expr.starts_with('"') && expr.ends_with('"') {
            return Some(VariableValue::String(expr[1..expr.len() - 1].to_string()));
        }

        None
    }
}

impl Default for VariableInspector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_and_get_variable() {
        let mut inspector = VariableInspector::new();
        inspector.set_variable(
            "x".to_string(),
            VariableValue::Integer(42),
            "int".to_string(),
            VariableScope::Local,
        );

        let var = inspector.get_variable("x").unwrap();
        assert_eq!(inspector.format_value(&var.value), "42");
    }

    #[test]
    fn test_evaluate() {
        let inspector = VariableInspector::new();

        assert_eq!(inspector.evaluate("42"), Some(VariableValue::Integer(42)));
        assert_eq!(inspector.evaluate("3.14"), Some(VariableValue::Float(3.14)));
        assert_eq!(
            inspector.evaluate("true"),
            Some(VariableValue::Boolean(true))
        );
    }
}
