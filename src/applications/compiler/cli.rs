use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::applications::compiler::build_system::{build_deterministic, init_project, BuildTarget, DeterministicBuildOptions};
use crate::applications::compiler::compiler_main::{CompileOptions, Compiler};
use crate::runtime::execution::nyx_vm::{format_eval_error as vm_format_eval_error, to_stringish as vm_to_stringish, Value as VmValue};
use crate::runtime::execution::ui_runtime::{execute_app as execute_vm_app, execute_bytecode_app};
use crate::systems::backend::llvm_backend::Target;
// use nyx_hypervisor::cli::Cli as HypervisorCli;

#[derive(Debug, Parser)]
#[command(
    name = "nyx",
    about = "Nyx compiler CLI",
    long_about = "Nyx Compiler CLI\n\n\
        The ultimate toolchain for the Nyx Programming Language.\n\
        You can execute files directly: `nyx app.nyx`\n\
        Or use subcommands for specific tasks, e.g. `nyx web dev`."
)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,

    /// Compatibility positional for 'nyx -- db shell' syntax
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, value_name = "ARGS")]
    _extra: Vec<String>,
}

#[derive(Debug, Subcommand)]
enum Command {
    New {
        project: String,
    },
    Build {
        #[arg(default_value = "examples/hello_world.nyx")]
        input: PathBuf,
    },
    Run {
        #[arg(default_value = "examples/hello_world.nyx")]
        input: PathBuf,
        /// Disable auto-opening the browser when running a web server.
        #[arg(long = "no-open", default_value_t = false)]
        no_open: bool,
        /// Disable file watching / hot-reload restarts for web servers.
        #[arg(long = "no-watch", default_value_t = false)]
        no_watch: bool,
        /// Execution target (ast or vm).
        #[arg(long, default_value = "ast")]
        target: String,
    },
    Compile {
        #[arg(default_value = "examples/hello_world.nyx")]
        input: PathBuf,
        #[arg(long, default_value = "x86_64")]
        target: String,
    },
    /// Ahead-Of-Time (AOT) bare-metal OS compilation (ELF)
    BuildOs {
        #[arg(default_value = "kernel.nyx")]
        input: PathBuf,
        #[arg(long, default_value = "native/x86_64-nyx-os.ld")]
        linker_script: PathBuf,
        #[arg(long, default_value = "build")]
        out_dir: PathBuf,
    },
    Check {
        #[arg(default_value = "examples/hello_world.nyx")]
        input: PathBuf,
    },
    /// Check all .nyx files under a directory (defaults to engines/)
    CheckAll {
        #[arg(default_value = "engines")]
        path: PathBuf,
    },
    Format {
        #[arg(default_value = "examples/hello_world.nyx")]
        input: PathBuf,
    },
    #[command(
        about = "UI engine tools",
        long_about = "UI Engine Runner & Tools\n\n\
            The `ui` command allows you to execute and build Nyx applications that \
            utilize the STDLIB / ui engine.\n\
            \n\
            Usage Examples:\n\
            - `nyx ui run app.nyx` (Runs a UI application window)\n\
            - `nyx ui test` (Runs the internal UI test suite)"
    )]
    Ui {
        #[command(subcommand)]
        cmd: UiCommand,
    },
    #[command(
        about = "Web development tools",
        long_about = "Web Development Backend & CLI\n\n\
            The `web` command provides tools for building and serving Nyx web applications.\n\
            \n\
            Usage Examples:\n\
            - `nyx web dev site.nyx` (Starts the dev server with hot reload)\n\
            - `nyx web run site.nyx` (Alias for `dev`)\n\
            - `nyx web build site.nyx --out-dir dist` (Compiles the app into a static folder)\n\
            \n\
            Required STDLIB Modules:\n\
            Ensure your app imports STDLIB / web / router and STDLIB / web / server."
    )]
    Web {
        #[command(subcommand)]
        cmd: WebCommand,
    },
    // /// Run the Nyx Hypervisor (VMM)
    // Hypervisor(HypervisorCli),
    Export {
        #[arg(default_value = "main.nyx")]
        input: PathBuf,
        #[arg(long, default_value = "exe")]
        format: String,
        #[arg(long, default_value = "build")]
        out_dir: PathBuf,
    },
    /// Database engine tools
    Db {
        #[command(subcommand)]
        cmd: DbCommand,
    },
    /// Alias for `db server`
    Server {
        #[arg(long, default_value = "9090")]
        port: u16,
    },
    /// Alias for `db shell`
    Shell {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value = "9090")]
        port: u16,
    },
}

#[derive(Debug, Subcommand)]
pub enum DbCommand {
    /// Start the Nyx Arrow-over-TCP server.
    Server {
        #[arg(long, default_value = "9090")]
        port: u16,
    },
    /// Start the interactive Nyx Shell.
    Shell {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value = "9090")]
        port: u16,
    },
}

#[derive(Debug, Subcommand)]
enum UiCommand {
    /// Run a UI application in a native window.
    Run {
        #[arg(default_value = "site.nyx")]
        input: Option<PathBuf>,
    },
    /// Start a dev environment for a UI app.
    Dev {
        #[arg(default_value = "site.nyx")]
        input: Option<PathBuf>,
    },
    /// Build a UI app.
    Build {
        #[arg(default_value = "site.nyx")]
        input: Option<PathBuf>,
    },
    /// Run the UI engine internal test suite.
    Test {
        input: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
#[command(
    about = "Web development tools",
    long_about = "Web Development Backend & CLI\n\n\
        The `web` command provides tools for building and serving Nyx web applications.\n\
        \n\
        Usage Examples:\n\
        - `nyx web dev site.nyx` (Starts the dev server with hot reload)\n\
        - `nyx web run site.nyx` (Alias for `dev`)\n\
        - `nyx web build site.nyx --out-dir dist` (Compiles the app into a static folder)\n\
        \n\
        Required STDLIB Modules:\n\
        Ensure your app imports STDLIB / web / router and STDLIB / web / server."
)]
enum WebCommand {
    /// Run a development server that renders `App()` and supports hot reload.
    #[command(alias = "run")]
    Dev {
        #[arg(default_value = "site.nyx")]
        input: PathBuf,
        #[arg(long, default_value = "0.0.0.0")]
        host: String,
        #[arg(long)]
        port: Option<u16>,
        /// Disable auto-opening the browser.
        #[arg(long = "no-open", default_value_t = false)]
        no_open: bool,
        /// Enable hot reload.
        #[arg(long = "hot-reload", default_value_t = false)]
        hot_reload: bool,
    },
    /// Build a static `dist/` folder for deployment (generates `index.html`).
    Build {
        #[arg(default_value = "site.nyx")]
        input: PathBuf,
        #[arg(long, default_value = "dist")]
        out_dir: PathBuf,
    },
    /// Serve a pre-built static directory (production-ready).
    Serve {
        #[arg(default_value = "dist")]
        dir: PathBuf,
        #[arg(long, default_value = "0.0.0.0")]
        host: String,
        #[arg(long, default_value = "8080")]
        port: u16,
    },
}

pub async fn run() -> Result<(), String> {
    let mut args = Args::parse();
    
    // Compatibility: If 'nyx -- db shell' was used, args.command will be None
    // but args._extra will contain ["db", "shell", ...].
    if args.command.is_none() && !args._extra.is_empty() {
        let mut fake_args = vec!["nyx".to_string()];
        fake_args.extend(args._extra);
        args = Args::parse_from(fake_args);
    }

    let mut compiler = Compiler::new()?;

    match args.command {
        None => {
            // No command: print help and exit
            use clap::CommandFactory;
            let mut cmd = Args::command();
            cmd.print_help().map_err(|e| e.to_string())?;
        }
        Some(Command::New { project }) => {
            init_project(PathBuf::from(project).as_path())?;
            println!("project created");
        }
        Some(Command::Build { input }) => {
            let source = std::fs::read_to_string(&input).map_err(|e| e.to_string())?;
            if is_probably_http_server_app(&source) {
                let manifest = build_deterministic(DeterministicBuildOptions {
                    input,
                    out_dir: PathBuf::from("dist"),
                    target: BuildTarget::Web,
                })?;
                println!("built target {}", manifest.target);
                return Ok(());
            }

            if source.contains("fn App") || source.contains("fn main") {
                let manifest = build_deterministic(DeterministicBuildOptions {
                    input,
                    out_dir: PathBuf::from("dist"),
                    target: BuildTarget::Linux,
                })?;
                println!("built target {}", manifest.target);
                println!("built static site to dist");
                return Ok(());
            }

            let output = compiler.compile(CompileOptions {
                input,
                output_dir: PathBuf::from("build"),
                module_name: "main".to_string(),
                target: Target::X86_64,
                emit_binary: false,
                is_shared: false,
                linker_script: None,
            })?;
            println!("LLVM IR: {}", output.llvm_ir_path);
        }
        Some(Command::Run {
            input,
            no_open,
            no_watch,
            target,
        }) => {
            let source = std::fs::read_to_string(&input).map_err(|e| e.to_string())?;
            if is_probably_http_server_app(&source) {
                let port = detect_port(&source).unwrap_or(8000);
                return crate::applications::compiler::web_preview::dev(
                    crate::applications::compiler::web_preview::WebDevOptions {
                        input,
                        host: "0.0.0.0".to_string(),
                        port,
                        open_browser: !no_open,
                        hot_reload: !no_watch,
                    },
                );
            }

            let target = parse_target(&target)?;
            let value = if matches!(target, Target::Bytecode) {
                execute_bytecode_app(&input).map_err(|e| vm_format_eval_error(&e))?
            } else {
                execute_vm_app(&input).map_err(|e| vm_format_eval_error(&e))?
            };

            if !matches!(value, VmValue::Null) {
                println!("{}", vm_to_stringish(&value));
            }
        }
        Some(Command::Compile { input, target }) => {
            let target = parse_target(&target)?;
            let output = compiler.compile(CompileOptions {
                input,
                output_dir: PathBuf::from("build"),
                module_name: "main".to_string(),
                target,
                emit_binary: true,
                is_shared: false,
                linker_script: None,
            })?;
            println!("LLVM IR: {}", output.llvm_ir_path);
            if let Some(bin) = output.binary_path {
                println!("Binary: {bin}");
            }
        }
        Some(Command::BuildOs { input, linker_script, out_dir }) => {
            let output = compiler.compile(CompileOptions {
                input,
                output_dir: out_dir,
                module_name: "kernel".to_string(),
                target: Target::FreestandingX86_64,
                emit_binary: true,
                is_shared: false,
                linker_script: Some(linker_script),
            })?;
            println!("SUCCESS: Bare-metal OS kernel built at {}", output.binary_path.unwrap_or_default());
        }
        Some(Command::Check { input }) => {
            let source = std::fs::read_to_string(input).map_err(|e| e.to_string())?;
            compiler.check(&source)?;
            println!("ok");
        }
        Some(Command::CheckAll { path }) => {
            let files = collect_nyx_files(&path)?;
            if files.is_empty() {
                return Err(format!("no .nyx files found under {}", path.display()));
            }

            let mut failures = Vec::new();
            for file in files {
                let source = std::fs::read_to_string(&file).map_err(|e| e.to_string())?;
                if let Err(err) = compiler.check(&source) {
                    failures.push((file, err));
                }
            }

            if failures.is_empty() {
                println!("ok");
                return Ok(());
            }

            for (file, err) in failures {
                eprintln!("{}: {}", file.display(), err);
            }
            return Err("one or more .nyx files failed to check".to_string());
        }
        Some(Command::Format { input }) => {
            let source = std::fs::read_to_string(&input).map_err(|e| e.to_string())?;
            let formatted = source
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            std::fs::write(input, format!("{formatted}\n")).map_err(|e| e.to_string())?;
            println!("formatted");
        }
        Some(Command::Ui { cmd }) => match cmd {
            UiCommand::Run { input } => {
                let path = input.unwrap_or_else(|| PathBuf::from("site.nyx"));
                println!("UI runtime: executing {}", path.display());
                let value = execute_vm_app(&path).map_err(|e| vm_format_eval_error(&e))?;
                if !matches!(value, VmValue::Null) {
                    println!("{}", vm_to_stringish(&value));
                }
            }
            UiCommand::Dev { input } => {
                let path = input.unwrap_or_else(|| PathBuf::from("site.nyx"));
                let source = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
                let port = detect_port(&source).unwrap_or(8000);
                return crate::applications::compiler::web_preview::dev(
                    crate::applications::compiler::web_preview::WebDevOptions {
                        input: path,
                        host: "0.0.0.0".to_string(),
                        port,
                        open_browser: true,
                        hot_reload: true,
                    },
                );
            }
            UiCommand::Build { input } => {
                let path = input.unwrap_or_else(|| PathBuf::from("site.nyx"));
                let out_dir = PathBuf::from("dist");
                crate::applications::compiler::web_preview::build(
                    crate::applications::compiler::web_preview::WebBuildOptions {
                        input: path,
                        out_dir: out_dir.clone(),
                    },
                )?;
                println!("built static site to {}", out_dir.display());
            }
            UiCommand::Test { input } => {
                let path = input.unwrap_or_else(|| {
                    PathBuf::from("engines/ui_engine/tests/ui_tests.nyx")
                });
                println!("UI tests: executing {}", path.display());
                let value = execute_vm_app(&path).map_err(|e| vm_format_eval_error(&e))?;
                if !matches!(value, VmValue::Null) {
                    println!("{}", vm_to_stringish(&value));
                }
            }
        },
        Some(Command::Web { cmd }) => match cmd {
            WebCommand::Dev {
                input,
                host,
                port,
                no_open,
                hot_reload,
            } => {
                let source = std::fs::read_to_string(&input).map_err(|e| e.to_string())?;
                let port = port.or_else(|| detect_port(&source)).unwrap_or(8000);
                return crate::applications::compiler::web_preview::dev(
                    crate::applications::compiler::web_preview::WebDevOptions {
                        input,
                        host,
                        port,
                        open_browser: !no_open,
                        hot_reload,
                    },
                );
            }
            WebCommand::Build { input, out_dir } => {
                let out_display = out_dir.display().to_string();
                crate::applications::compiler::web_preview::build(
                    crate::applications::compiler::web_preview::WebBuildOptions { 
                        input, 
                        out_dir 
                    },
                )?;
                println!("built static site to {}", out_display);
            }
            WebCommand::Serve { dir, host, port } => {
                return crate::applications::compiler::web_preview::serve(
                    crate::applications::compiler::web_preview::WebDevOptions {
                        input: dir,
                        host,
                        port,
                        open_browser: false,
                        hot_reload: false,
                    },
                );
            }
        },
        Some(Command::Export { input, format, out_dir }) => {
            let is_shared = matches!(format.as_str(), "dll" | "so" | "dylib");
            let target = Target::X86_64; // Default to host, can be expanded later
            let module_name = input.file_stem().unwrap_or_else(|| std::ffi::OsStr::new("main")).to_string_lossy().to_string();
            
            println!("Exporting {} to {} (format: {})...", input.display(), out_dir.display(), format);
            
            let output = compiler.compile(CompileOptions {
                input,
                output_dir: out_dir,
                module_name,
                target,
                emit_binary: true,
                is_shared,
                linker_script: None,
            })?;
            
            if let Some(bin) = output.binary_path {
                let bin_path = std::path::Path::new(&bin);
                let mut final_path = bin_path.to_path_buf();
                match format.as_str() {
                    "exe" => { final_path.set_extension("exe"); }
                    "dll" => { final_path.set_extension("dll"); }
                    "so" => { final_path.set_extension("so"); }
                    "bin" => { final_path.set_extension("bin"); }
                    _ => {}
                }
                
                if final_path != bin_path {
                    std::fs::rename(bin_path, &final_path).map_err(|e| e.to_string())?;
                }
                println!("SUCCESS: Exported to {}", final_path.display());
            } else {
                return Err("Compilation failed to produce a binary output.".to_string());
            }
        }
        Some(Command::Db { cmd }) => match cmd {
            DbCommand::Server { port } => {
                println!("Starting Nyx-Server on port {}...", port);
                crate::runtime::execution::nyx_server::NyxServer::start(port).await
                    .map_err(|e| e.to_string())?;
            }
            DbCommand::Shell { host, port } => {
                crate::runtime::execution::nyx_shell_client::run_shell(&host, port).await
                    .map_err(|e| e.to_string())?;
            }
        },
        Some(Command::Server { port }) => {
            println!("Starting Nyx-Server on port {}...", port);
            crate::runtime::execution::nyx_server::NyxServer::start(port).await
                .map_err(|e| e.to_string())?;
        }
        Some(Command::Shell { host, port }) => {
            crate::runtime::execution::nyx_shell_client::run_shell(&host, port).await
                .map_err(|e| e.to_string())?;
        }
    }

    let _engines = compiler.discover_engines();
    Ok(())
}

fn collect_nyx_files(path: &std::path::Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();

    if !path.exists() {
        return Err(format!("path does not exist: {}", path.display()));
    }

    if path.is_file() {
        if path.extension().and_then(|s| s.to_str()) == Some("nyx") {
            files.push(path.to_path_buf());
        }
        return Ok(files);
    }

    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let dir_name = dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if dir_name.starts_with('.') || dir_name == "target" || dir_name == "node_modules" {
            continue;
        }

        for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let entry_path = entry.path();
            if entry_path.is_dir() {
                stack.push(entry_path);
            } else if entry_path.extension().and_then(|s| s.to_str()) == Some("nyx") {
                files.push(entry_path);
            }
        }
    }

    files.sort();
    Ok(files)
}

fn parse_target(name: &str) -> Result<Target, String> {
    match name {
        "x86_64" => Ok(Target::X86_64),
        "arm64" | "aarch64" => Ok(Target::AArch64),
        "riscv" | "riscv64" => Ok(Target::RiscV64),
        "wasm" | "wasm32" => Ok(Target::Wasm32),
        "browser" | "browserjs" => Ok(Target::BrowserJs),
        "vm" | "bytecode" | "nyxb" => Ok(Target::Bytecode),
        "ast" => Ok(Target::Ast),
        _ => Err(format!("unsupported target '{name}'")),
    }
}

fn is_probably_http_server_app(source: &str) -> bool {
    source.contains("fn App") || source.contains("fn app")
}

fn detect_port(source: &str) -> Option<u16> {
    // Common patterns in Nyx examples:
    // - let port: u16 = 8000;
    // - let port = 8000;
    // - config.port = 8000;
    // - "...localhost:8000"
    fn parse_first_u16_after(haystack: &str, needle: &str) -> Option<u16> {
        let idx = haystack.find(needle)?;
        let after = &haystack[idx + needle.len()..];
        let eq_idx = after.find('=')?;
        let after_eq = &after[eq_idx + 1..];
        let digits: String = after_eq
            .chars()
            .skip_while(|c| c.is_whitespace())
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if digits.is_empty() {
            None
        } else {
            digits.parse::<u16>().ok()
        }
    }

    parse_first_u16_after(source, "let port")
        .or_else(|| parse_first_u16_after(source, "config.port"))
        .or_else(|| {
            let idx = source.find("localhost:")?;
            let after = &source[idx + "localhost:".len()..];
            let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
            if digits.is_empty() {
                None
            } else {
                digits.parse::<u16>().ok()
            }
        })
}
