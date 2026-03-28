//! Bytecode Loader
//! 
//! This module handles loading bytecode from files.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use crate::bytecode::{deserialize_module, serialize_module, BytecodeModule};
use crate::{VmError, VmResult};

/// Bytecode file loader
pub struct BytecodeLoader;

impl BytecodeLoader {
    /// Load bytecode from file
    pub fn load_file(path: &Path) -> VmResult<BytecodeModule> {
        let mut file = File::open(path)
            .map_err(|e| VmError::IoError(e.to_string()))?;
        
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|e| VmError::IoError(e.to_string()))?;
        
        Self::load_bytes(&bytes)
    }

    /// Load bytecode from bytes
    pub fn load_bytes(bytes: &[u8]) -> VmResult<BytecodeModule> {
        deserialize_module(bytes)
            .map_err(|e| VmError::InvalidOperand(e))
    }

    /// Save bytecode to file
    pub fn save_file(module: &BytecodeModule, path: &Path) -> VmResult<()> {
        let bytes = serialize_module(module)
            .map_err(|e| VmError::InvalidOperand(e))?;
        
        let mut file = File::create(path)
            .map_err(|e| VmError::IoError(e.to_string()))?;
        
        file.write_all(&bytes)
            .map_err(|e| VmError::IoError(e.to_string()))?;
        
        Ok(())
    }

    /// Get bytecode file extension
    pub fn file_extension() -> &'static str {
        "nyxb"
    }
}

/// Auto-detect file format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    /// Nyx bytecode
    Bytecode,
    /// Nyx source
    Source,
    /// LLVM IR
    LlvmIr,
    /// Unknown
    Unknown,
}

impl FileFormat {
    /// Detect format from file extension
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "nyxb" => FileFormat::Bytecode,
            "nyx" => FileFormat::Source,
            "ll" | "llvm" => FileFormat::LlvmIr,
            _ => FileFormat::Unknown,
        }
    }

    /// Detect format from file content
    pub fn from_content(content: &[u8]) -> Self {
        if content.len() >= 4 && &content[0..4] == b"NYXB" {
            FileFormat::Bytecode
        } else if content.starts_with(b"; ModuleID") {
            FileFormat::LlvmIr
        } else if is_probably_source(content) {
            FileFormat::Source
        } else {
            FileFormat::Unknown
        }
    }
}

fn is_probably_source(content: &[u8]) -> bool {
    // Fast, conservative checks: Nyx source is expected to be UTF-8 text.
    let Ok(text) = std::str::from_utf8(content) else {
        return false;
    };
    let trimmed = text.trim_start();
    if trimmed.is_empty() {
        return false;
    }

    // Common Nyx/Rust-like entry points.
    for prefix in ["fn ", "use ", "mod ", "let ", "struct ", "enum ", "//", "#!"] {
        if trimmed.starts_with(prefix) {
            return true;
        }
    }

    // Heuristic: if the content is mostly printable, treat it as source.
    let mut printable = 0usize;
    let mut total = 0usize;
    for b in content {
        total += 1;
        if b.is_ascii_graphic() || *b == b' ' || *b == b'\n' || *b == b'\r' || *b == b'\t' {
            printable += 1;
        }
    }
    printable * 100 / total.max(1) > 90
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_extension() {
        assert_eq!(BytecodeLoader::file_extension(), "nyxb");
    }

    #[test]
    fn test_format_detection() {
        assert_eq!(FileFormat::from_extension("nyxb"), FileFormat::Bytecode);
        assert_eq!(FileFormat::from_extension("nyx"), FileFormat::Source);
        assert_eq!(FileFormat::from_extension("ll"), FileFormat::LlvmIr);
    }

    #[test]
    fn test_format_from_content() {
        assert_eq!(FileFormat::from_content(b"NYXB"), FileFormat::Bytecode);
        assert_eq!(FileFormat::from_content(b"; ModuleID"), FileFormat::LlvmIr);
        assert_eq!(FileFormat::from_content(b"fn main()"), FileFormat::Source);
    }
}
