//! OS UI Framework Integration
//!
//! Provides system UI framework capabilities for operating systems including:
//! - Window compositor integration
//! - Surface management
//! - Multi-window UI rendering
//! - Hardware accelerated compositing
//! - Multi-monitor high-DPI support

use std::collections::BTreeMap;

/// Window handle for OS windows
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowHandle(pub u64);

/// Window configuration
#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub decorations: bool,
    pub resizable: bool,
    pub transparent: bool,
    pub always_on_top: bool,
    pub fullscreen: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Nyx Application".to_string(),
            x: 100,
            y: 100,
            width: 800,
            height: 600,
            decorations: true,
            resizable: true,
            transparent: false,
            always_on_top: false,
            fullscreen: false,
        }
    }
}

/// Display/Monitor information
#[derive(Debug, Clone)]
pub struct DisplayInfo {
    pub id: u32,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
    pub refresh_rate: u32,
    pub is_primary: bool,
}

/// Surface buffer format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceFormat {
    Bgra8Srgb,
    Rgba8Srgb,
    Rgba16Float,
}

/// OS compositor capabilities
#[derive(Debug, Clone)]
pub struct CompositorCapabilities {
    pub supports_overlay: bool,
    pub supports_blur: bool,
    pub supports_vsync: bool,
    pub max_texture_size: u32,
    pub max_layers: u32,
}

/// Window manager trait - implemented by platform backends
pub trait WindowManager: Send {
    /// Create a new window
    fn create_window(&mut self, config: WindowConfig) -> Result<WindowHandle, WindowError>;
    
    /// Destroy a window
    fn destroy_window(&mut self, handle: WindowHandle) -> Result<(), WindowError>;
    
    /// Get window bounds
    fn get_window_bounds(&self, handle: WindowHandle) -> Result<WindowConfig, WindowError>;
    
    /// Set window bounds
    fn set_window_bounds(&mut self, handle: WindowHandle, config: WindowConfig) -> Result<(), WindowError>;
    
    /// Set window title
    fn set_window_title(&mut self, handle: WindowHandle, title: &str) -> Result<(), WindowError>;
    
    /// Show window
    fn show_window(&mut self, handle: WindowHandle) -> Result<(), WindowError>;
    
    /// Hide window
    fn hide_window(&mut self, handle: WindowHandle) -> Result<(), WindowError>;
    
    /// Request window focus
    fn focus_window(&mut self, handle: WindowHandle) -> Result<(), WindowError>;
    
    /// Get available displays
    fn get_displays(&self) -> Result<Vec<DisplayInfo>, WindowError>;
    
    /// Get compositor capabilities
    fn get_compositor_caps(&self) -> CompositorCapabilities;
}

/// Surface for rendering
pub struct WindowSurface {
    pub window: WindowHandle,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
    pub format: SurfaceFormat,
}

/// Window error
#[derive(Debug, Clone)]
pub struct WindowError {
    pub message: String,
}

impl WindowError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self { message: msg.into() }
    }
}

/// OS UI application
pub struct OsUiApp {
    windows: BTreeMap<WindowHandle, WindowConfig>,
    displays: Vec<DisplayInfo>,
    focused_window: Option<WindowHandle>,
}

impl OsUiApp {
    pub fn new() -> Self {
        Self {
            windows: BTreeMap::new(),
            displays: Vec::new(),
            focused_window: None,
        }
    }
    
    pub fn create_window(&mut self, config: WindowConfig) -> Result<WindowHandle, WindowError> {
        let handle = WindowHandle(self.windows.len() as u64 + 1);
        self.windows.insert(handle, config);
        Ok(handle)
    }
    
    pub fn destroy_window(&mut self, handle: WindowHandle) -> Result<(), WindowError> {
        self.windows.remove(&handle);
        if self.focused_window == Some(handle) {
            self.focused_window = None;
        }
        Ok(())
    }
    
    pub fn set_focus(&mut self, handle: WindowHandle) -> Result<(), WindowError> {
        if self.windows.contains_key(&handle) {
            self.focused_window = Some(handle);
            Ok(())
        } else {
            Err(WindowError::new("Window not found"))
        }
    }
}

impl Default for OsUiApp {
    fn default() -> Self {
        Self::new()
    }
}

/// Input routing across windows
pub struct InputRouter {
    window_targets: BTreeMap<WindowHandle, InputTarget>,
}

impl InputRouter {
    pub fn new() -> Self {
        Self {
            window_targets: BTreeMap::new(),
        }
    }
    
    pub fn register_window(&mut self, window: WindowHandle, target: InputTarget) {
        self.window_targets.insert(window, target);
    }
    
    pub fn route_input(&self, window: WindowHandle, event: &InputEvent) -> bool {
        if let Some(target) = self.window_targets.get(&window) {
            target.dispatch(event)
        } else {
            false
        }
    }
}

/// Input target for dispatch
pub struct InputTarget {
    pub element_id: String,
}

impl InputTarget {
    pub fn dispatch(&self, event: &InputEvent) -> bool {
        // Dispatch to element
        true
    }
}

/// Input event
#[derive(Debug, Clone)]
pub struct InputEvent {
    pub event_type: InputEventType,
    pub x: f32,
    pub y: f32,
    pub key: Option<String>,
    pub modifiers: u32,
}

/// Input event type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEventType {
    MouseMove,
    MouseDown,
    MouseUp,
    KeyDown,
    KeyUp,
    Scroll,
    TouchStart,
    TouchMove,
    TouchEnd,
}

