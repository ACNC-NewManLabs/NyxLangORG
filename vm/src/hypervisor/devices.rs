//! Virtual Devices Module
//!
//! Provides virtualized hardware devices for VMs including console, storage, network, etc.

use super::cmos::CmosDevice;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Device type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    Console,
    Block,
    Network,
    Serial,
    Keyboard,
    Mouse,
    Timer,
    Rtc,
    Pic,
    IoApic,
    Pci,
}

/// Device I/O result
pub type DeviceResult<T> = Result<T, DeviceError>;

/// Device errors
#[derive(Debug)]
pub enum DeviceError {
    NotFound,
    NotSupported,
    IoError(String),
    InvalidOperation,
}

impl std::fmt::Display for DeviceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceError::NotFound => write!(f, "Device not found"),
            DeviceError::NotSupported => write!(f, "Operation not supported"),
            DeviceError::IoError(msg) => write!(f, "I/O error: {}", msg),
            DeviceError::InvalidOperation => write!(f, "Invalid operation"),
        }
    }
}

/// Base trait for all virtual devices
pub trait VirtualDevice: Send {
    /// Get device type
    fn device_type(&self) -> DeviceType;

    /// Get device name
    fn name(&self) -> &str;

    /// Read from I/O port
    fn read(&mut self, port: u16, size: usize) -> DeviceResult<u64>;

    /// Write to I/O port
    fn write(&mut self, port: u16, size: usize, value: u64) -> DeviceResult<()>;

    /// Handle interrupt
    fn interrupt(&mut self, irq: u8) -> DeviceResult<()>;

    /// Get framebuffer (only for display devices)
    fn get_framebuffer(&self) -> Option<Arc<Mutex<Vec<u32>>>> {
        None
    }

    /// Cast as Any for downcasting
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// Standard Framebuffer Configuration
pub const FB_WIDTH: usize = 800;
pub const FB_HEIGHT: usize = 600;
pub const FB_BASE: u64 = 0xFD000000;
pub const FB_SIZE: u64 = (FB_WIDTH * FB_HEIGHT * 4) as u64;

/// Console device (virtual display)
pub struct ConsoleDevice {
    name: String,
    pub width: u32,
    pub height: u32,
    pub framebuffer: Arc<Mutex<Vec<u32>>>,
    _cursor_x: u32,
    _cursor_y: u32,
}

impl ConsoleDevice {
    pub fn new() -> Self {
        Self {
            name: "virtio-console".to_string(),
            width: FB_WIDTH as u32,
            height: FB_HEIGHT as u32,
            framebuffer: Arc::new(Mutex::new(vec![0; FB_WIDTH * FB_HEIGHT])),
            _cursor_x: 0,
            _cursor_y: 0,
        }
    }
}

impl VirtualDevice for ConsoleDevice {
    fn device_type(&self) -> DeviceType {
        DeviceType::Console
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn read(&mut self, _port: u16, _size: usize) -> DeviceResult<u64> {
        Ok(0)
    }
    fn write(&mut self, port: u16, _size: usize, value: u64) -> DeviceResult<()> {
        if port == 0x03F8 {
            print!("{}", value as u8 as char);
        }
        Ok(())
    }
    fn interrupt(&mut self, _irq: u8) -> DeviceResult<()> {
        Ok(())
    }
    fn get_framebuffer(&self) -> Option<Arc<Mutex<Vec<u32>>>> {
        Some(Arc::clone(&self.framebuffer))
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Block device (virtual disk)
pub struct BlockDevice {
    name: String,
    _data: Vec<u8>,
}

impl BlockDevice {
    pub fn new(size: u64) -> Self {
        Self {
            name: "virtio-blk".to_string(),
            _data: vec![0u8; size as usize],
        }
    }
}

impl VirtualDevice for BlockDevice {
    fn device_type(&self) -> DeviceType {
        DeviceType::Block
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn read(&mut self, _port: u16, _size: usize) -> DeviceResult<u64> {
        Ok(0)
    }
    fn write(&mut self, _port: u16, _s: usize, _v: u64) -> DeviceResult<()> {
        Ok(())
    }
    fn interrupt(&mut self, _irq: u8) -> DeviceResult<()> {
        Ok(())
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Network device
pub struct NetworkDevice {
    name: String,
}

impl NetworkDevice {
    pub fn new() -> Self {
        Self {
            name: "virtio-net".to_string(),
        }
    }
}

impl VirtualDevice for NetworkDevice {
    fn device_type(&self) -> DeviceType {
        DeviceType::Network
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn read(&mut self, _port: u16, _size: usize) -> DeviceResult<u64> {
        Ok(0)
    }
    fn write(&mut self, _port: u16, _s: usize, _v: u64) -> DeviceResult<()> {
        Ok(())
    }
    fn interrupt(&mut self, _irq: u8) -> DeviceResult<()> {
        Ok(())
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Serial port device
pub struct SerialDevice {
    name: String,
}

impl SerialDevice {
    pub fn new() -> Self {
        Self {
            name: "serial".to_string(),
        }
    }
}

impl VirtualDevice for SerialDevice {
    fn device_type(&self) -> DeviceType {
        DeviceType::Serial
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn read(&mut self, _port: u16, _size: usize) -> DeviceResult<u64> {
        Ok(0)
    }
    fn write(&mut self, _port: u16, _p: usize, v: u64) -> DeviceResult<()> {
        print!("{}", v as u8 as char);
        Ok(())
    }
    fn interrupt(&mut self, _irq: u8) -> DeviceResult<()> {
        Ok(())
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Programmable Interrupt Controller
pub struct PicDevice {
    name: String,
}

impl PicDevice {
    pub fn new() -> Self {
        Self {
            name: "i8259-pic".to_string(),
        }
    }
}

impl VirtualDevice for PicDevice {
    fn device_type(&self) -> DeviceType {
        DeviceType::Pic
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn read(&mut self, _port: u16, _size: usize) -> DeviceResult<u64> {
        Ok(0)
    }
    fn write(&mut self, _port: u16, _p: usize, _v: u64) -> DeviceResult<()> {
        Ok(())
    }
    fn interrupt(&mut self, _irq: u8) -> DeviceResult<()> {
        Ok(())
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Device manager
pub struct DeviceManager {
    pub devices: HashMap<String, Box<dyn VirtualDevice>>,
    pub port_map: HashMap<u16, String>,
}
impl DeviceManager {
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
            port_map: HashMap::new(),
        }
    }

    pub fn add_device(&mut self, name: &str, device: Box<dyn VirtualDevice>) {
        self.devices.insert(name.to_string(), device);
    }

    pub fn get_device_mut(&mut self, name: &str) -> Option<&mut Box<dyn VirtualDevice>> {
        self.devices.get_mut(name)
    }

    pub fn get_console_framebuffer(&self) -> Option<Arc<Mutex<Vec<u32>>>> {
        if let Some(dev) = self.devices.get("console") {
            return dev.get_framebuffer();
        }
        None
    }

    pub fn port_read(&mut self, port: u16, size: usize) -> DeviceResult<u64> {
        if let Some(name) = self.port_map.get(&port) {
            let name = name.clone();
            if let Some(device) = self.devices.get_mut(&name) {
                return device.read(port, size);
            }
        }
        Ok(0xFFFFFFFFFFFFFFFFu64)
    }

    pub fn port_write(&mut self, port: u16, size: usize, value: u64) -> DeviceResult<()> {
        if let Some(name) = self.port_map.get(&port) {
            let name = name.clone();
            if let Some(device) = self.devices.get_mut(&name) {
                return device.write(port, size, value);
            }
        }
        Ok(())
    }
}

/// Create standard virtual devices
pub fn create_standard_devices(config: &super::vm::VmConfig) -> DeviceManager {
    let mut manager = DeviceManager::new();

    // PCI Host Bridge
    let mut pci_bridge = super::pci::PciHostBridge::new();

    // Primary Disk (VirtIO Block at 00:01.0)
    let blk0 = super::virtio_block::VirtioBlock::new("/tmp/nyx-disk")
        .unwrap_or_else(|_| super::virtio_block::VirtioBlock::new_ram(10 * 1024 * 1024));
    let blk0_pci = Arc::new(Mutex::new(super::pci::VirtioBlockPci::new_with_inner(
        blk0, 0x1000,
    )));
    pci_bridge.add_device(blk0_pci);

    // Optional ISO/CD-ROM (VirtIO Block at 00:02.0)
    if let Some(iso_path) = &config.iso {
        if !iso_path.is_empty() && iso_path != "None" {
            if let Ok(cd) = super::virtio_block::VirtioBlock::new(iso_path) {
                let cd_pci = Arc::new(Mutex::new(super::pci::VirtioBlockPci::new_with_inner(
                    cd, 0x2000,
                )));
                pci_bridge.add_device(cd_pci);
            }
        }
    }

    manager.add_device("pci", Box::new(pci_bridge));
    manager.port_map.insert(0xCF8, "pci".to_string());
    manager.port_map.insert(0xCFC, "pci".to_string());
    manager.port_map.insert(0xCFD, "pci".to_string());
    manager.port_map.insert(0xCFE, "pci".to_string());
    manager.port_map.insert(0xCFF, "pci".to_string());

    manager.add_device("console", Box::new(ConsoleDevice::new()));
    manager.add_device("serial", Box::new(SerialDevice::new()));
    manager.add_device("net0", Box::new(NetworkDevice::new()));
    manager.add_device("pic", Box::new(PicDevice::new()));

    manager.port_map.insert(0x3F8, "serial".to_string());
    manager.port_map.insert(0x3D4, "console".to_string());
    manager.port_map.insert(0x3D5, "console".to_string());

    manager.add_device("cmos", Box::new(CmosDevice::new()));
    manager.port_map.insert(0x70, "cmos".to_string());
    manager.port_map.insert(0x71, "cmos".to_string());

    manager
}
