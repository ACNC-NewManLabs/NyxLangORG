use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use base64::{engine::general_purpose, Engine as _};
#[cfg(feature = "ui")]
use font8x8::{UnicodeFonts, BASIC_FONTS};
use httpdate::fmt_http_date;
#[cfg(feature = "ui")]
use minifb::{KeyRepeat, MouseButton, Scale, ScaleMode, Window, WindowOptions};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use regex::Regex;
use rustls::pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
use rustls::{ServerConfig, ServerConnection, StreamOwned};
use sha1::{Digest, Sha1};
use socket2::{Domain, Protocol, Socket, Type};
#[cfg(feature = "ui")]
use std::cell::RefCell;

use super::nyx_vm::{EvalError, NyxVm, Value};

#[derive(Debug, Clone)]
pub struct NativeBridgeConfig {
    pub asset_root: PathBuf,
    pub runtime_name: String,
}

static RUNTIME_NAME: OnceLock<String> = OnceLock::new();
static ASSET_ROOT: OnceLock<PathBuf> = OnceLock::new();
static START_TIME: OnceLock<Instant> = OnceLock::new();
#[cfg(feature = "ui")]
thread_local! {
    static RENDERER_STATE: RefCell<Option<RendererState>> = RefCell::new(None);
}
static SOCKETS: OnceLock<Mutex<HashMap<i64, SocketEntry>>> = OnceLock::new();
static NEXT_SOCKET_ID: AtomicI64 = AtomicI64::new(1);
static WATCHERS: OnceLock<Mutex<HashMap<i64, WatchEntry>>> = OnceLock::new();
static NEXT_WATCH_ID: AtomicI64 = AtomicI64::new(1);
static TLS_CONNS: OnceLock<Mutex<HashMap<i64, TlsEntry>>> = OnceLock::new();
static NEXT_TLS_ID: AtomicI64 = AtomicI64::new(1);

#[cfg(feature = "ui")]
#[derive(Debug)]
struct RendererState {
    window: Window,
    buffer: Vec<u32>,
    width: usize,
    height: usize,
    clear_color: u32,
    last_mouse: (f32, f32),
    last_buttons: [bool; 3],
}

#[derive(Debug)]
enum SocketEntry {
    Pending { reuse_addr: bool, reuse_port: bool },
    Listener(TcpListener),
    Stream(TcpStream),
}

#[derive(Debug)]
struct WatchEntry {
    _watcher: RecommendedWatcher,
    rx: mpsc::Receiver<notify::Result<notify::Event>>,
}

#[derive(Debug)]
struct TlsEntry {
    stream: StreamOwned<ServerConnection, TcpStream>,
    peer_cert: Option<Vec<u8>>,
    cipher: Option<String>,
    verified: bool,
}

fn err(message: impl Into<String>) -> EvalError {
    EvalError {
        message: message.into(),
        stack: vec![],
    }
}

pub fn register_host_natives(vm: &mut NyxVm, config: &NativeBridgeConfig) {
    let _ = RUNTIME_NAME.set(config.runtime_name.clone());
    let _ = ASSET_ROOT.set(config.asset_root.clone());
    let _ = START_TIME.set(Instant::now());

    vm.register_native("runtime::name", runtime_name_native);
    vm.register_native("assets::read", assets_read_native);
    vm.register_native("surface::create", surface_create_native);
    vm.register_native("time::now_monotonic", time_now_monotonic_native);
    vm.register_native("time::sleep_ms", time_sleep_ms_native);
    vm.register_native("__native_time_ms", time_now_ms_native);
    vm.register_native("__native_time_ns", time_now_monotonic_native);
    vm.register_native("__native_sleep", time_sleep_ms_native);
    vm.register_native("__native_input_poll", input_poll_native);
    vm.register_native("input::poll", input_poll_native);
    vm.register_native("__native_renderer_init", renderer_init_native);
    vm.register_native("__native_renderer_init_wgpu", renderer_init_wgpu_native);
    vm.register_native("__native_renderer_render", renderer_render_native);
    vm.register_native(
        "__native_renderer_render_frame",
        renderer_render_frame_native,
    );
    vm.register_native("__native_renderer_present", renderer_present_native);
    vm.register_native("__native_renderer_resize", renderer_resize_native);
    vm.register_native("__native_frame_pacing", frame_pacing_native);
    vm.register_native("__native_should_exit", should_exit_native);
    vm.register_native("__native_renderer_shutdown", renderer_shutdown_native);
    vm.register_native("__native_reload_file", reload_file_native);
    vm.register_native("__native_measure_text", measure_text_native);
    vm.register_native("__native_measure_element", measure_element_native);
    vm.register_native("__native_create_socket", create_socket_native);
    vm.register_native("__native_close_socket", close_socket_native);
    vm.register_native("__native_bind_socket", bind_socket_native);
    vm.register_native("__native_listen_socket", listen_socket_native);
    vm.register_native("__native_accept_socket", accept_socket_native);
    vm.register_native("__native_connect_socket", connect_socket_native);
    vm.register_native("__native_read_socket", read_socket_native);
    vm.register_native("__native_write_socket", write_socket_native);
    vm.register_native("__native_set_socket_option", set_socket_option_native);
    vm.register_native("__native_fs_watch_create", fs_watch_create_native);
    vm.register_native("__native_fs_watch_poll", fs_watch_poll_native);
    vm.register_native("__native_fs_watch_destroy", fs_watch_destroy_native);
    vm.register_native("__native_fs_modified", fs_modified_native);
    vm.register_native("__native_fs_listdir", fs_listdir_native);
    vm.register_native("__native_fs_exists", fs_exists_native);
    vm.register_native("__native_fs_is_dir", fs_is_dir_native);
    vm.register_native("__native_regex_match", regex_match_native);
    vm.register_native("__native_sha1_base64", sha1_base64_native);
    vm.register_native("__native_file_read", file_read_native);
    vm.register_native("__native_tls_init_context", tls_init_context_native);
    vm.register_native("__native_tls_accept", tls_accept_native);
    vm.register_native("__native_tls_read", tls_read_native);
    vm.register_native("__native_tls_write", tls_write_native);
    vm.register_native("__native_tls_close", tls_close_native);
    vm.register_native("__native_tls_get_peer_cert", tls_get_peer_cert_native);
    vm.register_native("__native_tls_get_cipher", tls_get_cipher_native);
    vm.register_native("__native_tls_is_verified", tls_is_verified_native);

    for name in [
        "surface::resize",
        "surface::present",
        "fonts::enumerate",
        "devtools::emit",
        "platform::accessibility_update",
    ] {
        vm.register_native(name, |_vm, _args| Ok(Value::Null));
    }
}

fn runtime_name_native(_vm: &mut NyxVm, _args: &[Value]) -> Result<Value, EvalError> {
    Ok(Value::Str(
        RUNTIME_NAME
            .get()
            .cloned()
            .unwrap_or_else(|| "nyx".to_string()),
    ))
}

fn assets_read_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Str(asset_id)] = args else {
        return Err(err("assets::read(asset_id) expected"));
    };
    let path = ASSET_ROOT
        .get()
        .cloned()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(asset_id);
    let bytes = std::fs::read(path).map_err(|e| err(e.to_string()))?;
    Ok(Value::array(
        bytes
            .into_iter()
            .map(|b| Value::Int(i64::from(b)))
            .collect(),
    ))
}

fn surface_create_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let mut surface = HashMap::new();
    surface.insert("kind".to_string(), Value::Str("surface".to_string()));
    surface.insert("args".to_string(), Value::Int(args.len() as i64));
    Ok(Value::object(surface))
}

fn time_now_monotonic_native(_vm: &mut NyxVm, _args: &[Value]) -> Result<Value, EvalError> {
    let start = START_TIME.get_or_init(Instant::now);
    let elapsed = start.elapsed();
    let ns = elapsed.as_nanos() as i64;
    Ok(Value::Int(ns))
}

fn time_now_ms_native(_vm: &mut NyxVm, _args: &[Value]) -> Result<Value, EvalError> {
    let start = START_TIME.get_or_init(Instant::now);
    let elapsed = start.elapsed();
    let ms = elapsed.as_millis() as i64;
    Ok(Value::Int(ms))
}

fn time_sleep_ms_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Int(ms)] = args else {
        return Err(err("sleep_ms(ms) expected"));
    };
    if *ms > 0 {
        std::thread::sleep(Duration::from_millis(*ms as u64));
    }
    Ok(Value::Null)
}

#[cfg(feature = "ui")]
fn input_poll_native(_vm: &mut NyxVm, _args: &[Value]) -> Result<Value, EvalError> {
    let mut events = Vec::new();
    RENDERER_STATE.with(|cell| {
        let mut state_ref = cell.borrow_mut();
        let Some(state) = state_ref.as_mut() else {
            return Ok(Value::array(events));
        };

        if !state.window.is_open() {
            return Ok(Value::array(events));
        }

        state.window.update();

        for key in state.window.get_keys_pressed(KeyRepeat::No) {
            let mut evt = HashMap::new();
            evt.insert("type".to_string(), Value::Str("KeyDown".to_string()));
            evt.insert("key".to_string(), Value::Str(format!("{key:?}")));
            evt.insert("code".to_string(), Value::Int(key as i64));
            evt.insert("modifiers".to_string(), Value::Int(0));
            events.push(Value::object(evt));
        }

        for key in state.window.get_keys_released() {
            let mut evt = HashMap::new();
            evt.insert("type".to_string(), Value::Str("KeyUp".to_string()));
            evt.insert("key".to_string(), Value::Str(format!("{key:?}")));
            evt.insert("code".to_string(), Value::Int(key as i64));
            evt.insert("modifiers".to_string(), Value::Int(0));
            events.push(Value::object(evt));
        }

        if let Some((x, y)) = state.window.get_mouse_pos(minifb::MouseMode::Clamp) {
            let dx = x - state.last_mouse.0;
            let dy = y - state.last_mouse.1;
            if dx != 0.0 || dy != 0.0 {
                let mut evt = HashMap::new();
                evt.insert("type".to_string(), Value::Str("MouseMove".to_string()));
                evt.insert("x".to_string(), Value::Float(x as f64));
                evt.insert("y".to_string(), Value::Float(y as f64));
                evt.insert("dx".to_string(), Value::Float(dx as f64));
                evt.insert("dy".to_string(), Value::Float(dy as f64));
                events.push(Value::object(evt));
                state.last_mouse = (x, y);
            }
        }

        let buttons = [
            (MouseButton::Left, 0),
            (MouseButton::Middle, 1),
            (MouseButton::Right, 2),
        ];
        for (button, index) in buttons {
            let down = state.window.get_mouse_down(button);
            let prev = state.last_buttons[index];
            if down != prev {
                let mut evt = HashMap::new();
                evt.insert(
                    "type".to_string(),
                    Value::Str(if down {
                        "MouseDown".to_string()
                    } else {
                        "MouseUp".to_string()
                    }),
                );
                evt.insert("button".to_string(), Value::Int(index as i64));
                if let Some((x, y)) = state.window.get_mouse_pos(minifb::MouseMode::Clamp) {
                    evt.insert("x".to_string(), Value::Float(x as f64));
                    evt.insert("y".to_string(), Value::Float(y as f64));
                }
                events.push(Value::object(evt));
                state.last_buttons[index] = down;
            }
        }

        Ok(Value::array(events))
    })
}

#[cfg(not(feature = "ui"))]
fn input_poll_native(_vm: &mut NyxVm, _args: &[Value]) -> Result<Value, EvalError> {
    Ok(Value::array(Vec::new()))
}

#[cfg(feature = "ui")]
fn renderer_init_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [_backend, vsync, _antialias] = args else {
        return Err(err(
            "__native_renderer_init(backend, vsync, antialiasing) expected",
        ));
    };
    let vsync = match vsync {
        Value::Bool(b) => *b,
        _ => false,
    };

    init_renderer_state(1280, 720, "Nyx UI", vsync)
}

#[cfg(not(feature = "ui"))]
fn renderer_init_native(_vm: &mut NyxVm, _args: &[Value]) -> Result<Value, EvalError> {
    Ok(Value::Bool(false))
}

#[cfg(feature = "ui")]
fn renderer_init_wgpu_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Int(width), Value::Int(height), Value::Str(title)] = args else {
        return Err(err(
            "__native_renderer_init_wgpu(width, height, title) expected",
        ));
    };
    init_renderer_state(*width as usize, *height as usize, title, true)
}

#[cfg(not(feature = "ui"))]
fn renderer_init_wgpu_native(_vm: &mut NyxVm, _args: &[Value]) -> Result<Value, EvalError> {
    Ok(Value::Bool(false))
}

#[cfg(feature = "ui")]
fn init_renderer_state(
    width: usize,
    height: usize,
    title: &str,
    vsync: bool,
) -> Result<Value, EvalError> {
    let mut options = WindowOptions::default();
    options.resize = true;
    options.scale = Scale::X1;
    options.scale_mode = ScaleMode::Stretch;

    let mut window = match Window::new(title, width, height, options) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("RENDERER ERROR: Failed to create window: {}", e);
            return Err(err(e.to_string()));
        }
    };
    if vsync {
        window.set_target_fps(60);
    }

    let state = RendererState {
        window,
        buffer: vec![0x0; width * height],
        width,
        height,
        clear_color: 0x101014,
        last_mouse: (0.0, 0.0),
        last_buttons: [false; 3],
    };

    RENDERER_STATE.with(|cell| {
        *cell.borrow_mut() = Some(state);
    });
    Ok(Value::Bool(true))
}

#[cfg(not(feature = "ui"))]
#[allow(dead_code)]
fn init_renderer_state(
    _width: usize,
    _height: usize,
    _title: &str,
    _vsync: bool,
) -> Result<Value, EvalError> {
    Ok(Value::Bool(false))
}

#[cfg(feature = "ui")]
fn renderer_render_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [display_list, dirty_regions] = args else {
        return Err(err(
            "__native_renderer_render(display_list, dirty_regions) expected",
        ));
    };
    RENDERER_STATE.with(|cell| {
        let mut state_ref = cell.borrow_mut();
        let Some(state) = state_ref.as_mut() else {
            return Ok(Value::Bool(false));
        };

        let dirty_rects: Vec<Rect> = match dirty_regions {
            Value::Array(arr) => arr
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .iter()
                .filter_map(parse_rect)
                .collect(),
            _ => Vec::new(),
        };

        if dirty_rects.is_empty() {
            // Full redraw if no regions specified
            clear_buffer(&mut state.buffer, state.clear_color);
        } else {
            // Partial clear for performance
            for rect in &dirty_rects {
                clear_region(
                    &mut state.buffer,
                    state.width,
                    state.height,
                    *rect,
                    state.clear_color,
                );
            }
        }

        let entries: Vec<Value> = match display_list {
            Value::Array(arr) => arr.read().unwrap_or_else(|e| e.into_inner()).clone(),
            _ => return Err(err("display_list must be an array")),
        };

        for entry in entries {
            let Some((op_type, bounds, style, content)) = parse_display_entry(&entry) else {
                continue;
            };

            // Optimization: Skip if command doesn't intersect any dirty region
            if !dirty_rects.is_empty() && !dirty_rects.iter().any(|r| intersects(r, &bounds)) {
                continue;
            }

            match op_type.as_str() {
                "rect" => {
                    if let Some(shadow) = style.shadow {
                        draw_shadow(
                            &mut state.buffer,
                            state.width as usize,
                            state.height as usize,
                            bounds,
                            shadow,
                            &dirty_rects,
                        );
                    }
                    let color = style.fill_color.unwrap_or(Color::rgba(0.2, 0.2, 0.24, 1.0));
                    draw_rrect(
                        &mut state.buffer,
                        state.width as usize,
                        state.height as usize,
                        bounds,
                        style.corner_radius,
                        color,
                        &dirty_rects,
                    );
                }
                "text" => {
                    let color = style.fill_color.unwrap_or(Color::rgba(0.9, 0.9, 0.92, 1.0));
                    if let Some(Value::Str(text)) = content {
                        draw_text(
                            &mut state.buffer,
                            state.width as usize,
                            state.height as usize,
                            bounds,
                            &text,
                            color,
                            &dirty_rects,
                        );
                    }
                }
                "image" => {
                    let color = style.fill_color.unwrap_or(Color::rgba(0.3, 0.3, 0.34, 1.0));
                    draw_rect(
                        &mut state.buffer,
                        state.width as usize,
                        state.height as usize,
                        bounds,
                        color,
                        &dirty_rects,
                    );
                }
                _ => {}
            }
        }

        Ok(Value::Bool(true))
    })
}

#[cfg(not(feature = "ui"))]
fn renderer_render_native(_vm: &mut NyxVm, _args: &[Value]) -> Result<Value, EvalError> {
    Ok(Value::Bool(false))
}

#[cfg(feature = "ui")]
fn renderer_present_native(_vm: &mut NyxVm, _args: &[Value]) -> Result<Value, EvalError> {
    RENDERER_STATE.with(|cell| {
        let mut state_ref = cell.borrow_mut();
        let Some(state) = state_ref.as_mut() else {
            return Ok(Value::Null);
        };
        if state.window.is_open() {
            let width = state.width;
            let height = state.height;
            let buffer = std::mem::take(&mut state.buffer);
            let result = state
                .window
                .update_with_buffer(&buffer, width, height)
                .map_err(|e| err(e.to_string()));
            state.buffer = buffer;
            result?;
        }
        Ok(Value::Null)
    })
}

#[cfg(not(feature = "ui"))]
fn renderer_present_native(_vm: &mut NyxVm, _args: &[Value]) -> Result<Value, EvalError> {
    Ok(Value::Null)
}

#[cfg(feature = "ui")]
fn renderer_render_frame_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [display_list] = args else {
        return Err(err("__native_renderer_render_frame(display_list) expected"));
    };
    let display_list = match display_list {
        Value::Array(_) => display_list.clone(),
        _ => Value::array(vec![]),
    };
    let args = vec![display_list, Value::array(vec![])];
    renderer_render_native(_vm, &args)?;
    renderer_present_native(_vm, &[])?;
    Ok(Value::Bool(true))
}

#[cfg(not(feature = "ui"))]
fn renderer_render_frame_native(_vm: &mut NyxVm, _args: &[Value]) -> Result<Value, EvalError> {
    Ok(Value::Bool(false))
}

#[cfg(feature = "ui")]
fn frame_pacing_native(_vm: &mut NyxVm, _args: &[Value]) -> Result<Value, EvalError> {
    std::thread::sleep(Duration::from_millis(16));
    Ok(Value::Null)
}

#[cfg(not(feature = "ui"))]
fn frame_pacing_native(_vm: &mut NyxVm, _args: &[Value]) -> Result<Value, EvalError> {
    Ok(Value::Null)
}

#[cfg(feature = "ui")]
fn should_exit_native(_vm: &mut NyxVm, _args: &[Value]) -> Result<Value, EvalError> {
    let mut should_exit = true;
    RENDERER_STATE.with(|cell| {
        if let Some(state) = cell.borrow().as_ref() {
            should_exit = !state.window.is_open();
        }
    });
    Ok(Value::Bool(should_exit))
}

#[cfg(not(feature = "ui"))]
fn should_exit_native(_vm: &mut NyxVm, _args: &[Value]) -> Result<Value, EvalError> {
    Ok(Value::Bool(true))
}

#[cfg(feature = "ui")]
fn renderer_shutdown_native(_vm: &mut NyxVm, _args: &[Value]) -> Result<Value, EvalError> {
    RENDERER_STATE.with(|cell| {
        *cell.borrow_mut() = None;
    });
    Ok(Value::Null)
}

#[cfg(not(feature = "ui"))]
fn renderer_shutdown_native(_vm: &mut NyxVm, _args: &[Value]) -> Result<Value, EvalError> {
    Ok(Value::Null)
}

fn reload_file_native(vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Str(path)] = args else {
        return Err(err("__native_reload_file(path) expected"));
    };
    let path = PathBuf::from(path);
    vm.load_file("", &path).map_err(|e| err(e.message))?;
    Ok(Value::Bool(true))
}

#[cfg(feature = "ui")]
fn renderer_resize_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Int(width), Value::Int(height)] = args else {
        return Err(err("__native_renderer_resize(width, height) expected"));
    };
    RENDERER_STATE.with(|cell| {
        let mut state_ref = cell.borrow_mut();
        let Some(state) = state_ref.as_mut() else {
            return Ok(Value::Null);
        };
        let width = *width as usize;
        let height = *height as usize;
        state.width = width.max(1);
        state.height = height.max(1);
        state.buffer = vec![state.clear_color; state.width * state.height];
        Ok(Value::Null)
    })
}

#[cfg(not(feature = "ui"))]
fn renderer_resize_native(_vm: &mut NyxVm, _args: &[Value]) -> Result<Value, EvalError> {
    Ok(Value::Null)
}

fn measure_text_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Str(text), size] = args else {
        return Err(err("measure_text(text, font_size) expected"));
    };
    let font_size = numeric_to_f64(size)?;
    let width = font_size * 0.6 * text.chars().count() as f64;
    let height = font_size * 1.2;
    Ok(Value::array(vec![
        Value::Float(width),
        Value::Float(height),
    ]))
}

fn measure_element_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [_element, min_w, max_w, min_h, max_h] = args else {
        return Err(err(
            "measure_element(element, min_w, max_w, min_h, max_h) expected",
        ));
    };
    let min_w = numeric_to_f64(min_w)?;
    let max_w = numeric_to_f64(max_w)?;
    let min_h = numeric_to_f64(min_h)?;
    let max_h = numeric_to_f64(max_h)?;
    let width = if max_w > 0.0 { max_w } else { min_w };
    let height = if max_h > 0.0 { max_h } else { min_h };
    Ok(Value::array(vec![
        Value::Float(width),
        Value::Float(height),
    ]))
}

fn numeric_to_f64(value: &Value) -> Result<f64, EvalError> {
    match value {
        Value::Float(f) => Ok(*f),
        Value::Int(i) => Ok(*i as f64),
        _ => Err(err("expected numeric value")),
    }
}

#[cfg(feature = "ui")]
#[derive(Debug, Clone, Copy)]
struct Rect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

#[cfg(feature = "ui")]
#[derive(Debug, Clone, Copy)]
struct Color {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

#[cfg(feature = "ui")]
#[derive(Debug, Clone)]
struct PaintStyle {
    fill_color: Option<Color>,
    _stroke_color: Option<Color>,
    _stroke_width: f32,
    corner_radius: f32,
    shadow: Option<Shadow>,
}

#[cfg(feature = "ui")]
#[derive(Debug, Clone, Copy)]
struct Shadow {
    offset_x: f32,
    offset_y: f32,
    blur_radius: f32,
    color: Color,
}

#[cfg(feature = "ui")]
fn parse_display_entry(entry: &Value) -> Option<(String, Rect, PaintStyle, Option<Value>)> {
    let Value::Object(map_ref) = entry else {
        return None;
    };
    let map = map_ref.read().unwrap_or_else(|e| e.into_inner());
    let op_type = match map.get("op_type")? {
        Value::Str(s) => s.clone(),
        _ => return None,
    };
    let bounds = parse_rect(map.get("bounds")?)?;
    let style = parse_style(map.get("style"));
    let content = map.get("content").cloned();
    Some((op_type, bounds, style, content))
}

#[cfg(feature = "ui")]
fn parse_rect(value: &Value) -> Option<Rect> {
    let Value::Object(map_ref) = value else {
        return None;
    };
    let map = map_ref.read().unwrap_or_else(|e| e.into_inner());
    Some(Rect {
        x: numeric_to_f32(map.get("x")?)?,
        y: numeric_to_f32(map.get("y")?)?,
        width: numeric_to_f32(map.get("width")?)?,
        height: numeric_to_f32(map.get("height")?)?,
    })
}

#[cfg(feature = "ui")]
fn parse_style(value: Option<&Value>) -> PaintStyle {
    let Some(Value::Object(map_ref)) = value else {
        return PaintStyle {
            fill_color: None,
            _stroke_color: None,
            _stroke_width: 0.0,
            corner_radius: 0.0,
            shadow: None,
        };
    };
    let map = map_ref.read().unwrap_or_else(|e| e.into_inner());
    let fill_color = map.get("fill_color").and_then(parse_color);
    let stroke_color = map.get("stroke_color").and_then(parse_color);
    let stroke_width = map
        .get("stroke_width")
        .and_then(numeric_to_f32)
        .unwrap_or(0.0);
    let corner_radius = map
        .get("corner_radius")
        .and_then(numeric_to_f32)
        .unwrap_or(0.0);
    let shadow = map.get("shadow").and_then(parse_shadow);

    PaintStyle {
        fill_color,
        _stroke_color: stroke_color,
        _stroke_width: stroke_width,
        corner_radius,
        shadow,
    }
}

#[cfg(feature = "ui")]
fn parse_shadow(value: &Value) -> Option<Shadow> {
    let Value::Object(map_ref) = value else {
        return None;
    };
    let map = map_ref.read().unwrap_or_else(|e| e.into_inner());
    Some(Shadow {
        offset_x: numeric_to_f32(map.get("offset_x")?)?,
        offset_y: numeric_to_f32(map.get("offset_y")?)?,
        blur_radius: numeric_to_f32(map.get("blur_radius")?)?,
        color: parse_color(map.get("color")?)?,
    })
}

#[cfg(feature = "ui")]
fn parse_color(value: &Value) -> Option<Color> {
    let Value::Object(map_ref) = value else {
        return None;
    };
    let map = map_ref.read().unwrap_or_else(|e| e.into_inner());
    Some(Color {
        r: numeric_to_f32(map.get("r")?)?,
        g: numeric_to_f32(map.get("g")?)?,
        b: numeric_to_f32(map.get("b")?)?,
        a: numeric_to_f32(map.get("a")?)?,
    })
}

#[cfg(feature = "ui")]
fn numeric_to_f32(value: &Value) -> Option<f32> {
    match value {
        Value::Float(f) => Some(*f as f32),
        Value::Int(i) => Some(*i as f32),
        _ => None,
    }
}

#[cfg(feature = "ui")]
fn intersects(a: &Rect, b: &Rect) -> bool {
    a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + b.height && a.y + a.height > b.y
}

#[cfg(feature = "ui")]
fn is_inside_any(x: f32, y: f32, regions: &[Rect]) -> bool {
    if regions.is_empty() {
        return true;
    }
    for r in regions {
        if x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height {
            return true;
        }
    }
    false
}

#[cfg(feature = "ui")]
fn clear_region(buffer: &mut [u32], width: usize, height: usize, rect: Rect, color: u32) {
    let x0 = rect.x.max(0.0).floor() as i32;
    let y0 = rect.y.max(0.0).floor() as i32;
    let x1 = (rect.x + rect.width).min(width as f32).ceil() as i32;
    let y1 = (rect.y + rect.height).min(height as f32).ceil() as i32;
    for y in y0..y1 {
        let row = y as usize * width;
        for x in x0..x1 {
            buffer[row + x as usize] = color;
        }
    }
}

#[cfg(feature = "ui")]
impl Color {
    fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    fn to_u32(self) -> u32 {
        let r = (self.r.clamp(0.0, 1.0) * 255.0) as u32;
        let g = (self.g.clamp(0.0, 1.0) * 255.0) as u32;
        let b = (self.b.clamp(0.0, 1.0) * 255.0) as u32;
        (r << 16) | (g << 8) | b
    }
}

#[cfg(feature = "ui")]
fn clear_buffer(buffer: &mut [u32], color: u32) {
    for pixel in buffer.iter_mut() {
        *pixel = color;
    }
}

#[cfg(feature = "ui")]
fn draw_rect(
    buffer: &mut [u32],
    width: usize,
    height: usize,
    rect: Rect,
    color: Color,
    regions: &[Rect],
) {
    let x0 = rect.x.max(0.0).floor() as i32;
    let y0 = rect.y.max(0.0).floor() as i32;
    let x1 = (rect.x + rect.width).min(width as f32).ceil() as i32;
    let y1 = (rect.y + rect.height).min(height as f32).ceil() as i32;
    let src = color;
    for y in y0..y1 {
        let row = y as usize * width;
        let py = y as f32;
        for x in x0..x1 {
            let px = x as f32;
            if is_inside_any(px, py, regions) {
                let idx = row + x as usize;
                buffer[idx] = blend(buffer[idx], src);
            }
        }
    }
}

#[cfg(feature = "ui")]
fn draw_rrect(
    buffer: &mut [u32],
    width: usize,
    height: usize,
    rect: Rect,
    radius: f32,
    color: Color,
    regions: &[Rect],
) {
    if radius <= 0.0 {
        return draw_rect(buffer, width, height, rect, color, regions);
    }

    let x0 = rect.x.max(0.0).floor() as i32;
    let y0 = rect.y.max(0.0).floor() as i32;
    let x1 = (rect.x + rect.width).min(width as f32).ceil() as i32;
    let y1 = (rect.y + rect.height).min(height as f32).ceil() as i32;

    for y in y0..y1 {
        let row = y as usize * width;
        let py = y as f32;
        for x in x0..x1 {
            let px = x as f32;

            if !is_inside_any(px, py, regions) {
                continue;
            }

            // Check corners
            let inside = if px < rect.x + radius && py < rect.y + radius {
                // Top-left
                let dx = px - (rect.x + radius);
                let dy = py - (rect.y + radius);
                dx * dx + dy * dy <= radius * radius
            } else if px > rect.x + rect.width - radius && py < rect.y + radius {
                // Top-right
                let dx = px - (rect.x + rect.width - radius);
                let dy = py - (rect.y + radius);
                dx * dx + dy * dy <= radius * radius
            } else if px < rect.x + radius && py > rect.y + rect.height - radius {
                // Bottom-left
                let dx = px - (rect.x + radius);
                let dy = py - (rect.y + rect.height - radius);
                dx * dx + dy * dy <= radius * radius
            } else if px > rect.x + rect.width - radius && py > rect.y + rect.height - radius {
                // Bottom-right
                let dx = px - (rect.x + rect.width - radius);
                let dy = py - (rect.y + rect.height - radius);
                dx * dx + dy * dy <= radius * radius
            } else {
                true
            };

            if inside {
                let idx = row + x as usize;
                buffer[idx] = blend(buffer[idx], color);
            }
        }
    }
}

#[cfg(feature = "ui")]
fn draw_shadow(
    buffer: &mut [u32],
    width: usize,
    height: usize,
    rect: Rect,
    shadow: Shadow,
    regions: &[Rect],
) {
    // Simple multi-pass blur approximation
    let passes = 3;
    let mut current_color = shadow.color;
    current_color.a /= passes as f32;

    for i in 0..passes {
        let spread = (i as f32 + 1.0) * (shadow.blur_radius / passes as f32);
        let pass_rect = Rect {
            x: rect.x + shadow.offset_x - spread,
            y: rect.y + shadow.offset_y - spread,
            width: rect.width + spread * 2.0,
            height: rect.height + spread * 2.0,
        };
        draw_rrect(
            buffer,
            width,
            height,
            pass_rect,
            spread,
            current_color,
            regions,
        );
    }
}

#[cfg(feature = "ui")]
fn draw_text(
    buffer: &mut [u32],
    width: usize,
    height: usize,
    rect: Rect,
    text: &str,
    color: Color,
    regions: &[Rect],
) {
    let mut x = rect.x.max(0.0);
    let y = rect.y.max(0.0);
    let target_height = rect.height.max(8.0);
    let scale = (target_height / 8.0).max(1.0);
    for ch in text.chars() {
        draw_char(buffer, width, height, x, y, ch, scale, color, regions);
        x += 8.0 * scale;
        if x >= rect.x + rect.width {
            break;
        }
    }
}

#[cfg(feature = "ui")]
fn draw_char(
    buffer: &mut [u32],
    width: usize,
    height: usize,
    x: f32,
    y: f32,
    ch: char,
    scale: f32,
    color: Color,
    regions: &[Rect],
) {
    let Some(glyph) = BASIC_FONTS.get(ch) else {
        return;
    };
    for (row, row_bits) in glyph.iter().enumerate() {
        for col in 0..8 {
            if row_bits & (1 << col) == 0 {
                continue;
            }
            let px = x + (7 - col) as f32 * scale;
            let py = y + row as f32 * scale;
            for sy in 0..scale.ceil() as i32 {
                for sx in 0..scale.ceil() as i32 {
                    let fx = px as i32 + sx;
                    let fy = py as i32 + sy;
                    if fx < 0 || fy < 0 || fx >= width as i32 || fy >= height as i32 {
                        continue;
                    }

                    if is_inside_any(fx as f32, fy as f32, regions) {
                        let idx = fy as usize * width + fx as usize;
                        buffer[idx] = blend(buffer[idx], color);
                    }
                }
            }
        }
    }
}

#[cfg(feature = "ui")]
fn blend(dst: u32, src: Color) -> u32 {
    let sa = src.a.clamp(0.0, 1.0);
    if sa >= 1.0 {
        return src.to_u32();
    }
    let dr = ((dst >> 16) & 0xff) as f32 / 255.0;
    let dg = ((dst >> 8) & 0xff) as f32 / 255.0;
    let db = (dst & 0xff) as f32 / 255.0;
    let r = src.r * sa + dr * (1.0 - sa);
    let g = src.g * sa + dg * (1.0 - sa);
    let b = src.b * sa + db * (1.0 - sa);
    Color::rgba(r, g, b, 1.0).to_u32()
}

fn create_socket_native(_vm: &mut NyxVm, _args: &[Value]) -> Result<Value, EvalError> {
    let id = NEXT_SOCKET_ID.fetch_add(1, Ordering::SeqCst);
    let entry = SocketEntry::Pending {
        reuse_addr: false,
        reuse_port: false,
    };
    SOCKETS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| err("socket registry lock poisoned"))?
        .insert(id, entry);
    Ok(Value::Int(id))
}

fn close_socket_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Int(socket_id)] = args else {
        return Err(err("__native_close_socket(socket) expected"));
    };
    if let Some(map) = SOCKETS.get() {
        map.lock()
            .map_err(|_| err("socket registry lock poisoned"))?
            .remove(socket_id);
    }
    Ok(Value::Null)
}

fn bind_socket_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Int(socket_id), Value::Str(host), Value::Int(port)] = args else {
        return Err(err("__native_bind_socket(socket, host, port) expected"));
    };
    let mut sockets = SOCKETS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| err("socket registry lock poisoned"))?;
    let entry = sockets.remove(socket_id);
    let Some(SocketEntry::Pending {
        reuse_addr,
        reuse_port,
    }) = entry
    else {
        return Err(err("socket is not in pending state"));
    };

    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .map_err(|e: std::net::AddrParseError| err(e.to_string()))?;
    let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))
        .map_err(|e| err(e.to_string()))?;
    socket
        .set_reuse_address(reuse_addr)
        .map_err(|e| err(e.to_string()))?;
    let _ = reuse_port;
    socket.bind(&addr.into()).map_err(|e| err(e.to_string()))?;
    socket.listen(128).map_err(|e| err(e.to_string()))?;
    let listener: TcpListener = socket.into();
    listener
        .set_nonblocking(true)
        .map_err(|e| err(e.to_string()))?;

    sockets.insert(*socket_id, SocketEntry::Listener(listener));
    Ok(Value::Bool(true))
}

fn listen_socket_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Int(_socket_id), Value::Int(_backlog)] = args else {
        return Err(err("__native_listen_socket(socket, backlog) expected"));
    };
    Ok(Value::Bool(true))
}

fn accept_socket_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Int(socket_id)] = args else {
        return Err(err("__native_accept_socket(socket) expected"));
    };
    let mut sockets = SOCKETS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| err("socket registry lock poisoned"))?;
    let Some(SocketEntry::Listener(listener)) = sockets.get(socket_id) else {
        return Ok(Value::Null);
    };
    match listener.accept() {
        Ok((stream, addr)) => {
            stream
                .set_nonblocking(true)
                .map_err(|e| err(e.to_string()))?;
            let id = NEXT_SOCKET_ID.fetch_add(1, Ordering::SeqCst);
            sockets.insert(id, SocketEntry::Stream(stream));
            let (host, port) = format_addr(addr);
            Ok(Value::array(vec![
                Value::Int(id),
                Value::Str(host),
                Value::Int(port),
            ]))
        }
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(Value::Null),
        Err(e) => Err(err(e.to_string())),
    }
}

fn connect_socket_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Int(socket_id), Value::Str(host), Value::Int(port)] = args else {
        return Err(err("__native_connect_socket(socket, host, port) expected"));
    };
    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .map_err(|e: std::net::AddrParseError| err(e.to_string()))?;
    let stream = TcpStream::connect(addr).map_err(|e| err(e.to_string()))?;
    stream
        .set_nonblocking(true)
        .map_err(|e| err(e.to_string()))?;

    SOCKETS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| err("socket registry lock poisoned"))?
        .insert(*socket_id, SocketEntry::Stream(stream));
    Ok(Value::Bool(true))
}

fn read_socket_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Int(socket_id), Value::Int(size)] = args else {
        return Err(err("__native_read_socket(socket, size) expected"));
    };
    let mut sockets = SOCKETS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| err("socket registry lock poisoned"))?;
    let Some(SocketEntry::Stream(stream)) = sockets.get_mut(socket_id) else {
        return Ok(Value::Str(String::new()));
    };
    let mut buf = vec![0u8; (*size).max(0) as usize];
    match stream.read(&mut buf) {
        Ok(0) => Ok(Value::Str(String::new())),
        Ok(n) => {
            buf.truncate(n);
            Ok(Value::Str(String::from_utf8_lossy(&buf).to_string()))
        }
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(Value::Str(String::new())),
        Err(e) => Err(err(e.to_string())),
    }
}

fn write_socket_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Int(socket_id), Value::Str(data)] = args else {
        return Err(err("__native_write_socket(socket, data) expected"));
    };
    let mut sockets = SOCKETS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| err("socket registry lock poisoned"))?;
    let Some(SocketEntry::Stream(stream)) = sockets.get_mut(socket_id) else {
        return Ok(Value::Int(0));
    };
    match stream.write(data.as_bytes()) {
        Ok(n) => Ok(Value::Int(n as i64)),
        Err(e) => Err(err(e.to_string())),
    }
}

fn set_socket_option_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Int(socket_id), Value::Str(option), Value::Int(value)] = args else {
        return Err(err(
            "__native_set_socket_option(socket, option, value) expected",
        ));
    };
    let mut sockets = SOCKETS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| err("socket registry lock poisoned"))?;
    let Some(SocketEntry::Pending {
        reuse_addr,
        reuse_port,
    }) = sockets.get_mut(socket_id)
    else {
        return Ok(Value::Null);
    };
    match option.as_str() {
        "SO_REUSEADDR" => *reuse_addr = *value != 0,
        "SO_REUSEPORT" => *reuse_port = *value != 0,
        _ => {}
    }
    Ok(Value::Null)
}

fn format_addr(addr: SocketAddr) -> (String, i64) {
    let host = addr.ip().to_string();
    let port = addr.port() as i64;
    (host, port)
}

fn fs_watch_create_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Array(paths_rc)] = args else {
        return Err(err("__native_fs_watch_create(paths) expected"));
    };
    let (tx, rx) = mpsc::channel();
    let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    })
    .map_err(|e| err(e.to_string()))?;
    for path in paths_rc.read().unwrap_or_else(|e| e.into_inner()).iter() {
        let Value::Str(path) = path else {
            continue;
        };
        watcher
            .watch(PathBuf::from(path).as_path(), RecursiveMode::Recursive)
            .map_err(|e| err(e.to_string()))?;
    }
    let id = NEXT_WATCH_ID.fetch_add(1, Ordering::SeqCst);
    WATCHERS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| err("watcher registry lock poisoned"))?
        .insert(
            id,
            WatchEntry {
                _watcher: watcher,
                rx,
            },
        );
    Ok(Value::Int(id))
}

fn fs_watch_poll_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Int(watcher_id)] = args else {
        return Err(err("__native_fs_watch_poll(watcher) expected"));
    };
    let mut events = Vec::new();
    let Some(watchers) = WATCHERS.get() else {
        return Ok(Value::array(events));
    };
    let mut watchers = watchers
        .lock()
        .map_err(|_| err("watcher registry lock poisoned"))?;
    let Some(entry) = watchers.get_mut(watcher_id) else {
        return Ok(Value::array(events));
    };
    for event in entry.rx.try_iter().flatten() {
        let event_type = match event.kind {
            notify::EventKind::Create(_) => "create",
            notify::EventKind::Modify(_) => "modify",
            notify::EventKind::Remove(_) => "remove",
            notify::EventKind::Any => "any",
            _ => "other",
        };
        let path = event
            .paths
            .first()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let mut evt = HashMap::new();
        evt.insert("event_type".to_string(), Value::Str(event_type.to_string()));
        evt.insert("path".to_string(), Value::Str(path));
        let ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        evt.insert("timestamp".to_string(), Value::Int(ts));
        events.push(Value::object(evt));
    }
    Ok(Value::array(events))
}

fn fs_watch_destroy_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Int(watcher_id)] = args else {
        return Err(err("__native_fs_watch_destroy(watcher) expected"));
    };
    if let Some(watchers) = WATCHERS.get() {
        watchers
            .lock()
            .map_err(|_| err("watcher registry lock poisoned"))?
            .remove(watcher_id);
    }
    Ok(Value::Null)
}

fn fs_modified_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Str(path)] = args else {
        return Err(err("__native_fs_modified(path) expected"));
    };
    let meta = std::fs::metadata(path).map_err(|e| err(e.to_string()))?;
    let modified = meta.modified().map_err(|e| err(e.to_string()))?;
    Ok(Value::Str(fmt_http_date(modified)))
}

fn fs_listdir_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Str(path)] = args else {
        return Err(err("__native_fs_listdir(path) expected"));
    };
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(path).map_err(|e| err(e.to_string()))? {
        let entry = entry.map_err(|e| err(e.to_string()))?;
        if let Some(name) = entry.file_name().to_str() {
            entries.push(Value::Str(name.to_string()));
        }
    }
    Ok(Value::array(entries))
}

fn fs_exists_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Str(path)] = args else {
        return Err(err("__native_fs_exists(path) expected"));
    };
    Ok(Value::Bool(std::path::Path::new(path).exists()))
}

fn fs_is_dir_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Str(path)] = args else {
        return Err(err("__native_fs_is_dir(path) expected"));
    };
    Ok(Value::Bool(std::path::Path::new(path).is_dir()))
}

fn regex_match_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Str(pattern), Value::Str(text)] = args else {
        return Err(err("__native_regex_match(pattern, text) expected"));
    };
    let re = Regex::new(pattern).map_err(|e| err(e.to_string()))?;
    if let Some(caps) = re.captures(text) {
        let mut out = Vec::new();
        for i in 0..caps.len() {
            out.push(Value::Str(
                caps.get(i)
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_default(),
            ));
        }
        Ok(Value::array(out))
    } else {
        Ok(Value::array(Vec::new()))
    }
}

fn sha1_base64_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Str(data)] = args else {
        return Err(err("__native_sha1_base64(data) expected"));
    };
    let mut hasher = Sha1::new();
    hasher.update(data.as_bytes());
    let digest = hasher.finalize();
    Ok(Value::Str(general_purpose::STANDARD.encode(digest)))
}

fn file_read_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Str(path)] = args else {
        return Err(err("__native_file_read(path) expected"));
    };
    let contents = std::fs::read_to_string(path).map_err(|e| err(e.to_string()))?;
    Ok(Value::Str(contents))
}

fn tls_init_context_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Str(cert), Value::Str(key), Value::Array(_protocols), Value::Int(_min), Value::Int(_max)] =
        args
    else {
        return Err(err(
            "__native_tls_init_context(cert, key, protocols, min_ver, max_ver) expected",
        ));
    };
    let _ = build_server_config(cert, key)?;
    Ok(Value::Bool(true))
}

fn tls_accept_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [conn, Value::Str(cert), Value::Str(key)] = args else {
        return Err(err("__native_tls_accept(conn, cert, key) expected"));
    };
    let socket_id = extract_socket_id(conn)?;
    let mut sockets = SOCKETS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| err("socket registry lock poisoned"))?;
    let Some(SocketEntry::Stream(stream)) = sockets.remove(&socket_id) else {
        return Ok(Value::Null);
    };
    let config = build_server_config(cert, key)?;
    let conn = ServerConnection::new(config).map_err(|e| err(e.to_string()))?;
    let mut stream = StreamOwned::new(conn, stream);
    let _ = stream.conn.complete_io(&mut stream.sock);
    let peer_cert = stream
        .conn
        .peer_certificates()
        .and_then(|certs| certs.first().map(|c| c.to_vec()));
    let cipher = stream
        .conn
        .negotiated_cipher_suite()
        .map(|suite| format!("{:?}", suite.suite()));
    let verified = !stream.conn.is_handshaking();
    let id = NEXT_TLS_ID.fetch_add(1, Ordering::SeqCst);
    TLS_CONNS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| err("tls registry lock poisoned"))?
        .insert(
            id,
            TlsEntry {
                stream,
                peer_cert,
                cipher,
                verified,
            },
        );
    Ok(Value::Int(id))
}

fn tls_read_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Int(conn_id), Value::Int(size)] = args else {
        return Err(err("__native_tls_read(conn, size) expected"));
    };
    let mut conns = TLS_CONNS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| err("tls registry lock poisoned"))?;
    let Some(entry) = conns.get_mut(conn_id) else {
        return Ok(Value::Str(String::new()));
    };
    let mut buf = vec![0u8; (*size).max(0) as usize];
    match entry.stream.read(&mut buf) {
        Ok(0) => Ok(Value::Str(String::new())),
        Ok(n) => {
            buf.truncate(n);
            Ok(Value::Str(String::from_utf8_lossy(&buf).to_string()))
        }
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(Value::Str(String::new())),
        Err(e) => Err(err(e.to_string())),
    }
}

fn tls_write_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Int(conn_id), Value::Str(data)] = args else {
        return Err(err("__native_tls_write(conn, data) expected"));
    };
    let mut conns = TLS_CONNS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| err("tls registry lock poisoned"))?;
    let Some(entry) = conns.get_mut(conn_id) else {
        return Ok(Value::Int(0));
    };
    match entry.stream.write(data.as_bytes()) {
        Ok(n) => Ok(Value::Int(n as i64)),
        Err(e) => Err(err(e.to_string())),
    }
}

fn tls_close_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Int(conn_id)] = args else {
        return Err(err("__native_tls_close(conn) expected"));
    };
    if let Some(conns) = TLS_CONNS.get() {
        conns
            .lock()
            .map_err(|_| err("tls registry lock poisoned"))?
            .remove(conn_id);
    }
    Ok(Value::Null)
}

fn tls_get_peer_cert_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Int(conn_id)] = args else {
        return Err(err("__native_tls_get_peer_cert(conn) expected"));
    };
    let conns = TLS_CONNS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| err("tls registry lock poisoned"))?;
    let Some(entry) = conns.get(conn_id) else {
        return Ok(Value::Null);
    };
    if let Some(cert) = &entry.peer_cert {
        Ok(Value::Str(general_purpose::STANDARD.encode(cert)))
    } else {
        Ok(Value::Null)
    }
}

fn tls_get_cipher_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Int(conn_id)] = args else {
        return Err(err("__native_tls_get_cipher(conn) expected"));
    };
    let conns = TLS_CONNS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| err("tls registry lock poisoned"))?;
    let Some(entry) = conns.get(conn_id) else {
        return Ok(Value::Null);
    };
    Ok(entry
        .cipher
        .as_ref()
        .map(|c| Value::Str(c.clone()))
        .unwrap_or(Value::Null))
}

fn tls_is_verified_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let [Value::Int(conn_id)] = args else {
        return Err(err("__native_tls_is_verified(conn) expected"));
    };
    let conns = TLS_CONNS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| err("tls registry lock poisoned"))?;
    let Some(entry) = conns.get(conn_id) else {
        return Ok(Value::Bool(false));
    };
    Ok(Value::Bool(entry.verified))
}

fn build_server_config(cert_pem: &str, key_pem: &str) -> Result<Arc<ServerConfig>, EvalError> {
    let certs = read_certs(cert_pem)?;
    let key = read_private_key(key_pem)?;

    // NOTE: This configuration does not verify client certificates.
    // For production use with TLS client authentication, use:
    //   .with_client_cert_verifier(Arc::new(WebPkiClientVerifier::builder(...)))
    // Or use a proper TLS termination proxy in production deployments.
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| err(e.to_string()))?;
    Ok(Arc::new(config))
}

fn read_certs(pem: &str) -> Result<Vec<CertificateDer<'static>>, EvalError> {
    let mut out = Vec::new();
    for cert in CertificateDer::pem_slice_iter(pem.as_bytes()) {
        let cert = cert.map_err(|e| err(e.to_string()))?;
        out.push(cert);
    }
    if out.is_empty() {
        return Err(err("no certificates found"));
    }
    Ok(out)
}

fn read_private_key(pem: &str) -> Result<PrivateKeyDer<'static>, EvalError> {
    PrivateKeyDer::from_pem_slice(pem.as_bytes()).map_err(|e| err(e.to_string()))
}

fn extract_socket_id(conn: &Value) -> Result<i64, EvalError> {
    match conn {
        Value::Int(id) => Ok(*id),
        Value::Object(map_rc) => match map_rc
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get("socket")
        {
            Some(Value::Int(id)) => Ok(*id),
            _ => Err(err("connection missing socket id")),
        },
        _ => Err(err("invalid connection handle")),
    }
}
