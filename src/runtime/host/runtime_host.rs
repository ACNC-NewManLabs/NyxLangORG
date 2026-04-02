//! Runtime Host - Core Platform Abstraction
//!
//! This module defines the core runtime host interface that abstracts
//! over different platform backends (web, desktop, mobile, dev).

use crate::devtools::protocol::DevtoolsEnvelope;
use crate::graphics::renderer::display_list::DisplayList;
use std::path::PathBuf;

/// Surface handle for rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceHandle(pub u64);

/// Surface configuration
#[derive(Debug, Clone)]
pub struct SurfaceConfig {
    pub width: u32,
    pub height: u32,
    pub device_pixel_ratio: f32,
    pub vsync: bool,
}

/// Platform event types
#[derive(Debug, Clone)]
pub enum PlatformEvent {
    /// Frame tick (requestAnimationFrame equivalent)
    Tick,
    /// Window resized
    Resize { width: u32, height: u32 },
    /// Mouse move
    MouseMove { x: f32, y: f32 },
    /// Mouse down
    MouseDown { x: f32, y: f32, button: u32 },
    /// Mouse up
    MouseUp { x: f32, y: f32, button: u32 },
    /// Key down
    KeyDown { key: String, code: String },
    /// Key up
    KeyUp { key: String, code: String },
    /// Text input
    TextInput { text: String },
    /// Touch start
    TouchStart { x: f32, y: f32, id: u64 },
    /// Touch move
    TouchMove { x: f32, y: f32, id: u64 },
    /// Touch end
    TouchEnd { x: f32, y: f32, id: u64 },
    /// Scroll
    Scroll {
        x: f32,
        y: f32,
        delta_x: f32,
        delta_y: f32,
    },
    /// Focus gained
    FocusGained,
    /// Focus lost
    FocusLost,
    /// Window closed or quit requested
    Quit,
}

/// Semantics delta for accessibility
#[derive(Debug, Clone)]
pub struct SemanticsDelta {
    pub updates: Vec<SemanticsNode>,
    pub removals: Vec<String>,
}

/// Semantics node for accessibility
#[derive(Debug, Clone)]
pub struct SemanticsNode {
    pub id: String,
    pub label: String,
    pub role: String,
    pub value: String,
    pub is_focusable: bool,
    pub is_checked: bool,
}

/// Host error type
#[derive(Debug, Clone)]
pub struct HostError {
    pub message: String,
}

impl HostError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
        }
    }
}

pub trait RuntimeHost {
    /// Create a new rendering surface
    fn create_surface(&mut self, config: SurfaceConfig) -> Result<SurfaceHandle, HostError>;

    /// Poll for platform events
    fn poll_events(&mut self) -> Result<Vec<PlatformEvent>, HostError>;

    /// Read an asset by ID
    fn read_asset(&self, asset_id: &str) -> Result<Vec<u8>, HostError>;

    /// Emit a devtools event
    fn emit_devtools(&self, event: DevtoolsEnvelope) -> Result<(), HostError>;

    /// Publish semantics updates for accessibility
    fn publish_semantics(&self, delta: SemanticsDelta) -> Result<(), HostError>;

    /// Watch paths for hot reload
    fn watch_paths(&mut self, paths: &[PathBuf]) -> Result<(), HostError>;

    /// Render a frame to a surface
    fn render(
        &mut self,
        surface: SurfaceHandle,
        display_list: &DisplayList,
    ) -> Result<(), HostError> {
        let _ = surface;
        let _ = display_list;
        Ok(())
    }
}

/// Default implementation for optional methods
impl<T: RuntimeHost> RuntimeHost for Box<T> {
    fn create_surface(&mut self, config: SurfaceConfig) -> Result<SurfaceHandle, HostError> {
        (**self).create_surface(config)
    }

    fn poll_events(&mut self) -> Result<Vec<PlatformEvent>, HostError> {
        (**self).poll_events()
    }

    fn read_asset(&self, asset_id: &str) -> Result<Vec<u8>, HostError> {
        (**self).read_asset(asset_id)
    }

    fn emit_devtools(&self, event: DevtoolsEnvelope) -> Result<(), HostError> {
        (**self).emit_devtools(event)
    }

    fn publish_semantics(&self, delta: SemanticsDelta) -> Result<(), HostError> {
        (**self).publish_semantics(delta)
    }

    fn watch_paths(&mut self, paths: &[PathBuf]) -> Result<(), HostError> {
        (**self).watch_paths(paths)
    }

    fn render(
        &mut self,
        surface: SurfaceHandle,
        display_list: &DisplayList,
    ) -> Result<(), HostError> {
        (**self).render(surface, display_list)
    }
}
