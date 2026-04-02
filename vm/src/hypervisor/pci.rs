//! PCI Bus Subsystem
//! 
//! Implements a minimal PCI host bridge and configuration space access.

use std::sync::{Arc, Mutex};
use super::devices::{VirtualDevice, DeviceType, DeviceResult};
use super::virtio_block::VirtioBlock;

/// PCI Configuration Address Register
#[derive(Debug, Clone, Copy, Default)]
pub struct PciConfigAddress {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub register: u8,
    pub enabled: bool,
}

impl From<u32> for PciConfigAddress {
    fn from(val: u32) -> Self {
        Self {
            bus: ((val >> 16) & 0xFF) as u8,
            device: ((val >> 11) & 0x1F) as u8,
            function: ((val >> 8) & 0x07) as u8,
            register: (val & 0xFC) as u8,
            enabled: (val >> 31) != 0,
        }
    }
}

/// PCI Device Trait
pub trait PciDevice: Send + Sync {
    fn config_read(&mut self, reg: u8) -> u32;
    fn config_write(&mut self, reg: u8, value: u32);
}

/// PCI Host Bridge (Northbridge)
pub struct PciHostBridge {
    pub config_addr: PciConfigAddress,
    pub devices: Vec<Arc<Mutex<dyn PciDevice>>>,
}

impl PciHostBridge {
    pub fn new() -> Self {
        Self {
            config_addr: PciConfigAddress::default(),
            devices: Vec::new(),
        }
    }

    pub fn add_device(&mut self, device: Arc<Mutex<dyn PciDevice>>) {
        self.devices.push(device);
    }
}

impl VirtualDevice for PciHostBridge {
    fn device_type(&self) -> DeviceType { DeviceType::Pci }
    fn name(&self) -> &str { "pci-host" }

    fn read(&mut self, port: u16, _size: usize) -> DeviceResult<u64> {
        match port {
            0xCF8 => Ok(0), // Normally write-only for address
            0xCFC..=0xCFF => {
                if self.config_addr.enabled && self.config_addr.bus == 0 && self.config_addr.device < self.devices.len() as u8 {
                    let mut target = self.devices[self.config_addr.device as usize].lock().unwrap();
                    Ok(target.config_read(self.config_addr.register) as u64)
                } else {
                    Ok(0xFFFFFFFF)
                }
            }
            _ => Ok(0xFFFFFFFF),
        }
    }

    fn write(&mut self, port: u16, _size: usize, value: u64) -> DeviceResult<()> {
        match port {
            0xCF8 => {
                self.config_addr = PciConfigAddress::from(value as u32);
            }
            0xCFC..=0xCFF => {
                if self.config_addr.enabled && self.config_addr.bus == 0 && self.config_addr.device < self.devices.len() as u8 {
                    let mut target = self.devices[self.config_addr.device as usize].lock().unwrap();
                    target.config_write(self.config_addr.register, value as u32);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn interrupt(&mut self, _irq: u8) -> DeviceResult<()> { Ok(()) }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
}

/// VirtIO Block Device PCI Wrapper
pub struct VirtioBlockPci {
    pub vendor_id: u16,
    pub device_id: u16,
    pub status: u16,
    pub bar0: u32,
    pub inner: VirtioBlock,
}

impl VirtioBlockPci {
    pub fn new() -> Self {
        Self {
            vendor_id: 0x1AF4, // Red Hat
            device_id: 0x1001, // VirtIO Block
            status: 0x0001,    // Capabilities present
            bar0: 0x1000,
            inner: VirtioBlock::new("/tmp/nyx-disk").unwrap_or_else(|_| unsafe { std::mem::zeroed() }),
        }
    }
    
    pub fn new_with_inner(inner: VirtioBlock, bar0: u32) -> Self {
        Self {
            vendor_id: 0x1AF4,
            device_id: 0x1001,
            status: 0x0001,
            bar0,
            inner,
        }
    }
}

impl PciDevice for VirtioBlockPci {
    fn config_read(&mut self, reg: u8) -> u32 {
        match reg {
            0x00 => (self.device_id as u32) << 16 | (self.vendor_id as u32),
            0x08 => 0x01800000, // Mass Storage / Other
            0x10 => self.bar0 | 1, // I/O BAR bit
            0x2C => 0x00011AF4, // Subsystem ID / Vendor
            _ => 0,
        }
    }
    fn config_write(&mut self, reg: u8, value: u32) {
        if reg == 0x10 {
            self.bar0 = value & 0xFFFFFFFC;
        }
    }
}
