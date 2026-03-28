//! Nyx Compiler Diagnostics
//!
//! Comprehensive error handling framework with typed error classes,
//! builder pattern, error propagation, recovery mechanisms, and
//! structured logging with colored terminal output.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════════════
// RE-EXPORTS FROM NYX_DIAGNOSTICS
// ═══════════════════════════════════════════════════════════════════════════════

pub use nyx_diagnostics::{Severity, ErrorCategory, Position, Span, NyxError, codes, LogLevel, LogConfig};

// ═══════════════════════════════════════════════════════════════════════════════
// ERROR BUILDER MACRO AND FUNCTIONS
// ═══════════════════════════════════════════════════════════════════════════════

/// Macro to create errors with builder pattern
#[macro_export]
macro_rules! error {
    ($category:expr, $code:expr, $message:expr) => {
        $crate::core::diagnostics::NyxError::new($code, $message, $category)
    };
}

/// Create a compiler error
#[macro_export]
macro_rules! compiler_error {
    ($code:expr, $message:expr) => {
        $crate::core::diagnostics::NyxError::new(
            $code,
            $message,
            $crate::core::diagnostics::ErrorCategory::Compiler,
        )
    };
}

/// Create a syntax error
#[macro_export]
macro_rules! syntax_error {
    ($code:expr, $message:expr) => {
        $crate::core::diagnostics::NyxError::new(
            $code,
            $message,
            $crate::core::diagnostics::ErrorCategory::Syntax,
        )
    };
}

/// Create a type error
#[macro_export]
macro_rules! type_error {
    ($code:expr, $message:expr) => {
        $crate::core::diagnostics::NyxError::new(
            $code,
            $message,
            $crate::core::diagnostics::ErrorCategory::Type,
        )
    };
}

/// Create a runtime error
#[macro_export]
macro_rules! runtime_error {
    ($code:expr, $message:expr) => {
        $crate::core::diagnostics::NyxError::new(
            $code,
            $message,
            $crate::core::diagnostics::ErrorCategory::Runtime,
        )
    };
}

/// Create an IO error
#[macro_export]
macro_rules! io_error {
    ($code:expr, $message:expr) => {
        $crate::core::diagnostics::NyxError::new(
            $code,
            $message,
            $crate::core::diagnostics::ErrorCategory::Io,
        )
    };
}

/// Create a network error
#[macro_export]
macro_rules! network_error {
    ($code:expr, $message:expr) => {
        $crate::core::diagnostics::NyxError::new(
            $code,
            $message,
            $crate::core::diagnostics::ErrorCategory::Network,
        )
    };
}

/// Create a security error
#[macro_export]
macro_rules! security_error {
    ($code:expr, $message:expr) => {
        $crate::core::diagnostics::NyxError::new(
            $code,
            $message,
            $crate::core::diagnostics::ErrorCategory::Security,
        )
    };
}

/// Create an internal compiler error
#[macro_export]
macro_rules! internal_error {
    ($code:expr, $message:expr) => {
        $crate::core::diagnostics::NyxError::new(
            $code,
            $message,
            $crate::core::diagnostics::ErrorCategory::Internal,
        )
    };
}

// ═══════════════════════════════════════════════════════════════════════════════
// RESULT TYPE ALIAS
// ═══════════════════════════════════════════════════════════════════════════════

/// Result type alias for Nyx compiler operations
pub type Result<T, E = NyxError> = std::result::Result<T, E>;

// ═══════════════════════════════════════════════════════════════════════════════
// ERROR RECOVERY MECHANISMS
// ═══════════════════════════════════════════════════════════════════════════════

/// Recovery strategy for handling errors
#[derive(Debug, Clone)]
pub enum RecoveryStrategy {
    Skip,
    Default,
    Placeholder(String),
    Correct(String),
    Abort,
}

/// Error recovery result containing either a value or a recoverable error
#[derive(Debug)]
pub enum Recoverable<T> {
    Success(T),
    Recovered(T, NyxError),
    Unrecoverable(NyxError),
}

impl<T> Recoverable<T> {
    pub fn ok(self) -> Result<T> {
        match self {
            Recoverable::Success(v) => Ok(v),
            Recoverable::Recovered(v, _) => Ok(v),
            Recoverable::Unrecoverable(e) => Err(e),
        }
    }

    pub fn is_ok(&self) -> bool {
        !matches!(self, Recoverable::Unrecoverable(_))
    }

    pub fn is_err(&self) -> bool {
        matches!(self, Recoverable::Unrecoverable(_))
    }

    pub fn map<U, F>(self, f: F) -> Recoverable<U>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            Recoverable::Success(v) => Recoverable::Success(f(v)),
            Recoverable::Recovered(v, e) => Recoverable::Recovered(f(v), e),
            Recoverable::Unrecoverable(e) => Recoverable::Unrecoverable(e),
        }
    }

    pub fn unwrap_or(self, default: T) -> T {
        match self {
            Recoverable::Success(v) => v,
            Recoverable::Recovered(v, _) => v,
            Recoverable::Unrecoverable(_) => default,
        }
    }

    pub fn error(&self) -> Option<&NyxError> {
        match self {
            Recoverable::Success(_) => None,
            Recoverable::Recovered(_, e) => Some(e),
            Recoverable::Unrecoverable(e) => Some(e),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// BACKWARD COMPATIBILITY - DIAGNOSTIC AND DIAGNOSTICENGINE
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub code: &'static str,
    pub severity: Severity,
    pub message: String,
    pub span: Option<Span>,
    pub notes: Vec<String>,
    pub suggestions: Vec<String>,
}

impl Diagnostic {
    pub fn error(code: &'static str, msg: impl Into<String>) -> Self {
        Self {
            code,
            severity: Severity::Error,
            message: msg.into(),
            span: None,
            notes: vec![],
            suggestions: vec![],
        }
    }

    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    pub fn with_note(mut self, n: impl Into<String>) -> Self {
        self.notes.push(n.into());
        self
    }

    pub fn with_suggestion(mut self, s: impl Into<String>) -> Self {
        self.suggestions.push(s.into());
        self
    }

    pub fn into_nyx_error(self) -> NyxError {
        let category = match self.code.parse::<u32>() {
            Ok(n) => match n {
                1..=10 | 21..=30 => ErrorCategory::Compiler,
                11..=20 => ErrorCategory::Syntax,
                41..=50 => ErrorCategory::Type,
                51..=60 => ErrorCategory::Runtime,
                61..=70 => ErrorCategory::Io,
                71..=80 => ErrorCategory::Network,
                81..=90 => ErrorCategory::Security,
                91..=100 => ErrorCategory::Internal,
                _ => ErrorCategory::Compiler,
            },
            Err(_) => ErrorCategory::Compiler,
        };

        let mut error = NyxError::new(self.code.to_string(), self.message, category)
            .with_severity(self.severity)
            .with_notes(self.notes)
            .with_suggestions(self.suggestions);

        if let Some(span) = self.span {
            error = error.with_span_obj(&span);
        }

        error
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}[{}]: {}", self.severity, self.code, self.message)?;
        if let Some(span) = &self.span {
            write!(f, "\n  --> {}:{}", span.start.line, span.start.column)?;
        }
        for note in &self.notes {
            write!(f, "\n  note: {}", note)?;
        }
        for suggestion in &self.suggestions {
            write!(f, "\n  help: {}", suggestion)?;
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct DiagnosticEngine {
    pub diagnostics: Vec<Diagnostic>,
    pub nyx_errors: Vec<NyxError>,
}

impl DiagnosticEngine {
    pub fn emit(&mut self, d: Diagnostic) {
        self.diagnostics.push(d);
    }

    pub fn emit_error(&mut self, e: NyxError) {
        self.nyx_errors.push(e);
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == Severity::Error)
            || self.nyx_errors.iter().any(|e| e.severity == Severity::Error)
    }

    pub fn has_any_errors(&self) -> bool {
        self.has_errors() || !self.nyx_errors.is_empty()
    }

    pub fn error_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.severity == Severity::Error).count()
            + self.nyx_errors.iter().filter(|e| e.severity == Severity::Error).count()
    }

    pub fn warning_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.severity == Severity::Warning).count()
            + self.nyx_errors.iter().filter(|e| e.severity == Severity::Warning).count()
    }

    pub fn clear(&mut self) {
        self.diagnostics.clear();
        self.nyx_errors.clear();
    }

    pub fn into_nyx_errors(self) -> Vec<NyxError> {
        let mut errors = self.nyx_errors;
        for d in self.diagnostics {
            errors.push(d.into_nyx_error());
        }
        errors
    }

    pub fn print_all(&self) {
        for d in &self.diagnostics { eprintln!("{d}"); }
        for e in &self.nyx_errors { eprintln!("{e}"); }
    }

    pub fn print_all_pretty(&self) {
        self.print_all();
    }
}

pub fn pretty_print_error(e: &NyxError) -> String {
    format!("{e}")
}

pub fn log_error<T>(_res: &Result<T>, _cfg: &LogConfig) {
    if let Err(e) = _res {
        log_error_single(e, _cfg);
    }
}

pub fn log_errors(errors: &[NyxError], cfg: &LogConfig) {
    for e in errors {
        log_error_single(e, cfg);
    }
}

pub fn log_error_single(e: &NyxError, cfg: &LogConfig) {
    if cfg.level != LogLevel::Silent {
        eprintln!("{e}");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ERROR MITIGATION SYSTEM
// ═══════════════════════════════════════════════════════════════════════════════

pub trait Mitigation {
    fn apply<T>(&self, res: Result<T>) -> Result<T>;
    fn mitigate(&self, e: &NyxError) -> RecoveryStrategy;
    fn can_mitigate(&self, e: &NyxError) -> bool;
}

pub struct DefaultMitigation;

impl Mitigation for DefaultMitigation {
    fn apply<T>(&self, res: Result<T>) -> Result<T> {
        res
    }

    fn mitigate(&self, e: &NyxError) -> RecoveryStrategy {
        match e.category {
            ErrorCategory::Syntax | ErrorCategory::Compiler => RecoveryStrategy::Skip,
            ErrorCategory::Io | ErrorCategory::Network => RecoveryStrategy::Default,
            _ => RecoveryStrategy::Abort,
        }
    }

    fn can_mitigate(&self, e: &NyxError) -> bool {
        if e.recoverable {
            return true;
        }
        match e.category {
            ErrorCategory::Internal => false,
            _ => true,
        }
    }
}

pub fn apply_mitigation<T, M: Mitigation + ?Sized>(res: Result<T>, m: &M) -> Result<T> {
    m.apply(res)
}

#[cfg(test)]
mod diagnostics_tests;
