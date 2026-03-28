use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use winit::event_loop::{EventLoop, ControlFlow};
use winit::window::{Window, WindowBuilder};
use winit::event::{Event, WindowEvent, KeyboardInput, ElementState, VirtualKeyCode};


use crate::devtools::protocol::DevtoolsEnvelope;
use crate::graphics::backends::wgpu::renderer::WgpuRenderer;
use crate::accessibility::semantics::DesktopAccessibilityBridge;

use super::asset_host::AssetHost;
use super::runtime_host::{HostError, PlatformEvent, RuntimeHost, SemanticsDelta, SurfaceConfig, SurfaceHandle};

// Removed Debug, Clone since EventLoop is neither
pub struct DesktopHost {
    next_surface: u64,
    asset_host: AssetHost,
    windows: Arc<Mutex<HashMap<u64, DesktopWindow>>>,
    event_loop: Option<EventLoop<()>>,
    _accessibility_bridge: DesktopAccessibilityBridge,
}

// Removed Debug
pub struct DesktopWindow {
    window: Window,
    renderer: Option<WgpuRenderer>,
    _surface_id: u64,
    size: winit::dpi::PhysicalSize<u32>,
}

impl DesktopHost {
    pub fn new(asset_root: PathBuf) -> Self {
        Self {
            next_surface: 1,
            asset_host: AssetHost::new(asset_root),
            windows: Arc::new(Mutex::new(HashMap::new())),
            event_loop: None,
            _accessibility_bridge: DesktopAccessibilityBridge {},
        }
    }

    pub fn initialize(&mut self) -> Result<(), HostError> {
        let event_loop = EventLoop::new();
        
        self.event_loop = Some(event_loop);
        Ok(())
    }

    pub fn run_event_loop<F>(mut self, mut callback: F) -> Result<(), HostError>
    where
        F: FnMut(Vec<PlatformEvent>) -> Result<(), HostError> + 'static,
    {
        let event_loop = self.event_loop.take()
            .ok_or_else(|| HostError::new("NotInitialized"))?;

        let _windows = self.windows.clone();
        let _last_events: Vec<PlatformEvent> = Vec::new();

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;

            let mut events = Vec::new();

            match event {
                Event::WindowEvent { window_id, event } => {
                    if let Some(window_event) = self.handle_window_event(window_id, event) {
                        events.push(window_event);
                    }
                }
                Event::MainEventsCleared => {
                    // Process accumulated events
                    if !events.is_empty() {
                        if let Err(e) = callback(events) {
                            eprintln!("Error processing events: {}", e.message);
                        }
                    }
                }
                Event::RedrawRequested(window_id) => {
                    // Handle redraw
                    if let Err(e) = self.handle_redraw(window_id) {
                        eprintln!("Error handling redraw: {}", e.message);
                    }
                }
                _ => {}
            }
        });
    }

    fn handle_window_event(&self, window_id: winit::window::WindowId, event: WindowEvent) -> Option<PlatformEvent> {
        match event {
            WindowEvent::CloseRequested => {
                Some(PlatformEvent::Quit)
            }
            WindowEvent::Resized(size) => {
                self.update_window_size(window_id, size);
                None
            }
            WindowEvent::KeyboardInput { input, .. } => {
                self.handle_keyboard_input(window_id, input)
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.handle_mouse_input(window_id, state, button)
            }
            WindowEvent::CursorMoved { position, .. } => {
                Some(PlatformEvent::MouseMove {
                    x: position.x as f32,
                    y: position.y as f32,
                })
            }
            WindowEvent::Focused(focused) => {
                if focused {
                    Some(PlatformEvent::FocusGained)
                } else {
                    Some(PlatformEvent::FocusLost)
                }
            }
            _ => None,
        }
    }

    fn handle_keyboard_input(&self, _window_id: winit::window::WindowId, input: KeyboardInput) -> Option<PlatformEvent> {
        match input.state {
            ElementState::Pressed => {
                if let Some(virtual_keycode) = input.virtual_keycode {
                    let key = self.virtual_keycode_to_string(virtual_keycode);
                    Some(PlatformEvent::KeyDown {
                        key,
                        code: input.scancode.to_string(),
                    })
                } else {
                    None
                }
            }
            ElementState::Released => {
                if let Some(virtual_keycode) = input.virtual_keycode {
                    let key = self.virtual_keycode_to_string(virtual_keycode);
                    Some(PlatformEvent::KeyUp {
                        key,
                        code: input.scancode.to_string(),
                    })
                } else {
                    None
                }
            }
        }
    }

    fn handle_mouse_input(&self, _window_id: winit::window::WindowId, state: ElementState, button: winit::event::MouseButton) -> Option<PlatformEvent> {
        let button_u32 = match button {
            winit::event::MouseButton::Left => 0,
            winit::event::MouseButton::Right => 1,
            winit::event::MouseButton::Middle => 2,
            winit::event::MouseButton::Other(_) => 3,
        };

        match state {
            ElementState::Pressed => {
                Some(PlatformEvent::MouseDown {
                    x: 0.0,
                    y: 0.0,
                    button: button_u32,
                })
            }
            ElementState::Released => {
                Some(PlatformEvent::MouseUp {
                    x: 0.0,
                    y: 0.0,
                    button: button_u32,
                })
            }
        }
    }

    fn virtual_keycode_to_string(&self, keycode: VirtualKeyCode) -> String {
        match keycode {
            VirtualKeyCode::A => "a".to_string(),
            VirtualKeyCode::B => "b".to_string(),
            VirtualKeyCode::C => "c".to_string(),
            VirtualKeyCode::D => "d".to_string(),
            VirtualKeyCode::E => "e".to_string(),
            VirtualKeyCode::F => "f".to_string(),
            VirtualKeyCode::G => "g".to_string(),
            VirtualKeyCode::H => "h".to_string(),
            VirtualKeyCode::I => "i".to_string(),
            VirtualKeyCode::J => "j".to_string(),
            VirtualKeyCode::K => "k".to_string(),
            VirtualKeyCode::L => "l".to_string(),
            VirtualKeyCode::M => "m".to_string(),
            VirtualKeyCode::N => "n".to_string(),
            VirtualKeyCode::O => "o".to_string(),
            VirtualKeyCode::P => "p".to_string(),
            VirtualKeyCode::Q => "q".to_string(),
            VirtualKeyCode::R => "r".to_string(),
            VirtualKeyCode::S => "s".to_string(),
            VirtualKeyCode::T => "t".to_string(),
            VirtualKeyCode::U => "u".to_string(),
            VirtualKeyCode::V => "v".to_string(),
            VirtualKeyCode::W => "w".to_string(),
            VirtualKeyCode::X => "x".to_string(),
            VirtualKeyCode::Y => "y".to_string(),
            VirtualKeyCode::Z => "z".to_string(),
            VirtualKeyCode::Key1 => "1".to_string(),
            VirtualKeyCode::Key2 => "2".to_string(),
            VirtualKeyCode::Key3 => "3".to_string(),
            VirtualKeyCode::Key4 => "4".to_string(),
            VirtualKeyCode::Key5 => "5".to_string(),
            VirtualKeyCode::Key6 => "6".to_string(),
            VirtualKeyCode::Key7 => "7".to_string(),
            VirtualKeyCode::Key8 => "8".to_string(),
            VirtualKeyCode::Key9 => "9".to_string(),
            VirtualKeyCode::Key0 => "0".to_string(),
            VirtualKeyCode::Space => " ".to_string(),
            VirtualKeyCode::Return => "Enter".to_string(),
            VirtualKeyCode::Escape => "Escape".to_string(),
            VirtualKeyCode::Tab => "Tab".to_string(),
            VirtualKeyCode::Back => "Backspace".to_string(),
            VirtualKeyCode::Delete => "Delete".to_string(),
            VirtualKeyCode::Insert => "Insert".to_string(),
            VirtualKeyCode::Home => "Home".to_string(),
            VirtualKeyCode::End => "End".to_string(),
            VirtualKeyCode::PageUp => "PageUp".to_string(),
            VirtualKeyCode::PageDown => "PageDown".to_string(),
            VirtualKeyCode::Left => "ArrowLeft".to_string(),
            VirtualKeyCode::Right => "ArrowRight".to_string(),
            VirtualKeyCode::Up => "ArrowUp".to_string(),
            VirtualKeyCode::Down => "ArrowDown".to_string(),
            VirtualKeyCode::F1 => "F1".to_string(),
            VirtualKeyCode::F2 => "F2".to_string(),
            VirtualKeyCode::F3 => "F3".to_string(),
            VirtualKeyCode::F4 => "F4".to_string(),
            VirtualKeyCode::F5 => "F5".to_string(),
            VirtualKeyCode::F6 => "F6".to_string(),
            VirtualKeyCode::F7 => "F7".to_string(),
            VirtualKeyCode::F8 => "F8".to_string(),
            VirtualKeyCode::F9 => "F9".to_string(),
            VirtualKeyCode::F10 => "F10".to_string(),
            VirtualKeyCode::F11 => "F11".to_string(),
            VirtualKeyCode::F12 => "F12".to_string(),
            _ => format!("{:?}", keycode),
        }
    }

    fn update_window_size(&self, window_id: winit::window::WindowId, size: winit::dpi::PhysicalSize<u32>) {
        let mut windows = self.windows.lock().unwrap();
        if let Some(window) = windows.values_mut().find(|w| w.window.id() == window_id) {
            window.size = size;
            if let Some(renderer) = &mut window.renderer {
                renderer.resize(size);
            }
        }
    }

    fn handle_redraw(&self, _window_id: winit::window::WindowId) -> Result<(), HostError> {
        // This would trigger a frame render
        // For now, just return Ok
        Ok(())
    }
}

impl RuntimeHost for DesktopHost {
    fn create_surface(&mut self, config: SurfaceConfig) -> Result<SurfaceHandle, HostError> {
        let event_loop = self.event_loop.as_ref()
            .ok_or_else(|| HostError::new("NotInitialized"))?;

        let window = WindowBuilder::new()
            .with_inner_size(winit::dpi::PhysicalSize::new(config.width, config.height))
            .build(event_loop)
            .map_err(|e| HostError::new(format!("Failed to create window: {}", e)))?;

        let surface_id = self.next_surface;
        self.next_surface += 1;

        let desktop_window = DesktopWindow {
            window,
            renderer: None, // Will be initialized later
            _surface_id: surface_id,
            size: winit::dpi::PhysicalSize::new(config.width, config.height),
        };

        self.windows.lock().unwrap().insert(surface_id, desktop_window);
        Ok(SurfaceHandle(surface_id))
    }

    fn poll_events(&mut self) -> Result<Vec<PlatformEvent>, HostError> {
        // Events are handled in the event loop
        Ok(vec![])
    }

    fn read_asset(&self, asset_id: &str) -> Result<Vec<u8>, HostError> {
        self.asset_host.read(asset_id)
    }

    fn emit_devtools(&self, event: DevtoolsEnvelope) -> Result<(), HostError> {
        // Emit devtools event (would integrate with DevTools server)
        println!("DevTools event: {:?}", event);
        Ok(())
    }

    fn publish_semantics(&self, _delta: SemanticsDelta) -> Result<(), HostError> {
        // Publish accessibility semantics
        // self.accessibility_bridge.update_node(&delta.node_id, &delta.node);
        Ok(())
    }

    fn watch_paths(&mut self, paths: &[PathBuf]) -> Result<(), HostError> {
        // Set up file watching for hot reload
        for path in paths {
            println!("Watching path: {:?}", path);
        }
        Ok(())
    }
}

// Native renderer integration
impl DesktopHost {
    pub fn initialize_renderer(&mut self, surface_id: u64) -> Result<(), HostError> {
        let windows = self.windows.lock().unwrap();
        let _window = windows.get(&surface_id)
            .ok_or_else(|| HostError::new(format!("InvalidSurface: {:?}", surface_id)))?;

        // Initialize wgpu renderer
        // Note: This is a simplified version - actual implementation would be async
        println!("Initializing renderer for surface {}", surface_id);
        
        Ok(())
    }

    pub fn render_frame(&mut self, surface_id: u64, display_list: &crate::graphics::renderer::display_list::DisplayList) -> Result<(), HostError> {
        let mut windows = self.windows.lock().unwrap();
        let window = windows.get_mut(&surface_id)
            .ok_or_else(|| HostError::new(format!("InvalidSurface: {:?}", surface_id)))?;

        if let Some(renderer) = &mut window.renderer {
            // Render the frame
            renderer.render(display_list)
                .map_err(|e| HostError::new(e.to_string()))?;
        }

        Ok(())
    }

    pub fn get_window_size(&self, surface_id: u64) -> Result<(u32, u32), HostError> {
        let windows = self.windows.lock().unwrap();
        let window = windows.get(&surface_id)
            .ok_or_else(|| HostError::new(format!("InvalidSurface: {:?}", surface_id)))?;

        Ok((window.size.width, window.size.height))
    }

    pub fn set_window_title(&self, surface_id: u64, title: &str) -> Result<(), HostError> {
        let windows = self.windows.lock().unwrap();
        let window = windows.get(&surface_id)
            .ok_or_else(|| HostError::new(format!("InvalidSurface: {:?}", surface_id)))?;

        window.window.set_title(title);
        Ok(())
    }

    pub fn request_redraw(&self, surface_id: u64) -> Result<(), HostError> {
        let windows = self.windows.lock().unwrap();
        let window = windows.get(&surface_id)
            .ok_or_else(|| HostError::new(format!("InvalidSurface: {:?}", surface_id)))?;

        window.window.request_redraw();
        Ok(())
    }
}
