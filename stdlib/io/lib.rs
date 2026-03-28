use std::fs;
use std::io::{self, Write};
use std::path::Path;

pub fn read_file(path: impl AsRef<Path>) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| e.to_string())
}

pub fn write_file(path: impl AsRef<Path>, content: &str) -> Result<(), String> {
    fs::write(path, content).map_err(|e| e.to_string())
}

pub fn print_line(msg: &str) {
    println!("{msg}");
}

pub fn prompt(msg: &str) -> Result<String, String> {
    print!("{msg}");
    io::stdout().flush().map_err(|e| e.to_string())?;
    let mut buf = String::new();
    io::stdin().read_line(&mut buf).map_err(|e| e.to_string())?;
    Ok(buf.trim_end().to_string())
}
