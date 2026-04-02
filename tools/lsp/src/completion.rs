//! Nyx Code Completion Provider

use crate::index::GlobalIndex;
use lsp_types::{CompletionItem, CompletionItemKind, InsertTextFormat, Position};

/// Get completion suggestions at the given position
pub fn get_completions(
    source: &str,
    position: Position,
    index: &GlobalIndex,
    doc: &crate::analyzer::DocumentAnalyzer,
) -> Vec<CompletionItem> {
    let line_idx = position.line as usize;
    let lines: Vec<&str> = source.lines().collect();

    if line_idx >= lines.len() {
        return default_completions();
    }

    let line = lines[line_idx];
    let col = position.character as usize;

    // Get text before cursor
    let prefix = if col > line.len() { line } else { &line[..col] };

    let mut completions = Vec::new();

    // Check for member (dot-access) completions first
    let members = doc.get_members_at(position, index);
    if !members.is_empty() {
        return members
            .into_iter()
            .map(|m| CompletionItem {
                label: m.clone(),
                kind: Some(CompletionItemKind::FIELD),
                detail: Some("field".to_string()),
                insert_text: Some(m),
                insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                ..Default::default()
            })
            .collect();
    }

    // Check if we're in a fresh context
    let is_fresh_context =
        prefix.trim().is_empty() || prefix.ends_with(' ') || prefix.ends_with('\t');

    if is_fresh_context {
        // Add keywords and built-ins
        completions.extend(keyword_completions(prefix));
    } else {
        // Add identifier-based completions (local + global)
        completions.extend(identifier_completions(prefix, source, index));
    }

    // Always add built-in functions
    completions.extend(builtin_completions(prefix));

    // Filter by prefix
    if !prefix.is_empty() {
        let lower_prefix = prefix.to_lowercase();
        completions.retain(|c| {
            c.label.to_lowercase().starts_with(&lower_prefix)
                || c.filter_text
                    .as_ref()
                    .map(|f| f.to_lowercase().starts_with(&lower_prefix))
                    .unwrap_or(false)
        });
    }

    // If no matches, return defaults
    if completions.is_empty() {
        default_completions()
    } else {
        completions
    }
}

/// Default completions for fresh context
fn default_completions() -> Vec<CompletionItem> {
    let mut items = Vec::new();

    // Keywords
    items.push(keyword_item(
        "fn",
        "fn name(params) { ... }",
        "Function declaration",
    ));
    items.push(keyword_item(
        "let",
        "let name = value",
        "Variable declaration",
    ));
    items.push(keyword_item(
        "return",
        "return expression",
        "Return statement",
    ));
    items.push(keyword_item("if", "if condition { ... }", "Conditional"));
    items.push(keyword_item("else", "else { ... }", "Else branch"));
    items.push(keyword_item(
        "while",
        "while condition { ... }",
        "While loop",
    ));
    items.push(keyword_item("for", "for i in range { ... }", "For loop"));
    items.push(keyword_item(
        "module",
        "module name { ... }",
        "Module declaration",
    ));
    items.push(keyword_item("type", "type Name = ...", "Type alias"));

    // Types
    items.push(type_item("int", "Integer type"));
    items.push(type_item("float", "Float type"));
    items.push(type_item("bool", "Boolean type"));
    items.push(type_item("string", "String type"));
    items.push(type_item("void", "Void type"));

    // Built-in functions
    items.push(builtin_item("print", "print(value)", "Print to console"));
    items.push(builtin_item(
        "scheduler_run",
        "scheduler_run()",
        "Run scheduler",
    ));
    items.push(builtin_item(
        "io_read_file",
        "io_read_file(path)",
        "Read file",
    ));
    items.push(builtin_item(
        "io_write_file",
        "io_write_file(path, data)",
        "Write file",
    ));
    items.push(builtin_item(
        "crypto_hash64",
        "crypto_hash64(data)",
        "Hash data",
    ));

    items
}

/// Keyword completions
fn keyword_completions(prefix: &str) -> Vec<CompletionItem> {
    let mut items = Vec::new();

    let keywords = [
        ("fn", "fn name(params) { ... }", "Function declaration"),
        ("let", "let name = value", "Variable declaration"),
        ("return", "return expression", "Return statement"),
        ("if", "if condition { ... }", "Conditional"),
        ("else", "else { ... }", "Else branch"),
        ("while", "while condition { ... }", "While loop"),
        ("for", "for i in range { ... }", "For loop"),
        ("loop", "loop { ... }", "Infinite loop"),
        ("break", "break", "Break loop"),
        ("continue", "continue", "Continue loop"),
        ("module", "module name { ... }", "Module declaration"),
        ("type", "type Name = ...", "Type alias"),
    ];

    for (keyword, insert, detail) in keywords {
        if prefix.is_empty() || keyword.starts_with(&prefix.to_lowercase()) {
            items.push(keyword_item(keyword, insert, detail));
        }
    }

    items
}

/// Type completions
fn type_completions(prefix: &str) -> Vec<CompletionItem> {
    let mut items = Vec::new();

    let types = [
        ("int", "Integer type (64-bit)"),
        ("float", "Float type (64-bit)"),
        ("bool", "Boolean type"),
        ("string", "String type"),
        ("void", "Void type (no return)"),
    ];

    for (typ, detail) in types {
        if prefix.is_empty() || typ.starts_with(&prefix.to_lowercase()) {
            items.push(type_item(typ, detail));
        }
    }

    items
}

/// Identifier completions based on source
fn identifier_completions(prefix: &str, source: &str, index: &GlobalIndex) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Add global symbols from index first
    for symbol in index.all_symbols() {
        if !seen.contains(&symbol.name) && (prefix.is_empty() || symbol.name.starts_with(prefix)) {
            seen.insert(symbol.name.clone());
            items.push(CompletionItem {
                label: symbol.name.clone(),
                kind: Some(symbol_kind_to_completion_kind(symbol.kind)),
                detail: symbol
                    .description
                    .clone()
                    .or_else(|| Some("global".to_string())),
                insert_text: Some(symbol.name.clone()),
                insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                ..Default::default()
            });
        }
    }

    for line in source.lines() {
        // Find function definitions
        if let Some(name) = line.strip_prefix("fn ").and_then(|l| l.split('(').next()) {
            let name = name.trim();
            if !seen.contains(name) && (prefix.is_empty() || name.starts_with(prefix)) {
                seen.insert(name.to_string());
                items.push(CompletionItem {
                    label: name.to_string(),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some("function".to_string()),
                    insert_text: Some(name.to_string()),
                    insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                    ..Default::default()
                });
            }
        }

        // Find let bindings
        if let Some(name) = line.strip_prefix("let ").and_then(|l| l.split('=').next()) {
            let name = name.trim();
            if !seen.contains(name) && (prefix.is_empty() || name.starts_with(prefix)) {
                seen.insert(name.to_string());
                items.push(CompletionItem {
                    label: name.to_string(),
                    kind: Some(CompletionItemKind::VARIABLE),
                    detail: Some("variable".to_string()),
                    insert_text: Some(name.to_string()),
                    insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                    ..Default::default()
                });
            }
        }
    }

    // Add type completions
    items.extend(type_completions(prefix));

    // Add built-in functions
    items.extend(builtin_completions(prefix));

    items
}

/// Built-in function completions
fn builtin_completions(prefix: &str) -> Vec<CompletionItem> {
    let mut items = Vec::new();

    let builtins = [
        ("print", "print(value)", "Print to console"),
        ("scheduler_run", "scheduler_run()", "Run async scheduler"),
        ("scheduler_spawn", "scheduler_spawn(task)", "Spawn a task"),
        ("io_read_file", "io_read_file(path)", "Read file contents"),
        (
            "io_write_file",
            "io_write_file(path, data)",
            "Write file contents",
        ),
        (
            "io_list_dir",
            "io_list_dir(path)",
            "List directory contents",
        ),
        (
            "crypto_hash64",
            "crypto_hash64(data)",
            "Compute 64-bit hash",
        ),
        (
            "crypto_hash256",
            "crypto_hash256(data)",
            "Compute 256-bit hash",
        ),
        (
            "crypto_encrypt",
            "crypto_encrypt(data, key)",
            "Encrypt data",
        ),
        (
            "crypto_decrypt",
            "crypto_decrypt(data, key)",
            "Decrypt data",
        ),
        ("http_get", "http_get(url)", "HTTP GET request"),
        ("http_post", "http_post(url, body)", "HTTP POST request"),
        ("tcp_connect", "tcp_connect(host, port)", "TCP connect"),
        ("tcp_listen", "tcp_listen(port)", "TCP listen"),
        ("udp_send", "udp_send(addr, data)", "UDP send"),
        ("udp_recv", "udp_recv()", "UDP receive"),
    ];

    for (name, insert, detail) in builtins {
        if prefix.is_empty() || name.starts_with(&prefix.to_lowercase()) {
            items.push(builtin_item(name, insert, detail));
        }
    }

    items
}

/// Create a keyword completion item
fn keyword_item(label: &str, insert: &str, detail: &str) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        kind: Some(CompletionItemKind::KEYWORD),
        detail: Some(detail.to_string()),
        insert_text: Some(insert.to_string()),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        ..Default::default()
    }
}

/// Create a type completion item
fn type_item(label: &str, detail: &str) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        kind: Some(CompletionItemKind::TYPE_PARAMETER),
        detail: Some(detail.to_string()),
        insert_text: Some(label.to_string()),
        insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
        ..Default::default()
    }
}

/// Create a built-in function completion item
fn builtin_item(label: &str, insert: &str, detail: &str) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        kind: Some(CompletionItemKind::FUNCTION),
        detail: Some(detail.to_string()),
        insert_text: Some(insert.to_string()),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        ..Default::default()
    }
}

fn symbol_kind_to_completion_kind(kind: crate::index::SymbolKind) -> CompletionItemKind {
    use crate::index::SymbolKind;
    match kind {
        SymbolKind::FUNCTION => CompletionItemKind::FUNCTION,
        SymbolKind::STRUCT => CompletionItemKind::STRUCT,
        SymbolKind::ENUM => CompletionItemKind::ENUM,
        SymbolKind::INTERFACE => CompletionItemKind::INTERFACE,
        _ => CompletionItemKind::VARIABLE,
    }
}
