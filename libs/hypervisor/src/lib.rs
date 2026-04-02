//! Nyx Hypervisor (VMM)
//!
//! Provides the top-level Virtual Machine Monitor for managing Nyx VMs.
//! Includes a full custom UI chrome: title bar, sidebar, screenshot, and
//! close/minimize controls — all rendered directly into the minifb pixel buffer.

use nyx_vm::hypervisor::{VirtualMachine, VmConfig as LowLevelVmConfig, Architecture};
use nyx_diagnostics::{NyxError, ErrorCategory};
use minifb::{Window, WindowOptions, MouseButton, MouseMode};
use font8x8::UnicodeFonts;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub mod cli;

// ── Layout Constants ──────────────────────────────────────────────────────────
const TITLEBAR_H: usize = 36;
const SIDEBAR_W:  usize = 220;
const VM_W:       usize = 800;
const VM_H:       usize = 600;
const TOTAL_W:    usize = SIDEBAR_W + VM_W;
const TOTAL_H:    usize = TITLEBAR_H + VM_H;

// ── Colour Palette (0xFFRRGGBB) ───────────────────────────────────────────────
const C_BLACK:      u32 = 0xFF000000;
const C_TITLEBAR:   u32 = 0xFF0A0A0A;
const C_SIDEBAR:    u32 = 0xFF050505;
const C_DIVIDER:    u32 = 0xFF1A1A1A;
const C_ACCENT:     u32 = 0xFF00F2FF; // Nyx cyan
const C_ACCENT_DIM: u32 = 0xFF006B70;
const C_TEXT:       u32 = 0xFFFFFFFF;
const C_TEXT_DIM:   u32 = 0xFF666666;
const C_VM_BORDER:  u32 = 0xFF1F1F1F;

/// High-level Hypervisor Configuration
#[derive(Debug, Clone)]
pub struct HypervisorConfig {
    pub num_cpus:     usize,
    pub memory_mb:    u64,
    pub kernel_path:  String,
    pub iso_path:     String,
    pub architecture: String,
    pub accel:        bool,
}

/// The Nyx Hypervisor
pub struct Hypervisor {
    vm: Arc<Mutex<VirtualMachine>>,
}

impl Hypervisor {
    /// Create a new hypervisor instance
    pub fn new(config: HypervisorConfig) -> Result<Self, NyxError> {
        let arch = match config.architecture.to_lowercase().as_str() {
            "x86_64"  => Architecture::X86_64,
            "aarch64" => Architecture::AArch64,
            "riscv64" => Architecture::RiscV64,
            _ => Architecture::X86_64,
        };
        
        let vm_config = LowLevelVmConfig {
            num_cpus: config.num_cpus,
            memory: config.memory_mb * 1024 * 1024,
            architecture: arch,
            accel: config.accel,
            kernel: Some(config.kernel_path),
            iso: Some(config.iso_path),
            ..LowLevelVmConfig::default()
        };
        
        let vm = VirtualMachine::new(vm_config).map_err(|e| NyxError::new("H001", e, ErrorCategory::Runtime))?;
        Ok(Self { vm: Arc::new(Mutex::new(vm)) })
    }

    /// Launch the VM and open the display window with full UI chrome
    pub fn run(self) -> Result<(), NyxError> {
        println!("Nyx Hypervisor: Initializing VM...");

        // Initialize VM and optionally Load Kernel
        let mut kernel_info = None;
        {
            let vm = self.vm.lock().unwrap();
            if let Some(path) = &vm.config.kernel {
                if !path.is_empty() {
                    if let Ok(data) = std::fs::read(path) {
                        // Parse ELF entry point (ELF64 entry point is at offset 24)
                        let entry = if data.len() > 32 && &data[0..4] == b"\x7fELF" {
                             u64::from_le_bytes(data[24..32].try_into().unwrap_or([0; 8]))
                        } else {
                             0x100000 // Fallback
                        };
                        kernel_info = Some((data, entry));
                    }
                }
            }
        };
        
        {
            let mut vm = self.vm.lock().unwrap();
            if let Some((data, entry)) = kernel_info {
                vm.load_kernel(&data, entry).map_err(|e| NyxError::new("H004", e, ErrorCategory::Runtime))?;
                println!("Nyx Hypervisor: Kernel loaded (PC=0x{:x}).", entry);
            } else {
                println!("Nyx Hypervisor: Starting in BIOS-only mode (Reset Vector=0xFFFF0).");
            }
            vm.start().map_err(|e| NyxError::new("H005", e, ErrorCategory::Runtime))?;
        }

        let framebuffer = self.vm.lock().unwrap().devices().get_console_framebuffer();
        let vm_thread = Arc::clone(&self.vm);

        // Spawn VM execution thread
        std::thread::spawn(move || {
            loop {
                // Lock is acquired and released in this block
                let res = {
                    let mut vm = vm_thread.lock().unwrap();
                    vm.run(10000)
                };
                
                if let Err(e) = res {
                    eprintln!("VM execution error: {}", e);
                    break;
                }
                
                std::thread::sleep(Duration::from_millis(1)); // Briefly yield to GUI
            }
            println!("Nyx Hypervisor: VM halted.");
        });

        // ── GUI Window ────────────────────────────────────────────────────────
        // Pixel buffer for the whole window
        let mut buf = vec![C_BLACK; TOTAL_W * TOTAL_H];
        
        let mut window_opt = Window::new(
            "Nyx Hypervisor",
            TOTAL_W,
            TOTAL_H,
            WindowOptions {
                resize: false,
                ..WindowOptions::default()
            },
        ).ok();

        if let Some(ref mut window) = window_opt {
            window.set_target_fps(60);
            println!("Nyx Hypervisor: Display window opened ({TOTAL_W}×{TOTAL_H}).");
        } else {
            println!("Nyx Hypervisor: Running in headless mode (framebuffer only).");
        }

        // UI state
        let mut _screenshot_flash: Option<Instant> = None;
        let mut screenshot_count = 0u32;
        let mut frame_count = 0;
        let mut screenshot_requested;
        let fb_mutex_opt = framebuffer;

        loop {
            // If window exists, check for close (but not in the first 5 frames — minifb needs time to init)
            if frame_count > 5 {
                if let Some(ref window) = window_opt {
                    if !window.is_open() {
                        break;
                    }
                }
            }
            
            screenshot_requested = false;
            buf.fill(C_BLACK);

            // ── Mouse state ───────────────────────────────────────────────
            let mut mx = usize::MAX;
            let mut my = usize::MAX;
            let mut mouse_down = false;
            
            if let Some(ref window) = window_opt {
                let mouse_pos = window.get_mouse_pos(MouseMode::Discard);
                mouse_down = window.get_mouse_down(MouseButton::Left);
                if let Some((x, y)) = mouse_pos {
                    mx = x as usize;
                    my = y as usize;
                }
            }

            // Pass relative mouse to VM for guest-side UI
            if mx >= SIDEBAR_W && my >= TITLEBAR_H {
                let vx = (mx - SIDEBAR_W) as u32;
                let vy = (my - TITLEBAR_H) as u32;
                let buttons = if mouse_down { 1u32 } else { 0u32 };
                if let Ok(mut vm_lock) = self.vm.try_lock() {
                    vm_lock.update_mouse(vx, vy, buttons);
                    vm_lock.sync_framebuffer();
                }
            } else if let Ok(vm_lock) = self.vm.try_lock() {
                vm_lock.sync_framebuffer();
            }

            // ── Rendering ─────────────────────────────────────────────────
            let close_x1 = TOTAL_W - 40; let btn_y1 = 8;
            let min_x1 = TOTAL_W - 72;
            let ss_x1 = 12; let ss_x2 = SIDEBAR_W - 12;
            let ss_y1 = 190; let ss_y2 = 218;

            let hovering_close = mx >= close_x1 && mx <= (close_x1 + 28) && my >= btn_y1 && my <= (btn_y1 + 20);
            let _hovering_min = mx >= min_x1 && mx <= (min_x1 + 28) && my >= btn_y1 && my <= (btn_y1 + 20);
            let hovering_ss = mx >= ss_x1 && mx <= ss_x2 && my >= (TITLEBAR_H + ss_y1) && my <= (TITLEBAR_H + ss_y2);
            
            frame_count += 1;
            
            // Periodically check UI Health
            if frame_count % 500 == 0 {
                let mut active_pixels = 0;
                for p in &buf { if *p != C_BLACK { active_pixels += 1; } }
                if active_pixels > 1000 {
                    println!("Nyx Hypervisor: UI Health Check [OK] ({} active pixels)", active_pixels);
                } else {
                    println!("Nyx Hypervisor: UI Health Check [WAIT] (Displaying boot splash or black screen)");
                }
            }

            // Periodic screenshots every 1000 frames for diagnostics
            if frame_count % 1000 == 0 {
                screenshot_requested = true;
            }

            if mouse_down {
                if hovering_close { break; }
                if hovering_ss { screenshot_requested = true; }
            }

            // Draw Chrome
            draw_rect(&mut buf, TOTAL_W, 0, 0, TOTAL_W, TITLEBAR_H, C_TITLEBAR);
            draw_hline(&mut buf, TOTAL_W, 0, TITLEBAR_H - 1, TOTAL_W, C_DIVIDER);
            draw_text(&mut buf, TOTAL_W, 16, 10, "NYX TERMINAL | v1.0", C_ACCENT, 1);
            
            draw_rect(&mut buf, TOTAL_W, 0, TITLEBAR_H, SIDEBAR_W, VM_H, C_SIDEBAR);
            draw_vline(&mut buf, TOTAL_W, SIDEBAR_W - 1, TITLEBAR_H, TOTAL_H, C_DIVIDER);
            
            draw_text(&mut buf, TOTAL_W, 20, TITLEBAR_H + 30, "Virtual Machine: Running", C_TEXT, 1);
            draw_text(&mut buf, TOTAL_W, 20, TITLEBAR_H + 60, "Arch: x86_64", C_TEXT_DIM, 1);
            draw_text(&mut buf, TOTAL_W, 20, TITLEBAR_H + 80, "Accel: KVM", C_TEXT_DIM, 1);
            draw_text(&mut buf, TOTAL_W, 20, TITLEBAR_H + 110, "IO Ports: Active", C_ACCENT_DIM, 1);
            draw_text(&mut buf, TOTAL_W, 20, TITLEBAR_H + 130, "BIOS: Nyx BIOS", C_ACCENT_DIM, 1);

            // ── Blit VM Framebuffer ───────────────────────────────────────
            let content_x = SIDEBAR_W;
            let content_y = TITLEBAR_H;
            draw_border(&mut buf, TOTAL_W, content_x, content_y, VM_W, VM_H, C_VM_BORDER);

            let mut blitted = false;
            if let Some(ref fb_mutex) = fb_mutex_opt {
                if let Ok(fb) = fb_mutex.lock() {
                    if fb.len() == VM_W * VM_H {
                        for row in 0..VM_H {
                            let src_start = row * VM_W;
                            let dst_start = (content_y + row) * TOTAL_W + content_x;
                            if (dst_start + VM_W) <= buf.len() {
                                buf[dst_start..dst_start + VM_W].copy_from_slice(&fb[src_start..src_start+VM_W]);
                            }
                        }
                        blitted = true;
                    }
                }
            }
            if !blitted {
                draw_boot_splash(&mut buf, TOTAL_W, content_x, content_y, VM_W, VM_H);
            }

            if screenshot_requested {
                let path = format!("/home/surya/Nyx Programming Language/nyx_boot_log_{}.png", screenshot_count);
                save_screenshot(&buf, TOTAL_W, TOTAL_H, &path);
                println!("Nyx Hypervisor: Diagnostic screenshot saved to {}", path);
                _screenshot_flash = Some(Instant::now());
                screenshot_count += 1;
            }

            if let Some(ref mut window) = window_opt {
                let _ = window.update_with_buffer(&buf, TOTAL_W, TOTAL_H);
            }

            // Sync/Wait (60 FPS target)
            std::thread::sleep(Duration::from_millis(16));
            
            // Headless mode: run for 3000 frames, capturing screenshots for diagnostics
            if window_opt.is_none() && frame_count >= 3000 {
                break;
            }
        }
        
        println!("Nyx Hypervisor: Display window closed.");
        Ok(())
    }
}

// ── Rendering Primitives ─────────────────────────────────────────────────────

fn draw_rect(buf: &mut Vec<u32>, w: usize, x: usize, y: usize, rw: usize, rh: usize, color: u32) {
    for row in y..(y + rh) {
        let base = row * w;
        for col in x..(x + rw) {
            let idx = base + col;
            if idx < buf.len() { buf[idx] = color; }
        }
    }
}

fn draw_hline(buf: &mut Vec<u32>, w: usize, x: usize, y: usize, len: usize, color: u32) {
    let base = y * w;
    for col in x..(x + len) {
        let idx = base + col;
        if idx < buf.len() { buf[idx] = color; }
    }
}

fn draw_vline(buf: &mut Vec<u32>, w: usize, x: usize, y1: usize, y2: usize, color: u32) {
    for row in y1..y2 {
        let idx = row * w + x;
        if idx < buf.len() { buf[idx] = color; }
    }
}

fn draw_border(buf: &mut Vec<u32>, w: usize, x: usize, y: usize, rw: usize, rh: usize, color: u32) {
    draw_hline(buf, w, x, y, rw, color);
    draw_hline(buf, w, x, y + rh - 1, rw, color);
    draw_vline(buf, w, x, y, y + rh, color);
    draw_vline(buf, w, x + rw - 1, y, y + rh, color);
}

/// Render a string using the font8x8 bitmap font
fn draw_text(buf: &mut Vec<u32>, total_w: usize, x: usize, y: usize, text: &str, color: u32, scale: usize) {
    for (i, c) in text.chars().enumerate() {
        if let Some(glyph) = font8x8::BASIC_FONTS.get(c) {
            for gy in 0..8 {
                for gx in 0..8 {
                    if glyph[gy] & (1 << gx) != 0 {
                        for sy in 0..scale {
                            for sx in 0..scale {
                                let px = x + i * (8 * scale + scale) + gx * scale + sx;
                                let py = y + gy * scale + sy;
                                if px < total_w && py < (buf.len() / total_w) {
                                    buf[py * total_w + px] = color;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn save_screenshot(buf: &[u32], w: usize, h: usize, path: &str) {
    use image::{Rgb, RgbImage};
    let mut img = RgbImage::new(w as u32, h as u32);
    for y in 0..h {
        for x in 0..w {
            let p = buf[y * w + x];
            let r = ((p >> 16) & 0xFF) as u8;
            let g = ((p >> 8) & 0xFF) as u8;
            let b = (p & 0xFF) as u8;
            img.put_pixel(x as u32, y as u32, Rgb([r, g, b]));
        }
    }
    println!("Nyx Hypervisor: Saving screenshot to {} ({}x{})...", path, w, h);
    match img.save(path) {
        Ok(_) => println!("Nyx Hypervisor: Screenshot saved successfully."),
        Err(e) => eprintln!("Nyx Hypervisor error: Failed to save screenshot: {}", e),
    }
}

/// Draw the Nyx logo as a stylised pixel diamond/cross
fn draw_nyx_logo(buf: &mut Vec<u32>, buf_w: usize, x: usize, y: usize) {
    // Outer glow ring
    for i in 0..12usize {
        let px = x + 6 + (i % 4) * 3;
        let py = y + (i / 4) * 3;
        let idx = py * buf_w + px;
        if idx < buf.len() { buf[idx] = C_ACCENT_DIM; }
    }
    // Diamond core
    let pts = [(6,0),(3,3),(6,3),(9,3),(0,6),(3,6),(6,6),(9,6),(12,6),(3,9),(6,9),(9,9),(6,12)];
    for (dx, dy) in pts {
        for sr in 0..2usize {
            for sc in 0..2usize {
                let idx = (y + dy + sr) * buf_w + (x + dx + sc);
                if idx < buf.len() { buf[idx] = C_ACCENT; }
            }
        }
    }
    // Centre bright pixel
    let ci = (y + 6) * buf_w + (x + 6);
    if ci < buf.len() { buf[ci] = C_TEXT; }
}

/// Boot splash shown in the VM viewport before guest outputs anything
fn draw_boot_splash(buf: &mut Vec<u32>, buf_w: usize, x: usize, y: usize, w: usize, h: usize) {
    draw_rect(buf, buf_w, x, y, w, h, C_BLACK);

    let cx = x + w / 2;
    let cy = y + h / 2;

    // Crosshair
    draw_hline(buf, buf_w, cx - 40, cy, 80, C_DIVIDER);
    draw_vline(buf, buf_w, cx, cy - 40, cy + 40, C_DIVIDER);

    draw_nyx_logo(buf, buf_w, cx - 6, cy - 30);
    draw_text(buf, buf_w, cx - 40, cy + 18, "WAITING FOR GUEST OUTPUT", C_TEXT_DIM, 1);
    draw_text(buf, buf_w, cx - 20, cy + 32, "Nyx Hypervisor v1.0", C_ACCENT_DIM, 1);
}
