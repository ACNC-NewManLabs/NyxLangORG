// Copyright (c) 2026 SURYA SEKHAR ROY. All Rights Reserved.
// Nyx™ Entry Point
use nyx::applications::compiler::cli;

#[tokio::main]
async fn main() {
    // Initialize production logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    if let Err(err) = cli::run().await {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
