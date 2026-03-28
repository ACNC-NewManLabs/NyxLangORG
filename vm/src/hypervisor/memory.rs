//! Virtual Memory Management Module
//! 
//! Provides virtual memory management for VMs including page tables,
//! address translation, and memory virtualization.

use std::collections::HashMap;

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
        if self.present { bits |= 1; }
        if self.writable { bits |= 2; }
        if self.user_accessible { bits |= 4; }
        if self.write_through { bits |= 8; }
        if self.cache_disabled { bits |= 0x10; }
        if self.accessed { bits |= 0x20; }
        if self.dirty { bits |= 0x40; }
        if self.global { bits |= 0x100; }
        if self.execute_disabled { bits |= 0x8000000000000000u64; }
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
    Pml4,     // Level 4 (512GB entries)
    Pdpt,     // Level 3 (1GB entries)
    Pd,       // Level 2 (2MB entries)
    Pt,       // Level 1 (4KB entries)
}

/// Virtual memory manager for a VM
pub struct VirtualMemory {
    /// Page table root (CR3)
    pub root: u64,
    /// Guest physical memory
    guest_memory: Vec<u8>,
    /// Memory size
    memory_size: u64,
    /// Number of pages
    num_pages: u64,
    /// Active page table
    active_pt: PageTable,
    /// Shadow page tables
    shadow_pts: HashMap<u64, PageTable>,
}

impl VirtualMemory {
    /// Create new virtual memory
    pub fn new(memory_size: u64) -> Self {
        let num_pages = memory_size / 4096;
        
        Self {
            root: 0,
            guest_memory: vec![0u8; memory_size as usize],
            memory_size,
            num_pages,
            active_pt: PageTable::new(512),
            shadow_pts: HashMap::new(),
        }
    }
    
    /// Read from guest physical memory
    pub fn read_phys(&self, addr: GuestPhysicalAddr, size: usize) -> Result<u64, String> {
        let addr = addr.0 as usize;
        if addr + size > self.memory_size as usize {
            return Err("Invalid physical memory access".to_string());
        }
        
        match size {
            1 => Ok(self.guest_memory[addr] as u64),
            2 => Ok(u16::from_le_bytes([self.guest_memory[addr], self.guest_memory[addr + 1]]) as u64),
            4 => Ok(u32::from_le_bytes([self.guest_memory[addr], self.guest_memory[addr + 1],
                                         self.guest_memory[addr + 2], self.guest_memory[addr + 3]]) as u64),
            8 => Ok(u64::from_le_bytes([self.guest_memory[addr], self.guest_memory[addr + 1],
                                         self.guest_memory[addr + 2], self.guest_memory[addr + 3],
                                         self.guest_memory[addr + 4], self.guest_memory[addr + 5],
                                         self.guest_memory[addr + 6], self.guest_memory[addr + 7]])),
            _ => Err("Invalid size".to_string()),
        }
    }
    
    /// Write to guest physical memory
    pub fn write_phys(&mut self, addr: GuestPhysicalAddr, size: usize, value: u64) -> Result<(), String> {
        let addr = addr.0 as usize;
        if addr + size > self.memory_size as usize {
            return Err("Invalid physical memory access".to_string());
        }
        
        match size {
            1 => self.guest_memory[addr] = value as u8,
            2 => self.guest_memory[addr..addr + 2].copy_from_slice(&(value as u16).to_le_bytes()),
            4 => self.guest_memory[addr..addr + 4].copy_from_slice(&(value as u32).to_le_bytes()),
            8 => self.guest_memory[addr..addr + 8].copy_from_slice(&value.to_le_bytes()),
            _ => return Err("Invalid size".to_string()),
        }
        Ok(())
    }
    
    /// Translate virtual address to physical
    pub fn translate(&self, virt_addr: VirtAddr) -> Option<PhysAddr> {
        // Simplified page table walk
        let vpn = virt_addr.page_number();
        
        // Level 4 (PML4)
        let pml4_idx = ((vpn >> 27) & 0x1FF) as usize;
        let pml4_entry = self.active_pt.get_entry(pml4_idx)?;
        
        if !pml4_entry.is_present() {
            return None;
        }
        
        // For simplicity, return identity mapping
        Some(PhysAddr(virt_addr.0))
    }
    
    /// Map a virtual address to physical
    pub fn map(&mut self, virt: VirtAddr, phys: PhysAddr, flags: PteFlags) {
        let vpn = virt.page_number();
        let pml4_idx = ((vpn >> 27) & 0x509) as usize;
        
        self.active_pt.set_entry(pml4_idx, PageTableEntry::new(phys.frame_number(), flags));
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
        let layout = std::alloc::Layout::from_size_align(size as usize, 4096)
            .unwrap();
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
        vm.write_phys(GuestPhysicalAddr(0x1000), 4, 0xDEADBEEF).unwrap();
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

