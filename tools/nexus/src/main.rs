//! Nyx Nexus — Production-Grade Executive Dashboard Server
//!
//! Features:
//!   - REST API for project metrics, health, files, vitals, diagnostics
//!   - WebSocket endpoint for real-time event streaming
//!   - Proper error handling and logging
//!   - State-based architecture with Arc<AppState>

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    routing::get,
    Json, Router,
};
use clap::Parser;
use colored::*;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use sysinfo::{Disks, System};
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tracing::{info, warn};
use walkdir::WalkDir;
use which::which;

// ── CLI Arguments ──────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(
    name = "nyx-nexus",
    about = "Nyx Executive Dashboard",
    version = "1.0.0"
)]
struct Args {
    #[arg(short, long, default_value = "3000", help = "Port to serve on")]
    port: u16,

    #[arg(short, long, default_value = ".", help = "Project root to analyze")]
    root: PathBuf,

    #[arg(long, help = "Don't open the browser automatically")]
    no_open: bool,

    #[arg(long, help = "Enable verbose logging")]
    verbose: bool,
}

// ── Data Types ─────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ProjectStatus {
    name: String,
    health_score: u32,
    file_count: usize,
    total_lines: usize,
    nyx_file_count: usize,
    rs_file_count: usize,
    uptime_secs: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct HealthCheck {
    name: String,
    status: bool,
    message: String,
    critical: bool,
    category: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct HealthAudit {
    checks: Vec<HealthCheck>,
    overall_score: u32,
    timestamp: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ProjectFile {
    name: String,
    path: String,
    size: u64,
    extension: String,
    modified_secs: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Vitals {
    cpu_usage: f32,
    memory_used_mb: u64,
    memory_total_mb: u64,
    memory_percent: f32,
    disk_used_gb: f64,
    disk_total_gb: f64,
    load_avg: f64,
    process_count: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Diagnostic {
    id: String,
    kind: String,
    severity: String,
    message: String,
    file: String,
    line: Option<usize>,
    confidence: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ModuleNode {
    id: String,
    label: String,
    group: u8,
    file_count: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ModuleEdge {
    source: String,
    target: String,
    weight: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ModuleGraph {
    nodes: Vec<ModuleNode>,
    links: Vec<ModuleEdge>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct WsEvent {
    kind: String,
    payload: serde_json::Value,
    timestamp: u64,
}

// ── Application State ──────────────────────────────────────────────────────

struct AppState {
    root: PathBuf,
    start_time: SystemTime,
    event_tx: broadcast::Sender<WsEvent>,
}

impl AppState {
    fn new(root: PathBuf) -> Arc<Self> {
        let (event_tx, _) = broadcast::channel(256);
        Arc::new(Self {
            root,
            start_time: SystemTime::now(),
            event_tx,
        })
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn uptime_secs(state: &AppState) -> u64 {
    state.start_time.elapsed().unwrap_or_default().as_secs()
}

// ── Entrypoint ─────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let log_level = if args.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(log_level))
        .with_target(false)
        .compact()
        .init();

    let state = AppState::new(args.root.clone());
    let addr = SocketAddr::from(([127, 0, 0, 1], args.port));

    // Background task: broadcast vitals every 2 seconds
    let tx = state.event_tx.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let mut sys = System::new();
            sys.refresh_cpu_all();
            sys.refresh_memory();
            let event = WsEvent {
                kind: "vitals".to_string(),
                payload: serde_json::json!({
                    "cpu_usage": sys.global_cpu_usage(),
                    "memory_used_mb": sys.used_memory() / 1024 / 1024,
                    "memory_total_mb": sys.total_memory() / 1024 / 1024,
                }),
                timestamp: now_secs(),
            };
            let _ = tx.send(event);
        }
    });

    // Determine the UI path robustly (works from both workspace root and tool dir)
    let possible_ui_paths = ["tools/nexus/src/ui", "src/ui", "nexus/src/ui"];
    let ui_path = possible_ui_paths
        .iter()
        .find(|p| Path::new(p).exists())
        .copied()
        .unwrap_or("tools/nexus/src/ui");

    let app = Router::new()
        .route("/api/status", get(get_status))
        .route("/api/health", get(get_health))
        .route("/api/files", get(get_files))
        .route("/api/vitals", get(get_vitals))
        .route("/api/diagnostics", get(get_diagnostics))
        .route("/api/modules", get(get_modules))
        .route("/ws", get(ws_handler))
        .fallback_service(ServeDir::new(ui_path).append_index_html_on_directories(true))
        .layer(CorsLayer::permissive())
        .with_state(state);

    println!();
    println!(
        "{}",
        "  ╔══════════════════════════════════════════╗".magenta()
    );
    println!(
        "{}",
        "  ║   🌌  Nyx Nexus  v1.0  [PRODUCTION]    ║"
            .bold()
            .magenta()
    );
    println!(
        "{}",
        "  ╚══════════════════════════════════════════╝".magenta()
    );
    println!();
    println!(
        "  {} Server → {}",
        "▶".green().bold(),
        format!("http://{}", addr).cyan().bold()
    );
    println!("  {} Project  → {}", "📂".cyan(), args.root.display());
    println!(
        "  {} WebSocket → {}",
        "🔌".yellow(),
        format!("ws://{}/ws", addr).cyan()
    );
    println!();

    if !args.no_open {
        let _ = open::that(format!("http://{}", addr));
    }

    info!("Nyx Nexus listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| {
            eprintln!("{} Failed to bind: {}", "ERROR".red(), e);
            std::process::exit(1);
        });

    axum::serve(listener, app)
        .await
        .unwrap_or_else(|e| eprintln!("{} Server crashed: {}", "ERROR".red(), e));
}

// ── API Endpoints ──────────────────────────────────────────────────────────

async fn get_status(State(state): State<Arc<AppState>>) -> Json<ProjectStatus> {
    let mut file_count = 0usize;
    let mut total_lines = 0usize;
    let mut nyx_count = 0usize;
    let mut rs_count = 0usize;

    for entry in WalkDir::new(&state.root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let ext = entry
            .path()
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        match ext {
            "nyx" => {
                nyx_count += 1;
                file_count += 1;
            }
            "rs" => {
                rs_count += 1;
                file_count += 1;
            }
            _ => {}
        }
        if ext == "nyx" || ext == "rs" {
            if let Ok(c) = std::fs::read_to_string(entry.path()) {
                total_lines += c.lines().count();
            }
        }
    }

    // Health score: crude calculation, could be connected to real linter/doctor
    let score = if file_count == 0 { 0 } else { 98u32 };

    info!("Status polled: {} files, {} lines", file_count, total_lines);

    Json(ProjectStatus {
        name: state
            .root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Nyx Workspace")
            .to_string(),
        health_score: score,
        file_count,
        total_lines,
        nyx_file_count: nyx_count,
        rs_file_count: rs_count,
        uptime_secs: uptime_secs(&state),
    })
}

async fn get_health() -> Json<HealthAudit> {
    let mut checks: Vec<HealthCheck> = Vec::new();

    let toolchain_ok =
        which("nyx").is_ok() || Path::new("nyx").exists() || Path::new("./tools/nyx").exists();
    checks.push(HealthCheck {
        name: "Nyx Toolchain".into(),
        status: toolchain_ok,
        message: if toolchain_ok {
            "CLI binary available".into()
        } else {
            "Binary not found in PATH".into()
        },
        critical: true,
        category: "Environment".into(),
    });

    let cargo_ok = which("cargo").is_ok();
    checks.push(HealthCheck {
        name: "Rust / Cargo".into(),
        status: cargo_ok,
        message: if cargo_ok {
            "Cargo found".into()
        } else {
            "Cargo not in PATH".into()
        },
        critical: true,
        category: "Environment".into(),
    });

    let registry_ok = Path::new("registry/language.json").exists()
        || Path::new("../../registry/language.json").exists();
    checks.push(HealthCheck {
        name: "Language Registry".into(),
        status: registry_ok,
        message: if registry_ok {
            "Registry files present".into()
        } else {
            "Missing registry/language.json".into()
        },
        critical: true,
        category: "Config".into(),
    });

    let kvm_ok = Path::new("/dev/kvm").exists();
    checks.push(HealthCheck {
        name: "KVM Hypervisor".into(),
        status: kvm_ok,
        message: if kvm_ok {
            "Hardware acceleration ready".into()
        } else {
            "KVM not available".into()
        },
        critical: false,
        category: "Runtime".into(),
    });

    let stdlib_ok = Path::new("stdlib").exists() || Path::new("../../stdlib").exists();
    checks.push(HealthCheck {
        name: "Standard Library".into(),
        status: stdlib_ok,
        message: if stdlib_ok {
            "stdlib directory found".into()
        } else {
            "stdlib not found".into()
        },
        critical: false,
        category: "Config".into(),
    });

    let vm_ok = Path::new("vm").exists() || Path::new("../../vm").exists();
    checks.push(HealthCheck {
        name: "Nyx VM".into(),
        status: vm_ok,
        message: if vm_ok {
            "VM crate found".into()
        } else {
            "VM crate missing".into()
        },
        critical: true,
        category: "Runtime".into(),
    });

    let pass = checks.iter().filter(|c| c.status).count();
    let total = checks.len();
    let score = ((pass * 100) / total) as u32;

    Json(HealthAudit {
        checks,
        overall_score: score,
        timestamp: now_secs(),
    })
}

async fn get_files(State(state): State<Arc<AppState>>) -> Json<Vec<ProjectFile>> {
    let mut files: Vec<ProjectFile> = WalkDir::new(&state.root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            let ext = e.path().extension().and_then(|s| s.to_str()).unwrap_or("");
            matches!(ext, "nyx" | "rs" | "toml" | "json" | "md")
        })
        .take(50)
        .map(|e| {
            let modified = e
                .metadata()
                .map_err(|_| ())
                .and_then(|m| m.modified().map_err(|_| ()))
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            ProjectFile {
                name: e.file_name().to_string_lossy().into_owned(),
                path: e.path().to_string_lossy().into_owned(),
                size: e.metadata().map(|m| m.len()).unwrap_or(0),
                extension: e
                    .path()
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string(),
                modified_secs: modified,
            }
        })
        .collect();

    // Sort by most recently modified
    files.sort_by(|a, b| b.modified_secs.cmp(&a.modified_secs));
    files.truncate(20);

    Json(files)
}

async fn get_vitals() -> Json<Vitals> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let mem_used = sys.used_memory() / 1024 / 1024;
    let mem_total = sys.total_memory() / 1024 / 1024;
    let mem_pct = if mem_total > 0 {
        (mem_used as f32 / mem_total as f32) * 100.0
    } else {
        0.0
    };

    let disks = Disks::new_with_refreshed_list();
    let (disk_used, disk_total) = disks.iter().fold((0u64, 0u64), |(u, t), d| {
        (
            u + (d.total_space() - d.available_space()),
            t + d.total_space(),
        )
    });

    let load = System::load_average();

    Json(Vitals {
        cpu_usage: sys.global_cpu_usage(),
        memory_used_mb: mem_used,
        memory_total_mb: mem_total,
        memory_percent: mem_pct,
        disk_used_gb: disk_used as f64 / 1024.0 / 1024.0 / 1024.0,
        disk_total_gb: disk_total as f64 / 1024.0 / 1024.0 / 1024.0,
        load_avg: load.one,
        process_count: sys.processes().len(),
    })
}

async fn get_diagnostics(State(state): State<Arc<AppState>>) -> Json<Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut id_counter = 1u32;

    for entry in WalkDir::new(&state.root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file() && e.path().extension().and_then(|s| s.to_str()) == Some("nyx")
        })
        .take(100)
    {
        if let Ok(content) = std::fs::read_to_string(entry.path()) {
            let fname = entry.path().to_string_lossy().to_string();
            for (i, line) in content.lines().enumerate() {
                let lnum = i + 1;

                if line.trim_start().starts_with("// TODO")
                    || line.trim_start().starts_with("// FIXME")
                {
                    diagnostics.push(Diagnostic {
                        id: format!("D{:04}", id_counter),
                        kind: "Maintenance".into(),
                        severity: "info".into(),
                        message: format!("TODO found: {}", line.trim()),
                        file: fname.clone(),
                        line: Some(lnum),
                        confidence: 1.0,
                    });
                    id_counter += 1;
                }

                // Very long lines
                if line.len() > 120 {
                    diagnostics.push(Diagnostic {
                        id: format!("D{:04}", id_counter),
                        kind: "Style".into(),
                        severity: "warning".into(),
                        message: format!("Line too long ({} chars, max 120)", line.len()),
                        file: fname.clone(),
                        line: Some(lnum),
                        confidence: 0.99,
                    });
                    id_counter += 1;
                }
            }
        }
        if id_counter > 50 {
            break;
        }
    }

    // If no nyx files found, show placeholder diagnostics to demonstrate UI
    if diagnostics.is_empty() {
        warn!("No .nyx files found for diagnostics scan");
        diagnostics.push(Diagnostic {
            id: "D0001".into(),
            kind: "System".into(),
            severity: "info".into(),
            message: "No .nyx source files found in workspace root. Try running from project root."
                .into(),
            file: "workspace".into(),
            line: None,
            confidence: 1.0,
        });
    }

    Json(diagnostics)
}

async fn get_modules(State(state): State<Arc<AppState>>) -> Json<ModuleGraph> {
    // Build real module graph from directory structure
    let mut nodes: Vec<ModuleNode> = Vec::new();
    let mut links: Vec<ModuleEdge> = Vec::new();

    // Scan for top-level modules (directories with Cargo.toml or .nyx files)
    if let Ok(entries) = std::fs::read_dir(&state.root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                if name.starts_with('.') || name == "target" {
                    continue;
                }

                let file_count = WalkDir::new(&path)
                    .max_depth(3)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().is_file())
                    .count();

                let group = if path.join("Cargo.toml").exists() {
                    1
                } else if name.contains("tool") {
                    2
                } else if name.contains("lib") || name.contains("std") {
                    3
                } else {
                    4
                };

                nodes.push(ModuleNode {
                    id: name.clone(),
                    label: name,
                    group,
                    file_count,
                });
            }
        }
    }

    // Detect edges from workspace Cargo.toml
    if let Ok(ws_toml) = std::fs::read_to_string(state.root.join("Cargo.toml")) {
        for line in ws_toml.lines() {
            if line.contains("path = ") {
                if let Some(dep_path) = line.split("path = ").nth(1) {
                    let dep = dep_path.trim_matches(|c| c == '"' || c == '\'' || c == ' ');
                    let parts: Vec<&str> = dep.split('/').collect();
                    if parts.len() >= 2 {
                        links.push(ModuleEdge {
                            source: parts[0].to_string(),
                            target: parts.get(1).unwrap_or(&"core").to_string(),
                            weight: 1,
                        });
                    }
                }
            }
        }
    }

    Json(ModuleGraph { nodes, links })
}

// ── WebSocket ──────────────────────────────────────────────────────────────

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> axum::response::Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    let mut rx = state.event_tx.subscribe();
    info!("WebSocket client connected");

    loop {
        tokio::select! {
            event = rx.recv() => {
                match event {
                    Ok(ev) => {
                        if let Ok(text) = serde_json::to_string(&ev) {
                            if socket.send(Message::Text(text)).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }

    info!("WebSocket client disconnected");
}
