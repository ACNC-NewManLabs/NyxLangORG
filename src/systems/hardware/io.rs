//! I/O Port Operations Module

use super::inline_asm::io as asm_io;

pub struct Port {
    port: u16,
}
impl Port {
    pub const fn new(port: u16) -> Self {
        Self { port }
    }
    #[inline]
    pub fn readb(&self) -> u8 {
        unsafe { asm_io::inb(self.port) }
    }
    #[inline]
    pub fn writeb(&self, value: u8) {
        unsafe { asm_io::outb(self.port, value) }
    }
    #[inline]
    pub fn readw(&self) -> u16 {
        unsafe { asm_io::inw(self.port) }
    }
    #[inline]
    pub fn writew(&self, value: u16) {
        unsafe { asm_io::outw(self.port, value) }
    }
    #[inline]
    pub fn readl(&self) -> u32 {
        unsafe { asm_io::inl(self.port) }
    }
    #[inline]
    pub fn writel(&self, value: u32) {
        unsafe { asm_io::outl(self.port, value) }
    }
}

#[allow(dead_code)]
pub struct IoPort<T: Copy> {
    port: u16,
    _phantom: core::marker::PhantomData<T>,
}
impl<T: Copy> IoPort<T> {
    pub const fn new(port: u16) -> Self {
        Self {
            port,
            _phantom: core::marker::PhantomData,
        }
    }
}

pub mod ports {
    pub const KEYBOARD_DATA: u16 = 0x60;
    pub const KEYBOARD_COMMAND: u16 = 0x64;
    pub const CMOS_INDEX: u16 = 0x70;
    pub const CMOS_DATA: u16 = 0x71;
    pub const PIC_MASTER_CMD: u16 = 0x20;
    pub const PIC_MASTER_DATA: u16 = 0x21;
    pub const PIC_SLAVE_CMD: u16 = 0xA0;
    pub const PIC_SLAVE_DATA: u16 = 0xA1;
    pub const PIT_CHANNEL_0: u16 = 0x40;
    pub const PIT_CHANNEL_1: u16 = 0x41;
    pub const PIT_CHANNEL_2: u16 = 0x42;
    pub const PIT_COMMAND: u16 = 0x43;
    pub const VGA_INDEX: u16 = 0x3D4;
    pub const VGA_DATA: u16 = 0x3D5;
}

pub const PCI_ADDRESS: u16 = 0xCF8;
pub const PCI_DATA: u16 = 0xCFC;

pub struct PciConfig;
impl PciConfig {
    pub fn read(_bus: u8, _slot: u8, _func: u8, _offset: u8) -> u32 {
        0
    }
    pub fn write(_bus: u8, _slot: u8, _func: u8, _offset: u8, _value: u32) {}
    pub fn vendor_id(_bus: u8, _slot: u8) -> u16 {
        0xFFFF
    }
    pub fn device_exists(_bus: u8, _slot: u8) -> bool {
        false
    }
}
