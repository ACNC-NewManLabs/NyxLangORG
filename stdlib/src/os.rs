//! NYX OS Layer
//! Industrial FileSystem, Process, and Environment abstractions.

pub mod os {
    pub mod filesystem {
        use crate::error::{ErrorCategory, NyxError};
        use std::path::Path;

        pub fn read_file(path: &str) -> Result<Vec<u8>, NyxError> {
            std::fs::read(path).map_err(|e| {
                NyxError::new(
                    "OS001",
                    format!("Failed to read file '{}': {}", path, e),
                    ErrorCategory::Io,
                )
                .with_suggestion("Verify path exists and permissions are correct.")
            })
        }

        pub fn write_file(path: &str, data: &[u8]) -> Result<(), NyxError> {
            std::fs::write(path, data).map_err(|e| {
                NyxError::new(
                    "OS002",
                    format!("Failed to write file '{}': {}", path, e),
                    ErrorCategory::Io,
                )
                .with_suggestion("Check disk space and write permissions.")
            })
        }

        pub fn copy_file(from: &str, to: &str) -> Result<u64, NyxError> {
            std::fs::copy(from, to).map_err(|e| {
                NyxError::new(
                    "OS003",
                    format!("Failed to copy '{}' to '{}': {}", from, to, e),
                    ErrorCategory::Io,
                )
            })
        }

        pub fn rename(from: &str, to: &str) -> Result<(), NyxError> {
            std::fs::rename(from, to).map_err(|e| {
                NyxError::new(
                    "OS004",
                    format!("Failed to rename '{}' to '{}': {}", from, to, e),
                    ErrorCategory::Io,
                )
            })
        }

        pub fn remove_file(path: &str) -> Result<(), NyxError> {
            std::fs::remove_file(path).map_err(|e| {
                NyxError::new(
                    "OS005",
                    format!("Failed to remove file '{}': {}", path, e),
                    ErrorCategory::Io,
                )
            })
        }

        pub fn exists(path: &str) -> bool {
            Path::new(path).exists()
        }

        pub fn is_dir(path: &str) -> bool {
            Path::new(path).is_dir()
        }
    }

    pub mod process {
        pub fn exit(code: i32) -> ! {
            std::process::exit(code)
        }

        pub fn id() -> u32 {
            std::process::id()
        }
    }

    pub mod environment {
        pub fn get_var(key: &str) -> Option<String> {
            std::env::var(key).ok()
        }

        pub fn set_var(key: &str, value: &str) {
            std::env::set_var(key, value)
        }
    }
}

pub use os::*;
