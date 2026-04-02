//! NYX Core Library
//!
//! The foundational layer that works without an OS.
//! Provides essential types, traits, and utilities.

/// Initialize the core library
pub fn init() {
    // Core is always available, no runtime initialization needed
}

/// Shutdown the core library  
pub fn shutdown() {
    // Core cleanup if needed
}

// Internal modules
pub mod iter;
pub mod mem;
pub mod option;
pub mod primitive_extensions;
pub mod ptr;
pub mod result;
pub mod traits;

// Re-export core types from custom implementations
pub use self::iter::*;
pub use self::mem::*;
pub use self::option::Option;
pub use self::ptr::*;
pub use self::result::Result;
pub use self::traits::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_option_some() {
        let x = Option::Some(5);
        assert!(x.is_some());
        assert_eq!(x.unwrap(), 5);
    }

    #[test]
    fn test_option_none() {
        let x: Option<i32> = Option::None;
        assert!(x.is_none());
    }

    #[test]
    fn test_result_ok() {
        let x: Result<i32, &str> = Result::Ok(5);
        assert!(x.is_ok());
        assert_eq!(x.unwrap(), 5);
    }

    #[test]
    fn test_result_err() {
        let x: Result<i32, &str> = Result::Err("error");
        assert!(x.is_err());
        assert_eq!(x.unwrap_err(), "error");
    }
}
