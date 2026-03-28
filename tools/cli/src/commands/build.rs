use std::fs;
use std::path::Path;
use crate::builder::Builder;

pub fn execute(release: bool) -> Result<(), String> {
    let surn_path = Path::new("load.surn");
    if !surn_path.exists() {
        return Err("Could not find `load.surn` in the current directory".to_string());
    }

    let content = fs::read_to_string(surn_path).map_err(|e| e.to_string())?;
    let surn = crate::package_manager::NyxCargo::parse(&content).map_err(|e| e.to_string())?;

    let builder = Builder::new(Path::new(".").to_path_buf(), surn);
    builder.build(release)?;

    println!("    Finished {} target(s)", if release { "release" } else { "debug" });

    Ok(())
}
