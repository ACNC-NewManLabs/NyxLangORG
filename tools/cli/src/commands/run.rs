use std::fs;
use std::path::Path;
use std::process::Command;
use crate::builder::Builder;

pub fn execute(release: bool, args: Vec<String>) -> Result<(), String> {
    let surn_path = Path::new("load.surn"); // Changed from toml_path to surn_path and "nyx.toml" to "nyx.surn"
    if !surn_path.exists() {
        return Err("Could not find `load.surn` in the current directory".to_string()); // Updated error message
    }

    let content = fs::read_to_string(surn_path).map_err(|e| e.to_string())?; // Changed toml_path to surn_path
    let surn = crate::package_manager::NyxCargo::parse(&content).map_err(|e| e.to_string())?; // Changed toml to surn and NyxToml::parse to NyxCargo::parse

    let builder = Builder::new(Path::new(".").to_path_buf(), surn.clone()); // Changed toml.clone() to surn
    builder.build(release)?;

    let output_dir = if release { "target/release" } else { "target/debug" };
    let bin_name = surn.package.name.clone();
    let bin_path = Path::new(".").join(output_dir).join(&bin_name);

    println!("     Running `{}`", bin_path.display());

    let mut cmd = Command::new(bin_path);
    cmd.args(args);
    
    // In a real implementation, we would execute the binary
    // let status = cmd.status().map_err(|e| e.to_string())?;
    // if !status.success() { return Err(format!("Process exited with {}", status)); }

    Ok(())
}
