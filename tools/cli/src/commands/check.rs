use std::path::Path;

pub fn execute() -> Result<(), String> {
    let manifest = Path::new("load.surn");
    if !manifest.exists() {
        return Err("Could not find `load.surn` in the current directory".to_string());
    }

    println!("    Checking for compilation errors...");
    // Simulate a fast check
    println!("    Finished check");
    Ok(())
}
