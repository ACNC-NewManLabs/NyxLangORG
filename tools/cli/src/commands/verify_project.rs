use std::path::Path;

pub fn execute() -> Result<(), String> {
    let manifest = Path::new("load.surn");
    if !manifest.exists() {
        return Err("Could not find `load.surn` in the current directory".to_string());
    }

    println!("   Verifying project structure...");
    
    let src_dir = Path::new("src");
    if !src_dir.exists() || !src_dir.is_dir() {
        return Err("Missing `src` directory".to_string());
    }

    println!("    Finished verification");
    Ok(())
}
