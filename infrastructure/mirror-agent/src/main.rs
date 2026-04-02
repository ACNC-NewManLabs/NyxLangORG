use std::fs;
use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "nyx-mirror-agent")]
struct Args {
    #[arg(long, default_value = "http://127.0.0.1:8090")]
    registry: String,
    #[arg(long, default_value = "infrastructure/schemas/mirror_snapshot.json")]
    output: PathBuf,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse();
    let url = format!(
        "{}/api/v1/mirrors/snapshot",
        args.registry.trim_end_matches('/')
    );
    let res = reqwest::blocking::get(url).map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(format!("snapshot request failed: {}", res.status()));
    }
    let body = res.text().map_err(|e| e.to_string())?;
    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&args.output, body).map_err(|e| e.to_string())?;
    println!("mirror snapshot written to {}", args.output.display());
    Ok(())
}
