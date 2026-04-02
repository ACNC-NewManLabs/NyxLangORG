use clap::Parser;
use colored::*;
use nyx::core::lexer::lexer::Lexer;
use nyx::core::parser::neuro_parser::NeuroParser;
use nyx::core::ast::ast_nodes::{Program, ItemKind, Stmt, Expr};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "nyx-flow")]
#[command(about = "Nyx Visual Flow & Call Graph Generator", long_about = None)]
struct Args {
    /// Nyx file to analyze
    file: PathBuf,

    /// Output format (mermaid, dot)
    #[arg(short, long, default_value = "mermaid")]
    format: String,
}

fn main() {
    let args = Args::parse();

    if !args.file.exists() {
        eprintln!("Error: File not found: {}", args.file.display());
        return;
    }

    let source = fs::read_to_string(&args.file).unwrap_or_else(|e| {
        eprintln!("Error reading file: {}", e);
        std::process::exit(1);
    });

    let registry = match nyx::core::registry::language_registry::LanguageRegistry::load("registry/language.json") {
        Ok(r) => r,
        Err(_) => nyx::core::registry::language_registry::LanguageRegistry::default(),
    };

    let mut lexer = Lexer::from_source(source);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{}: {}", "Lexer Error".red(), e);
            return;
        }
    };
    
    let grammar = nyx::core::parser::grammar_engine::GrammarEngine::from_registry(&registry);
    let mut parser = NeuroParser::new(grammar);
    let program = match parser.parse(&tokens) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}: {}", "Parser Error".red(), e);
            return;
        }
    };

    let call_graph = build_call_graph(&program);

    if args.format == "mermaid" {
        print_mermaid(&call_graph);
    } else {
        println!("Format {} not yet supported.", args.format);
    }
}

fn build_call_graph(program: &Program) -> HashMap<String, HashSet<String>> {
    let mut graph = HashMap::new();
    
    for item in &program.items {
        if let ItemKind::Function(f) = &item.kind {
            let mut calls = HashSet::new();
            find_calls_in_stmts(&f.body, &mut calls);
            graph.insert(f.name.clone(), calls);
        }
    }
    
    graph
}

fn find_calls_in_stmts(stmts: &[Stmt], calls: &mut HashSet<String>) {
    for stmt in stmts {
        match stmt {
            Stmt::Expr(e) | Stmt::Return { expr: Some(e), .. } | Stmt::Let { expr: e, .. } => {
                find_calls_in_expr(e, calls);
            }
            Stmt::If { branches, else_body, .. } => {
                for branch in branches {
                    find_calls_in_stmts(&branch.body, calls);
                }
                if let Some(body) = else_body {
                    find_calls_in_stmts(body, calls);
                }
            }
            Stmt::While { body, .. } | Stmt::Loop { body, .. } | Stmt::ForIn { body, .. } => {
                find_calls_in_stmts(body, calls);
            }
            _ => {}
        }
    }
}

fn find_calls_in_expr(expr: &Expr, calls: &mut HashSet<String>) {
    match expr {
        Expr::Call { callee, args, .. } => {
            if let Expr::Identifier { name, .. } = callee.as_ref() {
                calls.insert(name.clone());
            }
            for arg in args {
                find_calls_in_expr(arg, calls);
            }
        }
        Expr::Binary { left, right, .. } => {
            find_calls_in_expr(left, calls);
            find_calls_in_expr(right, calls);
        }
        Expr::Unary { right, .. } => {
            find_calls_in_expr(right, calls);
        }
        // ... more cases if needed
        _ => {}
    }
}

fn print_mermaid(graph: &HashMap<String, HashSet<String>>) {
    println!("graph TD");
    for (caller, callees) in graph {
        if callees.is_empty() {
            println!("    {}", caller);
        } else {
            for callee in callees {
                println!("    {} --> {}", caller, callee);
            }
        }
    }
}
