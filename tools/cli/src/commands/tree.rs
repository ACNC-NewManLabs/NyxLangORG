use std::fs;
use std::path::Path;

pub fn execute() -> Result<(), String> {
    let surn_path = Path::new("load.surn");
    if !surn_path.exists() {
        return Err("Could not find `load.surn` in the current directory".to_string());
    }

    let content = fs::read_to_string(surn_path).map_err(|e| e.to_string())?;
    let surn = crate::package_manager::NyxCargo::parse(&content).map_err(|e| e.to_string())?;

    println!("{} v{}", surn.package.name, surn.package.version);
    for (name, _dep) in &surn.dependencies {
        println!("└── {} v*", name);
    }

    Ok(())
}
