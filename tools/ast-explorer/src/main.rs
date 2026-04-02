use clap::Parser;
use colored::*;
use nyx::core::lexer::lexer::Lexer;
use nyx::core::parser::neuro_parser::NeuroParser;
use nyx::core::ast::ast_nodes::{Program, ItemKind};
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "nyx-ast-explorer")]
#[command(about = "Nyx Abstract Syntax Tree Explorer", long_about = None)]
struct Args {
    /// Nyx file to explore
    file: PathBuf,

    /// Output format (text, json)
    #[arg(short, long, default_value = "text")]
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

    let registry = nyx::core::registry::language_registry::LanguageRegistry::load("registry/language.json").unwrap_or_default();

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

    if args.format == "json" {
        match serde_json::to_string_pretty(&program) {
            Ok(json) => println!("{}", json),
            Err(e) => eprintln!("JSON Error: {}", e),
        }
    } else {
        println!("{}", "============================================================".magenta());
        println!("{} {}", "AST Explorer:".bold(), args.file.display());
        println!("{}", "============================================================".magenta());
        
        // Basic indentation-based tree printer
        print_program(&program);
    }
}

fn print_program(program: &Program) {
    println!("{}", "Program".bold().blue());
    for item in &program.items {
        print_item(item, 1);
    }
}

fn print_item(item: &nyx::core::ast::ast_nodes::Item, indent: usize) {
    let pad = "  ".repeat(indent);
    match &item.kind {
        ItemKind::Function(f) => {
            println!("{}{}: {}", pad, "Function".green(), f.name);
        }
        ItemKind::Struct(s) => {
            println!("{}{}: {}", pad, "Struct".yellow(), s.name);
        }
        ItemKind::Enum(e) => {
            println!("{}{}: {}", pad, "Enum".cyan(), e.name);
        }
        ItemKind::Trait(t) => {
            println!("{}{}: {}", pad, "Trait".magenta(), t.name);
        }
        _ => {
            println!("{}{}: Unknown Item", pad, "Item".red());
        }
    }
}
