// ═══════════════════════════════════════════════════════════════════════════════
// DIAGNOSTICS TEST SUITE
// Comprehensive tests for all types, macros, traits, and functions in
// diagnostics.rs
// ═══════════════════════════════════════════════════════════════════════════════

use super::*;
use crate::core::lexer::token::{Position, Span};

// ───────────────────────────────────────────────────────────────────────────────
// HELPERS
// ───────────────────────────────────────────────────────────────────────────────

fn make_span(sl: usize, sc: usize, el: usize, ec: usize) -> Span {
    Span::new(
        Position::new(sl, sc, 0),
        Position::new(el, ec, 0),
    )
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1. ErrorCategory — Display and creation
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_error_category_display_compiler() {
    assert_eq!(ErrorCategory::Compiler.to_string(), "Compiler");
}

#[test]
fn test_error_category_display_syntax() {
    assert_eq!(ErrorCategory::Syntax.to_string(), "Syntax");
}

#[test]
fn test_error_category_display_type() {
    assert_eq!(ErrorCategory::Type.to_string(), "Type");
}

#[test]
fn test_error_category_display_runtime() {
    assert_eq!(ErrorCategory::Runtime.to_string(), "Runtime");
}

#[test]
fn test_error_category_display_io() {
    assert_eq!(ErrorCategory::Io.to_string(), "I/O");
}

#[test]
fn test_error_category_display_network() {
    assert_eq!(ErrorCategory::Network.to_string(), "Network");
}

#[test]
fn test_error_category_display_security() {
    assert_eq!(ErrorCategory::Security.to_string(), "Security");
}

#[test]
fn test_error_category_display_internal() {
    assert_eq!(ErrorCategory::Internal.to_string(), "Internal");
}

#[test]
fn test_error_category_equality() {
    assert_eq!(ErrorCategory::Compiler, ErrorCategory::Compiler);
    assert_ne!(ErrorCategory::Syntax, ErrorCategory::Type);
}

#[test]
fn test_error_category_clone() {
    let cat = ErrorCategory::Runtime;
    let cloned = cat.clone();
    assert_eq!(cat, cloned);
}

#[test]
fn test_all_error_categories_distinct() {
    let cats = [
        ErrorCategory::Compiler,
        ErrorCategory::Syntax,
        ErrorCategory::Type,
        ErrorCategory::Runtime,
        ErrorCategory::Io,
        ErrorCategory::Network,
        ErrorCategory::Security,
        ErrorCategory::Internal,
    ];
    for (i, a) in cats.iter().enumerate() {
        for (j, b) in cats.iter().enumerate() {
            if i == j {
                assert_eq!(a, b);
            } else {
                assert_ne!(a, b);
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. Severity — Display
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_severity_display_contains_keyword() {
    // Display contains ANSI codes + keyword
    assert!(Severity::Error.to_string().contains("error"));
    assert!(Severity::Warning.to_string().contains("warning"));
    assert!(Severity::Note.to_string().contains("note"));
    assert!(Severity::Help.to_string().contains("help"));
}

#[test]
fn test_severity_equality() {
    assert_eq!(Severity::Error, Severity::Error);
    assert_ne!(Severity::Error, Severity::Warning);
}

#[test]
fn test_severity_clone() {
    let s = Severity::Note;
    assert_eq!(s.clone(), Severity::Note);
}

#[test]
fn test_severity_debug() {
    let dbg = format!("{:?}", Severity::Help);
    assert_eq!(dbg, "Help");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. NyxError — creation and builder methods
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_nyx_error_new_defaults() {
    let e = NyxError::new(codes::UNEXPECTED_TOKEN, "msg", ErrorCategory::Compiler);
    assert_eq!(e.code, codes::UNEXPECTED_TOKEN);
    assert_eq!(e.message, "msg");
    assert_eq!(e.category, ErrorCategory::Compiler);
    assert_eq!(e.severity, Severity::Error);
    assert!(e.module.is_empty());
    assert!(e.details.file.is_none());
    assert!(e.details.line.is_none());
    assert!(e.details.column.is_none());
    assert!(e.details.end_line.is_none());
    assert!(e.details.end_column.is_none());
    assert!(e.details.notes.is_empty());
    assert!(e.details.suggestions.is_empty());
    assert!(!e.recoverable);
}

#[test]
fn test_nyx_error_with_module() {
    let e = NyxError::new(codes::UNDEFINED_SYMBOL, "undefined", ErrorCategory::Compiler)
        .with_module("parser");
    assert_eq!(e.module, "parser");
}

#[test]
fn test_nyx_error_with_file() {
    let e = NyxError::new(codes::TYPE_MISMATCH, "mismatch", ErrorCategory::Type)
        .with_file("main.nyx");
    assert_eq!(e.details.file.as_deref(), Some("main.nyx"));
}

#[test]
fn test_nyx_error_with_line() {
    let e = NyxError::new(codes::MISSING_RETURN, "missing return", ErrorCategory::Compiler)
        .with_line(42);
    assert_eq!(e.details.line, Some(42));
}

#[test]
fn test_nyx_error_with_column() {
    let e = NyxError::new(codes::DUPLICATE_BINDING, "dup", ErrorCategory::Compiler)
        .with_column(10);
    assert_eq!(e.details.column, Some(10));
}

#[test]
fn test_nyx_error_with_location() {
    let e = NyxError::new(codes::ARITY_MISMATCH, "arity", ErrorCategory::Compiler)
        .with_location(5, 15);
    assert_eq!(e.details.line, Some(5));
    assert_eq!(e.details.column, Some(15));
}

#[test]
fn test_nyx_error_with_span() {
    let e = NyxError::new(codes::INVALID_LITERAL, "lit", ErrorCategory::Compiler)
        .with_span((1, 2), (3, 4));
    assert_eq!(e.details.line, Some(1));
    assert_eq!(e.details.column, Some(2));
    assert_eq!(e.details.end_line, Some(3));
    assert_eq!(e.details.end_column, Some(4));
}

#[test]
fn test_nyx_error_with_span_obj() {
    let span = make_span(10, 5, 10, 15);
    let e = NyxError::new(codes::IR_ERROR, "ir", ErrorCategory::Compiler)
        .with_span_obj(&span);
    assert_eq!(e.details.line, Some(10));
    assert_eq!(e.details.column, Some(5));
    assert_eq!(e.details.end_line, Some(10));
    assert_eq!(e.details.end_column, Some(15));
}

#[test]
fn test_nyx_error_with_note() {
    let e = NyxError::new(codes::BORROW_VIOLATION, "borrow", ErrorCategory::Compiler)
        .with_note("note 1")
        .with_note("note 2");
    assert_eq!(e.details.notes.len(), 2);
    assert_eq!(e.details.notes[0], "note 1");
    assert_eq!(e.details.notes[1], "note 2");
}

#[test]
fn test_nyx_error_with_notes() {
    let notes = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    let e = NyxError::new(codes::BORROW_VIOLATION, "msg", ErrorCategory::Compiler)
        .with_notes(notes);
    assert_eq!(e.details.notes.len(), 3);
}

#[test]
fn test_nyx_error_with_suggestion() {
    let e = NyxError::new(codes::UNDEFINED_SYMBOL, "undef", ErrorCategory::Compiler)
        .with_suggestion("Did you mean 'foo'?");
    assert_eq!(e.details.suggestions.len(), 1);
    assert_eq!(e.details.suggestions[0], "Did you mean 'foo'?");
}

#[test]
fn test_nyx_error_with_suggestions() {
    let sugs = vec!["fix 1".to_string(), "fix 2".to_string()];
    let e = NyxError::new(codes::UNDEFINED_SYMBOL, "undef", ErrorCategory::Compiler)
        .with_suggestions(sugs);
    assert_eq!(e.details.suggestions.len(), 2);
}

#[test]
fn test_nyx_error_recoverable() {
    let e = NyxError::new(codes::PARSER_RECOVERABLE_ERROR, "rec", ErrorCategory::Syntax)
        .recoverable(true);
    assert!(e.recoverable);
    let e2 = e.recoverable(false);
    assert!(!e2.recoverable);
}

#[test]
fn test_nyx_error_with_severity() {
    let e = NyxError::new(codes::SEMANTIC_SHADOWING, "shadow", ErrorCategory::Compiler)
        .with_severity(Severity::Warning);
    assert_eq!(e.severity, Severity::Warning);
}

#[test]
fn test_nyx_error_context_prepends_message() {
    let e = NyxError::new(codes::IR_ERROR, "bad IR", ErrorCategory::Internal)
        .context("codegen");
    assert!(e.message.starts_with("codegen:"));
    assert!(e.message.contains("bad IR"));
}

#[test]
fn test_nyx_error_context_empty_does_not_change_message() {
    let e = NyxError::new(codes::IR_ERROR, "bad IR", ErrorCategory::Internal)
        .context("");
    assert_eq!(e.message, "bad IR");
}

#[test]
fn test_nyx_error_code_category_ranges() {
    let cases = [
        (codes::UNEXPECTED_TOKEN, "Compiler"),  // E001
        (codes::LEXER_INVALID_CHAR, "Lexer"),   // E011
        (codes::PARSER_SYNTAX_ERROR, "Parser"), // E022
        (codes::SEMANTIC_REDEFINITION, "Semantic"), // E032
        (codes::TYPE_MISMATCH_ERROR, "Type"),   // E041
        (codes::RUNTIME_DIVISION_BY_ZERO, "Runtime"), // E051
        (codes::IO_FILE_NOT_FOUND, "IO"),       // E061
        (codes::NET_CONNECTION_FAILED, "Network"), // E071
        (codes::SECURITY_AUTH_FAILED, "Security"), // E082
        (codes::INTERNAL_COMPILER_BUG, "Internal"), // E091
    ];
    for (code, expected) in cases {
        let e = NyxError::new(code, "test", ErrorCategory::Compiler);
        assert_eq!(e.code_category(), expected, "code={}", code);
    }
}

#[test]
fn test_nyx_error_code_category_unknown() {
    // E200 is out of range
    let e = NyxError::new("E200", "test", ErrorCategory::Compiler);
    assert_eq!(e.code_category(), "Unknown");
}

#[test]
fn test_nyx_error_display_basic() {
    let e = NyxError::new(codes::UNEXPECTED_TOKEN, "Unexpected '}'", ErrorCategory::Syntax);
    let s = e.to_string();
    assert!(s.contains("E001"));
    assert!(s.contains("Unexpected '}'"));
}

#[test]
fn test_nyx_error_display_with_file_and_location() {
    let e = NyxError::new(codes::UNEXPECTED_TOKEN, "tok", ErrorCategory::Syntax)
        .with_file("test.nyx")
        .with_location(10, 5);
    let s = e.to_string();
    assert!(s.contains("test.nyx"));
    assert!(s.contains("10"));
    assert!(s.contains("5"));
}

#[test]
fn test_nyx_error_display_with_span_range() {
    let e = NyxError::new(codes::UNEXPECTED_TOKEN, "tok", ErrorCategory::Syntax)
        .with_file("a.nyx")
        .with_span((1, 1), (5, 10));
    let s = e.to_string();
    // When start != end the output should show both
    assert!(s.contains("5")); // end_line
    assert!(s.contains("10")); // end_col
}

#[test]
fn test_nyx_error_display_with_module() {
    let e = NyxError::new(codes::INTERNAL_COMPILER_BUG, "bug", ErrorCategory::Internal)
        .with_module("codegen");
    let s = e.to_string();
    assert!(s.contains("codegen"));
}

#[test]
fn test_nyx_error_display_with_notes_and_suggestions() {
    let e = NyxError::new(codes::TYPE_MISMATCH, "mismatch", ErrorCategory::Type)
        .with_note("expected i32")
        .with_suggestion("cast to i32");
    let s = e.to_string();
    assert!(s.contains("note: expected i32"));
    assert!(s.contains("help: cast to i32"));
}

#[test]
fn test_nyx_error_implements_std_error() {
    let e = NyxError::new(codes::INTERNAL_PANIC, "panic", ErrorCategory::Internal);
    let _: &dyn std::error::Error = &e;
}

#[test]
fn test_nyx_error_source_chain() {
    use std::io;
    let io_err = io::Error::new(io::ErrorKind::NotFound, "file missing");
    let e = NyxError::new(codes::IO_FILE_NOT_FOUND, "outer", ErrorCategory::Io)
        .with_source(io_err);
    // Access the struct field directly (avoids trait method ambiguity)
    assert!(e.source.is_some());
}

#[test]
fn test_nyx_error_with_stack_trace() {
    let e = NyxError::new(codes::INTERNAL_COMPILER_BUG, "bug", ErrorCategory::Internal)
        .with_stack_trace();
    // stack_trace is Some (even if empty in release mode)
    assert!(e.details.stack_trace.is_some());
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Error codes module
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_error_codes_format() {
    // Every code should start with 'E'
    let all = [
        codes::UNEXPECTED_TOKEN, codes::TYPE_MISMATCH, codes::UNDEFINED_SYMBOL,
        codes::BORROW_VIOLATION, codes::DUPLICATE_BINDING, codes::ARITY_MISMATCH,
        codes::MISSING_RETURN, codes::INVALID_LITERAL, codes::INFINITE_LOOP,
        codes::IR_ERROR, codes::LEXER_INVALID_CHAR, codes::LEXER_UNTERMINATED_STRING,
        codes::PARSER_UNEXPECTED_TOKEN, codes::PARSER_SYNTAX_ERROR,
        codes::SEMANTIC_UNDEFINED_SYMBOL, codes::TYPE_MISMATCH_ERROR,
        codes::RUNTIME_DIVISION_BY_ZERO, codes::IO_FILE_NOT_FOUND,
        codes::NET_CONNECTION_FAILED, codes::SECURITY_PERMISSION_DENIED,
        codes::INTERNAL_COMPILER_BUG,
    ];
    for code in all {
        assert!(code.starts_with('E'), "Code {:?} does not start with 'E'", code);
        let num = &code[1..];
        assert!(num.parse::<u32>().is_ok(), "Code {:?} suffix is not numeric", code);
    }
}

#[test]
fn test_error_codes_values() {
    assert_eq!(codes::UNEXPECTED_TOKEN, "E001");
    assert_eq!(codes::TYPE_MISMATCH, "E002");
    assert_eq!(codes::UNDEFINED_SYMBOL, "E003");
    assert_eq!(codes::LEXER_INVALID_CHAR, "E011");
    assert_eq!(codes::PARSER_UNEXPECTED_TOKEN, "E021");
    assert_eq!(codes::SEMANTIC_UNDEFINED_SYMBOL, "E031");
    assert_eq!(codes::TYPE_MISMATCH_ERROR, "E041");
    assert_eq!(codes::RUNTIME_DIVISION_BY_ZERO, "E051");
    assert_eq!(codes::IO_FILE_NOT_FOUND, "E061");
    assert_eq!(codes::NET_CONNECTION_FAILED, "E071");
    assert_eq!(codes::SECURITY_PERMISSION_DENIED, "E081");
    assert_eq!(codes::INTERNAL_COMPILER_BUG, "E091");
    assert_eq!(codes::VERSION_INVALID_FORMAT, "E101");
    assert_eq!(codes::API_NOT_FOUND, "E105");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. Error macros
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_macro_error_generic() {
    let e = error!(ErrorCategory::Compiler, codes::UNEXPECTED_TOKEN, "bad token");
    assert_eq!(e.category, ErrorCategory::Compiler);
    assert_eq!(e.code, codes::UNEXPECTED_TOKEN);
    assert_eq!(e.message, "bad token");
}

#[test]
fn test_macro_compiler_error() {
    let e = compiler_error!(codes::UNEXPECTED_TOKEN, "compiler fail");
    assert_eq!(e.category, ErrorCategory::Compiler);
    assert_eq!(e.message, "compiler fail");
}

#[test]
fn test_macro_syntax_error() {
    let e = syntax_error!(codes::PARSER_SYNTAX_ERROR, "syntax fail");
    assert_eq!(e.category, ErrorCategory::Syntax);
    assert_eq!(e.code, codes::PARSER_SYNTAX_ERROR);
}

#[test]
fn test_macro_type_error() {
    let e = type_error!(codes::TYPE_MISMATCH_ERROR, "type fail");
    assert_eq!(e.category, ErrorCategory::Type);
    assert_eq!(e.code, codes::TYPE_MISMATCH_ERROR);
}

#[test]
fn test_macro_runtime_error() {
    let e = runtime_error!(codes::RUNTIME_DIVISION_BY_ZERO, "div by zero");
    assert_eq!(e.category, ErrorCategory::Runtime);
    assert_eq!(e.code, codes::RUNTIME_DIVISION_BY_ZERO);
}

#[test]
fn test_macro_io_error() {
    let e = io_error!(codes::IO_FILE_NOT_FOUND, "not found");
    assert_eq!(e.category, ErrorCategory::Io);
    assert_eq!(e.code, codes::IO_FILE_NOT_FOUND);
}

#[test]
fn test_macro_network_error() {
    let e = network_error!(codes::NET_CONNECTION_FAILED, "conn failed");
    assert_eq!(e.category, ErrorCategory::Network);
    assert_eq!(e.code, codes::NET_CONNECTION_FAILED);
}

#[test]
fn test_macro_security_error() {
    let e = security_error!(codes::SECURITY_AUTH_FAILED, "auth failed");
    assert_eq!(e.category, ErrorCategory::Security);
    assert_eq!(e.code, codes::SECURITY_AUTH_FAILED);
}

#[test]
fn test_macro_internal_error() {
    let e = internal_error!(codes::INTERNAL_COMPILER_BUG, "bug");
    assert_eq!(e.category, ErrorCategory::Internal);
    assert_eq!(e.code, codes::INTERNAL_COMPILER_BUG);
}

#[test]
fn test_macros_return_nyx_error_with_builder() {
    // Macros should return NyxError so builder methods can chain
    let e = syntax_error!(codes::PARSER_SYNTAX_ERROR, "err")
        .with_file("foo.nyx")
        .with_line(1)
        .with_note("fix this");
    assert_eq!(e.details.file.as_deref(), Some("foo.nyx"));
    assert_eq!(e.details.line, Some(1));
    assert_eq!(e.details.notes[0], "fix this");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6. From trait implementations
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_from_io_error_not_found() {
    use std::io;
    let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
    let nyx_err: NyxError = io_err.into();
    assert_eq!(nyx_err.code, codes::IO_FILE_NOT_FOUND);
    assert_eq!(nyx_err.category, ErrorCategory::Io);
}

#[test]
fn test_from_io_error_permission_denied() {
    use std::io;
    let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "denied");
    let nyx_err: NyxError = io_err.into();
    assert_eq!(nyx_err.code, codes::IO_PERMISSION_DENIED);
}

#[test]
fn test_from_io_error_invalid_input() {
    use std::io;
    let io_err = io::Error::new(io::ErrorKind::InvalidInput, "bad input");
    let nyx_err: NyxError = io_err.into();
    assert_eq!(nyx_err.code, codes::IO_INVALID_PATH);
}

#[test]
fn test_from_io_error_unexpected_eof() {
    use std::io;
    let io_err = io::Error::new(io::ErrorKind::UnexpectedEof, "eof");
    let nyx_err: NyxError = io_err.into();
    assert_eq!(nyx_err.code, codes::IO_EOF_ERROR);
}

#[test]
fn test_from_io_error_unknown_falls_back_to_read_error() {
    use std::io;
    let io_err = io::Error::new(io::ErrorKind::Other, "other");
    let nyx_err: NyxError = io_err.into();
    assert_eq!(nyx_err.code, codes::IO_READ_ERROR);
    assert_eq!(nyx_err.category, ErrorCategory::Io);
}

#[test]
fn test_from_pathbuf() {
    use std::path::PathBuf;
    let path = PathBuf::from("/bad/path/to/nowhere");
    let nyx_err: NyxError = path.into();
    assert_eq!(nyx_err.code, codes::IO_INVALID_PATH);
    assert!(nyx_err.message.contains("Invalid path"));
    assert_eq!(nyx_err.category, ErrorCategory::Io);
}

#[test]
fn test_from_string() {
    let s = String::from("something went wrong");
    let nyx_err: NyxError = s.into();
    assert_eq!(nyx_err.code, codes::INTERNAL_UNKNOWN_ERROR);
    assert_eq!(nyx_err.category, ErrorCategory::Internal);
    assert_eq!(nyx_err.message, "something went wrong");
}

#[test]
fn test_from_str() {
    let nyx_err: NyxError = "quick error".into();
    assert_eq!(nyx_err.code, codes::INTERNAL_UNKNOWN_ERROR);
    assert_eq!(nyx_err.message, "quick error");
}

#[test]
fn test_from_io_sets_module() {
    use std::io;
    let io_err = io::Error::new(io::ErrorKind::NotFound, "x");
    let e: NyxError = io_err.into();
    assert_eq!(e.module, "io");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 7. Recoverable<T> enum and methods
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_recoverable_success_ok() {
    let r: Recoverable<i32> = Recoverable::Success(42);
    assert!(r.is_ok());
    assert!(!r.is_err());
    assert_eq!(r.ok().unwrap(), 42);
}

#[test]
fn test_recoverable_recovered_is_ok() {
    let err = NyxError::new(codes::PARSER_RECOVERABLE_ERROR, "rec", ErrorCategory::Syntax);
    let r: Recoverable<i32> = Recoverable::Recovered(99, err);
    assert!(r.is_ok());
    assert!(!r.is_err());
    assert_eq!(r.ok().unwrap(), 99);
}

#[test]
fn test_recoverable_unrecoverable_is_err() {
    let err = NyxError::new(codes::INTERNAL_COMPILER_BUG, "fatal", ErrorCategory::Internal);
    let r: Recoverable<i32> = Recoverable::Unrecoverable(err);
    assert!(!r.is_ok());
    assert!(r.is_err());
    assert!(r.ok().is_err());
}

#[test]
fn test_recoverable_map_success() {
    let r: Recoverable<i32> = Recoverable::Success(5);
    let mapped = r.map(|v| v * 2);
    assert_eq!(mapped.ok().unwrap(), 10);
}

#[test]
fn test_recoverable_map_recovered() {
    let err = NyxError::new(codes::PARSER_RECOVERABLE_ERROR, "r", ErrorCategory::Syntax);
    let r: Recoverable<i32> = Recoverable::Recovered(5, err);
    let mapped = r.map(|v| v + 1);
    assert_eq!(mapped.ok().unwrap(), 6);
}

#[test]
fn test_recoverable_map_unrecoverable_preserves_error() {
    let err = NyxError::new(codes::INTERNAL_PANIC, "p", ErrorCategory::Internal);
    let r: Recoverable<i32> = Recoverable::Unrecoverable(err);
    let mapped = r.map(|v| v + 1);
    assert!(mapped.is_err());
}

#[test]
fn test_recoverable_unwrap_or_success() {
    let r: Recoverable<i32> = Recoverable::Success(7);
    assert_eq!(r.unwrap_or(0), 7);
}

#[test]
fn test_recoverable_unwrap_or_recovered() {
    let err = NyxError::new(codes::PARSER_RECOVERABLE_ERROR, "r", ErrorCategory::Syntax);
    let r: Recoverable<i32> = Recoverable::Recovered(7, err);
    assert_eq!(r.unwrap_or(0), 7);
}

#[test]
fn test_recoverable_unwrap_or_unrecoverable_returns_default() {
    let err = NyxError::new(codes::INTERNAL_PANIC, "p", ErrorCategory::Internal);
    let r: Recoverable<i32> = Recoverable::Unrecoverable(err);
    assert_eq!(r.unwrap_or(-1), -1);
}

#[test]
fn test_recoverable_error_none_on_success() {
    let r: Recoverable<i32> = Recoverable::Success(1);
    assert!(r.error().is_none());
}

#[test]
fn test_recoverable_error_some_on_recovered() {
    let err = NyxError::new(codes::PARSER_RECOVERABLE_ERROR, "r", ErrorCategory::Syntax);
    let r: Recoverable<i32> = Recoverable::Recovered(1, err);
    assert!(r.error().is_some());
    assert_eq!(r.error().unwrap().code, codes::PARSER_RECOVERABLE_ERROR);
}

#[test]
fn test_recoverable_error_some_on_unrecoverable() {
    let err = NyxError::new(codes::INTERNAL_COMPILER_BUG, "bug", ErrorCategory::Internal);
    let r: Recoverable<i32> = Recoverable::Unrecoverable(err);
    assert!(r.error().is_some());
}

// ═══════════════════════════════════════════════════════════════════════════════
// 8. RecoveryStrategy enum
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_recovery_strategy_skip_clone_debug() {
    let s = RecoveryStrategy::Skip;
    let c = s.clone();
    assert!(format!("{:?}", c).contains("Skip"));
}

#[test]
fn test_recovery_strategy_default_clone_debug() {
    let s = RecoveryStrategy::Default;
    let c = s.clone();
    assert!(format!("{:?}", c).contains("Default"));
}

#[test]
fn test_recovery_strategy_placeholder_contains_value() {
    let s = RecoveryStrategy::Placeholder("0".to_string());
    let dbg = format!("{:?}", s);
    assert!(dbg.contains("Placeholder"));
    assert!(dbg.contains("0"));
}

#[test]
fn test_recovery_strategy_correct_contains_value() {
    let s = RecoveryStrategy::Correct("fix".to_string());
    let dbg = format!("{:?}", s);
    assert!(dbg.contains("Correct"));
}

#[test]
fn test_recovery_strategy_abort_clone_debug() {
    let s = RecoveryStrategy::Abort;
    let c = s.clone();
    assert!(format!("{:?}", c).contains("Abort"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// 9. LogConfig and LogLevel
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_log_config_default() {
    let cfg = LogConfig::default();
    assert_eq!(cfg.level, LogLevel::Error);
    assert!(cfg.use_colors);
    assert!(!cfg.show_stack_traces);
    assert!(cfg.show_context);
    assert_eq!(cfg.max_errors, 50);
    assert!(!cfg.warnings_as_errors);
}

#[test]
fn test_log_level_ordering() {
    assert!(LogLevel::Silent < LogLevel::Error);
    assert!(LogLevel::Error < LogLevel::Warning);
    assert!(LogLevel::Warning < LogLevel::Info);
    assert!(LogLevel::Info < LogLevel::Debug);
}

#[test]
fn test_log_level_equality() {
    assert_eq!(LogLevel::Error, LogLevel::Error);
    assert_ne!(LogLevel::Error, LogLevel::Warning);
}

#[test]
fn test_log_level_copy() {
    let lv = LogLevel::Debug;
    let lv2 = lv; // Copy
    assert_eq!(lv, lv2);
}

#[test]
fn test_log_config_clone() {
    let cfg = LogConfig {
        level: LogLevel::Debug,
        use_colors: false,
        show_stack_traces: true,
        show_context: false,
        max_errors: 100,
        warnings_as_errors: true,
    };
    let c2 = cfg.clone();
    assert_eq!(c2.level, LogLevel::Debug);
    assert!(!c2.use_colors);
    assert!(c2.show_stack_traces);
    assert_eq!(c2.max_errors, 100);
    assert!(c2.warnings_as_errors);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 10. Logging functions (log_error, log_errors, pretty_print_error)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_log_error_on_ok_does_not_panic() {
    let result: Result<()> = Ok(());
    let cfg = LogConfig::default();
    log_error(&result, &cfg); // should not panic
}

#[test]
fn test_log_error_on_err_does_not_panic() {
    let e = NyxError::new(codes::UNEXPECTED_TOKEN, "tok", ErrorCategory::Compiler);
    let result: Result<()> = Err(e);
    let cfg = LogConfig::default();
    log_error(&result, &cfg); // should not panic (prints to stderr)
}

#[test]
fn test_log_error_silent_does_not_panic() {
    let e = NyxError::new(codes::UNEXPECTED_TOKEN, "tok", ErrorCategory::Compiler);
    let cfg = LogConfig { level: LogLevel::Silent, ..LogConfig::default() };
    log_error_single(&e, &cfg); // should not panic
}

#[test]
fn test_log_errors_empty_does_not_panic() {
    let errors: Vec<NyxError> = vec![];
    let cfg = LogConfig::default();
    log_errors(&errors, &cfg); // no-op, should not panic
}

#[test]
fn test_log_errors_multiple_does_not_panic() {
    let errors = vec![
        NyxError::new(codes::UNEXPECTED_TOKEN, "e1", ErrorCategory::Compiler),
        NyxError::new(codes::TYPE_MISMATCH, "e2", ErrorCategory::Type)
            .with_severity(Severity::Warning),
        NyxError::new(codes::UNDEFINED_SYMBOL, "e3", ErrorCategory::Compiler),
    ];
    let cfg = LogConfig { level: LogLevel::Warning, ..LogConfig::default() };
    log_errors(&errors, &cfg);
}

#[test]
fn test_pretty_print_error_contains_code_and_message() {
    let e = NyxError::new(codes::UNEXPECTED_TOKEN, "unexpected }", ErrorCategory::Syntax);
    let s = pretty_print_error(&e);
    assert!(s.contains("E001"));
    assert!(s.contains("unexpected }"));
}

#[test]
fn test_pretty_print_error_contains_file_location() {
    let e = NyxError::new(codes::PARSER_SYNTAX_ERROR, "bad", ErrorCategory::Syntax)
        .with_file("src/main.nyx")
        .with_line(20)
        .with_column(3);
    let s = pretty_print_error(&e);
    assert!(s.contains("src/main.nyx"));
    assert!(s.contains("20"));
    assert!(s.contains("3"));
}

#[test]
fn test_pretty_print_error_contains_notes_and_suggestions() {
    let e = NyxError::new(codes::TYPE_MISMATCH_ERROR, "mismatch", ErrorCategory::Type)
        .with_note("expected i32, found str")
        .with_suggestion("wrap in to_string()");
    let s = pretty_print_error(&e);
    assert!(s.contains("expected i32, found str"));
    assert!(s.contains("wrap in to_string()"));
}

#[test]
fn test_pretty_print_error_warning_severity() {
    let e = NyxError::new(codes::SEMANTIC_SHADOWING, "shadow", ErrorCategory::Compiler)
        .with_severity(Severity::Warning);
    let s = pretty_print_error(&e);
    assert!(s.contains("warning"));
}

#[test]
fn test_pretty_print_error_contains_module() {
    let e = NyxError::new(codes::INTERNAL_COMPILER_BUG, "bug", ErrorCategory::Internal)
        .with_module("optimizer");
    let s = pretty_print_error(&e);
    assert!(s.contains("optimizer"));
}

#[test]
fn test_pretty_print_error_multiline_span() {
    let e = NyxError::new(codes::PARSER_SYNTAX_ERROR, "span test", ErrorCategory::Syntax)
        .with_file("x.nyx")
        .with_span((1, 1), (5, 10));
    let s = pretty_print_error(&e);
    // For multiline span (end_line != line), the format should include both
    assert!(s.contains("5"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// 11. Diagnostic struct and conversion
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_diagnostic_error_creation() {
    let d = Diagnostic::error(codes::UNEXPECTED_TOKEN, "unexpected }");
    assert_eq!(d.code, codes::UNEXPECTED_TOKEN);
    assert_eq!(d.severity, Severity::Error);
    assert_eq!(d.message, "unexpected }");
    assert!(d.span.is_none());
    assert!(d.notes.is_empty());
    assert!(d.suggestions.is_empty());
}

#[test]
fn test_diagnostic_with_span() {
    let span = make_span(5, 2, 5, 8);
    let d = Diagnostic::error(codes::TYPE_MISMATCH, "mismatch")
        .with_span(span);
    assert!(d.span.is_some());
    let s = d.span.unwrap();
    assert_eq!(s.start.line, 5);
    assert_eq!(s.start.column, 2);
}

#[test]
fn test_diagnostic_with_note() {
    let d = Diagnostic::error(codes::UNDEFINED_SYMBOL, "undef")
        .with_note("check imports");
    assert_eq!(d.notes.len(), 1);
    assert_eq!(d.notes[0], "check imports");
}

#[test]
fn test_diagnostic_with_suggestion() {
    let d = Diagnostic::error(codes::UNDEFINED_SYMBOL, "undef")
        .with_suggestion("add `use foo;`");
    assert_eq!(d.suggestions.len(), 1);
    assert_eq!(d.suggestions[0], "add `use foo;`");
}

#[test]
fn test_diagnostic_display_basic() {
    let d = Diagnostic::error(codes::UNEXPECTED_TOKEN, "expected ;");
    let s = d.to_string();
    assert!(s.contains("E001"));
    assert!(s.contains("expected ;"));
}

#[test]
fn test_diagnostic_display_with_span() {
    let span = make_span(10, 4, 10, 8);
    let d = Diagnostic::error(codes::PARSER_SYNTAX_ERROR, "err").with_span(span);
    let s = d.to_string();
    assert!(s.contains("10")); // line
    assert!(s.contains("4")); // column
}

#[test]
fn test_diagnostic_display_with_notes_and_suggestions() {
    let d = Diagnostic::error(codes::TYPE_MISMATCH, "mm")
        .with_note("type info")
        .with_suggestion("fix it");
    let s = d.to_string();
    assert!(s.contains("note: type info"));
    assert!(s.contains("help: fix it"));
}

#[test]
fn test_diagnostic_into_nyx_error_preserves_message() {
    let d = Diagnostic::error(codes::TYPE_MISMATCH_ERROR, "type error")
        .with_note("a note")
        .with_suggestion("a fix");
    let e = d.into_nyx_error();
    assert_eq!(e.message, "type error");
    assert_eq!(e.details.notes.len(), 1);
    assert_eq!(e.details.suggestions.len(), 1);
    assert_eq!(e.severity, Severity::Error);
}

#[test]
fn test_diagnostic_into_nyx_error_with_span() {
    let span = make_span(3, 1, 3, 9);
    let d = Diagnostic::error(codes::PARSER_MISSING_TOKEN, "missing").with_span(span);
    let e = d.into_nyx_error();
    assert_eq!(e.details.line, Some(3));
    assert_eq!(e.details.column, Some(1));
    assert_eq!(e.details.end_line, Some(3));
    assert_eq!(e.details.end_column, Some(9));
}

#[test]
fn test_diagnostic_clone() {
    let d = Diagnostic::error(codes::UNEXPECTED_TOKEN, "tok");
    let d2 = d.clone();
    assert_eq!(d2.code, d.code);
    assert_eq!(d2.message, d.message);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 12. DiagnosticEngine
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_diagnostic_engine_default_is_empty() {
    let eng = DiagnosticEngine::default();
    assert!(!eng.has_errors());
    assert_eq!(eng.error_count(), 0);
    assert_eq!(eng.warning_count(), 0);
}

#[test]
fn test_diagnostic_engine_emit_error_sets_has_errors() {
    let mut eng = DiagnosticEngine::default();
    eng.emit(Diagnostic::error(codes::UNEXPECTED_TOKEN, "tok"));
    assert!(eng.has_errors());
    assert_eq!(eng.error_count(), 1);
}

#[test]
fn test_diagnostic_engine_emit_nyx_error() {
    let mut eng = DiagnosticEngine::default();
    let e = NyxError::new(codes::INTERNAL_COMPILER_BUG, "bug", ErrorCategory::Internal);
    eng.emit_error(e);
    assert!(eng.has_errors());
    assert_eq!(eng.error_count(), 1);
}

#[test]
fn test_diagnostic_engine_warning_does_not_set_has_errors() {
    let mut eng = DiagnosticEngine::default();
    let warn = NyxError::new(codes::SEMANTIC_SHADOWING, "shadow", ErrorCategory::Compiler)
        .with_severity(Severity::Warning);
    eng.emit_error(warn);
    // has_errors checks for Severity::Error only
    assert!(!eng.has_errors());
    assert_eq!(eng.warning_count(), 1);
}

#[test]
fn test_diagnostic_engine_warning_count() {
    let mut eng = DiagnosticEngine::default();
    let w1 = NyxError::new(codes::SEMANTIC_SHADOWING, "s1", ErrorCategory::Compiler)
        .with_severity(Severity::Warning);
    let w2 = NyxError::new(codes::SEMANTIC_SHADOWING, "s2", ErrorCategory::Compiler)
        .with_severity(Severity::Warning);
    eng.emit_error(w1);
    eng.emit_error(w2);
    assert_eq!(eng.warning_count(), 2);
}

#[test]
fn test_diagnostic_engine_error_count_multiple() {
    let mut eng = DiagnosticEngine::default();
    for i in 0..5 {
        eng.emit(Diagnostic::error(codes::UNEXPECTED_TOKEN, format!("e{}", i)));
    }
    assert_eq!(eng.error_count(), 5);
}

#[test]
fn test_diagnostic_engine_has_any_errors_with_nyx_errors() {
    let mut eng = DiagnosticEngine::default();
    let e = NyxError::new(codes::INTERNAL_COMPILER_BUG, "bug", ErrorCategory::Internal)
        .with_severity(Severity::Warning); // not a hard error but still a NyxError
    eng.emit_error(e);
    // has_any_errors = has_errors || !nyx_errors.is_empty()
    assert!(eng.has_any_errors());
}

#[test]
fn test_diagnostic_engine_clear() {
    let mut eng = DiagnosticEngine::default();
    eng.emit(Diagnostic::error(codes::UNEXPECTED_TOKEN, "e"));
    eng.emit_error(NyxError::new(codes::IR_ERROR, "ir", ErrorCategory::Compiler));
    eng.clear();
    assert!(!eng.has_errors());
    assert_eq!(eng.error_count(), 0);
    assert!(eng.diagnostics.is_empty());
    assert!(eng.nyx_errors.is_empty());
}

#[test]
fn test_diagnostic_engine_into_nyx_errors() {
    let mut eng = DiagnosticEngine::default();
    eng.emit(Diagnostic::error(codes::UNEXPECTED_TOKEN, "d1"));
    eng.emit(Diagnostic::error(codes::TYPE_MISMATCH, "d2"));
    eng.emit_error(NyxError::new(codes::IR_ERROR, "n1", ErrorCategory::Compiler));
    let errors = eng.into_nyx_errors();
    // 2 converted diagnostics + 1 original NyxError = 3 total
    assert_eq!(errors.len(), 3);
}

#[test]
fn test_diagnostic_engine_print_all_does_not_panic() {
    let mut eng = DiagnosticEngine::default();
    eng.emit(Diagnostic::error(codes::UNEXPECTED_TOKEN, "tok"));
    eng.emit_error(NyxError::new(codes::IR_ERROR, "ir", ErrorCategory::Compiler));
    eng.print_all(); // prints to stderr, should not panic
}

#[test]
fn test_diagnostic_engine_print_all_pretty_does_not_panic() {
    let mut eng = DiagnosticEngine::default();
    eng.emit_error(
        NyxError::new(codes::TYPE_MISMATCH_ERROR, "type mismatch", ErrorCategory::Type)
            .with_file("a.nyx")
            .with_line(1)
            .with_column(1)
    );
    eng.print_all_pretty();
}

// ═══════════════════════════════════════════════════════════════════════════════
// 13. ErrorMitigation trait and DefaultMitigation
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_default_mitigation_syntax_returns_skip() {
    let m = DefaultMitigation;
    let e = NyxError::new(codes::PARSER_SYNTAX_ERROR, "s", ErrorCategory::Syntax);
    let s = m.mitigate(&e);
    assert!(matches!(s, RecoveryStrategy::Skip));
}

#[test]
fn test_default_mitigation_compiler_returns_skip() {
    let m = DefaultMitigation;
    let e = NyxError::new(codes::UNEXPECTED_TOKEN, "t", ErrorCategory::Compiler);
    let s = m.mitigate(&e);
    assert!(matches!(s, RecoveryStrategy::Skip));
}

#[test]
fn test_default_mitigation_type_returns_abort() {
    let m = DefaultMitigation;
    let e = NyxError::new(codes::TYPE_MISMATCH_ERROR, "t", ErrorCategory::Type);
    let s = m.mitigate(&e);
    assert!(matches!(s, RecoveryStrategy::Abort));
}

#[test]
fn test_default_mitigation_runtime_returns_abort() {
    let m = DefaultMitigation;
    let e = NyxError::new(codes::RUNTIME_DIVISION_BY_ZERO, "dz", ErrorCategory::Runtime);
    let s = m.mitigate(&e);
    assert!(matches!(s, RecoveryStrategy::Abort));
}

#[test]
fn test_default_mitigation_io_returns_default() {
    let m = DefaultMitigation;
    let e = NyxError::new(codes::IO_FILE_NOT_FOUND, "f", ErrorCategory::Io);
    let s = m.mitigate(&e);
    assert!(matches!(s, RecoveryStrategy::Default));
}

#[test]
fn test_default_mitigation_network_returns_default() {
    let m = DefaultMitigation;
    let e = NyxError::new(codes::NET_TIMEOUT, "t", ErrorCategory::Network);
    let s = m.mitigate(&e);
    assert!(matches!(s, RecoveryStrategy::Default));
}

#[test]
fn test_default_mitigation_security_returns_abort() {
    let m = DefaultMitigation;
    let e = NyxError::new(codes::SECURITY_AUTH_FAILED, "a", ErrorCategory::Security);
    let s = m.mitigate(&e);
    assert!(matches!(s, RecoveryStrategy::Abort));
}

#[test]
fn test_default_mitigation_internal_returns_abort() {
    let m = DefaultMitigation;
    let e = NyxError::new(codes::INTERNAL_COMPILER_BUG, "b", ErrorCategory::Internal);
    let s = m.mitigate(&e);
    assert!(matches!(s, RecoveryStrategy::Abort));
}

#[test]
fn test_default_mitigation_can_mitigate_recoverable() {
    let m = DefaultMitigation;
    let e = NyxError::new(codes::RUNTIME_CUSTOM_ERROR, "r", ErrorCategory::Runtime)
        .recoverable(true);
    assert!(m.can_mitigate(&e));
}

#[test]
fn test_default_mitigation_can_mitigate_syntax() {
    let m = DefaultMitigation;
    let e = NyxError::new(codes::PARSER_SYNTAX_ERROR, "s", ErrorCategory::Syntax);
    assert!(m.can_mitigate(&e));
}

#[test]
fn test_default_mitigation_can_mitigate_compiler() {
    let m = DefaultMitigation;
    let e = NyxError::new(codes::UNEXPECTED_TOKEN, "t", ErrorCategory::Compiler);
    assert!(m.can_mitigate(&e));
}

#[test]
fn test_default_mitigation_cannot_mitigate_internal() {
    let m = DefaultMitigation;
    let e = NyxError::new(codes::INTERNAL_COMPILER_BUG, "b", ErrorCategory::Internal);
    assert!(!m.can_mitigate(&e));
}

// ═══════════════════════════════════════════════════════════════════════════════
// 14. apply_mitigation function
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_apply_mitigation_ok_gives_success() {
    let m = DefaultMitigation;
    let result: Result<i32> = Ok(42);
    let r = apply_mitigation(result, &m);
    assert!(r.is_ok());
    assert_eq!(r.ok().unwrap(), 42);
}

#[test]
fn test_apply_mitigation_err_non_recoverable_gives_unrecoverable() {
    let m = DefaultMitigation;
    let e = NyxError::new(codes::INTERNAL_COMPILER_BUG, "bug", ErrorCategory::Internal);
    let result: Result<i32> = Err(e);
    let r = apply_mitigation(result, &m);
    assert!(r.is_err());
}

#[test]
fn test_apply_mitigation_err_recoverable_still_unrecoverable_without_value() {
    // The current implementation always returns Unrecoverable for non-Ok paths
    // because it cannot produce a T from context alone.
    let m = DefaultMitigation;
    let e = NyxError::new(codes::PARSER_SYNTAX_ERROR, "syn", ErrorCategory::Syntax)
        .recoverable(true);
    let result: Result<i32> = Err(e);
    let r = apply_mitigation(result, &m);
    // Per implementation, Skip/Default/Placeholder/Correct all return Unrecoverable for now
    assert!(r.is_err());
}

// ═══════════════════════════════════════════════════════════════════════════════
// 15. Result type alias
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_result_type_alias_ok() {
    let r: Result<i32> = Ok(1);
    assert!(r.is_ok());
}

#[test]
fn test_result_type_alias_err() {
    let e = NyxError::new(codes::IR_ERROR, "ir", ErrorCategory::Compiler);
    let r: Result<i32> = Err(e);
    assert!(r.is_err());
}

// ═══════════════════════════════════════════════════════════════════════════════
// 16. Integration tests — chained builder + engine + pretty-print
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_integration_full_error_flow() {
    let mut engine = DiagnosticEngine::default();

    // Lexer error
    let lex_err = syntax_error!(codes::LEXER_INVALID_CHAR, "invalid character '@'")
        .with_file("test.nyx")
        .with_location(3, 12)
        .with_note("only ASCII identifiers are supported")
        .with_suggestion("remove the '@' character")
        .recoverable(true);
    engine.emit_error(lex_err);

    // Parser error
    let parse_err = syntax_error!(codes::PARSER_UNEXPECTED_TOKEN, "expected ';' after expression")
        .with_file("test.nyx")
        .with_location(7, 20);
    engine.emit_error(parse_err);

    // Type error
    let type_err = type_error!(codes::TYPE_MISMATCH_ERROR, "expected i32, found str")
        .with_file("test.nyx")
        .with_span((10, 5), (10, 12));
    engine.emit_error(type_err);

    assert!(engine.has_errors());
    assert_eq!(engine.error_count(), 3);

    let all_errors = engine.into_nyx_errors();
    assert_eq!(all_errors.len(), 3);

    for e in &all_errors {
        let pp = pretty_print_error(e);
        assert!(!pp.is_empty());
    }
}

#[test]
fn test_integration_diagnostic_converted_and_emitted() {
    let mut engine = DiagnosticEngine::default();

    let span = make_span(1, 1, 1, 5);
    let d = Diagnostic::error(codes::PARSER_MISSING_TOKEN, "missing ')'")
        .with_span(span)
        .with_note("unclosed parenthesis")
        .with_suggestion("add ')' at end of expression");

    engine.emit(d);

    let errors = engine.into_nyx_errors();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].details.notes.len(), 1);
    assert_eq!(errors[0].details.suggestions.len(), 1);
    assert_eq!(errors[0].details.line, Some(1));
}

#[test]
fn test_integration_from_io_error_in_result_chain() {
    fn may_fail() -> Result<()> {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
        Err(io_err.into())
    }

    let cfg = LogConfig::default();
    let res = may_fail();
    assert!(res.is_err());
    log_error(&res, &cfg);

    let e = res.unwrap_err();
    assert_eq!(e.code, codes::IO_FILE_NOT_FOUND);
}
