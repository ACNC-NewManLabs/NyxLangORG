//! Breakpoint Management for Nyx Debugger
#![allow(dead_code)]

use std::collections::HashMap;

/// A breakpoint in Nyx source
#[derive(Debug, Clone)]
pub struct Breakpoint {
    pub id: usize,
    pub file: String,
    pub line: Option<usize>,
    pub function: Option<String>,
    pub condition: Option<String>,
    pub enabled: bool,
    pub hit_count: usize,
}

/// Manager for all breakpoints
pub struct BreakpointManager {
    breakpoints: HashMap<usize, Breakpoint>,
    next_id: usize,
}

impl BreakpointManager {
    /// Create a new breakpoint manager
    pub fn new() -> Self {
        Self {
            breakpoints: HashMap::new(),
            next_id: 1,
        }
    }
    
    /// Add a line breakpoint
    pub fn add_line_breakpoint(&mut self, file: String, line: usize) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        
        let bp = Breakpoint {
            id,
            file,
            line: Some(line),
            function: None,
            condition: None,
            enabled: true,
            hit_count: 0,
        };
        
        self.breakpoints.insert(id, bp);
        id
    }
    
    /// Add a function breakpoint
    pub fn add_function_breakpoint(&mut self, file: String, function: String) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        
        let bp = Breakpoint {
            id,
            file,
            line: None,
            function: Some(function),
            condition: None,
            enabled: true,
            hit_count: 0,
        };
        
        self.breakpoints.insert(id, bp);
        id
    }
    
    /// Add a conditional breakpoint
    pub fn add_conditional_breakpoint(&mut self, file: String, line: usize, condition: String) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        
        let bp = Breakpoint {
            id,
            file: file.clone(),
            line: Some(line),
            function: None,
            condition: Some(condition),
            enabled: true,
            hit_count: 0,
        };
        
        self.breakpoints.insert(id, bp);
        id
    }
    
    /// Remove a breakpoint by ID
    pub fn remove(&mut self, id: usize) -> bool {
        self.breakpoints.remove(&id).is_some()
    }
    
    /// Enable a breakpoint
    pub fn enable(&mut self, id: usize) -> bool {
        if let Some(bp) = self.breakpoints.get_mut(&id) {
            bp.enabled = true;
            true
        } else {
            false
        }
    }
    
    /// Disable a breakpoint
    pub fn disable(&mut self, id: usize) -> bool {
        if let Some(bp) = self.breakpoints.get_mut(&id) {
            bp.enabled = false;
            true
        } else {
            false
        }
    }
    
    /// Get a breakpoint by ID
    pub fn get(&self, id: usize) -> Option<&Breakpoint> {
        self.breakpoints.get(&id)
    }
    
    /// Get all breakpoints
    pub fn all(&self) -> Vec<&Breakpoint> {
        self.breakpoints.values().collect()
    }
    
    /// Get enabled breakpoints for a file
    pub fn get_for_file(&self, file: &str) -> Vec<&Breakpoint> {
        self.breakpoints
            .values()
            .filter(|bp| bp.file == file && bp.enabled)
            .collect()
    }
    
    /// Check if a line has a breakpoint
    pub fn has_breakpoint_at(&self, file: &str, line: usize) -> bool {
        self.breakpoints
            .values()
            .any(|bp| bp.file == file && bp.line == Some(line) && bp.enabled)
    }
    
    /// Increment hit count for a breakpoint
    pub fn hit(&mut self, id: usize) {
        if let Some(bp) = self.breakpoints.get_mut(&id) {
            bp.hit_count += 1;
        }
    }
    
    /// List all breakpoints as strings
    pub fn list(&self) -> Vec<String> {
        let mut lines = Vec::new();
        
        for bp in self.breakpoints.values() {
            let status = if bp.enabled { "enabled" } else { "disabled" };
            let location = if let Some(line) = bp.line {
                format!("line {}", line)
            } else if let Some(ref func) = bp.function {
                format!("function {}", func)
            } else {
                "unknown".to_string()
            };
            
            lines.push(format!(
                "{}: {} at {} [hits: {}, {}]",
                bp.id, bp.file, location, bp.hit_count, status
            ));
        }
        
        lines
    }
}

impl Default for BreakpointManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_add_line_breakpoint() {
        let mut manager = BreakpointManager::new();
        let id = manager.add_line_breakpoint("test.nyx".to_string(), 10);
        assert_eq!(id, 1);
        
        let bp = manager.get(id).unwrap();
        assert_eq!(bp.line, Some(10));
        assert!(bp.enabled);
    }
    
    #[test]
    fn test_remove_breakpoint() {
        let mut manager = BreakpointManager::new();
        let id = manager.add_line_breakpoint("test.nyx".to_string(), 10);
        
        assert!(manager.remove(id));
        assert!(manager.get(id).is_none());
    }
    
    #[test]
    fn test_enable_disable() {
        let mut manager = BreakpointManager::new();
        let id = manager.add_line_breakpoint("test.nyx".to_string(), 10);
        
        manager.disable(id);
        assert!(!manager.get(id).unwrap().enabled);
        
        manager.enable(id);
        assert!(manager.get(id).unwrap().enabled);
    }
}
