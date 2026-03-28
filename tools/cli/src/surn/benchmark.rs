use std::time::Instant;
use std::fs;
use crate::package_manager::NyxCargo;

pub fn run_benchmark() {
    let path = "/tmp/million.surn";
    println!("Loading {}...", path);
    let start_load = Instant::now();
    let content = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            println!("Failed to read file: {}", e);
            return;
        }
    };
    println!("Loaded in {:?}", start_load.elapsed());

    println!("Parsing 1,000,000 lines of SURN...");
    let start_parse = Instant::now();
    match NyxCargo::parse(&content) {
        Ok(_) => {
            let elapsed = start_parse.elapsed();
            println!("SUCCESS: Parsed 1,000,000 lines in {:?}", elapsed);
            println!("Performance: approx {:.2} lines/sec", 1_000_000.0 / elapsed.as_secs_f64());
        },
        Err(e) => {
            println!("FAILED: {}", e);
        }
    }
}
