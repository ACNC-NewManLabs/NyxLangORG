use std::fs;
use std::path::Path;
use crate::templates::*;

pub struct Project;

impl Project {
    pub fn new_bin(path: &Path, name: &str) -> Result<(), String> {
        Self::create_dir_structure(path)?;
        
        let surn_content = NYX_SURN_TEMPLATE.replace("{name}", name);
        let main_content = MAIN_NYX_TEMPLATE.replace("{name}", name);
        
        fs::write(path.join("load.surn"), surn_content).map_err(|e| e.to_string())?;
        fs::write(path.join("src/main.nyx"), main_content).map_err(|e| e.to_string())?;
        fs::write(path.join(".gitignore"), GITIGNORE_TEMPLATE).map_err(|e| e.to_string())?;
        
        fs::create_dir_all(path.join("tests")).ok();
        fs::create_dir_all(path.join("examples")).ok();

        Ok(())
    }

    pub fn new_lib(path: &Path, name: &str) -> Result<(), String> {
        Self::create_dir_structure(path)?;
        
        let mut surn_content = NYX_SURN_TEMPLATE.replace("{name}", name);
        surn_content.push_str("\nlib:\n    name: \"{name}\"\n    path: \"src/lib.nyx\"\n".replace("{name}", name).as_str());
        
        fs::write(path.join("load.surn"), surn_content).map_err(|e| e.to_string())?;
        fs::write(path.join("src/lib.nyx"), "pub fn add(a: i32, b: i32) -> i32 { a + b }\n").map_err(|e| e.to_string())?;
        fs::write(path.join(".gitignore"), GITIGNORE_TEMPLATE).map_err(|e| e.to_string())?;

        Ok(())
    }

    fn create_dir_structure(path: &Path) -> Result<(), String> {
        // If the path is the current directory or an empty directory, allow it.
        // For simplicity in this toolchain, we just ensure src/ exists later.
        if path.exists() && path.is_dir() {
            let entries = fs::read_dir(path).map_err(|e| e.to_string())?;
            if entries.count() > 0 && path.join("load.surn").exists() {
                 return Err(format!("Destination path '{}' already contains a Nyx project", path.display()));
            }
        }
        fs::create_dir_all(path.join("src")).map_err(|e| e.to_string())?;
        Ok(())
    }
}
