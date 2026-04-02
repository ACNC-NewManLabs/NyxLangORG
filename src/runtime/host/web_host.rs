//! Web Host Implementation
//!
//! This module provides the web platform runtime host that handles
//! browser-based rendering and event handling.

use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};
use std::time::Duration;

use notify::{RecursiveMode, Watcher};

use crate::devtools::protocol::DevtoolsEnvelope;
use crate::runtime::execution::session::RuntimeSession;
use crate::runtime::execution::nyx_vm::{render_node as vm_render_node, to_stringish, Value as VmValue};
use crate::runtime::host::asset_host::AssetHost;
use crate::runtime::host::dev_host::DevWatcher;
use crate::runtime::host::runtime_host::{
    HostError, PlatformEvent, RuntimeHost, SemanticsDelta, SurfaceConfig, SurfaceHandle,
};

/// Font route prefix for web fonts
const FONT_ROUTE_PREFIX: &str = "/__nyx_fonts";

/// Embedded font data
static INTER_300: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/dist/fonts/inter_300.woff2"
));
static INTER_400: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/dist/fonts/inter_400.woff2"
));
static INTER_600: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/dist/fonts/inter_600.woff2"
));
static INTER_800: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/dist/fonts/inter_800.woff2"
));

/// Web build options
#[derive(Debug, Clone)]
pub struct WebBuildOptions {
    pub input: PathBuf,
    pub out_dir: PathBuf,
}

/// Web dev options
#[derive(Debug, Clone)]
pub struct WebDevOptions {
    pub input: PathBuf,
    pub host: String,
    pub port: u16,
    pub open_browser: bool,
    pub hot_reload: bool,
}

/// Web host state
pub struct WebHost {
    next_surface: u64,
    asset_host: AssetHost,
    dev_watcher: DevWatcher,
}

impl WebHost {
    /// Create a new web host
    pub fn new(asset_root: PathBuf) -> Self {
        Self {
            next_surface: 1,
            asset_host: AssetHost::new(asset_root),
            dev_watcher: DevWatcher::new(),
        }
    }

    /// Get the asset host
    pub fn asset_host(&self) -> &AssetHost {
        &self.asset_host
    }

    /// Get the dev watcher
    pub fn dev_watcher(&mut self) -> &mut DevWatcher {
        &mut self.dev_watcher
    }
}

impl RuntimeHost for WebHost {
    fn create_surface(&mut self, _config: SurfaceConfig) -> Result<SurfaceHandle, HostError> {
        let handle = SurfaceHandle(self.next_surface);
        self.next_surface += 1;
        Ok(handle)
    }

    fn poll_events(&mut self) -> Result<Vec<PlatformEvent>, HostError> {
        Ok(vec![PlatformEvent::Tick])
    }

    fn read_asset(&self, asset_id: &str) -> Result<Vec<u8>, HostError> {
        self.asset_host.read(asset_id)
    }

    fn emit_devtools(&self, _event: DevtoolsEnvelope) -> Result<(), HostError> {
        Ok(())
    }

    fn publish_semantics(&self, _delta: SemanticsDelta) -> Result<(), HostError> {
        Ok(())
    }

    fn watch_paths(&mut self, paths: &[PathBuf]) -> Result<(), HostError> {
        self.dev_watcher.set_paths(paths);
        Ok(())
    }
}

/// Runtime state for web server
struct RuntimeState {
    session: Mutex<RuntimeSession>,
    hot_reload: bool,
    version: Arc<AtomicU64>,
    applied_version: AtomicU64,
}

/// Start web dev server
pub fn dev(opts: WebDevOptions) -> Result<(), String> {
    let url = format!("http://localhost:{}", opts.port);
    
    print_dev_banner(&opts.input, opts.port, &url, opts.hot_reload);
    
    if opts.open_browser {
        if let Err(err) = open_browser(&url) {
            eprintln!("note: could not auto-open browser: {err}");
        }
    }

    let version = Arc::new(AtomicU64::new(0));
    let (_watcher, rx) = start_watcher(&opts.input, opts.hot_reload)?;
    
    if opts.hot_reload {
        let next_version = version.clone();
        std::thread::spawn(move || {
            while let Ok(event) = rx.recv() {
                if is_relevant_fs_event(&event) {
                    next_version.fetch_add(1, Ordering::Relaxed);
                }
            }
        });
    }

    let listener = std::net::TcpListener::bind(format!("{}:{}", opts.host, opts.port))
        .map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;

    let config = crate::runtime::execution::session::SessionConfig {
        entry_file: opts.input.clone(),
        engine_root: PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/engines/ui_engine")),
        runtime_name: "web".to_string(),
    };
    
    let mut session = RuntimeSession::new(config).map_err(|e| e.message)?;
    session.initialize().map_err(|e| e.message)?;
    
    let state = Arc::new(RuntimeState {
        session: Mutex::new(session),
        hot_reload: opts.hot_reload,
        version,
        applied_version: AtomicU64::new(0),
    });

    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let state = state.clone();
                std::thread::spawn(move || {
                    let _ = handle_connection(&mut stream, state);
                });
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(16));
            }
            Err(err) => return Err(err.to_string()),
        }
    }
}

/// Build for web
pub fn build(opts: WebBuildOptions) -> Result<(), String> {
    let config = crate::runtime::execution::session::SessionConfig {
        entry_file: opts.input.clone(),
        engine_root: PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/engines/ui_engine")),
        runtime_name: "web".to_string(),
    };
    
    let mut session = RuntimeSession::new(config).map_err(|e| e.message)?;
    session.initialize().map_err(|e| e.message)?;
    
    let value = session.render_route("/").map_err(|e| e.message)?;
    
    std::fs::create_dir_all(&opts.out_dir).map_err(|e| e.to_string())?;
    std::fs::write(
        opts.out_dir.join("index.html"),
        finalize_html(value, false, FontMode::Static),
    )
    .map_err(|e| e.to_string())?;

    let fonts_dir = opts.out_dir.join("fonts");
    std::fs::create_dir_all(&fonts_dir).map_err(|e| e.to_string())?;
    for (name, bytes) in [
        ("inter_300.woff2", INTER_300),
        ("inter_400.woff2", INTER_400),
        ("inter_600.woff2", INTER_600),
        ("inter_800.woff2", INTER_800),
    ] {
        std::fs::write(fonts_dir.join(name), bytes).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Start a production static file server for a directory
pub fn serve(opts: WebDevOptions) -> Result<(), String> {
    let url = format!("http://localhost:{}", opts.port);
    println!("NYX PRODUCTION SERVE");
    println!("────────────────────────");
    println!("Directory: {}", opts.input.display());
    println!("Port: {}", opts.port);
    println!("URL: {url}");
    
    let listener = std::net::TcpListener::bind(format!("{}:{}", opts.host, opts.port))
        .map_err(|e| e.to_string())?;
    
    let asset_host = Arc::new(AssetHost::new(opts.input));

    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let assets = asset_host.clone();
                std::thread::spawn(move || {
                    let _ = handle_static_connection(&mut stream, &assets);
                });
            }
            Err(err) => eprintln!("error: {err}"),
        }
    }
}

fn handle_connection(stream: &mut std::net::TcpStream, state: Arc<RuntimeState>) -> Result<(), String> {
    use std::io::Read;
    
    let mut buf = [0u8; 16 * 1024];
    let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
    if n == 0 {
        return Ok(());
    }
    let req = String::from_utf8_lossy(&buf[..n]);
    let first = req.lines().next().unwrap_or("GET / HTTP/1.1");
    let mut parts = first.split_whitespace();
    let _method = parts.next().unwrap_or("GET");
    let path = parts.next().unwrap_or("/");

    // Internal routes
    if path.starts_with(FONT_ROUTE_PREFIX) {
        return serve_font(stream, path);
    }
    if path == "/__nyx_fragment" {
        let html = render_fragment(&state).unwrap_or_else(error_fragment_html);
        return write_html(stream, &html);
    }
    if path == "/__nyx_reload" {
        return sse_reload(stream, state.version.clone());
    }

    // Static asset fallback from asset root (via RuntimeSession config)
    {
        let asset_path = if path == "/" { "index.html" } else { path.trim_start_matches('/') };
        let session = state.session.lock().unwrap();
        let root = session.config().entry_file.parent().unwrap_or(&session.config().entry_file);
        let full_path = root.join(asset_path);
        if full_path.exists() && full_path.is_file() {
            return serve_static_file(stream, &full_path);
        }
    }

    let html = render_request(&state, path).unwrap_or_else(error_page_html);
    write_html(stream, &html)
}

fn handle_static_connection(stream: &mut std::net::TcpStream, assets: &AssetHost) -> Result<(), String> {
    use std::io::Read;
    let mut buf = [0u8; 8 * 1024];
    let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
    if n == 0 { return Ok(()); }
    
    let req = String::from_utf8_lossy(&buf[..n]);
    let path = req.lines().next().unwrap_or("").split_whitespace().nth(1).unwrap_or("/");
    
    let asset_id = if path == "/" { "index.html" } else { path.trim_start_matches('/') };
    let full_path = assets.root.join(asset_id);
    
    if full_path.exists() && full_path.is_file() {
        serve_static_file(stream, &full_path)
    } else {
        write_response(stream, "404 Not Found", "text/html", "<h1>404 Not Found</h1>".as_bytes())
    }
}

fn serve_static_file(stream: &mut std::net::TcpStream, path: &std::path::Path) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let mime = get_mime_type(path.to_str().unwrap_or(""));
    write_response(stream, "200 OK", mime, &bytes)
}

fn get_mime_type(path: &str) -> &'static str {
    let ext = path.split('.').next_back().unwrap_or("");
    match ext.to_lowercase().as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css",
        "js" => "application/javascript",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "woff2" => "font/woff2",
        "txt" => "text/plain",
        _ => "application/octet-stream",
    }
}

fn write_response(stream: &mut std::net::TcpStream, status: &str, mime: &str, body: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let headers = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        status, mime, body.len()
    );
    stream.write_all(headers.as_bytes()).map_err(|e| e.to_string())?;
    stream.write_all(body).map_err(|e| e.to_string())
}

fn render_request(state: &RuntimeState, path: &str) -> Result<String, String> {
    let mut session = state.session.lock().map_err(|_| "session lock poisoned".to_string())?;
    maybe_patch_runtime(&mut session, state)?;
    let value = session.render_route(path).map_err(|e| e.message)?;
    Ok(finalize_html(value, state.hot_reload, FontMode::DevServer))
}

fn render_fragment(state: &RuntimeState) -> Result<String, String> {
    let mut session = state.session.lock().map_err(|_| "session lock poisoned".to_string())?;
    maybe_patch_runtime(&mut session, state)?;
    session.render_fragment().map_err(|e| e.message)
}

fn maybe_patch_runtime(session: &mut RuntimeSession, state: &RuntimeState) -> Result<(), String> {
    if !session.is_initialized() {
        session.initialize().map_err(|e| e.message)?;
    }

    if !state.hot_reload {
        return Ok(());
    }

    let version = state.version.load(Ordering::Relaxed);
    let applied = state.applied_version.load(Ordering::Relaxed);
    if version == applied {
        return Ok(());
    }

    let report = session.reload_entry_package().map_err(|e| e.message)?;
    if !report.errors.is_empty() {
        return Err(report.errors.join("\n"));
    }
    state.applied_version.store(version, Ordering::Relaxed);
    Ok(())
}

fn start_watcher(
    input: &PathBuf,
    watch: bool,
) -> Result<
    (
        Option<notify::RecommendedWatcher>,
        std::sync::mpsc::Receiver<notify::Result<notify::Event>>,
    ),
    String,
> {
    let (tx, rx) = std::sync::mpsc::channel();
    if !watch {
        return Ok((None, rx));
    }
    let mut watcher = notify::recommended_watcher(tx).map_err(|e| e.to_string())?;
    let watch_root = if input.is_dir() {
        input.as_path()
    } else {
        input.parent().unwrap_or(input.as_path())
    };
    watcher
        .watch(watch_root, RecursiveMode::Recursive)
        .map_err(|e| e.to_string())?;
    Ok((Some(watcher), rx))
}

fn print_dev_banner(file: &PathBuf, port: u16, url: &str, watch: bool) {
    println!("NYX DEV SERVER");
    println!("────────────────────────");
    println!("File: {}", file.display());
    println!("Port: {port}");
    println!("URL: {url}");
    println!("Hot Reload: {}", if watch { "Enabled" } else { "Disabled" });
}

fn sse_reload(stream: &mut std::net::TcpStream, version: Arc<AtomicU64>) -> Result<(), String> {
    use std::io::Write;
    
    stream
        .write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: keep-alive\r\n\r\n",
        )
        .map_err(|e| e.to_string())?;
    let mut last = version.load(Ordering::Relaxed);
    loop {
        let cur = version.load(Ordering::Relaxed);
        if cur != last {
            last = cur;
            let msg = format!("event: reload\ndata: {cur}\n\n");
            if stream.write_all(msg.as_bytes()).is_err() {
                break;
            }
            let _ = stream.flush();
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum FontMode {
    DevServer,
    Static,
}

fn finalize_html(v: VmValue, include_reload: bool, font_mode: FontMode) -> String {
    let mut content = match v {
        VmValue::Str(s) => s,
        VmValue::Node(n) => vm_render_node(&n),
        other => to_stringish(&other),
    };
    content = strip_external_font_imports(&content);
    let font_css = match font_mode {
        FontMode::DevServer => inter_font_css(FONT_ROUTE_PREFIX),
        FontMode::Static => inter_font_css("fonts"),
    };
    let reload_snippet = "<script>(function(){function applyFragment(html){var next=document.createElement('div');next.innerHTML=html;var repl=next.firstElementChild;var cur=document.getElementById('app-root');if(cur&&repl&&repl.id==='app-root'){cur.replaceWith(repl);}else{location.reload();}}function refresh(){fetch('/__nyx_fragment',{cache:'no-store'}).then(function(r){return r.text();}).then(applyFragment).catch(function(){location.reload();});}try{var es=new EventSource('/__nyx_reload');es.addEventListener('reload',function(){refresh();});}catch(e){}})();</script>";
    let mut html = if content.contains("<html") {
        content
    } else {
        format!("<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Nyx</title></head><body>{content}</body></html>")
    };
    if let Some(idx) = html.find("<head>") {
        html.insert_str(idx + 6, &format!("<style>{font_css}</style>"));
    }
    if include_reload {
        if let Some(idx) = html.rfind("</body>") {
            html.insert_str(idx, reload_snippet);
        } else {
            html.push_str(reload_snippet);
        }
    }
    html
}

fn strip_external_font_imports(css: &str) -> String {
    css.lines()
        .filter(|line| !line.contains("fonts.googleapis.com") && !line.contains("fonts.gstatic.com"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn inter_font_css(base: &str) -> String {
    let base = base.trim_end_matches('/');
    format!(
        "@font-face{{font-family:'Inter';font-style:normal;font-weight:300;font-display:swap;src:url('{base}/inter_300.woff2') format('woff2');}}\
@font-face{{font-family:'Inter';font-style:normal;font-weight:400;font-display:swap;src:url('{base}/inter_400.woff2') format('woff2');}}\
@font-face{{font-family:'Inter';font-style:normal;font-weight:600;font-display:swap;src:url('{base}/inter_600.woff2') format('woff2');}}\
@font-face{{font-family:'Inter';font-style:normal;font-weight:swap;src:url('{base}/inter_800.woff2') format:800;font-display('woff2');}}\
body{{font-family:'Inter',sans-serif;}}"
    )
}

fn write_html(stream: &mut std::net::TcpStream, body: &str) -> Result<(), String> {
    use std::io::Write;
    
    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-cache\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(resp.as_bytes()).map_err(|e| e.to_string())
}

fn serve_font(stream: &mut std::net::TcpStream, path: &str) -> Result<(), String> {
    use std::io::Write;
    
    let bytes = match path.trim_start_matches(FONT_ROUTE_PREFIX).trim_start_matches('/') {
        "inter_300.woff2" => INTER_300,
        "inter_400.woff2" => INTER_400,
        "inter_600.woff2" => INTER_600,
        "inter_800.woff2" => INTER_800,
        _ => return write_html(stream, "<h1>404</h1>"),
    };
    let headers = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: font/woff2\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        bytes.len()
    );
    stream.write_all(headers.as_bytes()).map_err(|e| e.to_string())?;
    stream.write_all(bytes).map_err(|e| e.to_string())
}

fn error_fragment_html(message: String) -> String {
    format!(
        "<div id=\"app-root\"><pre style=\"white-space:pre-wrap\">{}</pre></div>",
        escape(&message)
    )
}

fn error_page_html(message: String) -> String {
    format!(
        "<!doctype html><html><body><pre style=\"white-space:pre-wrap\">{}</pre></body></html>",
        escape(&message)
    )
}

fn escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "<")
        .replace('>', ">")
}

fn is_relevant_fs_event(event: &notify::Result<notify::Event>) -> bool {
    if let Ok(event) = event {
        matches!(event.kind, notify::EventKind::Create(_) | notify::EventKind::Modify(_) | notify::EventKind::Remove(_))
    } else {
        false
    }
}

fn open_browser(url: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/c", "start", url])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
