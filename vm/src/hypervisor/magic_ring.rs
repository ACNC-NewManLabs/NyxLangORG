//! Magic Ring FFI (Zero-Copy Host-Guest Communication)
//!
//! Provides a lock-free, shared-memory interface for high-speed
//! communication between the guest OS and the host hypervisor.

use super::memory::{GuestPhysicalAddr, VirtualMemory};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

/// Maximum entries in a ring buffer
pub const MAGIC_RING_SIZE: usize = 2048;

/// Magic Ring Commands
#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum MagicCommand {
    None = 0,
    Log = 1,
    CryptoEncrypt = 2,
    CryptoDecrypt = 3,
    TensorCopy = 4,
    HostExit = 5,
}

/// A request/response entry in the ring
#[repr(C)]
pub struct MagicEntry {
    pub cmd: AtomicU32,
    pub arg0: u64,
    pub arg1: u64,
    pub res: AtomicU32,
}

/// The Shared Magic Ring structure
#[repr(C)]
pub struct MagicRing {
    /// Head pointer (updated by guest)
    pub head: AtomicU32,
    /// Tail pointer (updated by host)
    pub tail: AtomicU32,
    /// Entries
    pub entries: [MagicEntry; MAGIC_RING_SIZE],
}

/// A thread-safe wrapper for the Magic Ring raw pointer
#[derive(Clone, Copy)]
pub struct SafeRingPtr(pub *mut MagicRing);
unsafe impl Send for SafeRingPtr {}
unsafe impl Sync for SafeRingPtr {}

/// Magic Ring Manager
pub struct MagicRingManager {
    /// Memory region dedicated to the ring (guest physical address)
    pub guest_addr: GuestPhysicalAddr,
    /// Shared state
    pub ring_ref: Arc<Mutex<Option<SafeRingPtr>>>,
}

impl MagicRingManager {
    pub fn new(guest_addr: GuestPhysicalAddr) -> Self {
        Self {
            guest_addr,
            ring_ref: Arc::new(Mutex::new(None)),
        }
    }

    /// Initialize the ring in guest memory
    pub fn init(&self, memory: &mut VirtualMemory) -> Result<(), String> {
        // Fast zero out the memory region
        let ring_size = std::mem::size_of::<MagicRing>();
        let mut mem_lock = memory.guest_memory.lock().unwrap();
        let start = self.guest_addr.0 as usize;

        if start + ring_size > mem_lock.len() {
            return Err("Magic Ring address out of bounds".to_string());
        }

        // Zero memory using fast slice fill
        mem_lock[start..start + ring_size].fill(0);
        drop(mem_lock);

        // Get host pointer to the memory
        let host_ptr = memory.get_host_ptr(self.guest_addr, ring_size)? as *mut MagicRing;
        *self.ring_ref.lock().unwrap() = Some(SafeRingPtr(host_ptr));

        Ok(())
    }

    /// Process pending entries in the ring
    pub fn process_requests(&self, memory: &VirtualMemory) {
        let lock = self.ring_ref.lock().unwrap();
        let ring_safe_ptr = match *lock {
            Some(ptr) => ptr,
            None => return,
        };
        let ring_ptr = ring_safe_ptr.0;

        unsafe {
            let ring = &*ring_ptr;
            let head = ring.head.load(Ordering::Acquire);
            let mut tail = ring.tail.load(Ordering::Acquire);

            while tail != head {
                let entry = &ring.entries[(tail % MAGIC_RING_SIZE as u32) as usize];
                let cmd_raw = entry.cmd.load(Ordering::Acquire);

                // Process commands
                match cmd_raw {
                    1 => {
                        // Log
                        // arg0: guest pointer to string, arg1: length
                        let guest_ptr = entry.arg0;
                        let len = entry.arg1 as usize;

                        if len < 1024 {
                            // Sanity check
                            let mut buf = vec![0u8; len];
                            let mem_lock = memory.guest_memory.lock().unwrap();
                            if guest_ptr as usize + len <= mem_lock.len() {
                                buf.copy_from_slice(
                                    &mem_lock[guest_ptr as usize..guest_ptr as usize + len],
                                );
                                if let Ok(s) = std::str::from_utf8(&buf) {
                                    eprintln!("[MagicRing] Guest Log: {}", s);
                                    entry.res.store(0, Ordering::Release);
                                } else {
                                    entry.res.store(1, Ordering::Release); // UTF-8 error
                                }
                            } else {
                                entry.res.store(2, Ordering::Release); // Out of bounds
                            }
                        } else {
                            entry.res.store(3, Ordering::Release); // Too long
                        }
                    }
                    2 => {
                        // CryptoEncrypt
                        // arg0: data ptr, arg1: length
                        eprintln!("[MagicRing] CryptoEncrypt size {}", entry.arg1);
                        entry.res.store(0, Ordering::Release);
                    }
                    3 => {
                        // CryptoDecrypt
                        eprintln!("[MagicRing] CryptoDecrypt size {}", entry.arg1);
                        entry.res.store(0, Ordering::Release);
                    }
                    4 => {
                        // TensorCopy
                        eprintln!(
                            "[MagicRing] TensorCopy from 0x{:x} to 0x{:x}",
                            entry.arg0, entry.arg1
                        );
                        entry.res.store(0, Ordering::Release);
                    }
                    5 => {
                        // HostExit
                        eprintln!("[MagicRing] Guest requested HostExit");
                        std::process::exit(0);
                    }
                    _ => {
                        entry.res.store(0xFFFFFFFF, Ordering::Release);
                    }
                }

                // Mark command processed
                entry.cmd.store(0, Ordering::Release);

                tail = tail.wrapping_add(1);
                ring.tail.store(tail, Ordering::Release);
            }
        }
    }
}
