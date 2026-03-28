use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use crate::runtime::compiler_bridge::incremental::incremental_patch_set;
use crate::runtime::execution::reload::ModulePatch;
use crate::runtime::execution::RuntimeError;

pub struct FileWatcher {
    watcher: RecommendedWatcher,
    sender: UnboundedWatcherSender,
    entry_path: PathBuf,
    version_counter: Arc<std::sync::atomic::AtomicU64>,
}

type UnboundedWatcherSender = Arc<tokio::sync::mpsc::UnboundedSender<FileWatcherEvent>>;

#[derive(Debug, Clone)]
pub struct FileWatcherEvent {
    pub event_type: FileWatcherEventType,
    pub file_path: PathBuf,
    pub patches: Option<Vec<ModulePatch>>,
}

#[derive(Debug, Clone)]
pub enum FileWatcherEventType {
    FileChanged,
    FileCreated,
    FileDeleted,
    DirectoryChanged,
}

impl FileWatcher {
    pub fn new<P: AsRef<Path>>(
        entry_path: P,
        sender: UnboundedSender<FileWatcherEvent>,
    ) -> Result<Self, FileWatcherError> {
        let entry_path = entry_path.as_ref().to_path_buf();
        let sender = Arc::new(sender);
        let version_counter = Arc::new(std::sync::atomic::AtomicU64::new(1));
        
        let sender_clone = sender.clone();
        let entry_path_clone = entry_path.clone();
        let version_counter_clone = version_counter.clone();
        
        let watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                match res {
                    Ok(event) => {
                        Self::handle_event(event, &entry_path_clone, &sender_clone, &version_counter_clone);
                    }
                    Err(e) => {
                        eprintln!("File watcher error: {:?}", e);
                    }
                }
            },
            Config::default().with_poll_interval(Duration::from_millis(100)),
        )?;

        let mut file_watcher = Self {
            watcher,
            sender,
            entry_path,
            version_counter,
        };

        // Start watching the entry directory
        file_watcher.start_watching()?;

        Ok(file_watcher)
    }

    fn start_watching(&mut self) -> Result<(), FileWatcherError> {
        let watch_path = self.entry_path.parent()
            .ok_or(FileWatcherError::InvalidEntryPath)?;
        
        self.watcher.watch(watch_path, RecursiveMode::Recursive)?;
        Ok(())
    }

    fn handle_event(
        event: Event,
        entry_path: &Path,
        sender: &UnboundedWatcherSender,
        version_counter: &Arc<std::sync::atomic::AtomicU64>,
    ) {
        for path in &event.paths {
            // Only process .nyx files
            if path.extension().map(|ext| ext != "nyx").unwrap_or(true) {
                continue;
            }

            let event_type = match event.kind {
                EventKind::Create(_) => FileWatcherEventType::FileCreated,
                EventKind::Modify(_) => FileWatcherEventType::FileChanged,
                EventKind::Remove(_) => FileWatcherEventType::FileDeleted,
                EventKind::Any => FileWatcherEventType::DirectoryChanged,
                _ => continue,
            };

            // Generate patches for file changes
            let patches = if matches!(event_type, FileWatcherEventType::FileChanged) {
                let version = version_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                match incremental_patch_set(entry_path, path, version) {
                    Ok(patches) => Some(patches),
                    Err(e) => {
                        eprintln!("Failed to generate patches for {:?}: {}", path, e);
                        None
                    }
                }
            } else {
                None
            };

            let watcher_event = FileWatcherEvent {
                event_type,
                file_path: path.clone(),
                patches,
            };

            if let Err(e) = sender.send(watcher_event) {
                eprintln!("Failed to send file watcher event: {}", e);
            }
        }
    }

    pub fn stop(&mut self) -> Result<(), FileWatcherError> {
        let watch_path = self.entry_path.parent()
            .ok_or(FileWatcherError::InvalidEntryPath)?;
        
        self.watcher.unwatch(watch_path)?;
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FileWatcherError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Notify error: {0}")]
    Notify(#[from] notify::Error),
    
    #[error("Invalid entry path")]
    InvalidEntryPath,
    
    #[error("Runtime error: {0}")]
    Runtime(#[from] RuntimeError),
}

pub struct DebouncedFileWatcher {
    inner: FileWatcher,
    debounce_duration: Duration,
    pending_events: std::collections::HashMap<PathBuf, std::time::Instant>,
}

impl DebouncedFileWatcher {
    pub fn new<P: AsRef<Path>>(
        entry_path: P,
        sender: UnboundedSender<FileWatcherEvent>,
        debounce_duration: Duration,
    ) -> Result<Self, FileWatcherError> {
        let inner = FileWatcher::new(entry_path, sender)?;
        Ok(Self {
            inner,
            debounce_duration,
            pending_events: std::collections::HashMap::new(),
        })
    }

    pub async fn process_events(&mut self) -> Result<(), FileWatcherError> {
        // This would integrate with the event processing loop
        // For now, just return Ok
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_file_watcher_creation() {
        let temp_dir = TempDir::new().unwrap();
        let entry_file = temp_dir.path().join("main.nyx");
        std::fs::write(&entry_file, "fn main() {}").unwrap();

        let (sender, _receiver) = mpsc::unbounded_channel();
        
        let watcher = FileWatcher::new(&entry_file, sender);
        assert!(watcher.is_ok());
    }
}
