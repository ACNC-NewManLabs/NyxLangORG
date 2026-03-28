//! Vulnerability Database for Security Scanner

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Information about the vulnerability database
#[derive(Debug, Clone)]
pub struct DbInfo {
    pub version: String,
    pub last_updated: String,
    pub total_vulnerabilities: usize,
    pub cwe_count: usize,
}

/// A vulnerability entry in the database
#[derive(Debug, Clone)]
pub struct VulnerabilityEntry {
    pub id: String,
    pub cwe_id: String,
    pub title: String,
    pub description: String,
    pub severity: String,
    pub affected_versions: Vec<String>,
    pub recommendation: String,
}

/// Vulnerability database
pub struct VulnerabilityDatabase {
    /// Database directory
    db_path: PathBuf,
    /// Cached vulnerabilities
    vulnerabilities: HashMap<String, VulnerabilityEntry>,
}

impl VulnerabilityDatabase {
    /// Create a new database
    pub fn new() -> Self {
        let db_path = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("nyx")
            .join("security-db");
        
        let mut db = Self {
            db_path,
            vulnerabilities: HashMap::new(),
        };
        
        // Initialize with built-in vulnerabilities
        db.init_builtin();
        
        db
    }
    
    /// Initialize with built-in vulnerability data
    fn init_builtin(&mut self) {
        let entries = vec![
            VulnerabilityEntry {
                id: "CWE-78".to_string(),
                cwe_id: "CWE-78".to_string(),
                title: "OS Command Injection".to_string(),
                description: "The application constructs part of an OS command using externally-influenced input".to_string(),
                severity: "critical".to_string(),
                affected_versions: vec!["all".to_string()],
                recommendation: "Avoid using user input in system calls. Use parameterized commands.".to_string(),
            },
            VulnerabilityEntry {
                id: "CWE-22".to_string(),
                cwe_id: "CWE-22".to_string(),
                title: "Path Traversal".to_string(),
                description: "The application uses external input to construct a pathname".to_string(),
                severity: "high".to_string(),
                affected_versions: vec!["all".to_string()],
                recommendation: "Validate and sanitize file paths. Use allowlists.".to_string(),
            },
            VulnerabilityEntry {
                id: "CWE-798".to_string(),
                cwe_id: "CWE-798".to_string(),
                title: "Use of Hard-coded Credentials".to_string(),
                description: "The program contains embedded credentials".to_string(),
                severity: "critical".to_string(),
                affected_versions: vec!["all".to_string()],
                recommendation: "Use environment variables or secure credential storage.".to_string(),
            },
            VulnerabilityEntry {
                id: "CWE-89".to_string(),
                cwe_id: "CWE-89".to_string(),
                title: "SQL Injection".to_string(),
                description: "The application constructs an SQL query using externally-influenced input".to_string(),
                severity: "critical".to_string(),
                affected_versions: vec!["all".to_string()],
                recommendation: "Use parameterized queries instead of string concatenation.".to_string(),
            },
            VulnerabilityEntry {
                id: "CWE-327".to_string(),
                cwe_id: "CWE-327".to_string(),
                title: "Use of Weak Cryptographic Algorithm".to_string(),
                description: "The use of a broken or weak cryptographic algorithm".to_string(),
                severity: "medium".to_string(),
                affected_versions: vec!["all".to_string()],
                recommendation: "Use strong algorithms like SHA-256, AES, or ChaCha20.".to_string(),
            },
            VulnerabilityEntry {
                id: "CWE-242".to_string(),
                cwe_id: "CWE-242".to_string(),
                title: "Use of Inherently Dangerous Function".to_string(),
                description: "The program calls a potentially dangerous function".to_string(),
                severity: "medium".to_string(),
                affected_versions: vec!["all".to_string()],
                recommendation: "Review unsafe code and minimize its usage.".to_string(),
            },
            VulnerabilityEntry {
                id: "CWE-190".to_string(),
                cwe_id: "CWE-190".to_string(),
                title: "Integer Overflow".to_string(),
                description: "The program performs arithmetic that may produce overflow".to_string(),
                severity: "medium".to_string(),
                affected_versions: vec!["all".to_string()],
                recommendation: "Use checked arithmetic or larger integer types.".to_string(),
            },
            VulnerabilityEntry {
                id: "CWE-1004".to_string(),
                cwe_id: "CWE-1004".to_string(),
                title: "Software Sensitive Information Vote".to_string(),
                description: "Application has a TODO in source code".to_string(),
                severity: "low".to_string(),
                affected_versions: vec!["all".to_string()],
                recommendation: "Address the TODO or create a tracking issue.".to_string(),
            },
        ];
        
        for entry in entries {
            self.vulnerabilities.insert(entry.id.clone(), entry);
        }
    }
    
    /// Update the database from remote source
    pub fn update(&self) -> Result<(), String> {
        // In a production system, this would fetch from a remote vulnerability database
        // For now, we just ensure the local database directory exists
        
        let db_dir = &self.db_path;
        
        if let Err(e) = fs::create_dir_all(db_dir) {
            return Err(format!("Failed to create database directory: {}", e));
        }
        
        // Write version info
        let version_file = db_dir.join("version.json");
        let version_info = serde_json::json!({
            "version": "1.0.0",
            "updated_at": SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        });
        
        if let Err(e) = fs::write(&version_file, version_info.to_string()) {
            return Err(format!("Failed to write version file: {}", e));
        }
        
        Ok(())
    }
    
    /// Get database information
    pub fn get_info(&self) -> DbInfo {
        DbInfo {
            version: "1.0.0".to_string(),
            last_updated: "2026-03-09".to_string(),
            total_vulnerabilities: self.vulnerabilities.len(),
            cwe_count: self.vulnerabilities.len(),
        }
    }
    
    /// Look up a vulnerability by CWE ID
    pub fn lookup(&self, cwe_id: &str) -> Option<&VulnerabilityEntry> {
        self.vulnerabilities.get(cwe_id)
    }
    
    /// Get all vulnerabilities
    pub fn all(&self) -> Vec<&VulnerabilityEntry> {
        self.vulnerabilities.values().collect()
    }
}

impl Default for VulnerabilityDatabase {
    fn default() -> Self {
        Self::new()
    }
}

// Helper module for directory paths
mod dirs {
    use std::path::PathBuf;
    
    pub fn data_local_dir() -> Option<PathBuf> {
        std::env::var("LOCALAPPDATA")
            .or_else(|_| std::env::var("XDG_DATA_HOME"))
            .or_else(|_| std::env::var("HOME"))
            .ok()
            .map(PathBuf::from)
            .or_else(|| Some(PathBuf::from(".")))
    }
}

