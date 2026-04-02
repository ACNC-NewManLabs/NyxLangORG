//! NYX Error Layer

pub mod error {
    pub use nyx_diagnostics::{ErrorCategory, NyxError, Severity};
    use std::fmt;

    pub trait Error: fmt::Debug + fmt::Display {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            None
        }

        /// Convert to a unified NyxError
        fn into_nyx_error(self) -> NyxError
        where
            Self: Sized,
        {
            NyxError::new("std::error", self.to_string(), ErrorCategory::Runtime)
        }
    }

    pub struct ErrorKind {
        pub code: String,
        pub msg: String,
    }

    impl ErrorKind {
        pub fn new(code: &str, msg: &str) -> ErrorKind {
            ErrorKind {
                code: code.to_string(),
                msg: msg.to_string(),
            }
        }

        pub fn to_nyx(&self) -> NyxError {
            NyxError::new(self.code.clone(), self.msg.clone(), ErrorCategory::Runtime)
        }
    }

    impl fmt::Display for ErrorKind {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "[{}]: {}", self.code, self.msg)
        }
    }

    impl fmt::Debug for ErrorKind {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "ErrorKind({}, {})", self.code, self.msg)
        }
    }
}

pub use error::*;
