//! Local APIC (Advanced Programmable Interrupt Controller)
//!
//! Provides interrupt management and timers for virtual CPUs.

use super::devices::{VirtualDevice, DeviceType, DeviceResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApicTimerMode {
    OneShot,
    Periodic,
}

pub struct LocalApic {
    pub id: u32,
    pub version: u32,
    pub lvt_timer: u32,
    pub timer_initial_count: u32,
    pub timer_current_count: u32,
    pub timer_divide_cfg: u32,
    pub enabled: bool,
}

impl LocalApic {
    pub fn new(id: u32) -> Self {
        Self {
            id,
            version: 0x14, // Version 1.4
            lvt_timer: 0x00010000, // Masked initially
            timer_initial_count: 0,
            timer_current_count: 0,
            timer_divide_cfg: 0,
            enabled: true,
        }
    }

    pub fn read_apic(&self, offset: u32) -> u32 {
        match offset {
            0x20 => self.id << 24,
            0x30 => self.version,
            0x320 => self.lvt_timer,
            0x380 => self.timer_initial_count,
            0x390 => self.timer_current_count,
            0x3E0 => self.timer_divide_cfg,
            _ => 0,
        }
    }

    pub fn write_apic(&mut self, offset: u32, value: u32) {
        match offset {
            0x0B0 => { /* EOI - End of Interrupt */ },
            0x320 => self.lvt_timer = value,
            0x380 => {
                self.timer_initial_count = value;
                self.timer_current_count = value;
            }
            0x3E0 => self.timer_divide_cfg = value,
            _ => {}
        }
    }
}

impl VirtualDevice for LocalApic {
    fn device_type(&self) -> DeviceType { DeviceType::Pic }
    fn name(&self) -> &str { "local-apic" }
    fn read(&mut self, port: u16, _size: usize) -> DeviceResult<u64> {
        Ok(self.read_apic(port as u32) as u64)
    }
    fn write(&mut self, port: u16, _size: usize, value: u64) -> DeviceResult<()> {
        self.write_apic(port as u32, value as u32);
        Ok(())
    }
    fn interrupt(&mut self, _irq: u8) -> DeviceResult<()> { Ok(()) }
    
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
