// Copyright (c) 2026 SURYA SEKHAR ROY. All Rights Reserved.
// Nyx™ Entry Point
use nyx::applications::compiler::cli;

#[tokio::main]
async fn main() {
    if let Err(err) = cli::run().await {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
