use clap::Parser;
use std::fs;
use std::path::PathBuf;
use std::collections::HashMap;
use std::time::Instant;
use nyx_vm::VmConfig;
use nyx::applications::compiler::compiler_main::Compiler;

#[derive(Parser, Debug)]
#[command(name = "nyx-profiler")]
#[command(about = "Nyx Execution Profiler", long_about = None)]
#[command(version)]
struct Cli {
    /// Command to profile
    #[arg(required = true)]
    command: String,

    /// Nyx file to profile
    file: PathBuf,
}

struct ProfileData {
    call_counts: HashMap<String, usize>,
    instruction_counts: HashMap<String, usize>,
}

impl ProfileData {
    fn new() -> Self {
        Self {
            call_counts: HashMap::new(),
            instruction_counts: HashMap::new(),
        }
    }
    
    fn print_report(&self, total_duration: std::time::Duration) {
        println!("============================================================");
        println!("                    NYX PROFILER REPORT                     ");
        println!("============================================================");
        println!("Total Execution Time: {:.2}ms", total_duration.as_secs_f64() * 1000.0);
        println!();
        
        println!("{:<30} | {:<12} | {:<12}", "Function", "Calls", "Instructions");
        println!("{:-<30}-+-{:-<12}-+-{:-<12}", "", "", "");
        
        let mut funcs: Vec<_> = self.call_counts.keys().collect();
        funcs.sort_by_key(|k| std::cmp::Reverse(self.instruction_counts.get(*k).unwrap_or(&0)));
        
        for func in funcs {
            let calls = self.call_counts.get(func).unwrap_or(&0);
            let instrs = self.instruction_counts.get(func).unwrap_or(&0);
            println!("{:<30} | {:<12} | {:<12}", func, calls, instrs);
        }
        
        println!("============================================================");
    }
}

fn main() {
    let cli = Cli::parse();

    if cli.command != "run" {
        eprintln!("Unsupported command: {}", cli.command);
        return;
    }

    if !cli.file.exists() {
        eprintln!("Error: File not found: {}", cli.file.display());
        std::process::exit(1);
    }

    let _source = fs::read_to_string(&cli.file).unwrap_or_else(|e| {
        eprintln!("Error reading file: {}", e);
        std::process::exit(1);
    });

    // Compile
    println!("Compiling {}...", cli.file.display());
    let mut compiler = match Compiler::from_registry_files("registry/language.json", "registry/engines.json") {
         Ok(c) => c,
         Err(_) => {
             eprintln!("Warning: could not load registries, using default behavior.");
             std::process::exit(1);
         }
    };
    
    let bytecode_module = match compiler.compile_to_bytecode(&cli.file) {
        Ok(module) => module,
        Err(e) => {
            eprintln!("Compilation failed: {}", e);
            std::process::exit(1);
        }
    };
    
    // Setup profiler
    use std::sync::{Arc, Mutex};
    let profile_data = Arc::new(Mutex::new(ProfileData::new()));
    
    let mut config = VmConfig::default();
    config.debug = false;
    config.enable_jit = false; // We need to run interpreted to intercept instructions for now
    
    let data_clone = Arc::clone(&profile_data);
    config.on_step = Some(Box::new(move |vm, _instr, _ip| {
        let frames = &vm.runtime().frames;
        if let Some(frame) = frames.last() {
             let func_name = format!("{}::{}", frame.module_name, frame.function.name);
             let mut data = data_clone.lock().unwrap();
             
             // Count instructions
             *data.instruction_counts.entry(func_name.clone()).or_insert(0) += 1;
             
             // Count calls (only on first instruction to avoid overcounting)
             // Alternatively, we could hook call/return instructions.
             // For simplicity, we just count IP == 0 as a call
             if _ip == 0 {
                  *data.call_counts.entry(func_name).or_insert(0) += 1;
             }
        }
        Ok(())
    }));

    let mut vm = nyx_vm::NyxVm::new(config);
    vm.load(bytecode_module);
    
    vm.register("print", 1, |args| {
        if let Some(arg) = args.first() {
            println!("{}", arg.to_string());
        }
        Ok(nyx_vm::bytecode::Value::Null)
    });

    println!("Profiling started...");
    let start_time = Instant::now();
    
    match vm.run("main") {
        Ok(v) => {
             let duration = start_time.elapsed();
             println!("\nProgram exited normally with return {:?}", v);
             profile_data.lock().unwrap().print_report(duration);
        },
        Err(e) => {
             let duration = start_time.elapsed();
             println!("\nProgram trapped / panicked: {:?}", e);
             profile_data.lock().unwrap().print_report(duration);
        }
    }
}
