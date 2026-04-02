use clap::Parser;
use colored::*;
use nyx::applications::compiler::compiler_main::Compiler;
use nyx_vm::{NyxVm, VmConfig};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(name = "nyx-bench")]
#[command(about = "Nyx Deterministic Benchmarking Suite", long_about = None)]
struct Args {
    /// Nyx file to benchmark
    file: PathBuf,

    /// Number of iterations (for wall-clock averaging)
    #[arg(short, long, default_value = "10")]
    iterations: usize,

    /// Output JSON instead of pretty-printed table
    #[arg(long)]
    json: bool,
}

#[derive(serde::Serialize)]
struct BenchResult {
    name: String,
    instructions: u64,
    avg_wall_clock_ns: u128,
    jitter_ns: u128,
    allocated_bytes: u64,
}

fn main() {
    let args = Args::parse();

    if !args.file.exists() {
        eprintln!("Error: File not found: {}", args.file.display());
        return;
    }

    let mut compiler =
        match Compiler::from_registry_files("registry/language.json", "registry/engines.json") {
            Ok(c) => c,
            Err(_) => {
                eprintln!("Error: Could not load compiler registries.");
                return;
            }
        };

    let module = match compiler.compile_to_bytecode(&args.file) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{}: {}", "Compilation Error".red(), e);
            return;
        }
    };

    println!(
        "{} Benchmark for: {}",
        "🌌".magenta(),
        args.file.display().to_string().bold()
    );
    println!(
        "{}",
        "============================================================".magenta()
    );
    println!(
        "{:<20} | {:>12} | {:>12} | {:>10} | {:>8}",
        "Function".bold(),
        "Instr".yellow(),
        "Avg Time".green(),
        "Jitter".red(),
        "Mem".blue()
    );

    let mut results = Vec::new();

    for func in &module.functions {
        if func.name.starts_with("bench_") || func.name == "main" {
            let result = run_bench(&module, &func.name, args.iterations);

            if !args.json {
                let jitter_pct = if result.avg_wall_clock_ns > 0 {
                    (result.jitter_ns as f64 / result.avg_wall_clock_ns as f64) * 100.0
                } else {
                    0.0
                };

                println!(
                    "{:<20} | {:>12} | {:>10}ns | {:>9.1}% | {:>7}B",
                    result.name.cyan(),
                    result.instructions,
                    result.avg_wall_clock_ns,
                    jitter_pct,
                    result.allocated_bytes
                );
            }
            results.push(result);
        }
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&results).unwrap());
    }
}

fn run_bench(
    module: &nyx_vm::bytecode::BytecodeModule,
    func_name: &str,
    iterations: usize,
) -> BenchResult {
    let mut times = Vec::with_capacity(iterations);
    let mut total_instr = 0u64;
    let mut allocated_bytes = 0u64;

    for i in 0..iterations {
        let mut vm_config = VmConfig::default();
        vm_config.debug = false;
        let mut vm = NyxVm::new(vm_config);
        vm.load(module.clone());

        let start = Instant::now();
        let _ = vm.run(func_name);
        let duration = start.elapsed();

        times.push(duration.as_nanos());

        if i == iterations - 1 {
            total_instr = vm.instruction_count();
            allocated_bytes = vm.total_allocated();
        }
    }

    let avg = times.iter().sum::<u128>() / iterations as u128;

    // Calculate Standard Deviation (Jitter)
    let variance = times
        .iter()
        .map(|&t| {
            let diff = t.abs_diff(avg);
            diff * diff
        })
        .sum::<u128>()
        / iterations as u128;
    let jitter = (variance as f64).sqrt() as u128;

    BenchResult {
        name: func_name.to_string(),
        instructions: total_instr,
        avg_wall_clock_ns: avg,
        jitter_ns: jitter,
        allocated_bytes,
    }
}
