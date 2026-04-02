use std::fs;
use std::path::PathBuf;

use clap::Parser;

use nyx::core::ast::ast_nodes::{Expr, ItemKind, Stmt};
use nyx::core::lexer::lexer::Lexer;
use nyx::core::parser::grammar_engine::GrammarEngine;
use nyx::core::parser::neuro_parser::NeuroParser;
use nyx::core::registry::language_registry::LanguageRegistry;

#[derive(Debug, Parser)]
struct Args {
    input: PathBuf,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse();
    let source = fs::read_to_string(&args.input).map_err(|e| e.to_string())?;

    let mut lexer = Lexer::from_source(source.clone());
    let _ = lexer.tokenize().map_err(|e| e.to_string())?;

    let registry = LanguageRegistry::default();
    let grammar = GrammarEngine::from_registry(&registry);

    // 1. Structural Audit via Bridge
    use nyx::devtools::bridge::Bridge;
    let bridge_issues = Bridge::audit_file(&args.input);
    let mut issues: Vec<String> = bridge_issues.iter().map(|e| e.to_string()).collect();

    // 2. Custom Lint Rules (Line length, etc)
    let tokens = {
        let mut lexer = Lexer::from_source(source.clone());
        lexer.tokenize().map_err(|e| e.to_string())?
    };
    let mut parser = NeuroParser::new(grammar);
    let ast = parser.parse(&tokens).map_err(|e| e.to_string())?;

    for (idx, line) in source.lines().enumerate() {
        let ln = idx + 1;
        if line.len() > 100 {
            issues.push(format!(
                "{}:{} line exceeds 100 chars",
                args.input.display(),
                ln
            ));
        }
    }

    for item in &ast.items {
        match &item.kind {
            ItemKind::Function(func) => {
                if func
                    .name
                    .chars()
                    .next()
                    .map(|c| c.is_uppercase())
                    .unwrap_or(false)
                {
                    issues.push(format!(
                        "{}:{} function name '{}' should be snake_case",
                        args.input.display(),
                        item.span.start.line,
                        func.name
                    ));
                }
                lint_stmts(&func.body, &args.input, &mut issues);
            }
            ItemKind::Struct(strct) => {
                if strct
                    .name
                    .chars()
                    .next()
                    .map(|c| c.is_lowercase())
                    .unwrap_or(false)
                {
                    issues.push(format!(
                        "{}:{} struct name '{}' should be PascalCase",
                        args.input.display(),
                        item.span.start.line,
                        strct.name
                    ));
                }
            }
            _ => {}
        }
    }

    if issues.is_empty() {
        println!("lint ok");
        Ok(())
    } else {
        for issue in issues {
            println!("lint: {issue}");
        }
        Err("lint failed".to_string())
    }
}

fn lint_stmts(stmts: &[Stmt], file: &PathBuf, issues: &mut Vec<String>) {
    for stmt in stmts {
        match stmt {
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for branch in branches {
                    if branch.body.is_empty() {
                        issues.push(format!("{}: empty if branch detected", file.display()));
                    }
                    lint_stmts(&branch.body, file, issues);
                }
                if let Some(eb) = else_body {
                    if eb.is_empty() {
                        issues.push(format!("{}: empty else branch detected", file.display()));
                    }
                    lint_stmts(eb, file, issues);
                }
            }
            Stmt::While { body, .. } => {
                if body.is_empty() {
                    issues.push(format!("{}: empty while loop detected", file.display()));
                }
                lint_stmts(body, file, issues);
            }
            Stmt::Loop { body, .. } => {
                if body.is_empty() {
                    issues.push(format!("{}: empty loop detected", file.display()));
                }
                let mut has_break = false;
                check_for_break(body, &mut has_break);
                if !has_break {
                    issues.push(format!(
                        "{}: loop might be infinite (no break found)",
                        file.display()
                    ));
                }
                lint_stmts(body, file, issues);
            }
            _ => {}
        }
    }
}

fn check_for_break(stmts: &[Stmt], has_break: &mut bool) {
    if *has_break {
        return;
    }
    for stmt in stmts {
        match stmt {
            Stmt::Break { .. } => *has_break = true,
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for branch in branches {
                    check_for_break(&branch.body, has_break);
                }
                if let Some(eb) = else_body {
                    check_for_break(eb, has_break);
                }
            }
            Stmt::Expr(Expr::Block { stmts: body, .. }) => check_for_break(body, has_break),
            _ => {}
        }
    }
}
