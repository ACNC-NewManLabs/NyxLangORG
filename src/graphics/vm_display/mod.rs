//! Virtual Machine Display Backend
//!
//! This module provides VM display integration for rendering guest OS UIs
//! inside the Nyx renderer, similar to QEMU display backends.

use std::sync::{Arc, RwLock};

/// VM display buffer format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmBufferFormat {
    /// 32-bit BGRA format
    Bgra32,
    /// 24-bit BGR format
    Bgr24,
    /// 16-bit RGB565 format
    Rgb565,
    /// 8-bit grayscale
    Grayscale8,
}

/// VM display surface configuration
#[derive(Debug, Clone)]
pub struct VmDisplayConfig {
    pub width: u32,
    pub height: u32,
    pub buffer_format: VmBufferFormat,
    pub pitch: u32,
    pub refresh_rate: u32,
}

/// VM display surface handle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VmDisplayHandle(pub u64);

/// VM display update region
#[derive(Debug, Clone)]
pub struct VmUpdateRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// VM display backend trait - implemented by VM integrations
pub trait VmDisplayBackend: Send {
    /// Connect to VM display
    fn connect(&mut self, config: VmDisplayConfig) -> Result<VmDisplayHandle, VmDisplayError>;
    
    /// Disconnect from VM display
    fn disconnect(&mut self, handle: VmDisplayHandle) -> Result<(), VmDisplayError>;
    
    /// Get current framebuffer
    fn get_framebuffer(&self, handle: VmDisplayHandle) -> Result<Vec<u8>, VmDisplayError>;
    
    /// Get dirty regions since last update
    fn get_dirty_regions(&self, handle: VmDisplayHandle) -> Result<Vec<VmUpdateRegion>, VmDisplayError>;
    
    /// Send input event to VM guest
    fn send_input(&mut self, handle: VmDisplayHandle, event: VmInputEvent) -> Result<(), VmDisplayError>;
    
    /// Check if VM is still running
    fn is_connected(&self, handle: VmDisplayHandle) -> bool;
}

/// VM input event types
#[derive(Debug, Clone)]
pub enum VmInputEvent {
    /// Keyboard event
    KeyDown { keycode: u32 },
    KeyUp { keycode: u32 },
    /// Mouse/pointer event
    PointerMove { x: i32, y: i32 },
    PointerDown { x: i32, y: i32, button: u32 },
    PointerUp { x: i32, y: i32, button: u32 },
    /// Mouse wheel
    Wheel { delta_x: i32, delta_y: i32 },
    /// Text input
    TextInput { text: String },
}

/// VM display error
#[derive(Debug, Clone)]
pub struct VmDisplayError {
    pub message: String,
}

impl VmDisplayError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self { message: msg.into() }
    }
}

/// VM display manager - coordinates VM display rendering
pub struct VmDisplayManager {
    backends: Vec<Box<dyn VmDisplayBackend>>,
    active_displays: RwLock<BTreeMap<VmDisplayHandle, VmDisplayConfig>>,
}

impl VmDisplayManager {
    pub fn new() -> Self {
        Self {
            backends: Vec::new(),
            active_displays: RwLock::new(BTreeMap::new()),
        }
    }
    
    pub fn register_backend(&mut self, backend: Box<dyn VmDisplayBackend>) {
        self.backends.push(backend);
    }
    
    pub fn create_display(&mut self, config: VmDisplayConfig) -> Result<VmDisplayHandle, VmDisplayError> {
        if let Some(backend) = self.backends.first_mut() {
            backend.connect(config)
        } else {
            Err(VmDisplayError::new("No VM display backend registered"))
        }
    }
}

use std::collections::BTreeMap;

