use std::env;

pub fn execute() -> Result<(), String> {
    let current_dir = env::current_dir().map_err(|e| e.to_string())?;
    let mut current = current_dir.as_path();

    loop {
        let manifest = current.join("load.surn");
        if manifest.exists() {
            println!("{}", manifest.display());
            return Ok(());
        }

        match current.parent() {
            Some(parent) => current = parent,
            None => return Err("Could not find `load.surn` in the current directory or any parent".to_string()),
        }
    }
}
