//! CMOS / RTC (Real-Time Clock) Device
//!
//! Emulates the standard Motorola 146818 CMOS/RTC chip used for 
//! timekeeping and non-volatile configuration storage.

use super::devices::{VirtualDevice, DeviceType, DeviceResult};
use chrono::{Datelike, Timelike, Local};

pub struct CmosDevice {
    pub index: u8,
    pub data: [u8; 128],
}

impl CmosDevice {
    pub fn new() -> Self {
        let mut cmos = Self {
            index: 0,
            data: [0; 128],
        };
        cmos.update_rtc();
        cmos
    }

    /// Update RTC registers from host system time
    pub fn update_rtc(&mut self) {
        let now = Local::now();
        self.data[0x00] = self.to_bcd(now.second() as u8);
        self.data[0x02] = self.to_bcd(now.minute() as u8);
        self.data[0x04] = self.to_bcd(now.hour() as u8);
        self.data[0x06] = self.to_bcd(now.weekday().number_from_monday() as u8);
        self.data[0x07] = self.to_bcd(now.day() as u8);
        self.data[0x08] = self.to_bcd(now.month() as u8);
        self.data[0x09] = self.to_bcd((now.year() % 100) as u8);
        self.data[0x32] = self.to_bcd((now.year() / 100) as u8); // Century
    }

    fn to_bcd(&self, val: u8) -> u8 {
        ((val / 10) << 4) | (val % 10)
    }

    /// Set memory size in CMOS (compatible with BIOS expectations)
    pub fn set_memory_size(&mut self, size_kb: u64) {
        let low = (size_kb.min(65535)) as u16;
        self.data[0x30] = (low & 0xFF) as u8;
        self.data[0x31] = (low >> 8) as u8;

        if size_kb > 65535 {
            let high = ((size_kb - 65535) / 64).min(65535) as u16;
            self.data[0x34] = (high & 0xFF) as u8;
            self.data[0x35] = (high >> 8) as u8;
        }
    }
}

impl VirtualDevice for CmosDevice {
    fn device_type(&self) -> DeviceType { DeviceType::Pic }
    fn name(&self) -> &str { "cmos-rtc" }

    fn read(&mut self, port: u16, _size: usize) -> DeviceResult<u64> {
        if port == 0x71 {
            if self.index < 0x0A {
                self.update_rtc();
            }
            Ok(self.data[self.index as usize & 0x7F] as u64)
        } else {
            Ok(0)
        }
    }

    fn write(&mut self, port: u16, _size: usize, value: u64) -> DeviceResult<()> {
        if port == 0x70 {
            self.index = (value as u8) & 0x7F;
        } else if port == 0x71 {
            self.data[self.index as usize & 0x7F] = value as u8;
        }
        Ok(())
    }

    fn interrupt(&mut self, _irq: u8) -> DeviceResult<()> { Ok(()) }
    
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
