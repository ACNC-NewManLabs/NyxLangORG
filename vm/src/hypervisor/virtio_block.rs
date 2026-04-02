//! VirtIO Block Device
//!
//! Provides virtualized disk access for guest operating systems.

use super::devices::{DeviceResult, DeviceType, VirtualDevice};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};

pub enum Storage {
    File(File),
    Ram(Vec<u8>),
}

pub struct VirtioBlock {
    pub storage: Storage,
    pub disk_size: u64,
}

impl VirtioBlock {
    pub fn new(path: &str) -> Result<Self, String> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| format!("Failed to open disk image: {}", e))?;
        let size = file.metadata().map_err(|e| e.to_string())?.len();
        Ok(Self {
            storage: Storage::File(file),
            disk_size: size,
        })
    }

    pub fn new_ram(size: u64) -> Self {
        Self {
            storage: Storage::Ram(vec![0u8; size as usize]),
            disk_size: size,
        }
    }

    pub fn read_sectors(&mut self, lba: u64, buf: &mut [u8]) -> Result<(), String> {
        let offset = (lba * 512) as usize;
        match &mut self.storage {
            Storage::File(f) => {
                f.seek(SeekFrom::Start(offset as u64))
                    .map_err(|e| e.to_string())?;
                f.read_exact(buf).map_err(|e| e.to_string())
            }
            Storage::Ram(data) => {
                if offset + buf.len() <= data.len() {
                    buf.copy_from_slice(&data[offset..offset + buf.len()]);
                    Ok(())
                } else {
                    Err("Out of bounds".to_string())
                }
            }
        }
    }

    pub fn write_sectors(&mut self, lba: u64, buf: &[u8]) -> Result<(), String> {
        let offset = (lba * 512) as usize;
        match &mut self.storage {
            Storage::File(f) => {
                f.seek(SeekFrom::Start(offset as u64))
                    .map_err(|e| e.to_string())?;
                f.write_all(buf).map_err(|e| e.to_string())
            }
            Storage::Ram(data) => {
                if offset + buf.len() <= data.len() {
                    data[offset..offset + buf.len()].copy_from_slice(buf);
                    Ok(())
                } else {
                    Err("Out of bounds".to_string())
                }
            }
        }
    }
}

impl VirtualDevice for VirtioBlock {
    fn device_type(&self) -> DeviceType {
        DeviceType::Block
    }
    fn name(&self) -> &str {
        "virtio-block"
    }
    fn read(&mut self, _port: u16, _size: usize) -> DeviceResult<u64> {
        Ok(0)
    }
    fn write(&mut self, _port: u16, _size: usize, _value: u64) -> DeviceResult<()> {
        Ok(())
    }
    fn interrupt(&mut self, _irq: u8) -> DeviceResult<()> {
        Ok(())
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl super::virtio::VirtioDevice for VirtioBlock {
    fn device_id(&self) -> u32 {
        2
    } // VirtIO Block
    fn config_size(&self) -> usize {
        8
    }
    fn reset(&mut self) {}

    fn notify(
        &mut self,
        _q_idx: u32,
        q: &mut super::virtio::VirtioQueue,
        mem: &mut super::memory::VirtualMemory,
    ) {
        while let Some(head_idx) = q.get_next_avail(mem) {
            // 1. Read header (Type, Priority, Sector)
            let mut head_buf = [0u8; 16];
            let desc_table_ptr = q.desc_table + (head_idx as u64 * 16);
            let addr = mem
                .read_phys(super::memory::GuestPhysicalAddr(desc_table_ptr), 8)
                .unwrap_or(0);
            let _ = mem.read_buf(super::memory::GuestPhysicalAddr(addr), &mut head_buf);

            let req_type = u32::from_le_bytes([head_buf[0], head_buf[1], head_buf[2], head_buf[3]]);
            let sector = u64::from_le_bytes([
                head_buf[8],
                head_buf[9],
                head_buf[10],
                head_buf[11],
                head_buf[12],
                head_buf[13],
                head_buf[14],
                head_buf[15],
            ]);

            // 2. Read/Write Data
            // (Simplified: assume next descriptor is data)
            let next_idx = mem
                .read_phys(super::memory::GuestPhysicalAddr(desc_table_ptr + 14), 2)
                .unwrap_or(0) as u16;
            let next_desc_ptr = q.desc_table + (next_idx as u64 * 16);
            let data_addr = mem
                .read_phys(super::memory::GuestPhysicalAddr(next_desc_ptr), 8)
                .unwrap_or(0);
            let data_len = mem
                .read_phys(super::memory::GuestPhysicalAddr(next_desc_ptr + 8), 4)
                .unwrap_or(0) as u32;

            match req_type {
                0 => {
                    // READ (VIRTIO_BLK_T_IN)
                    let mut buf = vec![0u8; data_len as usize];
                    let _ = self.read_sectors(sector, &mut buf);
                    let _ = mem.write_buf(super::memory::GuestPhysicalAddr(data_addr), &buf);
                }
                1 => {
                    // WRITE (VIRTIO_BLK_T_OUT)
                    let mut buf = vec![0u8; data_len as usize];
                    let _ = mem.read_buf(super::memory::GuestPhysicalAddr(data_addr), &mut buf);
                    let _ = self.write_sectors(sector, &buf);
                }
                _ => {}
            }

            // 3. Status byte
            // (Simplified: assume third descriptor is status)
            let status_idx = mem
                .read_phys(super::memory::GuestPhysicalAddr(next_desc_ptr + 14), 2)
                .unwrap_or(0) as u16;
            let status_desc_ptr = q.desc_table + (status_idx as u64 * 16);
            let status_addr = mem
                .read_phys(super::memory::GuestPhysicalAddr(status_desc_ptr), 8)
                .unwrap_or(0);
            let _ = mem.write_phys(super::memory::GuestPhysicalAddr(status_addr), 1, 0); // VIRTIO_BLK_S_OK

            q.add_used(mem, head_idx, data_len + 1);
        }
    }
}
