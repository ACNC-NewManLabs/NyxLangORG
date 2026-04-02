use std::io::{self, Write};
use std::collections::HashMap;
use colored::*;
use nyx::runtime::execution::{NyxVm, VmConfig, Value, eval_repl_line};

fn main() {
    println!("{}", "============================================================".blue());
    println!("{}", "                    NYX INTERACTIVE REPL                    ".bold().blue());
    println!("{}", "============================================================".blue());
    println!("Type your code and press Enter. Type 'exit' to quit.");
    println!("State is now persistent across lines.");
    println!();

    let vm_config = VmConfig::default();
        // config.debug = false;
    let mut vm = NyxVm::new(vm_config);
    let _locals = HashMap::<String, Value>::new();

    // Register a simple print function (optional, but good for testing)
    vm.register_native("print", |_vm, args| {
        for arg in args {
            println!("{}", arg);
        }
        Ok(Value::Null)
    });

    let mut buffer = String::new();
    loop {
        if buffer.is_empty() {
            print!("{}", "nyx> ".green());
        } else {
            print!("{}", "...  ".green());
        }
        io::stdout().flush().unwrap();

        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() {
            break;
        }

        let trimmed = line.trim();
        if trimmed == "exit" || trimmed == "quit" {
            break;
        }

        if trimmed.is_empty() && !buffer.is_empty() {
            // Execute on empty line to finish block
        } else {
            buffer.push_str(&line);
            // Simple heuristic for multi-line: check for open braces or lack of semicolon
            let open_braces = buffer.chars().filter(|&c| c == '{').count();
            let close_braces = buffer.chars().filter(|&c| c == '}').count();
            if open_braces > close_braces {
                continue;
            }
            if !trimmed.ends_with(';') && !trimmed.ends_with('}') && !trimmed.is_empty() {
                // If it doesn't look like a complete statement and it's not the first line, keep buffering
                if !buffer.contains('\n') || !trimmed.chars().last().unwrap_or(' ').is_alphanumeric() {
                     // continue; // Defer to a smarter check if needed
                }
            }
        }

        if buffer.trim().is_empty() {
            continue;
        }

        match eval_repl_line(&mut vm, &buffer) {
            Ok(val) => if !matches!(val, nyx::runtime::execution::Value::Null) {
                println!("=> {}", format!("{}", val).yellow());
            },
            Err(e) => eprintln!("{}: {:?}", "Error".red(), e),
        }

        buffer.clear();
    }
}
