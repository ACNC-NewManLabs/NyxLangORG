//! Dev Host Implementation
//!
//! This module provides development tooling integration including
//! file watching, hot reload, and devtools support.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};

/// File event kinds that trigger rebuilds
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileEventKind {
    Created,
    Modified,
    Removed,
    Any,
}

/// File change event
#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: PathBuf,
    pub kind: FileEventKind,
}

/// Dev watcher for file system monitoring
pub struct DevWatcher {
    watcher: Option<RecommendedWatcher>,
    paths: Vec<PathBuf>,
    rx: Option<mpsc::Receiver<Result<Event, notify::Error>>>,
}

impl DevWatcher {
    /// Create a new dev watcher
    pub fn new() -> Self {
        Self {
            watcher: None,
            paths: Vec::new(),
            rx: None,
        }
    }

    /// Set paths to watch
    pub fn set_paths(&mut self, paths: &[PathBuf]) {
        self.paths = paths.to_vec();
        
        // Create new watcher
        let (tx, rx) = mpsc::channel();
        
        self.watcher = Some(
            RecommendedWatcher::new(
                move |res| {
                    let _ = tx.send(res);
                },
                Config::default().with_poll_interval(Duration::from_secs(1)),
            )
            .expect("Failed to create watcher"),
        );
        
        self.rx = Some(rx);
        
        // Start watching
        if let Some(ref mut watcher) = self.watcher {
            for path in &self.paths {
                let mode = if path.is_dir() {
                    RecursiveMode::Recursive
                } else {
                    RecursiveMode::NonRecursive
                };
                let _ = watcher.watch(path, mode);
            }
        }
    }

    /// Check for file changes
    pub fn poll_changes(&mut self) -> Vec<FileChange> {
        let mut changes = Vec::new();
        
        if let Some(ref rx) = self.rx {
            while let Ok(result) = rx.try_recv() {
                if let Ok(event) = result {
                    for path in event.paths {
                        let kind = match event.kind {
                            notify::EventKind::Create(_) => FileEventKind::Created,
                            notify::EventKind::Modify(_) => FileEventKind::Modified,
                            notify::EventKind::Remove(_) => FileEventKind::Removed,
                            _ => FileEventKind::Any,
                        };
                        changes.push(FileChange { path, kind });
                    }
                }
            }
        }
        
        changes
    }

    /// Check if any watched paths have changes
    pub fn has_changes(&mut self) -> bool {
        !self.poll_changes().is_empty()
    }

    /// Get the watched paths
    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }
}

impl Default for DevWatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Hot reload configuration
#[derive(Debug, Clone)]
pub struct HotReloadConfig {
    /// Enable hot reload
    pub enabled: bool,
    /// Debounce delay in milliseconds
    pub debounce_ms: u64,
    /// Max retry count for failed reloads
    pub max_retries: u32,
    /// Paths to watch (relative to project root)
    pub watch_paths: Vec<String>,
}

impl Default for HotReloadConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            debounce_ms: 300,
            max_retries: 3,
            watch_paths: vec![
                "src/".to_string(),
                "tests/".to_string(),
            ],
        }
    }
}

/// Dev tools integration
pub struct DevToolsHost {
    /// Whether devtools are enabled
    pub enabled: bool,
    /// Devtools server port
    pub port: u16,
    /// Timeline recording enabled
    pub timeline_enabled: bool,
    /// Performance profiling enabled
    pub profiling_enabled: bool,
}

impl DevToolsHost {
    /// Create a new dev tools host
    pub fn new(port: u16) -> Self {
        Self {
            enabled: true,
            port,
            timeline_enabled: true,
            profiling_enabled: true,
        }
    }

    /// Disable devtools
    pub fn disable(&mut self) {
        self.enabled = false;
    }
}

impl Default for DevToolsHost {
    fn default() -> Self {
        Self::new(9222)
    }
}

/// Dev server state
pub struct DevServer {
    pub watcher: DevWatcher,
    pub hot_reload: HotReloadConfig,
    pub devtools: DevToolsHost,
    /// Current build version
    pub version: u64,
    /// Pending changes
    pub pending_changes: Vec<FileChange>,
}

impl DevServer {
    /// Create a new dev server
    pub fn new() -> Self {
        Self {
            watcher: DevWatcher::new(),
            hot_reload: HotReloadConfig::default(),
            devtools: DevToolsHost::default(),
            version: 0,
            pending_changes: Vec::new(),
        }
    }

    /// Start watching files
    pub fn watch(&mut self, paths: Vec<PathBuf>) {
        self.watcher.set_paths(&paths);
    }

    /// Poll for changes and update version
    pub fn poll(&mut self) -> bool {
        let changes = self.watcher.poll_changes();
        if !changes.is_empty() {
            self.pending_changes.extend(changes);
            self.version += 1;
            return true;
        }
        false
    }

    /// Get pending changes and clear them
    pub fn take_changes(&mut self) -> Vec<FileChange> {
        std::mem::take(&mut self.pending_changes)
    }
}

impl Default for DevServer {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a file path should trigger a rebuild
pub fn is_relevant_file(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy().to_lowercase();
        matches!(ext_str.as_str(), "nyx" | "rs" | "json" | "toml")
    } else {
        false
    }
}

/// Check if a file path should trigger a full rebuild
pub fn is_build_critical(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy().to_lowercase();
        // Cargo.toml, build.rs, etc. trigger full rebuild
        matches!(ext_str.as_str(), "toml" | "rs")
    } else {
        false
    }
}

