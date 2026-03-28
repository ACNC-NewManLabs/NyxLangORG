use std::io::{self, Write};
use colored::*;
use nyx::applications::compiler::compiler_main::Compiler;
use nyx_vm::{NyxVm, VmConfig, Value};

fn main() {
    println!("{}", "============================================================".blue());
    println!("{}", "                    NYX INTERACTIVE REPL                    ".bold().blue());
    println!("{}", "============================================================".blue());
    println!("Type your code and press Enter. Type 'exit' to quit.");
    println!("Use ';' at the end of a line for single expression or continue typing.");
    println!();

    let mut compiler = match Compiler::from_registry_files("registry/language.json", "registry/engines.json") {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Warning: could not load registries, using default behavior.");
            // We'll try to proceed or exit if critical
            return;
        }
    };

    let mut vm_config = VmConfig::default();
    vm_config.debug = false;
    let mut vm = NyxVm::new(vm_config);

    // Register a simple print function
    vm.register("print", 1, |args| {
        for arg in args {
            println!("{}", arg.to_string());
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
            // Execute what we have
        } else {
            buffer.push_str(&line);
            if !trimmed.ends_with(';') && !trimmed.ends_with('}') && !trimmed.is_empty() {
                continue;
            }
        }

        if buffer.trim().is_empty() {
            continue;
        }

        // Wrap in a main function if it looks like statements
        let source = if buffer.contains("fn main") {
            buffer.clone()
        } else {
            format!("fn main() {{ \n{}\n }}", buffer)
        };

        // Create a temporary file for the compiler (it expects a path)
        let tmp_file = "/tmp/repl_session.nyx";
        if fs_err::write(tmp_file, &source).is_err() {
            eprintln!("Error: Could not write session file.");
            buffer.clear();
            continue;
        }

        match compiler.compile_to_bytecode(std::path::Path::new(tmp_file)) {
            Ok(module) => {
                vm.load(module);
                match vm.run("main") {
                    Ok(val) => {
                        if val != Value::Null && val != Value::Unit {
                            println!("=> {:?}", val);
                        }
                    }
                    Err(e) => {
                        eprintln!("{}: {:?}", "Runtime Error".red(), e);
                    }
                }
            }
            Err(e) => {
                eprintln!("{}: {}", "Compilation Error".red(), e);
            }
        }

        buffer.clear();
    }
}

// Modifying to use std::fs instead of fs_err if fs_err is not in workspace
mod fs_err {
    pub fn write<P: AsRef<std::path::Path>, C: AsRef<[u8]>>(path: P, contents: C) -> std::io::Result<()> {
        std::fs::write(path, contents)
    }
}
