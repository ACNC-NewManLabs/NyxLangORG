pub const NYX_SURN_TEMPLATE: &str = r#"[package]
name = "{name}"
version = "1.0"
edition = "2024"

dependencies:
    stdlib: "1.0"

[profile.dev]
opt-level = 0
debug = true
"#;

pub const MAIN_NYX_TEMPLATE: &str = r#"/// Main entry point for {name}
fn main() {
    println("Hello, Nyx!");
}
"#;

pub const GITIGNORE_TEMPLATE: &str = r#"/target
load.bolt
"#;
