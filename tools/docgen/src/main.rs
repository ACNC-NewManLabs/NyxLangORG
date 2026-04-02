use std::fs;
use std::path::{Path, PathBuf};

use clap::Parser;

use nyx::core::ast::ast_nodes::{
    EnumDecl, FunctionDecl, ItemKind, StructDecl, TraitDecl, Type, TypePath,
};
use nyx::core::lexer::lexer::Lexer;
use nyx::core::parser::grammar_engine::GrammarEngine;
use nyx::core::parser::neuro_parser::NeuroParser;
use nyx::core::registry::language_registry::LanguageRegistry;

#[derive(Debug, Parser)]
struct Args {
    #[arg(default_value = ".")]
    root: PathBuf,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse();
    let mut files = Vec::new();
    walk(&args.root, &mut files)?;

    files.sort();
    let mut out = String::from("# Nyx Ecosystem Docs\n\n");

    let registry = LanguageRegistry::default();
    let grammar = GrammarEngine::from_registry(&registry);

    for file in files {
        out.push_str(&format!("## Module: `{}`\n\n", file.display()));

        let source = fs::read_to_string(&file).map_err(|e| e.to_string())?;
        let mut lexer = Lexer::from_source(source);
        let tokens = match lexer.tokenize() {
            Ok(t) => t,
            Err(_) => continue, // skip unparseable files
        };

        let mut parser = NeuroParser::new(grammar.clone());
        let ast = match parser.parse(&tokens) {
            Ok(a) => a,
            Err(_) => continue, // skip unparseable files
        };

        for item in &ast.items {
            match &item.kind {
                ItemKind::Function(func) => out.push_str(&format_function(func)),
                ItemKind::Struct(strct) => out.push_str(&format_struct(strct)),
                ItemKind::Enum(enm) => out.push_str(&format_enum(enm)),
                ItemKind::Trait(trt) => out.push_str(&format_trait(trt)),
                _ => {}
            }
        }
        out.push_str("\n---\n\n");
    }

    let target = args.root.join("docs/ecosystem_index.md");
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&target, out).map_err(|e| e.to_string())?;
    println!("generated {}", target.display());
    Ok(())
}

fn walk(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for ent in fs::read_dir(path).map_err(|e| e.to_string())? {
        let ent = ent.map_err(|e| e.to_string())?;
        let p = ent.path();
        if p.is_dir() {
            if p.ends_with("target") || p.ends_with(".git") || p.ends_with("node_modules") {
                continue;
            }
            walk(&p, files)?;
        } else if let Some(ext) = p.extension().and_then(|n| n.to_str()) {
            if ext == "nyx" {
                files.push(p);
            }
        }
    }
    Ok(())
}

fn type_to_string(ty: &Type) -> String {
    match ty {
        Type::Named(TypePath { segments }) => {
            let segs: Vec<String> = segments.iter().map(|s| s.name.clone()).collect();
            segs.join("::")
        }
        Type::Reference(inner) => format!("&{}", type_to_string(inner)),
        Type::MutReference(inner) => format!("&mut {}", type_to_string(inner)),
        Type::Pointer { mutable: true, to } => format!("*mut {}", type_to_string(to)),
        Type::Pointer { mutable: false, to } => format!("*const {}", type_to_string(to)),
        Type::Array(inner) => format!("[{}]", type_to_string(inner)),
        Type::Nullable(inner) => format!("{}?", type_to_string(inner)),
        Type::Infer => "_".to_string(),
        Type::Tuple(types) => {
            let ts: Vec<String> = types.iter().map(type_to_string).collect();
            format!("({})", ts.join(", "))
        }
        _ => "unknown".to_string(),
    }
}

fn format_function(func: &FunctionDecl) -> String {
    let mut sig = format!("### `fn {}`\n\n```nyx\nfn {}(", func.name, func.name);

    let params: Vec<String> = func
        .params
        .iter()
        .map(|p| format!("{}: {}", p.name, type_to_string(&p.param_type)))
        .collect();

    sig.push_str(&params.join(", "));
    sig.push(')');

    if let Some(ret) = &func.return_type {
        sig.push_str(&format!(" -> {}", type_to_string(ret)));
    }
    sig.push_str("\n```\n\n");
    sig
}

fn format_struct(strct: &StructDecl) -> String {
    let mut sig = format!(
        "### `struct {}`\n\n```nyx\nstruct {} {{\n",
        strct.name, strct.name
    );

    for field in &strct.fields {
        sig.push_str(&format!(
            "    {}: {},\n",
            field.name,
            type_to_string(&field.field_type)
        ));
    }

    sig.push_str("}\n```\n\n");
    sig
}

fn format_enum(enm: &EnumDecl) -> String {
    let mut sig = format!("### `enum {}`\n\n```nyx\nenum {} {{\n", enm.name, enm.name);

    for variant in &enm.variants {
        sig.push_str(&format!("    {},\n", variant.name()));
    }

    sig.push_str("}\n```\n\n");
    sig
}

fn format_trait(trt: &TraitDecl) -> String {
    format!("### `trait {}`\n\n", trt.name)
}
