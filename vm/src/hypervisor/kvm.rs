//! KVM Hardware Acceleration Backend
//!
//! Provides hardware-assisted virtualization using the Linux KVM (Kernel Virtual Machine) API.

use super::cpu::{CpuState, Register};
#[cfg(target_os = "linux")]
use kvm_bindings::{kvm_regs, kvm_sregs, kvm_userspace_memory_region};
#[cfg(target_os = "linux")]
use kvm_ioctls::{Kvm, VcpuExit, VcpuFd, VmFd};
use std::sync::{Arc, Mutex};

/// KVM Context for hardware acceleration
pub struct KvmContext {
    #[cfg(target_os = "linux")]
    pub kvm: Kvm,
    #[cfg(target_os = "linux")]
    pub vm_fd: VmFd,
    #[cfg(target_os = "linux")]
    pub vcpu_fds: Vec<VcpuFd>,
}

impl KvmContext {
    /// Create a new KVM context
    #[cfg(target_os = "linux")]
    pub fn new(_memory_size: u64, num_cpus: usize) -> Result<Self, String> {
        let kvm = Kvm::new().map_err(|e| format!("Failed to open /dev/kvm: {}", e))?;
        let vm_fd = kvm
            .create_vm()
            .map_err(|e| format!("Failed to create VM: {}", e))?;

        // We'll map the memory later in the VirtualMachine::new

        Ok(Self {
            kvm,
            vm_fd,
            vcpu_fds: Vec::with_capacity(num_cpus),
        })
    }

    /// Factory for fake context on non-linux platforms
    #[cfg(not(target_os = "linux"))]
    pub fn new(_memory_size: u64, _num_cpus: usize) -> Result<Self, String> {
        Err("KVM is only supported on Linux".to_string())
    }

    /// Map userspace memory to KVM guest
    #[cfg(target_os = "linux")]
    pub fn map_memory(
        &self,
        slot: u32,
        guest_addr: u64,
        memory: &Arc<Mutex<super::memory::PageAlignedBuffer>>,
    ) -> Result<(), String> {
        let mem_lock = memory.lock().unwrap();
        let size = mem_lock.len() as u64;
        let userspace_addr = mem_lock.as_ptr() as u64;

        let region = kvm_userspace_memory_region {
            slot,
            guest_phys_addr: guest_addr,
            memory_size: size,
            userspace_addr,
            flags: kvm_bindings::KVM_MEM_LOG_DIRTY_PAGES,
        };

        unsafe {
            self.vm_fd
                .set_user_memory_region(region)
                .map_err(|e| format!("Failed to set memory region: {}", e))?;
        }
        Ok(())
    }

    /// Create a VCPU
    #[cfg(target_os = "linux")]
    pub fn create_vcpu(&mut self, id: u8) -> Result<(), String> {
        let vcpu_fd = self
            .vm_fd
            .create_vcpu(id as u64)
            .map_err(|e| format!("Failed to create VCPU {}: {}", id, e))?;
        self.vcpu_fds.push(vcpu_fd);
        Ok(())
    }

    /// Set VCPU registers
    #[cfg(target_os = "linux")]
    pub fn set_regs(&self, vcpu_idx: usize, regs: kvm_regs) -> Result<(), String> {
        let vcpu = &self.vcpu_fds[vcpu_idx];
        vcpu.set_regs(&regs)
            .map_err(|e| format!("Failed to set regs: {}", e))
    }

    /// Set VCPU special registers
    #[cfg(target_os = "linux")]
    pub fn set_sregs(&self, vcpu_idx: usize, sregs: kvm_sregs) -> Result<(), String> {
        let vcpu = &self.vcpu_fds[vcpu_idx];
        vcpu.set_sregs(&sregs)
            .map_err(|e| format!("Failed to set sregs: {}", e))
    }

    /// Get VCPU registers
    #[cfg(target_os = "linux")]
    pub fn get_regs(&self, vcpu_idx: usize) -> Result<kvm_regs, String> {
        let vcpu = &self.vcpu_fds[vcpu_idx];
        vcpu.get_regs()
            .map_err(|e| format!("Failed to get regs: {}", e))
    }

    /// Run the VCPU
    #[cfg(target_os = "linux")]
    pub fn run_vcpu(&mut self, vcpu_idx: usize) -> Result<VcpuExit<'_>, String> {
        let vcpu = &mut self.vcpu_fds[vcpu_idx];
        vcpu.run()
            .map_err(|e| format!("KVM VCPU run failed: {}", e))
    }

    /// Synchronize Nyx CpuState to KVM registers
    #[cfg(target_os = "linux")]
    pub fn sync_regs_to_kvm(&self, vcpu_idx: usize, state: &CpuState) -> Result<(), String> {
        let mut regs = self.get_regs(vcpu_idx)?;

        // Map GPRs
        regs.rax = state.read_gpr(Register::Rax);
        regs.rbx = state.read_gpr(Register::Rbx);
        regs.rcx = state.read_gpr(Register::Rcx);
        regs.rdx = state.read_gpr(Register::Rdx);
        regs.rsi = state.read_gpr(Register::Rsi);
        regs.rdi = state.read_gpr(Register::Rdi);
        regs.rsp = state.read_gpr(Register::Rsp);
        regs.rbp = state.read_gpr(Register::Rbp);
        regs.r8 = state.read_gpr(Register::R8);
        regs.r9 = state.read_gpr(Register::R9);
        regs.r10 = state.read_gpr(Register::R10);
        regs.r11 = state.read_gpr(Register::R11);
        regs.r12 = state.read_gpr(Register::R12);
        regs.r13 = state.read_gpr(Register::R13);
        regs.r14 = state.read_gpr(Register::R14);
        regs.r15 = state.read_gpr(Register::R15);
        regs.rip = state.get_pc();
        regs.rflags = state.rflags | 0x2; // Basic bit 1 must be set

        self.set_regs(vcpu_idx, regs)?;

        // Sync FPU/SSE
        let mut fpu = self.vcpu_fds[vcpu_idx]
            .get_fpu()
            .map_err(|e| e.to_string())?;
        for i in 0..16 {
            // KVM x86_64 has 16 XMM registers
            let v = state.vreg[i];
            let bytes = v.to_le_bytes();
            fpu.xmm[i].copy_from_slice(&bytes);
        }
        self.vcpu_fds[vcpu_idx]
            .set_fpu(&fpu)
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    /// Synchronize KVM registers back to Nyx CpuState
    #[cfg(target_os = "linux")]
    pub fn sync_regs_from_kvm(&self, vcpu_idx: usize, state: &mut CpuState) -> Result<(), String> {
        let regs = self.get_regs(vcpu_idx)?;

        state.write_gpr(Register::Rax, regs.rax);
        state.write_gpr(Register::Rbx, regs.rbx);
        state.write_gpr(Register::Rcx, regs.rcx);
        state.write_gpr(Register::Rdx, regs.rdx);
        state.write_gpr(Register::Rsi, regs.rsi);
        state.write_gpr(Register::Rdi, regs.rdi);
        state.write_gpr(Register::Rsp, regs.rsp);
        state.write_gpr(Register::Rbp, regs.rbp);
        state.write_gpr(Register::R8, regs.r8);
        state.write_gpr(Register::R9, regs.r9);
        state.write_gpr(Register::R10, regs.r10);
        state.write_gpr(Register::R11, regs.r11);
        state.write_gpr(Register::R12, regs.r12);
        state.write_gpr(Register::R13, regs.r13);
        state.write_gpr(Register::R14, regs.r14);
        state.write_gpr(Register::R15, regs.r15);
        state.set_pc(regs.rip);
        state.rflags = regs.rflags;

        // Sync FPU/SSE back
        let fpu = self.vcpu_fds[vcpu_idx]
            .get_fpu()
            .map_err(|e| e.to_string())?;
        for i in 0..16 {
            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(&fpu.xmm[i]);
            state.vreg[i] = u128::from_le_bytes(bytes);
        }

        Ok(())
    }

    /// Get dirty log for a memory slot
    #[cfg(target_os = "linux")]
    pub fn get_dirty_log(&self, slot: u32, memory_size: u64) -> Result<Vec<u64>, String> {
        let dirty_log = self
            .vm_fd
            .get_dirty_log(slot, memory_size as usize)
            .map_err(|e| format!("Failed to get dirty log: {}", e))?;
        Ok(dirty_log)
    }
}
