use clap::Parser;
use colored::*;
use std::path::Path;
use which::which;

#[derive(Parser, Debug)]
#[command(name = "nyx-doctor")]
#[command(about = "Nyx Ecosystem Health Check", long_about = None)]
struct Args {}

fn main() {
    let _args = Args::parse();

    println!("{} Checking Nyx ecosystem health...", "🌌".magenta().bold());
    println!(
        "{}",
        "============================================================".magenta()
    );

    let mut issues = 0;

    // 1. Check for nyx binary
    if let Ok(path) = which("nyx") {
        println!("{} Nyx CLI: Found at {}", "✓".green(), path.display());
    } else if Path::new("./tools/nyx").exists() {
        println!(
            "{} Nyx CLI: Found in current directory (local development)",
            "✓".green()
        );
    } else {
        println!("{} Nyx CLI: Not found in PATH", "✗".red());
        issues += 1;
    }

    // 2. Check for registry files
    if Path::new("registry/language.json").exists() {
        println!("{} Language Registry: Found", "✓".green());
    } else {
        println!(
            "{} Language Registry: Missing (Expected registry/language.json)",
            "✗".red()
        );
        issues += 1;
    }

    if Path::new("registry/engines.json").exists() {
        println!("{} Engines Registry: Found", "✓".green());
    } else {
        println!(
            "{} Engines Registry: Missing (Expected registry/engines.json)",
            "✗".red()
        );
        issues += 1;
    }

    // 3. Check for compiler
    if which("rustc").is_ok() {
        println!("{} Rust Compiler: Found", "✓".green());
    } else {
        println!(
            "{} Rust Compiler: Missing (required for JIT/Tooling rebuilds)",
            "✗".red()
        );
        issues += 1;
    }

    // 4. Check for Hypervisor (Optional but recommended)
    if Path::new("/dev/kvm").exists() {
        println!("{} KVM Hypervisor: Available", "✓".green());
    } else {
        println!(
            "{} KVM Hypervisor: Not found (Performance might be degraded)",
            "!".yellow()
        );
    }

    println!(
        "{}",
        "============================================================".magenta()
    );
    if issues == 0 {
        println!(
            "{} Your Nyx environment is healthy and ready for production.",
            "SUCCESS".green().bold()
        );
    } else {
        println!(
            "{} Found {} issues. Please fix them for an optimal experience.",
            "WARNING".yellow().bold(),
            issues
        );
    }
}
