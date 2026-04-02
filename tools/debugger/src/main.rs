//! Nyx Debugger Implementation
//!
//! A command-line debugger for Nyx programs with:
//! - Breakpoint management
//! - Step execution
//! - Variable inspection

use crate::breakpoints::BreakpointManager;
use crate::inspector::VariableInspector;
use crate::runtime::{DebugRuntime, RuntimeState};
use clap::{Parser, Subcommand};
use colored::*;
use nyx::applications::compiler::compiler_main::Compiler;
use nyx_vm::VmConfig;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

mod breakpoints;
mod inspector;
mod runtime;

/// Debugger commands
#[derive(Debug, Subcommand)]
enum DebugCommand {
    /// Start debugging a program
    Run {
        /// Path to Nyx source file
        #[arg(default_value = "examples/hello_world.nyx")]
        file: PathBuf,
    },
    /// List breakpoints
    List,
    /// Add a breakpoint
    Break {
        /// Line number or function name
        target: String,
    },
    /// Remove a breakpoint
    Delete {
        /// Breakpoint ID
        id: usize,
    },
    /// Continue execution
    Continue,
    /// Step to next line
    Step,
    /// Step over function call
    Next,
    /// Step out of function
    Out,
    /// Show current stack trace
    Backtrace,
    /// Show local variables
    Locals,
    /// Print a variable value
    Print {
        /// Variable name
        name: String,
    },
    /// List functions
    Functions,
    /// Set debug level
    Verbose {
        /// Enable verbose output
        #[arg(default_value = "false")]
        enable: bool,
    },
}

#[derive(Debug, Parser)]
#[command(name = "nyx-debug", about = "Nyx Debugger")]
struct Args {
    #[command(subcommand)]
    command: DebugCommand,
}

fn main() {
    // Initialize logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();

    match args.command {
        DebugCommand::Run { file } => run_debugger(file),
        _ => println!("Use 'run' command to start a debug session."),
    }
}

/// Run the interactive debugger
fn run_debugger(file: PathBuf) {
    println!(
        "{}",
        "═══════════════════════════════════════════════════════════".magenta()
    );
    println!(
        "{}",
        "                    Nyx Debugger v1.0 [PRODUCTION]"
            .bold()
            .cyan()
    );
    println!(
        "{}",
        "═══════════════════════════════════════════════════════════".magenta()
    );
    println!();
    println!("{} Debugging: {}", "🛰".cyan(), file.display());

    if !file.exists() {
        eprintln!("{} File not found: {}", "Error:".red(), file.display());
        return;
    }

    let mut compiler =
        match Compiler::from_registry_files("registry/language.json", "registry/engines.json") {
            Ok(c) => c,
            Err(_) => {
                eprintln!("{} Failed to load registries.", "Critical:".red());
                return;
            }
        };

    println!("{} Compiling project...", "🔨".yellow());
    let bytecode_module = match compiler.compile_to_bytecode(&file) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{} Compilation failed: {}", "Error:".red(), e);
            return;
        }
    };

    println!(
        "{} Application ready. Found {} entry points.",
        "✓".green(),
        bytecode_module.functions.len()
    );

    // --- Industrial Debugger State ---
    let bpm = Arc::new(Mutex::new(BreakpointManager::new()));
    let inspector = Arc::new(Mutex::new(VariableInspector::new()));
    let runtime = Arc::new(Mutex::new(DebugRuntime::new()));

    let mut config = VmConfig::default();
    config.debug = true;
    config.enable_jit = false;

    let bpm_clone = Arc::clone(&bpm);
    let runtime_clone = Arc::clone(&runtime);
    let _inspector_clone = Arc::clone(&inspector);
    let file_clone = file.clone();

    config.on_step = Some(Box::new(move |vm, instr, _ip| {
        let mut rt = runtime_clone.lock().unwrap();
        let mut bpm = bpm_clone.lock().unwrap();

        let file_path = file_clone.to_str().unwrap_or("unknown");
        let current_line = instr.line;

        // 1. Check Breakpoints
        let is_breakpoint = bpm.has_breakpoint_at(file_path, current_line);
        let is_stepping = rt.state == RuntimeState::Stepping;

        if is_breakpoint || is_stepping {
            if is_breakpoint {
                println!("\n{} Hit breakpoint at line {}", "🛑".red(), current_line);
            }
            rt.state = RuntimeState::Paused;
            rt.set_line(current_line);

            // 2. Interactive Loop
            loop {
                print!("{}", "(nyx-db) > ".bold().yellow());
                std::io::Write::flush(&mut std::io::stdout()).unwrap();

                let mut input = String::new();
                std::io::stdin().read_line(&mut input).unwrap();
                let cmd = input.trim();

                match cmd {
                    "c" | "continue" => {
                        rt.state = RuntimeState::Running;
                        break;
                    }
                    "s" | "step" => {
                        rt.state = RuntimeState::Stepping;
                        break;
                    }
                    "bt" | "backtrace" => {
                        for frame in &vm.runtime().frames {
                            println!("  {} at line {}", frame.function.name, frame.ip);
                        }
                    }
                    "l" | "locals" => {
                        // Capture locals from VM stack (Simplified for hardening)
                        println!("\n{}", "--- LOCAL SCOPE ---".bold().cyan());
                        if let Some(frame) = vm.runtime().frames.last() {
                            println!("{} {}", "Function:".dimmed(), frame.function.name.bold());
                        }
                        println!("{} {:?}", "Stack Top:".dimmed(), vm.runtime().stack.last());
                    }
                    "src" | "list" => {
                        println!("\n{}", "--- SOURCE CONTEXT ---".bold().cyan());
                        if let Ok(source) = std::fs::read_to_string(file_path) {
                            let lines: Vec<&str> = source.lines().collect();
                            let start = current_line.saturating_sub(3);
                            let end = (current_line + 3).min(lines.len());
                            for i in start..end {
                                let line_idx = i + 1;
                                if line_idx == current_line {
                                    println!(
                                        "{} | {}",
                                        format!("{:>3} ►", line_idx).yellow().bold(),
                                        lines[i].bold()
                                    );
                                } else {
                                    println!(
                                        "{} | {}",
                                        format!("{:>3}  ", line_idx).dimmed(),
                                        lines[i].dimmed()
                                    );
                                }
                            }
                        } else {
                            println!("{} Could not read source.", "Error:".red());
                        }
                    }
                    "h" | "help" => {
                        println!("\n{}", "AVAILABLE COMMANDS:".bold().cyan());
                        println!("  {} - Continue execution", "c, continue".green());
                        println!("  {} - Step to next instruction", "s, step".green());
                        println!("  {} - Show stack backtrace", "bt, backtrace".green());
                        println!("  {} - Inspect local variables", "l, locals".green());
                        println!("  {} - Show source context", "src, list".green());
                        println!("  {} - Set breakpoint at line N", "b <N>".green());
                        println!("  {} - Quit session", "q, quit".red());
                    }
                    "q" | "quit" => {
                        std::process::exit(0);
                    }
                    _ => {
                        if cmd.starts_with("b ") {
                            if let Ok(line) = cmd[2..].parse::<usize>() {
                                bpm.add_line_breakpoint(file_path.to_string(), line);
                                println!("Breakpoint set at line {}", line);
                            }
                        } else {
                            println!("Unknown command. Type 'help' for options.");
                        }
                    }
                }
            }
        }

        Ok(())
    }));

    let mut vm = nyx_vm::NyxVm::new(config);
    vm.load(bytecode_module);

    // Register standard IO
    vm.register("print", 1, |args| {
        if let Some(arg) = args.first() {
            println!("{}", arg.to_string());
        }
        Ok(nyx_vm::bytecode::Value::Null)
    });

    println!(
        "{}",
        "\n🚀 Starting execution... (Type 'h' for help when paused)".cyan()
    );
    runtime.lock().unwrap().start("main");

    match vm.run("main") {
        Ok(v) => println!("\n{} Program exited with: {:?}", "🏁".green(), v),
        Err(e) => eprintln!("\n{} Execution Trap: {:?}", "💥".red(), e),
    }

    println!("\n{}", "Debug session finished.".dimmed());
}
