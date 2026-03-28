use clap::{Parser, Subcommand};
use colored::*;
use nyx_cli::dispatch_tool;
use nyx_cli::commands;
use std::process::exit;
use std::panic;

#[derive(Parser, Debug)]
#[command(name = "nyx")]
#[command(about = "Nyx Language Toolchain CLI", long_about = None)]
#[command(version)]
struct NyxCli {
    #[command(subcommand)]
    command: NyxCommands,
}

#[derive(Subcommand, Debug)]
enum NyxCommands {
    /// Format Nyx source code
    Format {
        /// Files or directories to format
        #[arg(value_name = "PATH", default_values = ["."])]
        paths: Vec<String>,
        /// Check mode: report unformatted files without writing
        #[arg(long, short = 'c')]
        check: bool,
        /// Show a unified diff instead of writing files
        #[arg(long, short = 'd')]
        diff: bool,
        /// Indentation width in spaces
        #[arg(long, default_value = "4")]
        indent: usize,
        /// Target maximum line length
        #[arg(long, default_value = "100")]
        max_line: usize,
        /// Suppress all output except errors
        #[arg(long, short = 'q')]
        quiet: bool,
        /// Verbose output
        #[arg(long, short = 'v')]
        verbose: bool,
    },
    /// Lint Nyx source code
    Lint {
        /// Files to lint
        files: Vec<String>,
    },
    /// Debug Nyx source code
    Debug {
        /// Subcommand for debugger (run, break, list)
        #[arg(default_value = "run")]
        cmd: String,
        /// File to debug
        file: Option<String>,
    },
    /// Generate documentation from Nyx source code
    Doc {
        /// Project directory to document
        #[arg(default_value = ".")]
        dir: String,
    },
    /// Run the Nyx Language Server Protocol
    Lsp,
    /// Run the Security Scanner on a Nyx codebase
    Security {
        /// Scan, dependencies, secrets, update-db, db-info
        cmd: String,
        /// Path to scan
        #[arg(default_value = ".")]
        path: String,
    },
    /// Run the Nyx Profiler
    Profile {
        /// Run, attach
        #[arg(default_value = "run")]
        cmd: String,
        /// File to profile
        file: Option<String>,
    },
    /// UI Toolchain
    Ui {
        /// new, run, build, dev, test, web, mobile, game, generate
        cmd: String,
        /// Arguments
        args: Vec<String>,
    },
    /// Collab Toolchain
    Ecosystem {
        /// insights, health
        cmd: String,
        /// Arguments
        args: Vec<String>,
    },
    /// AI Toolchain
    Ai {
        /// generate, optimize, debug, docs, testgen, analyze, complete
        cmd: String,
        /// Arguments
        args: Vec<String>,
    },
    /// Start the interactive Nyx REPL
    Repl,
    /// View disassembled Bytecode of a Nyx file
    CatBc {
        /// File to disassemble
        file: String,
    },
    /// Explore the Abstract Syntax Tree (AST) of a Nyx file
    Ast {
        /// File to explore
        file: String,
        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Benchmark a Nyx file (Deterministic)
    Bench {
        /// File to benchmark
        file: String,
        /// Number of iterations
        #[arg(short, long, default_value = "10")]
        iterations: usize,
    },
    /// Generate visual call graph (Mermaid)
    Flow {
        /// File to analyze
        file: String,
    },
    /// Check ecosystem health
    Doctor,
    /// Autonomous Optimization and Tuning
    Tune {
        /// build, health, optimize
        #[arg(default_value = "health")]
        cmd: String,
    },
    /// Start the Nyx Nexus Executive Dashboard
    Nexus {
        /// Port to run the server on
        #[arg(short, long, default_value = "3000")]
        port: u16,
        /// Project root directory
        #[arg(short, long, default_value = ".")]
        root: String,
        /// Do not open browser automatically
        #[arg(long)]
        no_open: bool,
        /// Enable verbose logging
        #[arg(long, short = 'v')]
        verbose: bool,
    },
    /// Verify cache integrity
    Verify,
    /// Run a Nyx source file
    Run {
        /// File to run
        file: String,
        /// Arguments to pass to the program
        #[arg(last = true)]
        args: Vec<String>,
    },
}

fn main() {
    let result = panic::catch_unwind(|| {
        run();
    });

    if let Err(_) = result {
        eprintln!("\n{}", "╔═══════════════════════════════════════════════════════════╗".red().bold());
        eprintln!("{}", "║ CRITICAL INTERNAL ERROR: Nyx Toolchain Sentinel Fault     ║".red().bold());
        eprintln!("{}", "╠═══════════════════════════════════════════════════════════╣".red().bold());
        eprintln!("║ {} The toolchain encountered an unexpected panic.          ║", "⚠".yellow());
        eprintln!("║ {} Suggestion: Run 'nyx doctor' to repair environment.    ║", "🛰".cyan());
        eprintln!("{}", "╚═══════════════════════════════════════════════════════════╝".red().bold());
        exit(101);
    }
}

fn run() {
    let cli = NyxCli::parse();
    
    match &cli.command {
        NyxCommands::Format { paths, check, diff, indent, max_line, quiet, verbose } => {
            let mut args = Vec::new();
            if *check { args.push("--check"); }
            if *diff { args.push("--diff"); }
            let indent_s = indent.to_string();
            args.push("--indent");
            args.push(&indent_s);
            let max_line_s = max_line.to_string();
            args.push("--max-line");
            args.push(&max_line_s);
            if *quiet { args.push("--quiet"); }
            if *verbose { args.push("--verbose"); }
            
            for path in paths {
                args.push(path.as_str());
            }
            dispatch_tool("tools/formatter/Cargo.toml", "nyx_formatter", args);
        }
        NyxCommands::Lint { files } => {
            dispatch_tool("tools/linter/Cargo.toml", "nyx_linter", files.iter().map(|s| s.as_str()).collect());
        }
        NyxCommands::Debug { cmd, file } => {
            let mut args = vec![cmd.as_str()];
            if let Some(f) = file { args.push(f.as_str()); }
            dispatch_tool("tools/debugger/Cargo.toml", "nyx-debugger", args);
        }
        NyxCommands::Doc { dir } => {
            dispatch_tool("tools/docgen/Cargo.toml", "nyx_docgen", vec![dir.as_str()]);
        }
        NyxCommands::Lsp => {
            dispatch_tool("tools/lsp/Cargo.toml", "nyx-lsp", vec![]);
        }
        NyxCommands::Security { cmd, path } => {
            dispatch_tool("tools/security/Cargo.toml", "nyx-security", vec![cmd.as_str(), path.as_str()]);
        }
        NyxCommands::Profile { cmd, file } => {
            let mut args = vec![cmd.as_str()];
            if let Some(f) = file { args.push(f.as_str()); }
            dispatch_tool("tools/profiler/Cargo.toml", "nyx-profiler", args);
        }
        NyxCommands::Ui { cmd, args } => {
            let mut all_args = vec![cmd.as_str()];
            all_args.extend(args.iter().map(|s| s.as_str()));
            dispatch_tool("engines/ui_engine/cli/Cargo.toml", "nyx-ui", all_args);
        }
        NyxCommands::Ecosystem { cmd, args } => {
            let (crate_path, bin_name) = if cmd == "insights" {
                ("collab/nyx-collab-cli/Cargo.toml", "nyx-collab")
            } else {
                ("autonomous/nyx-autotune/Cargo.toml", "nyx-autotune")
            };
            let tool_cmd = if cmd == "insights" { "ecosystem-insights" } else { "ecosystem-health" };
            let mut all_args = vec![tool_cmd];
            all_args.extend(args.iter().map(|s| s.as_str()));
            dispatch_tool(crate_path, bin_name, all_args);
        }
        NyxCommands::Ai { cmd, args } => {
            let mut all_args = vec![cmd.as_str()];
            all_args.extend(args.iter().map(|s| s.as_str()));
            dispatch_tool("ai/nyx-ai/Cargo.toml", "nyx-ai", all_args);
        }
        NyxCommands::Repl => {
            dispatch_tool("tools/repl/Cargo.toml", "nyx-repl", vec![]);
        }
        NyxCommands::CatBc { file } => {
            dispatch_tool("tools/viewer/Cargo.toml", "nyx-viewer", vec![file.as_str()]);
        }
        NyxCommands::Ast { file, format } => {
            dispatch_tool("tools/ast-explorer/Cargo.toml", "nyx-ast-explorer", vec!["--format", format.as_str(), file.as_str()]);
        }
        NyxCommands::Bench { file, iterations } => {
            let iter_s = iterations.to_string();
            dispatch_tool("tools/bench/Cargo.toml", "nyx-bench", vec!["--iterations", &iter_s, file.as_str()]);
        }
        NyxCommands::Flow { file } => {
            dispatch_tool("tools/flow/Cargo.toml", "nyx-flow", vec![file.as_str()]);
        }
        NyxCommands::Doctor => {
            dispatch_tool("tools/doctor/Cargo.toml", "nyx-doctor", vec![]);
        }
        NyxCommands::Tune { cmd } => {
            dispatch_tool("autonomous/nyx-autotune/Cargo.toml", "nyx-autotune", vec![cmd.as_str()]);
        }
        NyxCommands::Nexus { port, root, no_open, verbose } => {
            let port_s = port.to_string();
            let mut args = vec!["--port", &port_s, "--root", root.as_str()];
            if *no_open { args.push("--no-open"); }
            if *verbose { args.push("--verbose"); }
            dispatch_tool("tools/nexus/Cargo.toml", "nyx-nexus", args);
        }
        NyxCommands::Verify => {
            if let Err(e) = commands::verify::execute() {
                eprintln!("{} {}", "error:".red().bold(), e);
                exit(1);
            }
        }
        NyxCommands::Run { file, args } => {
            let mut all_args = vec!["run", file.as_str()];
            all_args.extend(args.iter().map(|s| s.as_str()));
            dispatch_tool("Cargo.toml", "nyx", all_args);
        }
    }
}
