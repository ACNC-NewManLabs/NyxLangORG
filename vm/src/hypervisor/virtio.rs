//! VirtIO Implementation Module
//!
//! Provides industry-standard VirtIO device emulation for high-performance I/O.

use super::devices::{DeviceResult, DeviceType, VirtualDevice};
use super::memory::{GuestPhysicalAddr, VirtualMemory};
use std::sync::{Arc, Mutex};

/// VirtIO descriptor flags
pub const VRING_DESC_F_NEXT: u16 = 1;
pub const VRING_DESC_F_WRITE: u16 = 2;

/// VirtIO Queue structure
#[derive(Clone)]
pub struct VirtioQueue {
    pub desc_table: u64,
    pub avail_ring: u64,
    pub used_ring: u64,
    pub size: u16,
    pub last_avail_idx: u16,
}

impl VirtioQueue {
    pub fn new(size: u16) -> Self {
        Self {
            desc_table: 0,
            avail_ring: 0,
            used_ring: 0,
            size,
            last_avail_idx: 0,
        }
    }

    pub fn get_next_avail(&mut self, memory: &VirtualMemory) -> Option<u16> {
        let avail_ring_ptr = self.avail_ring;
        let guest_idx = memory
            .read_phys(GuestPhysicalAddr(avail_ring_ptr + 2), 2)
            .ok()? as u16;
        if self.last_avail_idx == guest_idx {
            return None;
        }

        let ring_offset = 4 + (self.last_avail_idx % self.size) as u64 * 2;
        let desc_idx = memory
            .read_phys(GuestPhysicalAddr(avail_ring_ptr + ring_offset), 2)
            .ok()? as u16;
        self.last_avail_idx = self.last_avail_idx.wrapping_add(1);
        Some(desc_idx)
    }

    pub fn add_used(&mut self, memory: &mut VirtualMemory, desc_idx: u16, len: u32) {
        let used_ring_ptr = self.used_ring;
        let used_idx = memory
            .read_phys(GuestPhysicalAddr(used_ring_ptr + 2), 2)
            .unwrap_or(0) as u16;

        let elem_offset = 4 + (used_idx % self.size) as u64 * 8;
        let _ = memory.write_phys(
            GuestPhysicalAddr(used_ring_ptr + elem_offset),
            4,
            desc_idx as u64,
        );
        let _ = memory.write_phys(
            GuestPhysicalAddr(used_ring_ptr + elem_offset + 4),
            4,
            len as u64,
        );
        let _ = memory.write_phys(
            GuestPhysicalAddr(used_ring_ptr + 2),
            2,
            used_idx.wrapping_add(1) as u64,
        );
    }
}

/// VirtIO Device Trait
pub trait VirtioDevice: Send {
    fn device_id(&self) -> u32;
    fn config_size(&self) -> usize;
    fn reset(&mut self);
    fn notify(&mut self, queue_idx: u32, queue: &mut VirtioQueue, memory: &mut VirtualMemory);
}

/// VirtIO MMIO Transport
pub struct VirtioMmio {
    device: Arc<Mutex<dyn VirtioDevice>>,
    queues: Vec<VirtioQueue>,
    selected_queue: u32,
    status: u32,
    features: u32,
}

impl VirtioMmio {
    pub fn new(device: Arc<Mutex<dyn VirtioDevice>>) -> Self {
        Self {
            device,
            queues: vec![VirtioQueue::new(128); 4],
            selected_queue: 0,
            status: 0,
            features: 0,
        }
    }
}

impl VirtualDevice for VirtioMmio {
    fn device_type(&self) -> DeviceType {
        DeviceType::Pci
    }
    fn name(&self) -> &str {
        "virtio-mmio"
    }

    fn read(&mut self, port: u16, _size: usize) -> DeviceResult<u64> {
        match port {
            0x00 => Ok(0x74726976),
            0x08 => Ok(self.device.lock().unwrap().device_id() as u64),
            _ => Ok(0),
        }
    }

    fn write(&mut self, port: u16, _size: usize, value: u64) -> DeviceResult<()> {
        match port {
            0x14 => self.features = value as u32,
            0x30 => self.selected_queue = value as u32,
            0x40 => self.queues[self.selected_queue as usize].desc_table = value,
            0x50 => {
                // QUEUE_NOTIFY
                let q_idx = value as u32;
                if q_idx < self.queues.len() as u32 {
                    // This would normally trigger the VMM thread notification
                }
            }
            0x70 => self.status = value as u32,
            _ => {}
        }
        Ok(())
    }

    fn interrupt(&mut self, _irq: u8) -> DeviceResult<()> {
        Ok(())
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// VirtIO Console Implementation
pub struct VirtioConsole {
    id: u32,
}

impl VirtioConsole {
    pub fn new() -> Self {
        Self { id: 3 }
    }
}

impl VirtioDevice for VirtioConsole {
    fn device_id(&self) -> u32 {
        self.id
    }
    fn config_size(&self) -> usize {
        8
    }
    fn reset(&mut self) {}
    fn notify(&mut self, _q_idx: u32, _q: &mut VirtioQueue, _mem: &mut VirtualMemory) {}
}
