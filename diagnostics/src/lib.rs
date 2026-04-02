use serde::{Deserialize, Serialize};
use std::fmt;
use std::backtrace::Backtrace;
use std::io;
use std::path::PathBuf;

// ─── Source Position ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Position {
    pub line: usize,
    pub column: usize,
    pub offset: usize,
}

impl Position {
    pub fn new(line: usize, column: usize, offset: usize) -> Self {
        Self { line, column, offset }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}

impl Span {
    pub fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    pub fn merge(self, other: Span) -> Self {
        Span {
            start: self.start,
            end: other.end,
        }
    }

    pub fn to(self, end: Position) -> Span {
        Span {
            start: self.start,
            end,
        }
    }
}

// ─── Diagnostics ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
    Note,
    Help,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => write!(f, "\x1b[1;31merror\x1b[0m"),
            Severity::Warning => write!(f, "\x1b[1;33mwarning\x1b[0m"),
            Severity::Note => write!(f, "\x1b[1;36mnote\x1b[0m"),
            Severity::Help => write!(f, "\x1b[1;32mhelp\x1b[0m"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCategory {
    Compiler,
    Syntax,
    Type,
    Runtime,
    Io,
    Network,
    Security,
    Internal,
    Extension,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LogLevel {
    Silent,
    Error,
    Warning,
    Info,
    Debug,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    pub level: LogLevel,
    pub use_colors: bool,
    pub show_stack_traces: bool,
    pub show_context: bool,
    pub max_errors: usize,
    pub warnings_as_errors: bool,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Error,
            use_colors: true,
            show_stack_traces: false,
            show_context: true,
            max_errors: 50,
            warnings_as_errors: false,
        }
    }
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorCategory::Compiler => write!(f, "Compiler"),
            ErrorCategory::Syntax => write!(f, "Syntax"),
            ErrorCategory::Type => write!(f, "Type"),
            ErrorCategory::Runtime => write!(f, "Runtime"),
            ErrorCategory::Io => write!(f, "I/O"),
            ErrorCategory::Network => write!(f, "Network"),
            ErrorCategory::Security => write!(f, "Security"),
            ErrorCategory::Internal => write!(f, "Internal"),
            ErrorCategory::Extension => write!(f, "Extension"),
        }
    }
}

pub mod codes {
    pub const STDLIB_GENERIC: &str = "S000";
    pub const STDLIB_IO_ERROR: &str = "S001";
    pub const STDLIB_WEBSERVER_ERROR: &str = "S002";
    pub const STDLIB_FILESYSTEM_ERROR: &str = "S003";

    // E001-E010: General compiler errors (backward compatibility)
    pub const COMPILER_GENERIC: &str = "E001";
    pub const COMPILER_INIT_FAILED: &str = "E002";
    pub const COMPILER_MODULE_NOT_FOUND: &str = "E003";
    pub const UNEXPECTED_TOKEN: &str = "E001";
    pub const TYPE_MISMATCH: &str = "E002";
    pub const UNDEFINED_SYMBOL: &str = "E003";
    pub const BORROW_VIOLATION: &str = "E004";
    pub const DUPLICATE_BINDING: &str = "E005";
    pub const ARITY_MISMATCH: &str = "E006";
    pub const MISSING_RETURN: &str = "E007";
    pub const INVALID_LITERAL: &str = "E008";
    pub const INFINITE_LOOP: &str = "E009";
    pub const IR_ERROR: &str = "E010";

    // E011-E020: Lexer errors
    pub const LEXER_UNEXPECTED_CHAR: &str = "E011";
    pub const LEXER_UNTERMINATED_STRING: &str = "E012";
    pub const LEXER_INVALID_NUMBER: &str = "E013";
    pub const LEXER_INVALID_CHAR: &str = "E011";
    pub const LEXER_UNTERMINATED_COMMENT: &str = "E013";
    pub const LEXER_EOF_IN_STRING: &str = "E015";
    pub const LEXER_ILLEGAL_CHARACTER: &str = "E016";
    pub const LEXER_INVALID_ESCAPE: &str = "E017";
    pub const LEXER_MALFORMED_TOKEN: &str = "E018";
    pub const LEXER_INVALID_IDENTIFIER: &str = "E019";
    pub const LEXER_BUFFER_OVERFLOW: &str = "E020";

    // E021-E030: Parser errors
    pub const PARSER_UNEXPECTED_TOKEN: &str = "E021";
    pub const PARSER_MISSING_SEMICOLON: &str = "E022";
    pub const PARSER_INVALID_EXPRESSION: &str = "E023";
    pub const PARSER_SYNTAX_ERROR: &str = "E022";
    pub const PARSER_MISSING_TOKEN: &str = "E023";
    pub const PARSER_INVALID_STATEMENT: &str = "E025";
    pub const PARSER_UNEXPECTED_EOF: &str = "E026";
    pub const PARSER_RECOVERY_FAILED: &str = "E027";
    pub const PARSER_AMBIGUOUS_SYNTAX: &str = "E028";
    pub const PARSER_INVALID_ATTR: &str = "E029";
    pub const PARSER_RECOVERABLE_ERROR: &str = "E030";

    // E031-E040: Semantic errors
    pub const SEMANTIC_UNDEFINED_VARIABLE: &str = "E031";
    pub const SEMANTIC_REDECLARATION: &str = "E032";
    pub const SEMANTIC_SCOPE_ERROR: &str = "E033";
    pub const SEMANTIC_UNDEFINED_SYMBOL: &str = "E031";
    pub const SEMANTIC_REDEFINITION: &str = "E032";
    pub const SEMANTIC_WRONG_ARITY: &str = "E033";
    pub const SEMANTIC_IMMUTABLE_BINDING: &str = "E034";
    pub const SEMANTIC_SHADOWING: &str = "E035";
    pub const SEMANTIC_VISIBILITY_VIOLATION: &str = "E036";
    pub const SEMANTIC_CYCLIC_DEPENDENCY: &str = "E037";
    pub const SEMANTIC_DUPLICATE_IMPORT: &str = "E038";
    pub const SEMANTIC_MISSING_IMPORT: &str = "E039";
    pub const SEMANTIC_INVALID_PATH: &str = "E040";

    // E041-E050: Type errors
    pub const TYPE_MISMATCH_ERROR: &str = "E041";
    pub const TYPE_INCOMPATIBLE: &str = "E043";
    pub const TYPE_INVALID_CAST: &str = "E043";
    pub const TYPE_UNDEFINED_FIELD: &str = "E044";
    pub const TYPE_ARITY_MISMATCH: &str = "E045";
    pub const TYPE_AMBIGUOUS_METHOD: &str = "E046";
    pub const TYPE_PRIVATE_ACCESS: &str = "E047";
    pub const TYPE_TRAIT_NOT_IMPLEMENTED: &str = "E048";
    pub const TYPE_ASSOCIATED_TYPE_ERROR: &str = "E049";
    pub const TYPE_LIFETIME_ERROR: &str = "E050";
    pub const TYPE_INFERENCE_FAILED: &str = "E042";
    pub const TYPE_UNSUPPORTED_OP: &str = "E044";
    pub const TYPE_CAST_FAILED: &str = "E045";
    pub const TYPE_GENERIC_MISMATCH: &str = "E046";
    pub const TYPE_CONSTRAINT_FAILED: &str = "E047";

    // E051-E060: Runtime errors
    pub const RUNTIME_DIVISION_BY_ZERO: &str = "E051";
    pub const RUNTIME_OVERFLOW: &str = "E052";
    pub const RUNTIME_NULL_POINTER: &str = "E053";
    pub const RUNTIME_OUT_OF_BOUNDS: &str = "E054";
    pub const RUNTIME_STACK_OVERFLOW: &str = "E055";
    pub const RUNTIME_HEAP_OVERFLOW: &str = "E056";
    pub const RUNTIME_INVALID_MEMORY: &str = "E057";
    pub const RUNTIME_THREAD_PANIC: &str = "E058";
    pub const RUNTIME_UNWIND_FAILED: &str = "E059";
    pub const RUNTIME_CUSTOM_ERROR: &str = "E060";

    // E061-E070: IO errors
    pub const IO_FILE_NOT_FOUND: &str = "E061";
    pub const IO_PERMISSION_DENIED: &str = "E062";
    pub const IO_INVALID_PATH: &str = "E063";
    pub const IO_READ_ERROR: &str = "E064";
    pub const IO_WRITE_ERROR: &str = "E065";
    pub const IO_DIRECTORY_ERROR: &str = "E066";
    pub const IO_SYNC_ERROR: &str = "E067";
    pub const IO_EOF_ERROR: &str = "E068";
    pub const IO_INVALID_ENCODING: &str = "E069";
    pub const IO_RESOURCE_EXHAUSTED: &str = "E070";

    // E071-E080: Network errors
    pub const NET_CONNECTION_FAILED: &str = "E071";
    pub const NET_CONNECTION_REFUSED: &str = "E072";
    pub const NET_TIMEOUT: &str = "E073";
    pub const NET_DNS_ERROR: &str = "E074";
    pub const NET_SSL_ERROR: &str = "E075";
    pub const NET_INVALID_RESPONSE: &str = "E076";
    pub const NET_REQUEST_FAILED: &str = "E077";
    pub const NET_PROTOCOL_ERROR: &str = "E078";
    pub const NET_HOST_UNREACHABLE: &str = "E079";
    pub const NET_MAX_CONNECTIONS: &str = "E080";

    // E081-E090: Security errors
    pub const SECURITY_PERMISSION_DENIED: &str = "E081";
    pub const SECURITY_AUTH_FAILED: &str = "E082";
    pub const SECURITY_INVALID_TOKEN: &str = "E083";
    pub const SECURITY_CSRF_VIOLATION: &str = "E084";
    pub const SECURITY_SANITIZATION_FAILED: &str = "E085";
    pub const SECURITY_INJECTION_DETECTED: &str = "E086";
    pub const SECURITY_FORBIDDEN: &str = "E087";
    pub const SECURITY_UNAUTHORIZED: &str = "E088";
    pub const SECURITY_TAMPER_DETECTED: &str = "E089";
    pub const SECURITY_VALIDATION_FAILED: &str = "E090";

    // E091-E100: Internal errors
    pub const INTERNAL_COMPILER_BUG: &str = "E091";
    pub const INTERNAL_PANIC: &str = "E092";
    pub const INTERNAL_ASSERTION_FAILED: &str = "E093";
    pub const INTERNAL_CRASH: &str = "E094";
    pub const INTERNAL_UNREACHABLE: &str = "E095";
    pub const INTERNAL_TYPE_SYSTEM_BUG: &str = "E096";
    pub const INTERNAL_CODEGEN_ERROR: &str = "E097";
    pub const INTERNAL_OPTIMIZER_ERROR: &str = "E098";
    pub const INTERNAL_LINKER_ERROR: &str = "E099";
    pub const INTERNAL_UNKNOWN_ERROR: &str = "E100";

    // E101-E110: Version and stability errors
    pub const VERSION_INVALID_FORMAT: &str = "E101";
    pub const VERSION_PARSE_ERROR: &str = "E102";
    pub const API_STABILITY_ERROR: &str = "E103";
    pub const API_DEPRECATED: &str = "E104";
    pub const API_NOT_FOUND: &str = "E105";
    pub const PLUGIN_NOT_FOUND: &str = "E106";
}

#[derive(Debug, Default)]
pub struct ErrorDetails {
    pub file: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub end_line: Option<u32>,
    pub end_column: Option<u32>,
    pub stack_trace: Option<String>,
    pub notes: Vec<String>,
    pub suggestions: Vec<String>,
}

#[derive(Debug)]
pub struct NyxError {
    pub code: String,
    pub message: String,
    pub module: String,
    pub category: ErrorCategory,
    pub severity: Severity,
    pub recoverable: bool,
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
    pub details: Box<ErrorDetails>,
}

impl NyxError {
    pub fn new(code: impl Into<String>, message: impl Into<String>, category: ErrorCategory) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            module: String::new(),
            category,
            severity: Severity::Error,
            recoverable: false,
            source: None,
            details: Box::new(ErrorDetails::default()),
        }
    }

    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_module(mut self, module: impl Into<String>) -> Self {
        self.module = module.into();
        self
    }

    pub fn with_file(mut self, file: impl Into<String>) -> Self {
        self.details.file = Some(file.into());
        self
    }

    pub fn with_line(mut self, line: u32) -> Self {
        self.details.line = Some(line);
        self
    }

    pub fn with_column(mut self, column: u32) -> Self {
        self.details.column = Some(column);
        self
    }

    pub fn with_location(mut self, line: u32, column: u32) -> Self {
        self.details.line = Some(line);
        self.details.column = Some(column);
        self
    }

    pub fn with_span(mut self, start: (u32, u32), end: (u32, u32)) -> Self {
        self.details.line = Some(start.0);
        self.details.column = Some(start.1);
        self.details.end_line = Some(end.0);
        self.details.end_column = Some(end.1);
        self
    }

    pub fn with_span_obj(mut self, span: &Span) -> Self {
        self.details.line = Some(span.start.line as u32);
        self.details.column = Some(span.start.column as u32);
        self.details.end_line = Some(span.end.line as u32);
        self.details.end_column = Some(span.end.column as u32);
        self
    }

    pub fn with_stack_trace(mut self) -> Self {
        self.details.stack_trace = Some(Backtrace::capture().to_string());
        self
    }

    pub fn with_source<E: std::error::Error + Send + Sync + 'static>(mut self, source: E) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.details.notes.push(note.into());
        self
    }

    pub fn with_notes(mut self, notes: impl IntoIterator<Item = String>) -> Self {
        self.details.notes.extend(notes);
        self
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.details.suggestions.push(suggestion.into());
        self
    }

    pub fn with_suggestions(mut self, suggestions: impl IntoIterator<Item = String>) -> Self {
        self.details.suggestions.extend(suggestions);
        self
    }

    pub fn recoverable(mut self, recoverable: bool) -> Self {
        self.recoverable = recoverable;
        self
    }

    pub fn context<C: Into<String>>(mut self, context: C) -> Self {
        let ctx = context.into();
        if !ctx.is_empty() {
            self.message = format!("{}: {}", ctx, self.message);
        }
        self
    }

    pub fn code_category(&self) -> &str {
        if self.code.len() < 3 { return "Unknown"; }
        match &self.code[0..1] {
            "E" => {
                let num_part = &self.code[1..];
                let n = num_part.parse::<u32>().unwrap_or(0);
                match n {
                    1..=10 => "Compiler",
                    11..=20 => "Lexer",
                    21..=30 => "Parser",
                    31..=40 => "Semantic",
                    41..=50 => "Type",
                    51..=60 => "Runtime",
                    61..=70 => "IO",
                    71..=80 => "Network",
                    81..=90 => "Security",
                    91..=100 => "Internal",
                    _ => "Unknown",
                }
            }
            "S" => "STDLIB",
            _ => "Unknown",
        }
    }
}

impl fmt::Display for NyxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}[{}]: {}", self.severity, self.code, self.message)?;
        
        if !self.module.is_empty() {
            write!(f, " (module: {})", self.module)?;
        }

        if let Some(file) = &self.details.file {
            if let (Some(line), Some(column)) = (self.details.line, self.details.column) {
                write!(f, "\n  --> {}:{}:{}", file, line, column)?;
                if let (Some(end_line), Some(end_column)) = (self.details.end_line, self.details.end_column) {
                    if end_line != line || end_column != column {
                        write!(f, " to {}:{}", end_line, end_column)?;
                    }
                }
            } else {
                write!(f, "\n  --> {}", file)?;
            }
        }

        for note in &self.details.notes {
            write!(f, "\n  note: {}", note)?;
        }
        for suggestion in &self.details.suggestions {
            write!(f, "\n  help: {}", suggestion)?;
        }

        Ok(())
    }
}

impl std::error::Error for NyxError {}

impl From<io::Error> for NyxError {
    fn from(err: io::Error) -> Self {
        let code = match err.kind() {
            io::ErrorKind::NotFound => codes::IO_FILE_NOT_FOUND,
            io::ErrorKind::PermissionDenied => codes::IO_PERMISSION_DENIED,
            io::ErrorKind::InvalidInput => codes::IO_INVALID_PATH,
            io::ErrorKind::UnexpectedEof => codes::IO_EOF_ERROR,
            io::ErrorKind::ResourceBusy => codes::IO_RESOURCE_EXHAUSTED,
            _ => codes::IO_READ_ERROR,
        };
        NyxError::new(code, err.to_string(), ErrorCategory::Io)
            .with_module("io".to_string())
    }
}

impl From<PathBuf> for NyxError {
    fn from(path: PathBuf) -> Self {
        NyxError::new(
            codes::IO_INVALID_PATH,
            format!("Invalid path: {}", path.display()),
            ErrorCategory::Io,
        )
    }
}

impl From<String> for NyxError {
    fn from(s: String) -> Self {
        NyxError::new(codes::INTERNAL_UNKNOWN_ERROR, s, ErrorCategory::Internal)
    }
}

impl From<&str> for NyxError {
    fn from(s: &str) -> Self {
        NyxError::new(codes::INTERNAL_UNKNOWN_ERROR, s.to_string(), ErrorCategory::Internal)
    }
}
