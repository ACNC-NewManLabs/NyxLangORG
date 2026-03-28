use clap::{Parser, Subcommand};
use colored::*;
use nyx_cli::commands;
use nyx_cli::surn_tools;
use std::process::exit;
use std::panic;

#[derive(Parser, Debug)]
#[command(name = "surn")]
#[command(about = "SURN Package Manager for Nyx", long_about = None)]
#[command(version)]
struct SurnCli {
    #[command(subcommand)]
    command: SurnCommands,
}

#[derive(Subcommand, Debug)]
enum SurnCommands {
    /// Create a new Nyx project
    New {
        /// Project name
        name: String,
        /// Create a library project
        #[arg(long)]
        lib: bool,
    },
    /// Add a dependency to load.surn
    Add {
        /// Package name
        package: String,
        /// Version requirement
        #[arg(long)]
        version: Option<String>,
        /// Git repository URL
        #[arg(long)]
        git: Option<String>,
        /// Local path
        #[arg(long)]
        path: Option<String>,
        /// Features to enable (comma separated)
        #[arg(long)]
        features: Option<String>,
    },
    /// Build the current project
    Build {
        /// Build in release mode
        #[arg(long)]
        release: bool,
    },
    /// Run the current project
    Run {
        /// Build in release mode
        #[arg(long)]
        release: bool,
        /// Arguments to pass to the binary
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Run tests
    Test {
        /// Run tests in release mode
        #[arg(long)]
        release: bool,
        /// Specific test to run
        test_name: Option<String>,
    },

    /// Display the dependency tree
    Tree,
    /// Remove build artifacts
    Clean {
        /// Remove only release artifacts
        #[arg(long)]
        release: bool,
    },

    /// Output project metadata in JSON format
    Metadata,

    /// Find the path to the load.surn manifest
    LocateProject,
    /// Validate the project structure
    VerifyProject,
    /// Initialize a new Nyx project in the current directory
    Init {
        /// Project name
        name: Option<String>,
        /// Create a library project
        #[arg(long)]
        lib: bool,
    },
    /// Check code for errors without producing a binary
    Check,
    /// Automatically fix code warnings
    Fix,
    /// Generate the load.bolt lockfile
    GenerateLockfile,
    /// Remove a dependency from load.surn
    Remove {
        /// Package to remove
        package: String,
    },
    /// Update dependencies in load.bolt
    Update,
    /// Resolve dependencies without downloading
    Resolve,
    /// Generate the load.bolt lockfile (alias for generate-lockfile)
    Lock,
    /// Download dependencies
    Fetch,
    /// Package the project for publishing
    Package,
    /// Publish the project to a registry
    Publish,
    /// Install a Nyx binary or project dependencies
    Install {
        /// Path to the binary or package name
        path: Option<String>,
        /// Use only cached packages — no network access
        #[arg(long)]
        offline: bool,
    },
    /// Uninstall a Nyx binary
    Uninstall {
        /// Name of the package to uninstall
        package: String,
    },

    /// SURN Tools
    #[command(about = "Format a SURN file")]
    Fmt { file: String },
    #[command(about = "Benchmark SURN parser")]
    Bench,
    Convert { path: String, target: String },
    /// Verify cache integrity
    Verify,
}

fn main() {
    let result = panic::catch_unwind(|| {
        run();
    });

    if let Err(_) = result {
        eprintln!("\n{}", "╔═══════════════════════════════════════════════════════════╗".red().bold());
        eprintln!("{}", "║ CRITICAL INTERNAL ERROR: SURN™ Sentinel Fault             ║".red().bold());
        eprintln!("{}", "╠═══════════════════════════════════════════════════════════╣".red().bold());
        eprintln!("║ {} The toolchain encountered an unexpected panic.          ║", "⚠".yellow());
        eprintln!("{}", "╚═══════════════════════════════════════════════════════════╝".red().bold());
        exit(101);
    }
}

fn run() {
    println!("{}", "SURN™ - The Industrial Package Manager for Nyx™".cyan().bold());
    println!("{}", "Copyright (c) 2026 Surya. All rights reserved.\n".dimmed());

    let cli = SurnCli::parse();
    
    match &cli.command {
        SurnCommands::New { name, lib } => {
            if let Err(e) = commands::new::execute(name.clone(), *lib) {
                eprintln!("{} {}", "error:".red().bold(), e);
                exit(1);
            }
        }
        SurnCommands::Add { package, version, git, path, features } => {
            if let Err(e) = commands::add::execute(package.clone(), version.clone(), git.clone(), path.clone(), features.clone()) {
                eprintln!("{} {}", "error:".red().bold(), e);
                exit(1);
            }
        }
        SurnCommands::Build { release } => {
            if let Err(e) = commands::build::execute(*release) {
                eprintln!("{} {}", "error:".red().bold(), e);
                exit(1);
            }
        }
        SurnCommands::Run { release, args } => {
            if let Err(e) = commands::run::execute(*release, args.clone()) {
                eprintln!("{} {}", "error:".red().bold(), e);
                exit(1);
            }
        }
        SurnCommands::Test { release, test_name } => {
            if let Err(e) = commands::test::execute(*release, test_name.clone()) {
                eprintln!("{} {}", "error:".red().bold(), e);
                exit(1);
            }
        }
        SurnCommands::Tree => {
            if let Err(e) = commands::tree::execute() {
                eprintln!("{} {}", "error:".red().bold(), e);
                exit(1);
            }
        }
        SurnCommands::Clean { release } => {
            if let Err(e) = commands::clean::execute(*release) {
                eprintln!("{} {}", "error:".red().bold(), e);
                exit(1);
            }
        }
        SurnCommands::Metadata => {
            if let Err(e) = commands::metadata::execute() {
                eprintln!("{} {}", "error:".red().bold(), e);
                exit(1);
            }
        }
        SurnCommands::LocateProject => {
            if let Err(e) = commands::locate_project::execute() {
                eprintln!("{} {}", "error:".red().bold(), e);
                exit(1);
            }
        }
        SurnCommands::VerifyProject => {
            if let Err(e) = commands::verify_project::execute() {
                eprintln!("{} {}", "error:".red().bold(), e);
                exit(1);
            }
        }
        SurnCommands::Init { name, lib } => {
            if let Err(e) = commands::init::execute(name.clone(), *lib) {
                eprintln!("{} {}", "error:".red().bold(), e);
                exit(1);
            }
        }
        SurnCommands::Check => {
            if let Err(e) = commands::check::execute() {
                eprintln!("{} {}", "error:".red().bold(), e);
                exit(1);
            }
        }
        SurnCommands::Fix => {
            if let Err(e) = commands::fix::execute() {
                eprintln!("{} {}", "error:".red().bold(), e);
                exit(1);
            }
        }
        SurnCommands::GenerateLockfile => {
            if let Err(e) = commands::generate_lockfile::execute() {
                eprintln!("{} {}", "error:".red().bold(), e);
                exit(1);
            }
        }
        SurnCommands::Remove { package } => {
            if let Err(e) = commands::remove::execute(package.clone()) {
                eprintln!("{} {}", "error:".red().bold(), e);
                exit(1);
            }
        }
        SurnCommands::Update => {
            if let Err(e) = commands::update::execute() {
                eprintln!("{} {}", "error:".red().bold(), e);
                exit(1);
            }
        }
        SurnCommands::Resolve => {
            if let Err(e) = commands::generate_lockfile::execute() {
                eprintln!("{} {}", "error:".red().bold(), e);
                exit(1);
            }
        }
        SurnCommands::Lock => {
            if let Err(e) = commands::generate_lockfile::execute() {
                eprintln!("{} {}", "error:".red().bold(), e);
                exit(1);
            }
        }
        SurnCommands::Fetch => {
            if let Err(e) = commands::fetch::execute() {
                eprintln!("{} {}", "error:".red().bold(), e);
                exit(1);
            }
        }
        SurnCommands::Package => {
            if let Err(e) = commands::package::execute() {
                eprintln!("{} {}", "error:".red().bold(), e);
                exit(1);
            }
        }
        SurnCommands::Publish => {
            if let Err(e) = commands::publish::execute() {
                eprintln!("{} {}", "error:".red().bold(), e);
                exit(1);
            }
        }
        SurnCommands::Install { path, offline } => {
            if let Err(e) = commands::install::execute(path.clone(), *offline) {
                eprintln!("{} {}", "error:".red().bold(), e);
                exit(1);
            }
        }
        SurnCommands::Uninstall { package } => {
            if let Err(e) = commands::uninstall::execute(package.clone()) {
                eprintln!("{} {}", "error:".red().bold(), e);
                exit(1);
            }
        }
        SurnCommands::Fmt { file } => {
            if let Err(e) = surn_tools::surn_fmt(&file) {
                eprintln!("{} {}", "error:".red().bold(), e);
                exit(1);
            }
            println!("{} {}", "formatted:".green().bold(), file);
        }
        SurnCommands::Bench => {
            nyx_cli::surn::benchmark::run_benchmark();
        }
        SurnCommands::Convert { path, target } => {
            match surn_tools::surn_convert(path, target) {
                Ok(json) => println!("{}", json),
                Err(e) => {
                    eprintln!("{} {}", "error:".red().bold(), e);
                    exit(1);
                }
            }
        }
        SurnCommands::Verify => {
            if let Err(e) = commands::verify::execute() {
                eprintln!("{} {}", "error:".red().bold(), e);
                exit(1);
            }
        }
    }
}
