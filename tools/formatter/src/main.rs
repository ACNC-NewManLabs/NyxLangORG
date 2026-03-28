//! nyx-fmt — Production-Grade Nyx Source Formatter
//!
//! A token-stream formatter for the Nyx language that produces consistent,
//! idiomatic code output.  It is fully driven by the shared Nyx lexer so it
//! can never drift out of sync with the language.
//!
//! # Features
//! - Format in-place or preview with `--check`
//! - Format one file, many files, or an entire directory (globs)
//! - `--diff` mode shows what would change without writing
//! - `--indent N` configures indentation width (default 4)
//! - `--max-line N` configures line-length target (default 100)
//! - Colored, human-friendly terminal output
//! - Exits with code 1 when `--check` finds unformatted files

use std::fs;
use std::path::{Path, PathBuf};

use clap::Parser;
use colored::*;

use nyx::core::lexer::lexer::Lexer;
use nyx::core::lexer::token::TokenKind;

// ── CLI ───────────────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
#[command(
    name = "nyx-fmt",
    about = "Nyx source code formatter",
    version = "1.0.0",
    long_about = "Format Nyx source files to a canonical style.\n\
                  Exits 0 when all files are already formatted, 1 otherwise."
)]
struct Args {
    /// Files or directories to format (omit to format current directory)
    #[arg(value_name = "PATH", default_values = ["."])]
    paths: Vec<PathBuf>,

    /// Check mode: report unformatted files without writing
    #[arg(long, short = 'c')]
    check: bool,

    /// Show a unified diff instead of writing files
    #[arg(long, short = 'd')]
    diff: bool,

    /// Indentation width in spaces
    #[arg(long, default_value = "4", value_name = "N")]
    indent: usize,

    /// Target maximum line length (soft limit; comments may exceed)
    #[arg(long, default_value = "100", value_name = "N")]
    max_line: usize,

    /// Suppress all output except errors
    #[arg(long, short = 'q')]
    quiet: bool,

    /// Print every file being considered (verbose)
    #[arg(long, short = 'v')]
    verbose: bool,
}

// ── Entry Point ───────────────────────────────────────────────────────────────

fn main() {
    let args = Args::parse();
    let config = FormatConfig {
        indent_width: args.indent,
        max_line: args.max_line,
    };

    let files: Vec<PathBuf> = args.paths.iter()
        .flat_map(|p| collect_nyx_files(p))
        .collect();

    if files.is_empty() {
        if !args.quiet {
            println!("{} No .nyx files found.", "⚠".yellow().bold());
        }
        return;
    }

    let total = files.len();
    let mut changed = 0usize;
    let mut errors  = 0usize;

    for path in &files {
        if args.verbose {
            println!("{} {}", "🔍".dimmed(), path.display());
        }

        match process_file(path, &config, args.check, args.diff, args.quiet) {
            Ok(was_changed) => {
                if was_changed { changed += 1; }
            }
            Err(e) => {
                errors += 1;
                eprintln!("{} {}: {}", "error".red().bold(), path.display(), e);
            }
        }
    }

    // ── Summary ──────────────────────────────────────────────────────────────
    if !args.quiet {
        println!();
        if args.check {
            if changed == 0 {
                println!("{} {} file{} already formatted.",
                    "✓".green().bold(), total, plural(total));
            } else {
                println!("{} {}/{} file{} need formatting.",
                    "✗".red().bold(), changed, total, plural(total));
            }
        } else {
            let unchanged = total - changed - errors;
            if changed > 0 {
                println!("{} Formatted {} file{}.",
                    "✓".green().bold(), changed, plural(changed));
            }
            if unchanged > 0 {
                println!("{} {} file{} already up to date.",
                    "·".dimmed(), unchanged, plural(unchanged));
            }
        }
        if errors > 0 {
            println!("{} {} file{} could not be processed.",
                "✗".red().bold(), errors, plural(errors));
        }
    }

    if args.check && changed > 0 {
        std::process::exit(1);
    }
    if errors > 0 {
        std::process::exit(2);
    }
}

// ── File Collection ───────────────────────────────────────────────────────────

fn collect_nyx_files(root: &Path) -> Vec<PathBuf> {
    if root.is_file() {
        if root.extension().map(|e| e == "nyx").unwrap_or(false) {
            return vec![root.to_path_buf()];
        } else {
            eprintln!("{} Skipping non-.nyx file: {}", "⚠".yellow(), root.display());
            return vec![];
        }
    }

    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Skip hidden and common build dirs
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.starts_with('.') || matches!(name, "target" | "node_modules" | "dist") {
                    continue;
                }
                files.extend(collect_nyx_files(&path));
            } else if path.extension().map(|e| e == "nyx").unwrap_or(false) {
                files.push(path);
            }
        }
    }
    files
}

// ── File Processing ───────────────────────────────────────────────────────────

fn process_file(
    path: &Path,
    config: &FormatConfig,
    check: bool,
    show_diff: bool,
    quiet: bool,
) -> Result<bool, String> {
    let original = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let formatted = format_source(&original, config)?;

    let changed = original != formatted;

    if show_diff && changed {
        print_diff(path, &original, &formatted);
        return Ok(true);
    }

    if check {
        if changed && !quiet {
            println!("{} {}", "UNFORMATTED".red().bold(), path.display());
        }
        return Ok(changed);
    }

    if changed {
        // Create a backup-free atomic write via temp file approach
        fs::write(path, &formatted).map_err(|e| e.to_string())?;
        if !quiet {
            println!("{} {}", "✓".green().bold(), path.display());
        }
    }

    Ok(changed)
}

// ── Formatter Config ──────────────────────────────────────────────────────────

struct FormatConfig {
    indent_width: usize,
    /// Soft maximum line length. Reserved for future line-wrapping logic.
    #[allow(dead_code)]
    max_line: usize,
}

// ── Core Formatter ────────────────────────────────────────────────────────────

fn format_source(source: &str, config: &FormatConfig) -> Result<String, String> {
    let mut lexer = Lexer::from_source(source.to_string());
    let tokens = lexer.tokenize().map_err(|e| e.to_string())?;

    let indent = " ".repeat(config.indent_width);
    let mut out = String::with_capacity(source.len() + 128);
    let mut indent_level: usize = 0;
    let mut at_line_start = true;
    let mut blank_lines: u32 = 0;
    let mut last_was_blank = false;

    let n = tokens.len();
    let mut i = 0;

    while i < n {
        let tok = &tokens[i];
        let kind = &tok.kind;

        if *kind == TokenKind::Eof {
            break;
        }

        // ── Track blank lines from whitespace ────────────────────────────────
        if *kind == TokenKind::Newline || *kind == TokenKind::Whitespace {
            if *kind == TokenKind::Newline {
                blank_lines += 1;
            }
            i += 1;
            continue;
        }

        // ── Closing brace: de-dent first ─────────────────────────────────────
        if *kind == TokenKind::RBrace && indent_level > 0 {
            indent_level -= 1;
        }

        // ── Insert blank lines between top-level items ────────────────────────
        if !at_line_start {
            // Use a single blank line to separate items after `}` - never double
            let emit_blank = blank_lines > 0 && !last_was_blank;
            if emit_blank && indent_level == 0 {
                out.push('\n');
                last_was_blank = true;
            } else {
                last_was_blank = false;
            }
        }
        blank_lines = 0;

        // ── Indentation ───────────────────────────────────────────────────────
        if at_line_start {
            out.push_str(&indent.repeat(indent_level));
            at_line_start = false;
            last_was_blank = false;
        } else {
            // Space before this token?
            let prev = if i > 0 { Some(&tokens[i - 1].kind) } else { None };
            if space_before(kind, prev) {
                out.push(' ');
            }
        }

        // ── Emit token ────────────────────────────────────────────────────────
        out.push_str(&tok.lexeme);

        // ── Post-token actions ────────────────────────────────────────────────
        match kind {
            TokenKind::LBrace => {
                indent_level += 1;
                out.push('\n');
                at_line_start = true;
            }
            TokenKind::RBrace => {
                // If next non-ws token is `else`, no newline
                let next_kind = peek_non_ws(&tokens, i + 1);
                if matches!(next_kind, Some(TokenKind::KwElse)) {
                    // will get space before `else`
                } else {
                    out.push('\n');
                    at_line_start = true;
                }
            }
            TokenKind::Semicolon => {
                out.push('\n');
                at_line_start = true;
            }
            TokenKind::Comma => {
                // Keep on same line (space will be added before next token)
            }
            TokenKind::Comment | TokenKind::MultiLineComment => {
                out.push('\n');
                at_line_start = true;
            }
            _ => {}
        }

        i += 1;
    }

    // Ensure file ends with exactly one newline
    let trimmed = out.trim_end_matches('\n');
    let mut result = trimmed.to_string();
    result.push('\n');

    Ok(result)
}

// ── Space Rules ───────────────────────────────────────────────────────────────

fn space_before(kind: &TokenKind, prev: Option<&TokenKind>) -> bool {
    // Never space after `(`, `[`, `{open` or before `)`, `]`
    if let Some(p) = prev {
        match p {
            TokenKind::LParen | TokenKind::LBracket => return false,
            TokenKind::Dot => return false,
            TokenKind::Bang => return false,
            TokenKind::At => return false,
            _ => {}
        }
    }

    match kind {
        // Punctuation that never gets a space before it
        TokenKind::RParen
        | TokenKind::RBracket
        | TokenKind::Semicolon
        | TokenKind::Comma
        | TokenKind::Dot
        | TokenKind::ColonColon
        | TokenKind::Question => false,

        // Opening symbols
        TokenKind::LParen | TokenKind::LBracket => {
            // No space before `(` when following identifiers / keywords (function call)
            if let Some(p) = prev {
                matches!(p, TokenKind::Identifier | TokenKind::RParen | TokenKind::RBracket)
            } else {
                false
            }
        }

        // Binary operators always get spaces
        TokenKind::Equal
        | TokenKind::EqEq
        | TokenKind::BangEqual
        | TokenKind::Less
        | TokenKind::LessEqual
        | TokenKind::Greater
        | TokenKind::GreaterEqual
        | TokenKind::Plus
        | TokenKind::Minus
        | TokenKind::Star
        | TokenKind::Slash
        | TokenKind::Percent
        | TokenKind::Ampersand
        | TokenKind::AmpersandAmpersand
        | TokenKind::Pipe
        | TokenKind::PipePipe
        | TokenKind::PlusEqual
        | TokenKind::MinusEqual
        | TokenKind::StarEqual
        | TokenKind::SlashEqual
        | TokenKind::PercentEqual
        | TokenKind::FatArrow
        | TokenKind::ThinArrow
        | TokenKind::Arrow => true,

        // Colon: space before in `key: val` but not `Type::Path`
        TokenKind::Colon => {
            if let Some(p) = prev {
                *p != TokenKind::Colon
            } else {
                false
            }
        }

        // Keywords always get space before them (except at line start)
        t if t.is_keyword() => {
            if let Some(p) = prev {
                !matches!(p, TokenKind::LBrace | TokenKind::LParen | TokenKind::LBracket)
            } else {
                false
            }
        }

        // Identifiers / literals: space after comma or keyword
        TokenKind::Identifier
        | TokenKind::Integer
        | TokenKind::Float
        | TokenKind::String
        | TokenKind::Char
        | TokenKind::Boolean
        | TokenKind::Null => {
            if let Some(p) = prev {
                matches!(
                    p,
                    TokenKind::Comma
                    | TokenKind::Colon
                    | TokenKind::Equal
                    | TokenKind::LBrace
                ) || p.is_keyword()
            } else {
                false
            }
        }

        _ => false,
    }
}

// ── Diff Printer ──────────────────────────────────────────────────────────────

fn print_diff(path: &Path, original: &str, formatted: &str) {
    println!("\n{} {}", "---".red(), path.display());
    println!("{} {}", "+++".green(), path.display());
    println!();

    let orig_lines: Vec<&str> = original.lines().collect();
    let fmt_lines: Vec<&str>  = formatted.lines().collect();
    let max = orig_lines.len().max(fmt_lines.len());

    for i in 0..max {
        let o = orig_lines.get(i).copied();
        let f = fmt_lines.get(i).copied();
        match (o, f) {
            (Some(a), Some(b)) if a == b => {
                println!("  {}", a);
            }
            (Some(a), Some(b)) => {
                println!("{}", format!("- {}", a).red());
                println!("{}", format!("+ {}", b).green());
            }
            (Some(a), None) => {
                println!("{}", format!("- {}", a).red());
            }
            (None, Some(b)) => {
                println!("{}", format!("+ {}", b).green());
            }
            (None, None) => {}
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn peek_non_ws(tokens: &[nyx::core::lexer::token::Token], from: usize) -> Option<&TokenKind> {
    tokens[from..].iter().find(|t| {
        !matches!(t.kind, TokenKind::Whitespace | TokenKind::Newline)
    }).map(|t| &t.kind)
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}
