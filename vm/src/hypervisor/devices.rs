//! Virtual Devices Module
//! 
//! Provides virtualized hardware devices for VMs including console, storage, network, etc.

use std::collections::HashMap;
use std::sync::Mutex;

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
    fn read(&self, port: u16, size: usize) -> DeviceResult<u64>;
    
    /// Write to I/O port
    fn write(&self, port: u16, size: usize, value: u64) -> DeviceResult<()>;
    
    /// Handle interrupt
    fn interrupt(&mut self, irq: u8) -> DeviceResult<()>;
}

/// Console device (virtual display)
pub struct ConsoleDevice {
    name: String,
    width: u32,
    height: u32,
    framebuffer: Vec<u32>,
    cursor_x: u32,
    cursor_y: u32,
}

impl ConsoleDevice {
    pub fn new() -> Self {
        Self {
            name: "virtio-console".to_string(),
            width: 800,
            height: 600,
            framebuffer: vec![0; 800 * 600],
            cursor_x: 0,
            cursor_y: 0,
        }
    }
    
    pub fn set_resolution(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.framebuffer.resize((width * height) as usize, 0);
    }
    
    /// Write pixel to framebuffer
    pub fn write_pixel(&mut self, x: u32, y: u32, color: u32) {
        if x < self.width && y < self.height {
            self.framebuffer[(y * self.width + x) as usize] = color;
        }
    }
    
    /// Clear the screen
    pub fn clear(&mut self, color: u32) {
        self.framebuffer.fill(color);
    }
}

impl Default for ConsoleDevice {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualDevice for ConsoleDevice {
    fn device_type(&self) -> DeviceType {
        DeviceType::Console
    }
    
    fn name(&self) -> &str {
        &self.name
    }
    
    fn read(&self, port: u16, size: usize) -> DeviceResult<u64> {
        // Simplified console I/O
        Ok(0)
    }
    
    fn write(&self, port: u16, size: usize, value: u64) -> DeviceResult<()> {
        match port {
            0x03F8 => {
                // Serial output
                print!("{}", value as u8 as char);
            }
            _ => {}
        }
        Ok(())
    }
    
    fn interrupt(&mut self, _irq: u8) -> DeviceResult<()> {
        Ok(())
    }
}

/// Block device (virtual disk)
pub struct BlockDevice {
    name: String,
    sector_size: u32,
    num_sectors: u64,
    data: Vec<u8>,
    read_only: bool,
}

impl BlockDevice {
    pub fn new(size: u64) -> Self {
        Self {
            name: "virtio-blk".to_string(),
            sector_size: 512,
            num_sectors: size / 512,
            data: vec![0u8; size as usize],
            read_only: false,
        }
    }
    
    /// Load data from file
    pub fn load_file(&mut self, path: &str) -> std::io::Result<()> {
        use std::fs::File;
        use std::io::Read;
        
        let mut file = File::open(path)?;
        file.read_exact(&mut self.data)?;
        Ok(())
    }
    
    /// Read sector
    pub fn read_sector(&self, sector: u64, buf: &mut [u8]) -> DeviceResult<()> {
        if sector >= self.num_sectors {
            return Err(DeviceError::IoError("Sector out of range".to_string()));
        }
        
        let offset = (sector * self.sector_size as u64) as usize;
        let end = offset + buf.len();
        
        if end > self.data.len() {
            return Err(DeviceError::IoError("Read beyond device".to_string()));
        }
        
        buf.copy_from_slice(&self.data[offset..end]);
        Ok(())
    }
    
    /// Write sector
    pub fn write_sector(&mut self, sector: u64, buf: &[u8]) -> DeviceResult<()> {
        if self.read_only {
            return Err(DeviceError::IoError("Device is read-only".to_string()));
        }
        
        if sector >= self.num_sectors {
            return Err(DeviceError::IoError("Sector out of range".to_string()));
        }
        
        let offset = (sector * self.sector_size as u64) as usize;
        let end = offset + buf.len();
        
        if end > self.data.len() {
            return Err(DeviceError::IoError("Write beyond device".to_string()));
        }
        
        self.data[offset..end].copy_from_slice(buf);
        Ok(())
    }
}

impl VirtualDevice for BlockDevice {
    fn device_type(&self) -> DeviceType {
        DeviceType::Block
    }
    
    fn name(&self) -> &str {
        &self.name
    }
    
    fn read(&self, _port: u16, size: usize) -> DeviceResult<u64> {
        Ok(0)
    }
    
    fn write(&self, _port: u16, _size: usize, _value: u64) -> DeviceResult<()> {
        Ok(())
    }
    
    fn interrupt(&mut self, _irq: u8) -> DeviceResult<()> {
        Ok(())
    }
}

/// Network device
pub struct NetworkDevice {
    name: String,
    mac_address: [u8; 6],
    tx_buffer: Vec<u8>,
    rx_buffer: Vec<u8>,
    connected: bool,
}

impl NetworkDevice {
    pub fn new() -> Self {
        Self {
            name: "virtio-net".to_string(),
            mac_address: [0x52, 0x54, 0x00, 0x12, 0x34, 0x56],
            tx_buffer: Vec::new(),
            rx_buffer: Vec::new(),
            connected: false,
        }
    }
    
    pub fn mac_address(&self) -> &[u8; 6] {
        &self.mac_address
    }
    
    /// Transmit packet
    pub fn transmit(&mut self, data: &[u8]) -> DeviceResult<usize> {
        self.tx_buffer.extend_from_slice(data);
        Ok(data.len())
    }
    
    /// Receive packet
    pub fn receive(&mut self, data: &mut [u8]) -> DeviceResult<usize> {
        if self.rx_buffer.is_empty() {
            return Err(DeviceError::IoError("No data available".to_string()));
        }
        
        let len = data.len().min(self.rx_buffer.len());
        data[..len].copy_from_slice(&self.rx_buffer[..len]);
        self.rx_buffer.drain(..len);
        Ok(len)
    }
}

impl Default for NetworkDevice {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualDevice for NetworkDevice {
    fn device_type(&self) -> DeviceType {
        DeviceType::Network
    }
    
    fn name(&self) -> &str {
        &self.name
    }
    
    fn read(&self, _port: u16, size: usize) -> DeviceResult<u64> {
        Ok(0)
    }
    
    fn write(&self, _port: u16, _size: usize, _value: u64) -> DeviceResult<()> {
        Ok(())
    }
    
    fn interrupt(&mut self, _irq: u8) -> DeviceResult<()> {
        Ok(())
    }
}

/// Serial port device
pub struct SerialDevice {
    name: String,
    buffer: Vec<u8>,
}

impl SerialDevice {
    pub fn new() -> Self {
        Self {
            name: "serial".to_string(),
            buffer: Vec::new(),
        }
    }
    
    /// Write byte
    pub fn write_byte(&mut self, byte: u8) {
        // Output to stdout
        print!("{}", byte as char);
        self.buffer.push(byte);
    }
    
    /// Read byte
    pub fn read_byte(&mut self) -> Option<u8> {
        self.buffer.pop()
    }
}

impl Default for SerialDevice {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualDevice for SerialDevice {
    fn device_type(&self) -> DeviceType {
        DeviceType::Serial
    }
    
    fn name(&self) -> &str {
        &self.name
    }
    
    fn read(&self, _port: u16, _size: usize) -> DeviceResult<u64> {
        Ok(0)
    }
    
    fn write(&self, _port: u16, _size: usize, value: u64) -> DeviceResult<()> {
        self.write_byte(value as u8);
        Ok(())
    }
    
    fn interrupt(&mut self, _irq: u8) -> DeviceResult<()> {
        Ok(())
    }
}

/// Programmable Interrupt Controller
pub struct PicDevice {
    name: String,
    irq_mask: u16,
    irq_request: u8,
    isr: u8,
    irr: u8,
}

impl PicDevice {
    pub fn new() -> Self {
        Self {
            name: "i8259-pic".to_string(),
            irq_mask: 0xFFFF,
            irq_request: 0,
            isr: 0,
            irr: 0,
        }
    }
    
    /// Raise interrupt
    pub fn raise_irq(&mut self, irq: u8) {
        self.irr |= 1 << irq;
    }
    
    /// Get interrupt request
    pub fn get_irq(&mut self) -> Option<u8> {
        for i in 0..8 {
            if self.irr & (1 << i) != 0 && self.irq_mask & (1 << i) == 0 {
                return Some(i);
            }
        }
        None
    }
    
    /// Acknowledge interrupt
    pub fn acknowledge(&mut self, irq: u8) {
        self.irr &= !(1 << irq);
        self.isr &= !(1 << irq);
    }
}

impl Default for PicDevice {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualDevice for PicDevice {
    fn device_type(&self) -> DeviceType {
        DeviceType::Pic
    }
    
    fn name(&self) -> &str {
        &self.name
    }
    
    fn read(&self, port: u16, size: usize) -> DeviceResult<u64> {
        match port & 1 {
            0 => Ok(self.irr as u64),
            1 => Ok(self.isr as u64),
            _ => Ok(0),
        }
    }
    
    fn write(&self, port: u16, size: usize, value: u64) -> DeviceResult<()> {
        match port & 1 {
            0 => {
                // Command port
                if value & 0x20 != 0 {
                    // End of interrupt
                }
            }
            1 => {
                // Data port - mask
                self.irq_mask = value as u16;
            }
            _ => {}
        }
        Ok(())
    }
    
    fn interrupt(&mut self, _irq: u8) -> DeviceResult<()> {
        Ok(())
    }
}

/// Device manager
pub struct DeviceManager {
    devices: HashMap<String, Box<dyn VirtualDevice>>,
    port_map: HashMap<u16, String>,
}

impl DeviceManager {
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
            port_map: HashMap::new(),
        }
    }
    
    /// Add a device
    pub fn add_device(&mut self, name: &str, device: Box<dyn VirtualDevice>) {
        self.devices.insert(name.to_string(), device);
    }
    
    /// Get device by name
    pub fn get_device(&self, name: &str) -> Option<&Box<dyn VirtualDevice>> {
        self.devices.get(name)
    }
    
    /// Get mutable device
    pub fn get_device_mut(&mut self, name: &str) -> Option<&mut Box<dyn VirtualDevice>> {
        self.devices.get_mut(name)
    }
    
    /// Remove device
    pub fn remove_device(&mut self, name: &str) -> Option<Box<dyn VirtualDevice>> {
        self.devices.remove(name)
    }
    
    /// Read from port
    pub fn port_read(&self, port: u16, size: usize) -> DeviceResult<u64> {
        if let Some(name) = self.port_map.get(&port) {
            if let Some(device) = self.devices.get(name) {
                return device.read(port, size);
            }
        }
        
        // Check all devices
        for device in self.devices.values() {
            if let Ok(value) = device.read(port, size) {
                return Ok(value);
            }
        }
        
        Ok(0xFFFFFFFFFFFFFFFFu64)
    }
    
    /// Write to port
    pub fn port_write(&mut self, port: u16, size: usize, value: u64) -> DeviceResult<()> {
        if let Some(name) = self.port_map.get(&port) {
            if let Some(device) = self.devices.get(name) {
                return device.write(port, size, value);
            }
        }
        
        // Check all devices
        for device in self.devices.values() {
            if device.write(port, size, value).is_ok() {
                return Ok(());
            }
        }
        
        Ok(())
    }
    
    /// List all devices
    pub fn list_devices(&self) -> Vec<(String, DeviceType)> {
        self.devices.iter()
            .map(|(name, dev)| (name.clone(), dev.device_type()))
            .collect()
    }
}

impl Default for DeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Create standard virtual devices
pub fn create_standard_devices() -> DeviceManager {
    let mut manager = DeviceManager::new();
    
    // Add console
    manager.add_device("console", Box::new(ConsoleDevice::new()));
    
    // Add serial
    manager.add_device("serial", Box::new(SerialDevice::new()));
    
    // Add block device (10GB)
    manager.add_device("blk0", Box::new(BlockDevice::new(10 * 1024 * 1024 * 1024)));
    
    // Add network
    manager.add_device("net0", Box::new(NetworkDevice::new()));
    
    // Add PIC
    manager.add_device("pic", Box::new(PicDevice::new()));
    
    // Map I/O ports
    manager.port_map.insert(0x3F8, "serial".to_string());  // COM1
    manager.port_map.insert(0x3F9, "serial".to_string());
    manager.port_map.insert(0x3D4, "console".to_string()); // VGA
    manager.port_map.insert(0x3D5, "console".to_string());
    
    manager
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_manager() {
        let mut manager = DeviceManager::new();
        manager.add_device("test", Box::new(SerialDevice::new()));
        
        assert!(manager.get_device("test").is_some());
    }

    #[test]
    fn test_block_device() {
        let mut device = BlockDevice::new(1024);
        let data = [1u8, 2, 3, 4];
        
        device.write_sector(0, &data).unwrap();
        
        let mut read_buf = [0u8; 4];
        device.read_sector(0, &mut read_buf).unwrap();
        
        assert_eq!(data, read_buf);
    }
}

