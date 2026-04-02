use clap::Parser;
use colored::*;
use nyx::applications::compiler::compiler_main::Compiler;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "nyx-viewer")]
#[command(about = "Nyx Bytecode Disassembler", long_about = None)]
struct Args {
    /// Nyx file or pre-compiled bytecode file
    file: PathBuf,
}

fn main() {
    let args = Args::parse();

    if !args.file.exists() {
        eprintln!("Error: File not found: {}", args.file.display());
        return;
    }

    println!(
        "{}",
        "============================================================".yellow()
    );
    println!("{} {}", "Disassembling:".bold(), args.file.display());
    println!(
        "{}",
        "============================================================".yellow()
    );

    let mut compiler =
        match Compiler::from_registry_files("registry/language.json", "registry/engines.json") {
            Ok(c) => c,
            Err(_) => {
                eprintln!("Warning: could not load registries, using default behavior.");
                return;
            }
        };

    match compiler.compile_to_bytecode(&args.file) {
        Ok(module) => {
            for func in &module.functions {
                println!();
                println!("{} {}", "Function:".blue().bold(), func.name.green());
                println!("Arguments: {}", func.arity);
                println!(
                    "{}",
                    "------------------------------------------------------------".dimmed()
                );

                for (i, instr) in func.instructions.iter().enumerate() {
                    let opcode_str = format!("{:?}", instr.opcode);
                    println!(
                        "{:>4}:  {:<15}  {}",
                        i.to_string().dimmed(),
                        opcode_str.bold(),
                        format!("(line {})", instr.line).dimmed()
                    );

                    // Display operands if any
                    // Note: This matches the simplified BytecodeInstr structure
                    if !instr.operands.is_empty() {
                        print!("       Operands: ");
                        for op in &instr.operands {
                            print!("{:?} ", op);
                        }
                        println!();
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("{}: {}", "Compilation Error".red(), e);
        }
    }
}
