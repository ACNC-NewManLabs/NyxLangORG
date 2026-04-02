use std::io::{self, Write};

fn main() {
    println!("Nyx 2.0 Unified REPL");
    println!("Type 'exit' to quit.");

    let stdin = io::stdin();
    let mut input = String::new();

    loop {
        print!("nyx> ");
        io::stdout().flush().unwrap();
        input.clear();

        if stdin.read_line(&mut input).is_err() {
            break;
        }

        let line = input.trim();
        if line == "exit" {
            break;
        }
        
        if line.is_empty() {
            continue;
        }

        // Simulating the JIT evaluation feedback loop
        println!("Execution Result: [Hot Reloaded & Evaluated]");
    }
}
