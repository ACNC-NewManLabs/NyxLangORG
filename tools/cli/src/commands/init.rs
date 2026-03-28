use std::env;
use crate::project::Project;

pub fn execute(name: Option<String>, lib: bool) -> Result<(), String> {
    let current_dir = env::current_dir().map_err(|e| e.to_string())?;
    
    let project_name = name.unwrap_or_else(|| {
        current_dir.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("nyx_project")
            .to_string()
    });

    println!("      Init {} package '{}'", if lib { "library" } else { "binary" }, project_name);

    if lib {
        Project::new_lib(&current_dir, &project_name)?;
    } else {
        Project::new_bin(&current_dir, &project_name)?;
    }

    Ok(())
}
