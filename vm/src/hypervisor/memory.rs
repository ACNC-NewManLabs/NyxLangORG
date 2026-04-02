//! Virtual Memory Management Module
//!
//! Provides virtual memory management for VMs including page tables,
//! address translation, and memory virtualization.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Guest physical address
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuestPhysicalAddr(pub u64);

/// Host virtual address
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostVirtualAddr(pub usize);

/// Virtual address
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtAddr(pub u64);

/// Physical address
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysAddr(pub u64);

impl VirtAddr {
    /// Get page number
    pub fn page_number(&self) -> u64 {
        self.0 >> 12
    }

    /// Get offset in page
    pub fn offset(&self) -> u64 {
        self.0 & 0xFFF
    }
}

impl PhysAddr {
    /// Get frame number
    pub fn frame_number(&self) -> u64 {
        self.0 >> 12
    }

    /// Get offset in frame
    pub fn offset(&self) -> u64 {
        self.0 & 0xFFF
    }
}

/// Page table entry flags
#[derive(Debug, Clone, Copy)]
pub struct PteFlags {
    pub present: bool,
    pub writable: bool,
    pub user_accessible: bool,
    pub write_through: bool,
    pub cache_disabled: bool,
    pub accessed: bool,
    pub dirty: bool,
    pub global: bool,
    pub execute_disabled: bool,
}

impl PteFlags {
    pub fn new() -> Self {
        Self {
            present: false,
            writable: false,
            user_accessible: false,
            write_through: false,
            cache_disabled: false,
            accessed: false,
            dirty: false,
            global: false,
            execute_disabled: false,
        }
    }

    pub fn bits(&self) -> u64 {
        let mut bits = 0u64;
        if self.present {
            bits |= 1;
        }
        if self.writable {
            bits |= 2;
        }
        if self.user_accessible {
            bits |= 4;
        }
        if self.write_through {
            bits |= 8;
        }
        if self.cache_disabled {
            bits |= 0x10;
        }
        if self.accessed {
            bits |= 0x20;
        }
        if self.dirty {
            bits |= 0x40;
        }
        if self.global {
            bits |= 0x100;
        }
        if self.execute_disabled {
            bits |= 0x8000000000000000u64;
        }
        bits
    }
}

impl Default for PteFlags {
    fn default() -> Self {
        Self::new()
    }
}

/// Page table entry
#[derive(Debug, Clone, Copy)]
pub struct PageTableEntry {
    pub value: u64,
}

impl PageTableEntry {
    pub fn new(frame: u64, flags: PteFlags) -> Self {
        Self {
            value: (frame << 12) | flags.bits(),
        }
    }

    pub fn is_present(&self) -> bool {
        (self.value & 1) != 0
    }

    pub fn is_writable(&self) -> bool {
        (self.value & 2) != 0
    }

    pub fn frame_address(&self) -> u64 {
        self.value & 0x000FFFFFFFFFF000u64
    }

    pub fn flags(&self) -> PteFlags {
        PteFlags {
            present: (self.value & 1) != 0,
            writable: (self.value & 2) != 0,
            user_accessible: (self.value & 4) != 0,
            write_through: (self.value & 8) != 0,
            cache_disabled: (self.value & 0x10) != 0,
            accessed: (self.value & 0x20) != 0,
            dirty: (self.value & 0x40) != 0,
            global: (self.value & 0x100) != 0,
            execute_disabled: (self.value & 0x8000000000000000u64) != 0,
        }
    }
}

/// Page table level
#[derive(Debug, Clone, Copy)]
pub enum PageTableLevel {
    Pml4, // Level 4 (512GB entries)
    Pdpt, // Level 3 (1GB entries)
    Pd,   // Level 2 (2MB entries)
    Pt,   // Level 1 (4KB entries)
}

/// Virtual memory manager for a VM
pub struct VirtualMemory {
    /// Page table root (CR3)
    pub root: u64,
    /// Guest physical memory
    pub guest_memory: Arc<Mutex<PageAlignedBuffer>>,
    /// Memory size
    memory_size: u64,
    /// Number of pages
    _num_pages: u64,
    /// Active page table
    active_pt: PageTable,
    /// Shadow page tables
    _shadow_pts: HashMap<u64, PageTable>,
}

/// Page-aligned buffer for guest memory
#[derive(Debug)]
pub struct PageAlignedBuffer {
    ptr: *mut u8,
    layout: std::alloc::Layout,
}

impl serde::Serialize for PageAlignedBuffer {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(self.as_slice())
    }
}

impl<'de> serde::Deserialize<'de> for PageAlignedBuffer {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bytes: Vec<u8> = serde::Deserialize::deserialize(deserializer)?;
        let mut buf = Self::new(bytes.len());
        buf.copy_from_slice(&bytes);
        Ok(buf)
    }
}

impl PageAlignedBuffer {
    pub fn new(size: usize) -> Self {
        // KVM requires 4KB alignment
        let layout = std::alloc::Layout::from_size_align(size, 4096).unwrap();
        let ptr = unsafe { std::alloc::alloc_zeroed(layout) };
        if ptr.is_null() {
            panic!("Failed to allocate guest memory of size {}", size);
        }
        Self { ptr, layout }
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.layout.size()) }
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.layout.size()) }
    }
}

impl std::ops::Deref for PageAlignedBuffer {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl std::ops::DerefMut for PageAlignedBuffer {
    fn deref_mut(&mut self) -> &mut [u8] {
        self.as_mut_slice()
    }
}

unsafe impl Send for PageAlignedBuffer {}
unsafe impl Sync for PageAlignedBuffer {}

impl Drop for PageAlignedBuffer {
    fn drop(&mut self) {
        unsafe { std::alloc::dealloc(self.ptr, self.layout) };
    }
}

impl Clone for PageAlignedBuffer {
    fn clone(&self) -> Self {
        let mut new = Self::new(self.layout.size());
        new.copy_from_slice(self.as_slice());
        new
    }
}

impl VirtualMemory {
    /// Create new virtual memory
    pub fn new(memory_size: u64) -> Self {
        let num_pages = memory_size / 4096;

        Self {
            root: 0,
            guest_memory: Arc::new(Mutex::new(PageAlignedBuffer::new(memory_size as usize))),
            memory_size,
            _num_pages: num_pages,
            active_pt: PageTable::new(512),
            _shadow_pts: HashMap::new(),
        }
    }

    /// Read from guest physical memory
    pub fn read_phys(&self, addr: GuestPhysicalAddr, size: usize) -> Result<u64, String> {
        let addr_val = addr.0 as usize;
        if addr_val + size > self.memory_size as usize {
            return Err(format!(
                "Invalid physical memory access at 0x{:x}",
                addr_val
            ));
        }

        let mem = self.guest_memory.lock().unwrap();
        match size {
            1 => Ok(mem[addr_val] as u64),
            2 => {
                let bytes = [mem[addr_val], mem[addr_val + 1]];
                Ok(u16::from_le_bytes(bytes) as u64)
            }
            4 => {
                let bytes = [
                    mem[addr_val],
                    mem[addr_val + 1],
                    mem[addr_val + 2],
                    mem[addr_val + 3],
                ];
                Ok(u32::from_le_bytes(bytes) as u64)
            }
            8 => {
                let bytes = [
                    mem[addr_val],
                    mem[addr_val + 1],
                    mem[addr_val + 2],
                    mem[addr_val + 3],
                    mem[addr_val + 4],
                    mem[addr_val + 5],
                    mem[addr_val + 6],
                    mem[addr_val + 7],
                ];
                Ok(u64::from_le_bytes(bytes))
            }
            _ => Err(format!("Unsupported read size: {}", size)),
        }
    }

    /// Read a buffer from guest physical memory
    pub fn read_buf(&self, addr: GuestPhysicalAddr, buf: &mut [u8]) -> Result<(), String> {
        let addr_val = addr.0 as usize;
        if addr_val + buf.len() > self.memory_size as usize {
            return Err(format!(
                "Invalid physical memory access at 0x{:x}",
                addr_val
            ));
        }
        let mem = self.guest_memory.lock().unwrap();
        buf.copy_from_slice(&mem[addr_val..addr_val + buf.len()]);
        Ok(())
    }

    /// Write to guest physical memory
    pub fn write_phys(
        &mut self,
        addr: GuestPhysicalAddr,
        size: usize,
        value: u64,
    ) -> Result<(), String> {
        let addr_val = addr.0 as usize;
        if addr_val + size > self.memory_size as usize {
            return Err(format!(
                "Invalid physical memory access at 0x{:x}",
                addr_val
            ));
        }

        let mut mem = self.guest_memory.lock().unwrap();
        match size {
            1 => mem[addr_val] = value as u8,
            2 => mem[addr_val..addr_val + 2].copy_from_slice(&(value as u16).to_le_bytes()),
            4 => mem[addr_val..addr_val + 4].copy_from_slice(&(value as u32).to_le_bytes()),
            8 => mem[addr_val..addr_val + 8].copy_from_slice(&value.to_le_bytes()),
            _ => return Err(format!("Unsupported write size: {}", size)),
        }
        Ok(())
    }

    /// Write a buffer to guest physical memory
    pub fn write_buf(&mut self, addr: GuestPhysicalAddr, buf: &[u8]) -> Result<(), String> {
        let addr_val = addr.0 as usize;
        if addr_val + buf.len() > self.memory_size as usize {
            return Err(format!(
                "Invalid physical memory access at 0x{:x}",
                addr_val
            ));
        }
        let mut mem = self.guest_memory.lock().unwrap();
        mem[addr_val..addr_val + buf.len()].copy_from_slice(buf);
        Ok(())
    }

    /// Translate virtual address to physical using 4-level page tables (x86_64)
    pub fn translate(&self, virt_addr: VirtAddr, cr3: u64) -> Option<PhysAddr> {
        if cr3 == 0 {
            // Paging disabled or root not set; return identity map for now
            return Some(PhysAddr(virt_addr.0));
        }

        let _vpn = virt_addr.page_number();
        let offset = virt_addr.offset();

        // Level indices
        let pml4_idx = ((virt_addr.0 >> 39) & 0x1FF) as u64;
        let pdpt_idx = ((virt_addr.0 >> 30) & 0x1FF) as u64;
        let pd_idx = ((virt_addr.0 >> 21) & 0x1FF) as u64;
        let pt_idx = ((virt_addr.0 >> 12) & 0x1FF) as u64;

        // 1. PML4
        let pml4_entry_addr = cr3 + (pml4_idx * 8);
        let pml4_val = self.read_phys(GuestPhysicalAddr(pml4_entry_addr), 8).ok()?;
        let pml4_entry = PageTableEntry { value: pml4_val };
        if !pml4_entry.is_present() {
            return None;
        }

        // 2. PDPT
        let pdpt_addr = pml4_entry.frame_address() + (pdpt_idx * 8);
        let pdpt_val = self.read_phys(GuestPhysicalAddr(pdpt_addr), 8).ok()?;
        let pdpt_entry = PageTableEntry { value: pdpt_val };
        if !pdpt_entry.is_present() {
            return None;
        }

        // 1GB huge page check
        if (pdpt_entry.value & 0x80) != 0 {
            let phys = (pdpt_entry.frame_address() & !0x3FFFFFFF) | (virt_addr.0 & 0x3FFFFFFF);
            return Some(PhysAddr(phys));
        }

        // 3. PD
        let pd_addr = pdpt_entry.frame_address() + (pd_idx * 8);
        let pd_val = self.read_phys(GuestPhysicalAddr(pd_addr), 8).ok()?;
        let pd_entry = PageTableEntry { value: pd_val };
        if !pd_entry.is_present() {
            return None;
        }

        // 2MB huge page check
        if (pd_entry.value & 0x80) != 0 {
            let phys = (pd_entry.frame_address() & !0x1FFFFF) | (virt_addr.0 & 0x1FFFFF);
            return Some(PhysAddr(phys));
        }

        // 4. PT
        let pt_addr = pd_entry.frame_address() + (pt_idx * 8);
        let pt_val = self.read_phys(GuestPhysicalAddr(pt_addr), 8).ok()?;
        let pt_entry = PageTableEntry { value: pt_val };
        if !pt_entry.is_present() {
            return None;
        }

        let phys = pt_entry.frame_address() | offset;
        Some(PhysAddr(phys))
    }

    /// Map a virtual address to physical
    pub fn map(&mut self, virt: VirtAddr, phys: PhysAddr, flags: PteFlags) {
        let vpn = virt.page_number();
        let pml4_idx = ((vpn >> 27) & 0x509) as usize;

        self.active_pt
            .set_entry(pml4_idx, PageTableEntry::new(phys.frame_number(), flags));
    }

    /// Get host pointer to guest physical memory
    pub fn get_host_ptr(&self, addr: GuestPhysicalAddr, size: usize) -> Result<*mut u8, String> {
        let addr = addr.0 as usize;
        if addr + size > self.memory_size as usize {
            return Err("Invalid physical memory access".to_string());
        }

        let mem = self.guest_memory.lock().unwrap();
        Ok(unsafe { mem.as_ptr().add(addr) as *mut u8 })
    }

    /// Get memory size
    pub fn memory_size(&self) -> u64 {
        self.memory_size
    }
}

/// Page table structure
#[derive(Debug)]
pub struct PageTable {
    pub entries: Vec<PageTableEntry>,
}

impl PageTable {
    pub fn new(num_entries: usize) -> Self {
        Self {
            entries: vec![PageTableEntry { value: 0 }; num_entries],
        }
    }

    pub fn get_entry(&self, idx: usize) -> Option<PageTableEntry> {
        self.entries.get(idx).copied()
    }

    pub fn set_entry(&mut self, idx: usize, entry: PageTableEntry) {
        if idx < self.entries.len() {
            self.entries[idx] = entry;
        }
    }

    pub fn clear(&mut self) {
        for entry in &mut self.entries {
            *entry = PageTableEntry { value: 0 };
        }
    }
}

/// Memory region for DMA
#[derive(Debug, Clone)]
pub struct DmaRegion {
    pub guest_addr: u64,
    pub host_addr: usize,
    pub size: usize,
}

/// Physical memory allocator
pub struct PhysFrameAllocator {
    /// Total frames
    pub total_frames: u64,
    /// Available frames
    pub available_frames: u64,
    /// Frame bitmap
    bitmap: Vec<u8>,
}

impl PhysFrameAllocator {
    pub fn new(num_frames: u64) -> Self {
        let bitmap_size = (num_frames + 7) / 8;
        Self {
            total_frames: num_frames,
            available_frames: num_frames,
            bitmap: vec![0u8; bitmap_size as usize],
        }
    }

    /// Allocate a frame
    pub fn allocate(&mut self) -> Option<u64> {
        for (i, byte) in self.bitmap.iter_mut().enumerate() {
            if *byte != 0xFF {
                for bit in 0..8 {
                    if (*byte & (1 << bit)) == 0 {
                        *byte |= 1 << bit;
                        self.available_frames -= 1;
                        return Some((i * 8 + bit) as u64);
                    }
                }
            }
        }
        None
    }

    /// Free a frame
    pub fn free(&mut self, frame: u64) {
        let idx = frame as usize / 8;
        let bit = frame as usize % 8;

        if idx < self.bitmap.len() {
            self.bitmap[idx] &= !(1 << bit);
            self.available_frames += 1;
        }
    }
}

/// Memory slot for VM
#[derive(Debug, Clone)]
pub struct MemorySlot {
    pub guest_phys_start: u64,
    pub size: u64,
    pub host_ptr: usize,
}

impl MemorySlot {
    pub fn new(guest_phys_start: u64, size: u64) -> Self {
        // Allocate host memory
        let layout = std::alloc::Layout::from_size_align(size as usize, 4096).unwrap();
        let host_ptr = unsafe { std::alloc::alloc(layout) } as usize;

        Self {
            guest_phys_start,
            size,
            host_ptr,
        }
    }

    pub fn contains_gpa(&self, gpa: u64) -> bool {
        gpa >= self.guest_phys_start && gpa < self.guest_phys_start + self.size
    }

    pub fn offset(&self, gpa: u64) -> Option<usize> {
        if self.contains_gpa(gpa) {
            Some((gpa - self.guest_phys_start) as usize + self.host_ptr)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_virtual_memory() {
        let mut vm = VirtualMemory::new(1024 * 1024);

        // Write and read back
        vm.write_phys(GuestPhysicalAddr(0x1000), 4, 0xDEADBEEF)
            .unwrap();
        let value = vm.read_phys(GuestPhysicalAddr(0x1000), 4).unwrap();

        assert_eq!(value, 0xDEADBEEF);
    }

    #[test]
    fn test_page_table_entry() {
        let flags = PteFlags {
            present: true,
            writable: true,
            ..Default::default()
        };

        let entry = PageTableEntry::new(0x1000, flags);
        assert!(entry.is_present());
        assert!(entry.is_writable());
    }
}
