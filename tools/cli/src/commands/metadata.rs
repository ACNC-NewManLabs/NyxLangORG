use std::fs;
use std::path::Path;
use serde_json::json;

pub fn execute() -> Result<(), String> {
    let surn_path = Path::new("load.surn");
    if !surn_path.exists() {
        return Err("Could not find `load.surn` in the current directory".to_string());
    }

    let content = fs::read_to_string(surn_path).map_err(|e| e.to_string())?;
    let surn = crate::package_manager::NyxCargo::parse(&content).map_err(|e| e.to_string())?;

    let metadata = json!({
        "packages": [
            {
                "name": surn.package.name,
                "version": surn.package.version,
                "id": format!("{} {} (path+file://{})", surn.package.name, surn.package.version, Path::new(".").canonicalize().unwrap().display()),
                "source": null,
                "dependencies": surn.dependencies.keys().collect::<Vec<_>>(),
                "targets": [
                    {
                        "kind": ["bin"],
                        "name": surn.package.name,
                        "src_path": "src/main.nyx"
                    }
                ],
                "features": surn.features,
                "manifest_path": "load.surn"
            }
        ],
        "workspace_root": Path::new(".").canonicalize().unwrap().display().to_string(),
        "target_directory": "target",
        "version": 1
    });

    println!("{}", serde_json::to_string_pretty(&metadata).unwrap());

    Ok(())
}
