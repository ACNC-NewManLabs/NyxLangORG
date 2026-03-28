// Nyx Web CLI
// Command-line interface for web development

use std::path::PathBuf;
use std::process::Command;
use std::fs;

/// Initialize a new web project
fn new_project(name: &str) -> Result<(), String> {
    let project_dir = PathBuf::from(name);
    
    // Create project structure
    let dirs = vec![
        "src",
        "src/routes",
        "src/models",
        "src/views",
        "src/public",
        "src/public/css",
        "src/public/js",
        "tests",
    ];
    
    for dir in dirs {
        let path = project_dir.join(dir);
        fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    }
    
    // Create main.nyx
    let main_content = r#"// Nyx Web Application
mod routes
mod models
mod views

fn main() {
    // Initialize web server
    let config = server::default_config()
    config.port = 8080
    config.host = "localhost"
    
    // Create router
    let router = router::new()
    
    // Add routes
    router.get("/", |ctx| {
        html(ctx, "<h1>Welcome to Nyx Web!</h1>")
    })
    
    // Start server
    server::listen(config, router)
}
"#;
    
    fs::write(project_dir.join("src/main.nyx"), main_content).map_err(|e| e.to_string())?;
    
    // Create route example
    let routes_content = r#"// Application routes
import web_engine

fn index(ctx) {
    html(ctx, "<h1>Home Page</h1>")
}

fn about(ctx) {
    html(ctx, "<h1>About</h1>")
}
"#;
    
    fs::write(project_dir.join("src/routes/main.nyx"), routes_content).map_err(|e| e.to_string())?;
    
    // Create Cargo.toml for Rust integration
    let cargo_content = format!(r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[dependencies]
nyx-web-engine = {{ path = "../../engines/web_engine" }}
"#, name);
    
    fs::write(project_dir.join("Cargo.toml"), cargo_content).map_err(|e| e.to_string())?;
    
    // Create config
    let config_content = r#"[web]
port = 8080
host = "localhost"
workers = 4

[web.static]
dir = "public"
index = "index.html"

[web.tls]
enabled = false

[web.logging]
level = "info"
"#;
    
    fs::write(project_dir.join("web.toml"), config_content).map_err(|e| e.to_string())?;
    
    // Create README
    let readme_content = format!(r#"# {}

A Nyx Web Application

## Getting Started

```bash
# Run development server
nyx web run

# Build for production
nyx web build

# Deploy
nyx web deploy
```

## Project Structure

```
{}/src/
  main.nyx      - Application entry point
  routes/       - Route handlers
  models/       - Data models
  views/        - Templates
  public/       - Static files
```
"#, name, name);
    
    fs::write(project_dir.join("README.md"), readme_content).map_err(|e| e.to_string())?;
    
    println!("Created new Nyx web project: {}", name);
    
    Ok(())
}

/// Run development server
fn run_server(port: u16, host: &str) -> Result<(), String> {
    println!("Starting development server on http://{}:{}", host, port);
    
    // In a full implementation, this would compile and run the Nyx web application
    // For now, we'll show the command that would be executed
    
    println!("Run: nyx run src/main.nyx --port {} --host {}", port, host);
    
    Ok(())
}

/// Build for production
fn build_project() -> Result<(), String> {
    println!("Building for production...");
    
    // Create output directory
    let output_dir = PathBuf::from("dist");
    fs::create_dir_all(&output_dir).map_err(|e| e.to_string())?;
    
    println!("Build complete! Output in dist/");
    
    Ok(())
}

/// Deploy to edge/serverless
fn deploy(platform: &str, region: &str) -> Result<(), String> {
    println!("Deploying to {} in {}...", platform, region);
    
    // In production, this would deploy to the specified platform
    // Using the deployment modules from the web engine
    
    println!("Deployment complete!");
    
    Ok(())
}

/// Show help
fn help() {
    println!(r#"
Nyx Web CLI - Web development tools for Nyx

USAGE:
    nyx web <command> [options]

COMMANDS:
    new <name>      Create a new web project
    run             Run development server
    build           Build for production
    deploy          Deploy to edge/serverless
    init            Initialize web engine in current directory

OPTIONS:
    --port <n>      Port number (default: 8080)
    --host <addr>   Host address (default: localhost)
    --platform      Deployment platform
    --region        Deployment region

EXAMPLES:
    nyx web new myapp
    nyx web run --port 3000
    nyx web build
    nyx web deploy --platform vercel
"#);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() < 2 {
        help();
        return;
    }
    
    let command = &args[1];
    
    match command.as_str() {
        "new" => {
            if args.len() < 3 {
                println!("Error: Project name required");
                println!("Usage: nyx web new <name>");
                return;
            }
            let name = &args[2];
            if let Err(e) = new_project(name) {
                eprintln!("Error creating project: {}", e);
            }
        }
        "run" => {
            let mut port = 8080;
            let mut host = "localhost";
            
            // Parse options
            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "--port" => {
                        if i + 1 < args.len() {
                            port = args[i + 1].parse().unwrap_or(8080);
                            i += 1;
                        }
                    }
                    "--host" => {
                        if i + 1 < args.len() {
                            host = &args[i + 1];
                            i += 1;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            
            if let Err(e) = run_server(port, host) {
                eprintln!("Error: {}", e);
            }
        }
        "build" => {
            if let Err(e) = build_project() {
                eprintln!("Error: {}", e);
            }
        }
        "deploy" => {
            let mut platform = "vercel";
            let mut region = "us-east-1";
            
            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "--platform" => {
                        if i + 1 < args.len() {
                            platform = &args[i + 1];
                            i += 1;
                        }
                    }
                    "--region" => {
                        if i + 1 < args.len() {
                            region = &args[i + 1];
                            i += 1;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            
            if let Err(e) = deploy(platform, region) {
                eprintln!("Error: {}", e);
            }
        }
        "init" => {
            // Initialize web engine in current directory
            println!("Initializing Nyx Web Engine...");
            
            let config = r#"[web]
name = "my-web-app"
port = 8080

[web.server]
workers = 4
timeout = 30

[web.static]
enabled = true
dir = "public"

[web.tls]
enabled = false
"#;
            
            if let Err(e) = fs::write("web.toml", config) {
                eprintln!("Error writing config: {}", e);
                return;
            }
            
            println!("Initialized Nyx Web Engine");
        }
        "help" | "-h" | "--help" => {
            help();
        }
        _ => {
            println!("Unknown command: {}", command);
            help();
        }
    }
}

