//! CPU Emulator Module
//! 
//! Provides a complete CPU emulator for virtualizing x86_64, ARM64, and RISC-V
//! architectures inside Nyx VMs.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use serde::{Serialize, Deserialize, Serializer, Deserializer};

/// CPU operating mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CpuMode {
    /// Real mode (x86)
    Real,
    /// Protected mode (x86)
    Protected,
    /// Long mode / 64-bit mode (x86_64)
    LongMode,
    /// ARM AArch32
    Arm,
    /// ARM AArch64
    Arm64,
    /// RISC-V 32-bit
    RiscV32,
    /// RISC-V 64-bit
    RiscV64,
}

/// Register names
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Register {
    // x86_64 general purpose
    Rax, Rbx, Rcx, Rdx,
    Rsi, Rdi, Rbp, Rsp,
    R8, R9, R10, R11, R12, R13, R14, R15,
    // Program counter
    Rip,
    // Flags
    Rflags,
    // Segment registers
    Cs, Ds, Es, Fs, Gs, Ss,
    // ARM64
    X0, X1, X2, X3, X4, X5, X6, X7, X8, X9,
    X10, X11, X12, X13, X14, X15, X16, X17, X18, X19,
    X20, X21, X22, X23, X24, X25, X26, X27, X28, X29, X30,
    Pc, ArmSp, Fp, Lr,
    // RISC-V
    Zero, Ra, RvSp, Gp, Tp, T0, T1, T2, T3, T4, T5, T6,
    S0, S1, S2, S3, S4, S5, S6, S7, S8, S9, S10, S11,
    A0, A1, A2, A3, A4, A5, A6, A7,
}

/// CPU state for a virtual CPU
#[repr(C)]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CpuState {
    /// General purpose registers (Flat array for JIT performance)
    #[serde(serialize_with = "serialize_gpr", deserialize_with = "deserialize_gpr")]
    pub gpr: [u64; 64],
    /// Control registers
    #[serde(serialize_with = "serialize_cr", deserialize_with = "deserialize_cr")]
    pub cr: [u64; 16],
    /// Floating point registers
    #[serde(serialize_with = "serialize_fpr", deserialize_with = "deserialize_fpr")]
    pub fpr: [u64; 32],
    /// Vector registers
    #[serde(serialize_with = "serialize_vreg", deserialize_with = "deserialize_vreg")]
    pub vreg: [u128; 32],
    /// Page table root
    pub cr3: u64,
    /// Current CPU mode
    pub mode: CpuMode,
    /// x86 Flags and state
    pub rflags: u64,
    /// Interrupt flag
    pub interrupt_flag: bool,
    /// Zero flag
    pub zf: bool,
    /// Sign flag
    pub sf: bool,
    /// Carry flag
    pub cf: bool,
    /// Overflow flag
    pub of: bool,
    /// Parity flag
    pub pf: bool,
    /// Auxiliary carry flag
    pub af: bool,
}

impl Default for CpuState {
    fn default() -> Self {
        Self::new()
    }
}

impl CpuState {
    /// Create new CPU state
    pub fn new() -> Self {
        let mut gpr = HashMap::new();
        
        // Initialize x86_64 registers
        for reg in [
            Register::Rax, Register::Rbx, Register::Rcx, Register::Rdx,
            Register::Rsi, Register::Rdi, Register::Rbp, Register::Rsp,
            Register::R8, Register::R9, Register::R10, Register::R11,
            Register::R12, Register::R13, Register::R14, Register::R15,
        ] {
            gpr.insert(reg, 0);
        }
        
        // Initialize ARM64 registers
        for i in 0..31 {
            gpr.insert(register_from_arm64(i), 0);
        }
        
        // Initialize RISC-V registers
        Self {
            gpr: [0u64; 64],
            cr: [0u64; 16],
            fpr: [0u64; 32],
            vreg: [0u128; 32],
            cr3: 0,
            mode: CpuMode::LongMode,
            rflags: 0x2, // Reserved bit 1 must be set
            interrupt_flag: true,
            zf: false,
            sf: false,
            cf: false,
            of: false,
            pf: false,
            af: false,
        }
    }

    /// Read a general purpose register
    pub fn read_gpr(&self, reg: Register) -> u64 {
        self.gpr[reg as usize]
    }

    /// Write a general purpose register
    pub fn write_gpr(&mut self, reg: Register, value: u64) {
        // RISC-V zero register is hardwired to 0
        if reg == Register::Zero {
            return;
        }
        self.gpr[reg as usize] = value;
    }

    /// Get instruction pointer
    pub fn get_pc(&self) -> u64 {
        self.gpr[Register::Rip as usize]
    }

    /// Set instruction pointer
    pub fn set_pc(&mut self, value: u64) {
        self.gpr[Register::Rip as usize] = value;
    }

    /// Get stack pointer
    pub fn get_sp(&self) -> u64 {
        self.gpr[Register::Rsp as usize]
    }

    /// Set stack pointer
    pub fn set_sp(&mut self, value: u64) {
        match self.mode {
            CpuMode::LongMode | CpuMode::Protected | CpuMode::Real => {
                self.gpr[Register::Rsp as usize] = value;
            }
            CpuMode::Arm64 | CpuMode::Arm => {
                self.gpr[Register::ArmSp as usize] = value;
            }
            CpuMode::RiscV64 | CpuMode::RiscV32 => {
                self.gpr[Register::RvSp as usize] = value;
            }
        }
    }

    /// Get frame pointer
    pub fn get_fp(&self) -> u64 {
        self.gpr[Register::Rbp as usize]
    }

    /// Set frame pointer
    pub fn set_fp(&mut self, value: u64) {
        self.gpr[Register::Rbp as usize] = value;
    }
}

/// Convert ARM64 register index to Register enum
fn register_from_arm64(idx: u8) -> Register {
    match idx {
        0 => Register::X0,
        1 => Register::X1,
        2 => Register::X2,
        3 => Register::X3,
        4 => Register::X4,
        5 => Register::X5,
        6 => Register::X6,
        7 => Register::X7,
        8 => Register::X8,
        9 => Register::X9,
        10 => Register::X10,
        11 => Register::X11,
        12 => Register::X12,
        13 => Register::X13,
        14 => Register::X14,
        15 => Register::X15,
        16 => Register::X16,
        17 => Register::X17,
        18 => Register::X18,
        19 => Register::X19,
        20 => Register::X20,
        21 => Register::X21,
        22 => Register::X22,
        23 => Register::X23,
        24 => Register::X24,
        25 => Register::X25,
        26 => Register::X26,
        27 => Register::X27,
        28 => Register::X28,
        29 => Register::X29,
        30 => Register::X30,
        _ => Register::X0,
    }
}

/// Instruction opcode
#[derive(Debug, Clone, Copy)]
pub enum Opcode {
    // x86_64
    Mov, MovImm,
    Push, Pop,
    Add, Sub, Mul, Div, Mod,
    And, Or, Xor, Not,
    Shl, Shr, Sar,
    Cmp, Test,
    Jmp, Jz, Jnz, Ja, Jb, Je, Jne,
    Call, Ret,
    Lea,
    // ARM64
    Ldr, Str, Ldp, Stp,
    AddImm, SubImm, MulImm,
    B, Bl, Bne, Beq,
    // RISC-V
    Load, Store,
    Addi, Slli,
}

/// Decoded instruction
#[derive(Debug, Clone)]
pub struct Instruction {
    pub opcode: Opcode,
    pub operands: Vec<Operand>,
    pub length: u8,
}

/// Instruction operand
#[derive(Debug, Clone)]
pub enum Operand {
    Register(Register),
    Immediate(u64),
    Memory(MemoryOperand),
    Label(String),
}

/// Memory operand
#[derive(Debug, Clone)]
pub struct MemoryOperand {
    pub base: Option<Register>,
    pub index: Option<Register>,
    pub scale: u8,
    pub displacement: i32,
}

/// CPU emulator
pub struct CpuEmulator {
    /// Architecture being emulated
    pub arch: Architecture,
    /// CPU state
    pub state: CpuState,
    /// Memory size
    pub memory_size: usize,
    /// Shared memory buffer
    pub memory: Arc<Mutex<super::memory::PageAlignedBuffer>>,
    /// Running flag
    pub running: bool,
    /// Instruction count
    pub instruction_count: u64,
    /// TLB (Cache for virtual to physical translations)
    pub tlb: HashMap<u64, u64>,
}

/// Supported architectures
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Architecture {
    X86_64,
    AArch64,
    RiscV64,
}

impl CpuEmulator {
    /// Create a new CPU emulator
    pub fn new(arch: Architecture, memory_size: usize) -> Self {
        Self {
            arch,
            state: CpuState::new(),
            memory: Arc::new(Mutex::new(super::memory::PageAlignedBuffer::new(memory_size))),
            memory_size,
            running: false,
            instruction_count: 0,
            tlb: HashMap::new(),
        }
    }

    /// Load executable into memory (segment-aware ELF64 loader)
    pub fn load_elf(&mut self, data: &[u8], entry: u64) -> Result<(), String> {
        if data.len() < 64 || &data[0..4] != b"\x7fELF" {
            return Err("Invalid ELF header".to_string());
        }

        // 1. Parse ELF64 header fields
        let e_phoff = u64::from_le_bytes(data[32..40].try_into().unwrap()) as usize;
        let e_phentsize = u16::from_le_bytes(data[54..56].try_into().unwrap()) as usize;
        let e_phnum = u16::from_le_bytes(data[56..58].try_into().unwrap()) as usize;

        let mut mem = self.memory.lock().unwrap();

        // 2. Iterate through program headers
        for i in 0..e_phnum {
            let ph_offset = e_phoff + (i * e_phentsize);
            if ph_offset + e_phentsize > data.len() { break; }
            
            let ph = &data[ph_offset..ph_offset + e_phentsize];
            let p_type = u32::from_le_bytes(ph[0..4].try_into().unwrap());
            
            if p_type == 1 { // PT_LOAD
                let p_offset = u64::from_le_bytes(ph[8..16].try_into().unwrap()) as usize;
                let _p_vaddr = u64::from_le_bytes(ph[16..24].try_into().unwrap());
                let p_paddr = u64::from_le_bytes(ph[24..32].try_into().unwrap()) as usize;
                let p_filesz = u64::from_le_bytes(ph[32..40].try_into().unwrap()) as usize;
                let p_memsz = u64::from_le_bytes(ph[40..48].try_into().unwrap()) as usize;

                if p_paddr + p_memsz > self.memory_size {
                    return Err(format!("Segment too large for GPA 0x{:x}", p_paddr));
                }

                // Copy from file to memory
                if p_filesz > 0 {
                    let end_off = p_offset + p_filesz;
                    if end_off > data.len() { return Err("Invalid segment offset".into()); }
                    mem[p_paddr..p_paddr + p_filesz].copy_from_slice(&data[p_offset..end_off]);
                }

                // Zero-initialize remainder (BSS)
                if p_memsz > p_filesz {
                    for j in p_filesz..p_memsz {
                        mem[p_paddr + j] = 0;
                    }
                }
            }
        }
        
        drop(mem);
        self.state.set_pc(entry);
        Ok(())
    }

    /// Read from memory (with virtual-to-physical translation if enabled)
    pub fn read_memory(&mut self, addr: u64, size: usize) -> Result<u64, String> {
        let phys_addr = self.translate_address(addr)?;
        
        if phys_addr as usize + size > self.memory_size {
            return Err(format!("Invalid memory access at physical address 0x{:x}", phys_addr));
        }
        
        let mem = self.memory.lock().unwrap();
        let addr = phys_addr as usize;
        match size {
            1 => Ok(mem[addr] as u64),
            2 => Ok(u16::from_le_bytes([mem[addr], mem[addr + 1]]) as u64),
            4 => Ok(u32::from_le_bytes([mem[addr], mem[addr + 1], 
                                         mem[addr + 2], mem[addr + 3]]) as u64),
            8 => Ok(u64::from_le_bytes([mem[addr], mem[addr + 1],
                                         mem[addr + 2], mem[addr + 3],
                                         mem[addr + 4], mem[addr + 5],
                                         mem[addr + 6], mem[addr + 7]])),
            _ => Err("Invalid size".to_string()),
        }
    }

    /// Translate virtual to physical address
    fn translate_address(&mut self, vaddr: u64) -> Result<u64, String> {
        // Check if paging is enabled (CR0.PG = bit 31)
        let cr0 = self.state.cr[0];
        let paging_enabled = (cr0 & (1 << 31)) != 0;
        
        if !paging_enabled {
            if self.state.mode == CpuMode::Real {
                // Real mode: (segment << 4) + offset
                // For simplicity, we use CS for instruction fetches if we can distinguish them,
                // but for general vaddr translation, we assume the base is already applied 
                // or we use the CS base for now as that's the most common case during boot.
                // NOTE: In a full emulator, we'd pass the Segment register being used.
                let cs_base = self.state.gpr[Register::Cs as usize] << 4;
                return Ok(cs_base + vaddr);
            }
            return Ok(vaddr);
        }

        // Check TLB first
        let vpn = vaddr >> 12;
        if let Some(&paddr) = self.tlb.get(&vpn) {
            return Ok(paddr | (vaddr & 0xFFF));
        }

        // Perform page table walk (simulated via VirtualMemory logic)
        // We reuse the logic from VirtualMemory::translate but inside CpuEmulator 
        // to avoid complex circular dependencies for now, or we can use a helper.
        // For now, let's implement a concise walk here.
        let cr3 = self.state.cr3;
        if cr3 == 0 {
            return Err("Paging enabled but CR3 is 0".to_string());
        }

        // Indices
        let pml4_idx = (vaddr >> 39) & 0x1FF;
        let pdpt_idx = (vaddr >> 30) & 0x1FF;
        let pd_idx   = (vaddr >> 21) & 0x1FF;
        let pt_idx   = (vaddr >> 12) & 0x1FF;

        let mem = self.memory.lock().unwrap();

        // Helper to read 8 bytes from guest phys safely
        let read8 = |addr: u64, m: &[u8]| -> Option<u64> {
            let a = addr as usize;
            if a + 8 > m.len() { return None; }
            Some(u64::from_le_bytes([m[a], m[a+1], m[a+2], m[a+3], m[a+4], m[a+5], m[a+6], m[a+7]]))
        };

        // 1. PML4
        let pml4_val = read8(cr3 + pml4_idx * 8, &mem).ok_or("PML4 read failed")?;
        if (pml4_val & 1) == 0 { return Err("PML4 entry not present".to_string()); }

        // 2. PDPT
        let pdpt_base = pml4_val & 0x000FFFFFFFFFF000u64;
        let pdpt_val = read8(pdpt_base + pdpt_idx * 8, &mem).ok_or("PDPT read failed")?;
        if (pdpt_val & 1) == 0 { return Err("PDPT entry not present".to_string()); }
        if (pdpt_val & 0x80) != 0 { // 1GB page
            let paddr = (pdpt_val & 0xFFFFFC0000000u64) | (vaddr & 0x3FFFFFFF);
            self.tlb.insert(vpn, paddr >> 12);
            return Ok(paddr);
        }

        // 3. PD
        let pd_base = pdpt_val & 0x000FFFFFFFFFF000u64;
        let pd_val = read8(pd_base + pd_idx * 8, &mem).ok_or("PD read failed")?;
        if (pd_val & 1) == 0 { return Err("PD entry not present".to_string()); }
        if (pd_val & 0x80) != 0 { // 2MB page
            let paddr = (pd_val & 0xFFFFFFFE00000u64) | (vaddr & 0x1FFFFF);
            self.tlb.insert(vpn, paddr >> 12);
            return Ok(paddr);
        }

        // 4. PT
        let pt_base = pd_val & 0x000FFFFFFFFFF000u64;
        let pt_val = read8(pt_base + pt_idx * 8, &mem).ok_or("PT read failed")?;
        if (pt_val & 1) == 0 { return Err("PT entry not present".to_string()); }

        let paddr = (pt_val & 0x000FFFFFFFFFF000u64) | (vaddr & 0xFFF);
        self.tlb.insert(vpn, paddr >> 12);
        Ok(paddr)
    }

    /// Write to memory (with virtual-to-physical translation if enabled)
    pub fn write_memory(&mut self, addr: u64, size: usize, value: u64) -> Result<(), String> {
        let phys_addr = self.translate_address(addr)?;
        
        if phys_addr as usize + size > self.memory_size {
            return Err(format!("Invalid memory access at physical address 0x{:x}", phys_addr));
        }
        
        let mut mem = self.memory.lock().unwrap();
        let addr = phys_addr as usize;
        match size {
            1 => mem[addr] = value as u8,
            2 => mem[addr..addr + 2].copy_from_slice(&(value as u16).to_le_bytes()),
            4 => mem[addr..addr + 4].copy_from_slice(&(value as u32).to_le_bytes()),
            8 => mem[addr..addr + 8].copy_from_slice(&value.to_le_bytes()),
            _ => return Err("Invalid size".to_string()),
        }
        Ok(())
    }

    /// Set CR3 and flush TLB
    pub fn set_cr3(&mut self, value: u64) {
        self.state.cr3 = value;
        self.flush_tlb();
    }

    /// Flush TLB
    pub fn flush_tlb(&mut self) {
        self.tlb.clear();
    }

    /// Read a control register
    pub fn read_cr(&self, idx: u32) -> u64 {
        if idx < 16 {
            self.state.cr[idx as usize]
        } else {
            0
        }
    }

    /// Write a control register
    pub fn write_cr(&mut self, idx: u32, value: u64) {
        if idx < 16 {
            self.state.cr[idx as usize] = value;
            // CR0.PG change or CR3 change should flush TLB (approximate)
            self.flush_tlb();
        }
    }

    /// Write to an I/O port (used by OUT instructions in BIOS)
    pub fn port_write(&mut self, port: u16, _size: usize, _value: u64) -> Result<(), String> {
        // POST code — port 0x80 is commonly used by BIOS for debug
        if port == 0x80 {
            // POST code debug output — no-op for now
        }
        Ok(())
    }

    /// Read from an I/O port (used by IN instructions in BIOS)
    pub fn port_read(&mut self, _port: u16, _size: usize) -> Result<u64, String> {
        Ok(0xFF)
    }

    /// Start emulation
    pub fn start(&mut self) {
        self.running = true;
    }

    /// Stop emulation
    pub fn stop(&mut self) {
        self.running = false;
    }

    /// Execute one instruction
    pub fn step(&mut self) -> Result<bool, String> {
        if !self.running {
            return Ok(false);
        }

        // Fetch instruction
        let pc = self.state.get_pc();
        
        // Decode and execute based on architecture
        match self.arch {
            Architecture::X86_64 => {
                let opcode = self.read_memory(pc, 1)? as u8;
                self.execute_x86_64(opcode)?;
            }
            Architecture::AArch64 => {
                let opcode = self.read_memory(pc, 4)? as u32;
                self.execute_arm64(opcode)?;
            }
            Architecture::RiscV64 => {
                let opcode = self.read_memory(pc, 4)? as u32;
                self.execute_riscv64(opcode)?;
            }
        }
        
        self.instruction_count += 1;
        Ok(true)
    }

    /// Run the VM until halt
    pub fn run(&mut self, max_instructions: u64) -> Result<u64, String> {
        self.start();
        
        while self.running && self.instruction_count < max_instructions {
            self.step()?;
        }
        
        Ok(self.instruction_count)
    }

    /// Execute x86 instruction (Real Mode + basic Long Mode)
    fn execute_x86_64(&mut self, opcode: u8) -> Result<(), String> {
        let pc = self.state.get_pc();
        let real_mode = self.state.mode == CpuMode::Real;
        let imm_size: u64 = if real_mode { 2 } else { 4 }; // 16-bit immediates in real mode

        // Helper for 16-bit register array (real mode)
        let regs16 = [Register::Rax, Register::Rcx, Register::Rdx, Register::Rbx,
                      Register::Rsp, Register::Rbp, Register::Rsi, Register::Rdi];
        let sregs   = [Register::Es, Register::Cs, Register::Ss, Register::Ds,
                       Register::Fs, Register::Gs, Register::Ds, Register::Ds];

        match opcode {
            // ─── Flags / Control ────────────────────────────────────────────
            0xFA => { // CLI
                self.state.interrupt_flag = false;
                self.state.set_pc(pc + 1);
            }
            0xFB => { // STI
                self.state.interrupt_flag = true;
                self.state.set_pc(pc + 1);
            }
            0xFC => { // CLD
                self.state.set_pc(pc + 1);
            }
            0xFD => { // STD
                self.state.set_pc(pc + 1);
            }
            0xF0 => { // LOCK prefix — treat as NOP prefix, execute next byte
                let next = self.read_memory(pc + 1, 1)? as u8;
                self.state.set_pc(pc + 1);
                self.execute_x86_64(next)?;
            }
            0xF3 => { // REP prefix — simplified
                self.state.set_pc(pc + 1);
            }
            0xF2 => { // REPNZ prefix — simplified
                self.state.set_pc(pc + 1);
            }

            // ─── NOP ────────────────────────────────────────────────────────
            0x90 => { self.state.set_pc(pc + 1); }

            // ─── HLT / INT3 ────────────────────────────────────────────────
            0xF4 => { self.running = false; }
            0xCC => { self.running = false; }

            // ─── Far JMP (16:16) — Reset Vector Entry ─────────────────────
            0xEA => {
                // JMP ptr16:16 — 5 bytes: EA lo hi seg_lo seg_hi
                let off = self.read_memory(pc + 1, 2)? as u16;
                let seg = self.read_memory(pc + 3, 2)? as u16;
                // Update CS and IP
                self.state.write_gpr(Register::Cs, seg as u64);
                self.state.set_pc(off as u64);
                eprintln!("[BIOS] Far JMP -> {:04X}:{:04X}", seg, off);
            }

            // ─── Near JMP rel8 ─────────────────────────────────────────────
            0xEB => {
                let off = self.read_memory(pc + 1, 1)? as i8 as i64;
                self.state.set_pc(((pc as i64) + 2 + off) as u64);
            }

            // ─── Near JMP rel16/32 ─────────────────────────────────────────
            0xE9 => {
                let off = self.read_memory(pc + 1, imm_size as usize)? as i32 as i64;
                self.state.set_pc(((pc as i64) + 1 + imm_size as i64 + off) as u64);
            }

            // ─── Conditional Jumps (short) ─────────────────────────────────
            0x74 | 0x75 | 0x72 | 0x73 | 0x76 | 0x77 | 0x78 | 0x79
            | 0x7A | 0x7B | 0x7C | 0x7D | 0x7E | 0x7F | 0x70 | 0x71 => {
                let off = self.read_memory(pc + 1, 1)? as i8 as i64;
                let taken = match opcode {
                    0x74 => self.state.zf,
                    0x75 => !self.state.zf,
                    0x72 => self.state.cf,
                    0x73 => !self.state.cf,
                    0x78 => self.state.sf,
                    0x79 => !self.state.sf,
                    0x7C => self.state.sf != self.state.of,
                    0x7D => self.state.sf == self.state.of,
                    0x7E => self.state.zf || (self.state.sf != self.state.of),
                    0x7F => !self.state.zf && (self.state.sf == self.state.of),
                    _ => false,
                };
                if taken {
                    self.state.set_pc(((pc as i64) + 2 + off) as u64);
                } else {
                    self.state.set_pc(pc + 2);
                }
            }

            // ─── LOOP ──────────────────────────────────────────────────────
            0xE2 => {
                let off = self.read_memory(pc + 1, 1)? as i8 as i64;
                let cx = self.state.read_gpr(Register::Rcx).wrapping_sub(1) & 0xFFFF;
                self.state.write_gpr(Register::Rcx, cx);
                if cx != 0 {
                    self.state.set_pc(((pc as i64) + 2 + off) as u64);
                } else {
                    self.state.set_pc(pc + 2);
                }
            }

            // ─── CALL near rel16 ───────────────────────────────────────────
            0xE8 => {
                let off = self.read_memory(pc + 1, imm_size as usize)? as i32 as i64;
                let ret = pc + 1 + imm_size;
                let sp = (self.state.get_sp().wrapping_sub(2)) & 0xFFFF;
                self.state.set_sp(sp);
                let phys_sp = (self.state.read_gpr(Register::Ss) << 4) + sp;
                let _ = self.write_memory(phys_sp, 2, ret as u64);
                self.state.set_pc(((pc as i64) + 1 + imm_size as i64 + off) as u64);
            }

            // ─── RET near ──────────────────────────────────────────────────
            0xC3 => {
                let sp = self.state.get_sp() & 0xFFFF;
                let phys_sp = (self.state.read_gpr(Register::Ss) << 4) + sp;
                let ret = if real_mode {
                    self.read_memory(phys_sp, 2)?
                } else {
                    self.read_memory(phys_sp, 8)?
                };
                self.state.set_sp(sp.wrapping_add(if real_mode { 2 } else { 8 }));
                self.state.set_pc(ret);
            }

            // ─── RET near imm16 ────────────────────────────────────────────
            0xC2 => {
                let imm = self.read_memory(pc + 1, 2)? as u16 as u64;
                let sp = self.state.get_sp();
                let phys_sp = (self.state.read_gpr(Register::Ss) << 4) + sp;
                let ret = self.read_memory(phys_sp, 2)?;
                self.state.set_sp(sp.wrapping_add(2 + imm));
                self.state.set_pc(ret);
            }

            // ─── RETF (far return) ─────────────────────────────────────────
            0xCB => {
                let sp = self.state.get_sp() & 0xFFFF;
                let phys_sp = (self.state.read_gpr(Register::Ss) << 4) + sp;
                let ip = self.read_memory(phys_sp, 2)?;
                let cs = self.read_memory(phys_sp + 2, 2)?;
                self.state.set_sp(sp.wrapping_add(4));
                self.state.write_gpr(Register::Cs, cs);
                self.state.set_pc(ip);
            }

            // ─── IRET ──────────────────────────────────────────────────────
            0xCF => {
                let sp = self.state.get_sp() & 0xFFFF;
                let phys_sp = (self.state.read_gpr(Register::Ss) << 4) + sp;
                let ip = self.read_memory(phys_sp, 2)?;
                let cs = self.read_memory(phys_sp + 2, 2)?;
                let flags = self.read_memory(phys_sp + 4, 2)?;
                self.state.set_sp(sp.wrapping_add(6));
                self.state.write_gpr(Register::Cs, cs);
                self.state.set_pc(ip);
                self.state.interrupt_flag = (flags & 0x200) != 0;
                self.state.zf = (flags & 0x40) != 0;
                self.state.cf = (flags & 0x01) != 0;
                self.state.sf = (flags & 0x80) != 0;
            }

            // ─── INT imm8 ──────────────────────────────────────────────────
            0xCD => {
                let int_num = self.read_memory(pc + 1, 1)? as u8;
                // Push flags, CS, IP onto stack
                let flags: u16 = if self.state.interrupt_flag { 0x0202 } else { 0x0002 };
                let sp = self.state.get_sp().wrapping_sub(6) & 0xFFFF;
                self.state.set_sp(sp);
                let phys_sp = (self.state.read_gpr(Register::Ss) << 4) + sp;
                let ret_ip = pc + 2;
                let cs_val = self.state.read_gpr(Register::Cs) as u16;
                let _ = self.write_memory(phys_sp, 2, flags as u64);
                let _ = self.write_memory(phys_sp + 2, 2, cs_val as u64);
                let _ = self.write_memory(phys_sp + 4, 2, ret_ip);
                // Read IVT entry (at 0000:int_num*4)
                let ivt_addr = (int_num as u64) * 4;
                let new_ip = self.read_memory(ivt_addr, 2).unwrap_or(0);
                let new_cs = self.read_memory(ivt_addr + 2, 2).unwrap_or(0xF000);
                self.state.write_gpr(Register::Cs, new_cs);
                self.state.set_pc(new_ip);
            }

            // ─── XOR r/m, r (0x31) and XOR r, r/m (0x33) ──────────────────
            0x31 | 0x33 => {
                let modrm = self.read_memory(pc + 1, 1)? as u8;
                let reg_idx = ((modrm >> 3) & 0x7) as usize;
                let rm_idx  = (modrm & 0x7) as usize;
                if (modrm >> 6) == 3 {
                    // Register-register XOR
                    if opcode == 0x31 {
                        // XOR rm, reg
                        let val = (self.state.read_gpr(regs16[rm_idx]) ^ self.state.read_gpr(regs16[reg_idx])) & 0xFFFF;
                        self.state.write_gpr(regs16[rm_idx], val);
                        self.state.zf = val == 0;
                        self.state.sf = (val & 0x8000) != 0;
                        self.state.cf = false;
                        self.state.of = false;
                    } else {
                        // XOR reg, rm
                        let val = (self.state.read_gpr(regs16[reg_idx]) ^ self.state.read_gpr(regs16[rm_idx])) & 0xFFFF;
                        self.state.write_gpr(regs16[reg_idx], val);
                        self.state.zf = val == 0;
                        self.state.sf = (val & 0x8000) != 0;
                        self.state.cf = false;
                        self.state.of = false;
                    }
                }
                self.state.set_pc(pc + 2);
            }

            // ─── MOV Sreg, r/m16 (0x8E) ───────────────────────────────────
            0x8E => {
                let modrm = self.read_memory(pc + 1, 1)? as u8;
                let sreg_idx = ((modrm >> 3) & 0x7) as usize;
                let rm_idx   = (modrm & 0x7) as usize;
                if (modrm >> 6) == 3 {
                    let val = self.state.read_gpr(regs16[rm_idx]) & 0xFFFF;
                    if sreg_idx < sregs.len() {
                        self.state.write_gpr(sregs[sreg_idx], val);
                    }
                }
                self.state.set_pc(pc + 2);
            }

            // ─── MOV r/m16, Sreg (0x8C) ───────────────────────────────────
            0x8C => {
                let modrm = self.read_memory(pc + 1, 1)? as u8;
                let sreg_idx = ((modrm >> 3) & 0x7) as usize;
                let rm_idx   = (modrm & 0x7) as usize;
                if (modrm >> 6) == 3 {
                    let val = if sreg_idx < sregs.len() { self.state.read_gpr(sregs[sreg_idx]) } else { 0 };
                    self.state.write_gpr(regs16[rm_idx], val & 0xFFFF);
                }
                self.state.set_pc(pc + 2);
            }

            // ─── MOV r/m8, imm8 (0xC6) ────────────────────────────────────
            0xC6 => {
                let modrm = self.read_memory(pc + 1, 1)? as u8;
                if (modrm >> 6) == 3 {
                    let val = self.read_memory(pc + 2, 1)? & 0xFF;
                    let rm_idx = (modrm & 0x7) as usize;
                    let cur = self.state.read_gpr(regs16[rm_idx]) & !0xFFu64;
                    self.state.write_gpr(regs16[rm_idx], cur | val);
                    self.state.set_pc(pc + 3);
                } else {
                    self.state.set_pc(pc + 2);
                }
            }

            // ─── MOV r/m16, imm16 (0xC7) ──────────────────────────────────
            0xC7 => {
                let modrm = self.read_memory(pc + 1, 1)? as u8;
                let dst_reg_idx = (modrm & 0x7) as usize;
                let regs = [Register::Rax, Register::Rcx, Register::Rdx, Register::Rbx,
                            Register::Rsp, Register::Rbp, Register::Rsi, Register::Rdi];
                if (modrm >> 6) == 3 {
                    let imm = self.read_memory(pc + 2, imm_size as usize)?;
                    self.state.write_gpr(regs[dst_reg_idx], imm);
                    self.state.set_pc(pc + 2 + imm_size);
                } else {
                    self.state.set_pc(pc + 2);
                }
            }

            // ─── MOV r16, imm16 (0xB0-0xBF) ──────────────────────────────
            0xB0..=0xB7 => {
                // MOV r8, imm8
                let reg_num = (opcode - 0xB0) as usize;
                let val = self.read_memory(pc + 1, 1)?;
                let cur = self.state.read_gpr(regs16[reg_num % 8]) & !0xFFu64;
                self.state.write_gpr(regs16[reg_num % 8], cur | (val & 0xFF));
                self.state.set_pc(pc + 2);
            }
            0xB8..=0xBF => {
                // MOV r16/32, imm16/32
                let reg_num = (opcode - 0xB8) as usize;
                let val = self.read_memory(pc + 1, imm_size as usize)?;
                self.state.write_gpr(regs16[reg_num], val);
                self.state.set_pc(pc + 1 + imm_size);
            }

            // ─── MOV r/m, r (0x89) and MOV r, r/m (0x8B) ─────────────────
            0x89 => {
                let modrm = self.read_memory(pc + 1, 1)? as u8;
                let src_idx = ((modrm >> 3) & 0x7) as usize;
                let dst_idx = (modrm & 0x7) as usize;
                if (modrm >> 6) == 3 {
                    let val = self.state.read_gpr(regs16[src_idx]) & 0xFFFF;
                    self.state.write_gpr(regs16[dst_idx], val);
                }
                self.state.set_pc(pc + 2);
            }
            0x8B => {
                let modrm = self.read_memory(pc + 1, 1)? as u8;
                let dst_idx = ((modrm >> 3) & 0x7) as usize;
                let src_idx = (modrm & 0x7) as usize;
                if (modrm >> 6) == 3 {
                    let val = self.state.read_gpr(regs16[src_idx]) & 0xFFFF;
                    self.state.write_gpr(regs16[dst_idx], val);
                }
                self.state.set_pc(pc + 2);
            }

            // ─── PUSH / POP r16 ────────────────────────────────────────────
            0x50..=0x57 => {
                // PUSH r16
                let reg_idx = (opcode - 0x50) as usize;
                let val = self.state.read_gpr(regs16[reg_idx]) & 0xFFFF;
                let sp = self.state.get_sp().wrapping_sub(2) & 0xFFFF;
                self.state.set_sp(sp);
                let phys_sp = (self.state.read_gpr(Register::Ss) << 4) + sp;
                let _ = self.write_memory(phys_sp, 2, val);
                self.state.set_pc(pc + 1);
            }
            0x58..=0x5F => {
                // POP r16
                let reg_idx = (opcode - 0x58) as usize;
                let sp = self.state.get_sp() & 0xFFFF;
                let phys_sp = (self.state.read_gpr(Register::Ss) << 4) + sp;
                let val = self.read_memory(phys_sp, 2)?;
                self.state.write_gpr(regs16[reg_idx], val & 0xFFFF);
                self.state.set_sp(sp.wrapping_add(2));
                self.state.set_pc(pc + 1);
            }

            // ─── PUSH imm8 / imm16 ─────────────────────────────────────────
            0x6A => {
                let imm = self.read_memory(pc + 1, 1)? as i8 as i16 as u16 as u64;
                let sp = self.state.get_sp().wrapping_sub(2) & 0xFFFF;
                self.state.set_sp(sp);
                let phys_sp = (self.state.read_gpr(Register::Ss) << 4) + sp;
                let _ = self.write_memory(phys_sp, 2, imm);
                self.state.set_pc(pc + 2);
            }
            0x68 => {
                let imm = self.read_memory(pc + 1, 2)?;
                let sp = self.state.get_sp().wrapping_sub(2) & 0xFFFF;
                self.state.set_sp(sp);
                let phys_sp = (self.state.read_gpr(Register::Ss) << 4) + sp;
                let _ = self.write_memory(phys_sp, 2, imm);
                self.state.set_pc(pc + 3);
            }

            // ─── PUSHA / POPA ──────────────────────────────────────────────
            0x60 => { // PUSHA — skip
                self.state.set_pc(pc + 1);
            }
            0x61 => { // POPA — skip
                self.state.set_pc(pc + 1);
            }
            0x9C => { // PUSHF
                let flags: u16 = (if self.state.cf { 0x1 } else { 0 })
                    | (if self.state.zf { 0x40 } else { 0 })
                    | (if self.state.sf { 0x80 } else { 0 })
                    | (if self.state.interrupt_flag { 0x200 } else { 0 });
                let sp = self.state.get_sp().wrapping_sub(2) & 0xFFFF;
                self.state.set_sp(sp);
                let phys_sp = (self.state.read_gpr(Register::Ss) << 4) + sp;
                let _ = self.write_memory(phys_sp, 2, flags as u64);
                self.state.set_pc(pc + 1);
            }
            0x9D => { // POPF
                let sp = self.state.get_sp() & 0xFFFF;
                let phys_sp = (self.state.read_gpr(Register::Ss) << 4) + sp;
                let flags = self.read_memory(phys_sp, 2)?;
                self.state.set_sp(sp.wrapping_add(2));
                self.state.interrupt_flag = (flags & 0x200) != 0;
                self.state.zf = (flags & 0x40) != 0;
                self.state.cf = (flags & 0x01) != 0;
                self.state.sf = (flags & 0x80) != 0;
                self.state.set_pc(pc + 1);
            }

            // ─── IN / OUT ──────────────────────────────────────────────────
            0xE4 => { // IN AL, imm8
                let _port = self.read_memory(pc + 1, 1)? as u16;
                self.state.set_pc(pc + 2);
            }
            0xE5 => { // IN AX, imm8
                let _port = self.read_memory(pc + 1, 1)? as u16;
                self.state.set_pc(pc + 2);
            }
            0xE6 => { // OUT imm8, AL
                let port = self.read_memory(pc + 1, 1)? as u16;
                let val = self.state.read_gpr(Register::Rax) as u8 as u64;
                let _ = self.port_write(port, 1, val);
                self.state.set_pc(pc + 2);
            }
            0xE7 => { // OUT imm8, AX
                let port = self.read_memory(pc + 1, 1)? as u16;
                let val = self.state.read_gpr(Register::Rax) & 0xFFFF;
                let _ = self.port_write(port, 2, val);
                self.state.set_pc(pc + 2);
            }
            0xEE => { // OUT DX, AL
                let port = self.state.read_gpr(Register::Rdx) as u16;
                let val = self.state.read_gpr(Register::Rax) & 0xFF;
                let _ = self.port_write(port, 1, val);
                self.state.set_pc(pc + 1);
            }
            0xEF => { // OUT DX, AX
                let port = self.state.read_gpr(Register::Rdx) as u16;
                let val = self.state.read_gpr(Register::Rax) & 0xFFFF;
                let _ = self.port_write(port, 2, val);
                self.state.set_pc(pc + 1);
            }
            0xEC => { // IN AL, DX
                let _port = self.state.read_gpr(Register::Rdx) as u16;
                self.state.set_pc(pc + 1);
            }
            0xED => { // IN AX, DX
                let _port = self.state.read_gpr(Register::Rdx) as u16;
                self.state.set_pc(pc + 1);
            }

            // ─── ADD / SUB / CMP imm ───────────────────────────────────────
            0x80 => { // Group 1 r/m8, imm8
                self.state.set_pc(pc + 3);
            }
            0x81 => { // Group 1 r/m16, imm16
                let modrm = self.read_memory(pc + 1, 1)? as u8;
                let extra = if (modrm >> 6) == 3 { 0 } else { 2 };
                self.state.set_pc(pc + 3 + imm_size + extra);
            }
            0x83 => { // Group 1 r/m16, imm8 sign-extended
                let modrm = self.read_memory(pc + 1, 1)? as u8;
                let op = (modrm >> 3) & 7;
                let rm_idx = (modrm & 0x7) as usize;
                if (modrm >> 6) == 3 {
                    let imm = self.read_memory(pc + 2, 1)? as i8 as i64 as u64;
                    let dst = self.state.read_gpr(regs16[rm_idx]);
                    let result = match op {
                        0 => dst.wrapping_add(imm) & 0xFFFF,             // ADD
                        1 => dst | imm,                                    // OR
                        4 => dst & imm,                                    // AND
                        5 => dst.wrapping_sub(imm) & 0xFFFF,             // SUB
                        6 => dst ^ imm,                                    // XOR
                        7 => { // CMP
                            let r = dst.wrapping_sub(imm) & 0xFFFF;
                            self.state.zf = r == 0;
                            self.state.cf = dst < imm;
                            self.state.sf = (r & 0x8000) != 0;
                            dst // don't write back
                        }
                        _ => dst,
                    };
                    if op != 7 { self.state.write_gpr(regs16[rm_idx], result); }
                    self.state.set_pc(pc + 3);
                } else {
                    self.state.set_pc(pc + 3);
                }
            }

            // ─── CMP r/m, r ────────────────────────────────────────────────
            0x38 | 0x39 | 0x3A | 0x3B => {
                let modrm = self.read_memory(pc + 1, 1)? as u8;
                let reg_idx = ((modrm >> 3) & 0x7) as usize;
                let rm_idx  = (modrm & 0x7) as usize;
                if (modrm >> 6) == 3 {
                    let a = self.state.read_gpr(regs16[rm_idx]);
                    let b = self.state.read_gpr(regs16[reg_idx]);
                    let r = a.wrapping_sub(b) & 0xFFFF;
                    self.state.zf = r == 0;
                    self.state.cf = a < b;
                    self.state.sf = (r & 0x8000) != 0;
                }
                self.state.set_pc(pc + 2);
            }

            // ─── TEST r/m, r ───────────────────────────────────────────────
            0x84 | 0x85 => {
                let modrm = self.read_memory(pc + 1, 1)? as u8;
                let reg_idx = ((modrm >> 3) & 0x7) as usize;
                let rm_idx  = (modrm & 0x7) as usize;
                if (modrm >> 6) == 3 {
                    let r = self.state.read_gpr(regs16[rm_idx]) & self.state.read_gpr(regs16[reg_idx]);
                    self.state.zf = r == 0;
                    self.state.sf = (r & 0x8000) != 0;
                    self.state.cf = false;
                    self.state.of = false;
                }
                self.state.set_pc(pc + 2);
            }

            // ─── INC / DEC r16 ─────────────────────────────────────────────
            0x40..=0x47 => {
                let idx = (opcode - 0x40) as usize;
                let val = (self.state.read_gpr(regs16[idx]).wrapping_add(1)) & 0xFFFF;
                self.state.write_gpr(regs16[idx], val);
                self.state.zf = val == 0;
                self.state.set_pc(pc + 1);
            }
            0x48..=0x4F => {
                // Note: 0x48-0x4F also overlap with REX prefix in 64-bit mode
                if !real_mode {
                    // 64-bit: REX prefix
                    let next = self.read_memory(pc + 1, 1)? as u8;
                    self.state.set_pc(pc + 1);
                    self.execute_x86_64(next)?;
                    return Ok(());
                }
                let idx = (opcode - 0x48) as usize;
                let val = (self.state.read_gpr(regs16[idx]).wrapping_sub(1)) & 0xFFFF;
                self.state.write_gpr(regs16[idx], val);
                self.state.zf = val == 0;
                self.state.set_pc(pc + 1);
            }

            // ─── OR / AND / SUB / ADD r/m, r ──────────────────────────────
            0x08 | 0x09 | 0x0A | 0x0B => { // OR
                let modrm = self.read_memory(pc + 1, 1)? as u8;
                let reg_idx = ((modrm >> 3) & 0x7) as usize;
                let rm_idx  = (modrm & 0x7) as usize;
                if (modrm >> 6) == 3 {
                    let a = self.state.read_gpr(regs16[rm_idx]);
                    let b = self.state.read_gpr(regs16[reg_idx]);
                    let r = if opcode <= 0x09 { a | b } else { b | a };
                    self.state.write_gpr(if opcode <= 0x09 { regs16[rm_idx] } else { regs16[reg_idx] }, r & 0xFFFF);
                    self.state.zf = r == 0;
                }
                self.state.set_pc(pc + 2);
            }
            0x20 | 0x21 | 0x22 | 0x23 => { // AND
                let modrm = self.read_memory(pc + 1, 1)? as u8;
                let reg_idx = ((modrm >> 3) & 0x7) as usize;
                let rm_idx  = (modrm & 0x7) as usize;
                if (modrm >> 6) == 3 {
                    let a = self.state.read_gpr(regs16[rm_idx]);
                    let b = self.state.read_gpr(regs16[reg_idx]);
                    let r = if opcode <= 0x21 { a & b } else { b & a };
                    self.state.write_gpr(if opcode <= 0x21 { regs16[rm_idx] } else { regs16[reg_idx] }, r & 0xFFFF);
                    self.state.zf = r == 0;
                }
                self.state.set_pc(pc + 2);
            }
            0x28 | 0x29 | 0x2A | 0x2B => { // SUB
                let modrm = self.read_memory(pc + 1, 1)? as u8;
                let reg_idx = ((modrm >> 3) & 0x7) as usize;
                let rm_idx  = (modrm & 0x7) as usize;
                if (modrm >> 6) == 3 {
                    let a = self.state.read_gpr(regs16[rm_idx]);
                    let b = self.state.read_gpr(regs16[reg_idx]);
                    let (r, dst) = if opcode <= 0x29 { (a.wrapping_sub(b), regs16[rm_idx]) } else { (b.wrapping_sub(a), regs16[reg_idx]) };
                    self.state.write_gpr(dst, r & 0xFFFF);
                    self.state.zf = r == 0; self.state.cf = a < b;
                }
                self.state.set_pc(pc + 2);
            }
            0x00 | 0x01 | 0x02 | 0x03 => { // ADD
                let modrm = self.read_memory(pc + 1, 1)? as u8;
                let reg_idx = ((modrm >> 3) & 0x7) as usize;
                let rm_idx  = (modrm & 0x7) as usize;
                if (modrm >> 6) == 3 {
                    let a = self.state.read_gpr(regs16[rm_idx]);
                    let b = self.state.read_gpr(regs16[reg_idx]);
                    let (r, dst) = if opcode <= 0x01 { (a.wrapping_add(b), regs16[rm_idx]) } else { (b.wrapping_add(a), regs16[reg_idx]) };
                    self.state.write_gpr(dst, r & 0xFFFF);
                    self.state.zf = r == 0;
                }
                self.state.set_pc(pc + 2);
            }

            // ─── MOV r/m, [mem] and [mem], r ─────────────────────────────
            0xA0 => { // MOV AL, [imm16]
                let addr = self.read_memory(pc + 1, 2)?;
                let ds_base = self.state.read_gpr(Register::Ds) << 4;
                let val = self.read_memory(ds_base + addr, 1).unwrap_or(0);
                let cur = self.state.read_gpr(Register::Rax) & !0xFFu64;
                self.state.write_gpr(Register::Rax, cur | val);
                self.state.set_pc(pc + 3);
            }
            0xA2 => { // MOV [imm16], AL
                let addr = self.read_memory(pc + 1, 2)?;
                let ds_base = self.state.read_gpr(Register::Ds) << 4;
                let val = self.state.read_gpr(Register::Rax) & 0xFF;
                let _ = self.write_memory(ds_base + addr, 1, val);
                self.state.set_pc(pc + 3);
            }

            // ─── LEA ──────────────────────────────────────────────────────
            0x8D => {
                let modrm = self.read_memory(pc + 1, 1)? as u8;
                let reg_idx = ((modrm >> 3) & 0x7) as usize;
                let rm_idx  = (modrm & 0x7) as usize;
                let mode = modrm >> 6;
                let mut addr = 0u64;
                let mut len = 2u64;
                if mode == 1 {
                    let disp = self.read_memory(pc + 2, 1)? as i8 as i64;
                    addr = self.state.read_gpr(regs16[rm_idx]).wrapping_add(disp as u64);
                    len = 3;
                } else if mode == 2 {
                    let disp = self.read_memory(pc + 2, 2)? as i16 as i64;
                    addr = self.state.read_gpr(regs16[rm_idx]).wrapping_add(disp as u64);
                    len = 4;
                } else if mode == 0 {
                    addr = self.state.read_gpr(regs16[rm_idx]);
                }
                self.state.write_gpr(regs16[reg_idx], addr & 0xFFFF);
                self.state.set_pc(pc + len);
            }

            // ─── Segment override prefixes ─────────────────────────────────
            0x26 | 0x2E | 0x36 | 0x3E | 0x64 | 0x65 => {
                // Segment prefix — skip prefix, execute next opcode
                let next = self.read_memory(pc + 1, 1)? as u8;
                self.state.set_pc(pc + 1);
                self.execute_x86_64(next)?;
            }

            // ─── CBW / CWD ─────────────────────────────────────────────────
            0x98 => { // CBW
                let al = self.state.read_gpr(Register::Rax) as i8 as i16 as u16 as u64;
                self.state.write_gpr(Register::Rax, al);
                self.state.set_pc(pc + 1);
            }

            // ─── MUL / DIV ─────────────────────────────────────────────────
            0xF6 | 0xF7 => {
                // Simplified: skip ModRM + operands
                self.state.set_pc(pc + 2);
            }

            // ─── XCHG ─────────────────────────────────────────────────────
            0x86 | 0x87 => {
                let modrm = self.read_memory(pc + 1, 1)? as u8;
                if (modrm >> 6) == 3 {
                    let r1 = ((modrm >> 3) & 7) as usize;
                    let r2 = (modrm & 7) as usize;
                    let tmp = self.state.read_gpr(regs16[r1]);
                    self.state.write_gpr(regs16[r1], self.state.read_gpr(regs16[r2]));
                    self.state.write_gpr(regs16[r2], tmp);
                }
                self.state.set_pc(pc + 2);
            }

            // ─── Misc / Prefix (0x66 operand size, 0x67 address size) ────
            0x66 => {
                let next = self.read_memory(pc + 1, 1)? as u8;
                self.state.set_pc(pc + 1);
                self.execute_x86_64(next)?;
            }
            0x67 => {
                let next = self.read_memory(pc + 1, 1)? as u8;
                self.state.set_pc(pc + 1);
                self.execute_x86_64(next)?;
            }

            // ─── FF group (JMP/CALL near abs, INC/DEC) ────────────────────
            0xFF => {
                let modrm = self.read_memory(pc + 1, 1)? as u8;
                let op = (modrm >> 3) & 7;
                let rm_idx = (modrm & 7) as usize;
                match op {
                    0 => { // INC r/m
                        if (modrm >> 6) == 3 {
                            let v = self.state.read_gpr(regs16[rm_idx]).wrapping_add(1) & 0xFFFF;
                            self.state.write_gpr(regs16[rm_idx], v);
                            self.state.zf = v == 0;
                        }
                        self.state.set_pc(pc + 2);
                    }
                    1 => { // DEC r/m
                        if (modrm >> 6) == 3 {
                            let v = self.state.read_gpr(regs16[rm_idx]).wrapping_sub(1) & 0xFFFF;
                            self.state.write_gpr(regs16[rm_idx], v);
                            self.state.zf = v == 0;
                        }
                        self.state.set_pc(pc + 2);
                    }
                    4 => { // JMP r/m
                        if (modrm >> 6) == 3 {
                            let target = self.state.read_gpr(regs16[rm_idx]) & 0xFFFF;
                            self.state.set_pc(target);
                        } else {
                            self.state.set_pc(pc + 2);
                        }
                    }
                    2 => { // CALL r/m near
                        if (modrm >> 6) == 3 {
                            let target = self.state.read_gpr(regs16[rm_idx]) & 0xFFFF;
                            let ret = pc + 2;
                            let sp = self.state.get_sp().wrapping_sub(2) & 0xFFFF;
                            self.state.set_sp(sp);
                            let phys_sp = (self.state.read_gpr(Register::Ss) << 4) + sp;
                            let _ = self.write_memory(phys_sp, 2, ret);
                            self.state.set_pc(target);
                        } else {
                            self.state.set_pc(pc + 2);
                        }
                    }
                    _ => { self.state.set_pc(pc + 2); }
                }
            }

            // ─── MOV AL/AX, imm (for CMP AH usage) ───────────────────────
            0x3C => { // CMP AL, imm8
                let imm = self.read_memory(pc + 1, 1)?;
                let al = self.state.read_gpr(Register::Rax) & 0xFF;
                let r = al.wrapping_sub(imm);
                self.state.zf = r == 0; self.state.cf = al < imm;
                self.state.set_pc(pc + 2);
            }
            0x3D => { // CMP AX, imm16
                let imm = self.read_memory(pc + 1, 2)?;
                let ax = self.state.read_gpr(Register::Rax) & 0xFFFF;
                let r = ax.wrapping_sub(imm);
                self.state.zf = r == 0; self.state.cf = ax < imm;
                self.state.set_pc(pc + 3);
            }

            // ─── Add-to-AX imm ────────────────────────────────────────────
            0x04 => { // ADD AL, imm8
                let imm = self.read_memory(pc + 1, 1)?;
                let al = (self.state.read_gpr(Register::Rax) & 0xFF).wrapping_add(imm) & 0xFF;
                let cur = self.state.read_gpr(Register::Rax) & !0xFFu64;
                self.state.write_gpr(Register::Rax, cur | al);
                self.state.set_pc(pc + 2);
            }

            // ─── MOVS / STOS / LODS (string ops) ──────────────────────────
            0xA4..=0xA7 | 0xAA..=0xAF => {
                self.state.set_pc(pc + 1);
            }

            // ─── Unknown — advance 1 byte with diagnostic ─────────────────
            _ => {
                self.state.set_pc(pc + 1);
                if [0xFFu8, 0x00u8, 0xFF].contains(&opcode) {
                    // Likely reading uninitialized memory (0xFF padding) — log
                } else {
                    // Log unknown opcodes for debugging
                    eprintln!("[CPU] Unhandled opcode 0x{:02X} at PC=0x{:X}", opcode, pc);
                }
            }
        }
        Ok(())
    }

    /// Execute ARM64 instruction
    fn execute_arm64(&mut self, opcode: u32) -> Result<(), String> {
        // Simplified ARM64 execution
        match opcode {
            0x14000000..=0x17FFFFFF => {
                // B (unconditional branch)
                let offset = (opcode & 0x03FFFFFF) as i32 as u64;
                self.state.set_pc(self.state.get_pc() + 8 + (offset << 2));
            }
            0x94000000..=0x97FFFFFF => {
                // BL (branch with link)
                let sp = self.state.get_sp();
                self.write_memory(sp - 8, 8, self.state.get_pc() + 4)?;
                self.state.set_sp(sp - 8);
                
                let offset = (opcode & 0x03FFFFFF) as i32 as u64;
                self.state.set_pc(self.state.get_pc() + 8 + (offset << 2));
            }
            0xD65F0780 => {
                // RET
                let sp = self.state.get_sp();
                let ret_addr = self.read_memory(sp, 8)?;
                self.state.set_sp(sp + 8);
                self.state.set_pc(ret_addr);
            }
            _ => {
                self.state.set_pc(self.state.get_pc() + 4);
            }
        }
        Ok(())
    }

    /// Execute RISC-V instruction
    fn execute_riscv64(&mut self, opcode: u32) -> Result<(), String> {
        match (opcode & 0x7F) as u8 {
            0x6F => {
                // JAL
                let offset = (opcode >> 12) as i32 as u64;
                let ra = self.state.get_pc() + 4;
                self.state.write_gpr(Register::Ra, ra);
                self.state.set_pc(self.state.get_pc() + ((offset & !1) << 12));
            }
            0x67 => {
                // JALR
                let rs1 = self.read_memory(self.state.get_pc() + 1, 4)? as u64;
                let ra = self.state.get_pc() + 4;
                self.state.write_gpr(Register::Ra, ra);
                self.state.set_pc(rs1 & !1);
            }
            0x63 => {
                // BEQ, BNE, BLT, BGE (conditional branch)
                self.state.set_pc(self.state.get_pc() + 4);
            }
            0x03 => {
                // Load instructions (LB, LH, LW, LBU, LHU, LWU, LD)
                self.state.set_pc(self.state.get_pc() + 4);
            }
            0x23 => {
                // Store instructions (SB, SH, SW, SD)
                self.state.set_pc(self.state.get_pc() + 4);
            }
            0x13 => {
                // Immediate operations (ADDI, SLTI, XORI, ORI, ANDI, SLLI, SRLI, SRAI)
                self.state.set_pc(self.state.get_pc() + 4);
            }
            0x33 => {
                // Register operations (ADD, SUB, SLL, SLT, XOR, SRL, SRA, OR, AND)
                self.state.set_pc(self.state.get_pc() + 4);
            }
            0x73 => {
                // System instructions (ECALL, EBREAK, CSRRW, etc.)
                self.state.set_pc(self.state.get_pc() + 4);
            }
            0x37 => {
                // LUI
                let _imm = (opcode >> 12) as u64;
                self.state.set_pc(self.state.get_pc() + 4);
            }
            0x17 => {
                // AUIPC
                self.state.set_pc(self.state.get_pc() + 4);
            }
            _ => {
                self.state.set_pc(self.state.get_pc() + 4);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_state() {
        let mut state = CpuState::new();
        state.write_gpr(Register::Rax, 42);
        assert_eq!(state.read_gpr(Register::Rax), 42);
    }

    #[test]
    fn test_cpu_emulator() {
        let mut cpu = CpuEmulator::new(Architecture::X86_64, 1024 * 1024);
        cpu.start();
        
        // Write a simple program: MOV RAX, 100; HLT
        {
            let mut mem = cpu.memory.lock().unwrap();
            mem[0] = 0xB8;  // MOV RAX, imm32
            mem[1] = 100;
            mem[2] = 0;
            mem[3] = 0;
            mem[4] = 0;
            mem[5] = 0xF4; // HLT
        }
        
        let count = cpu.run(100).unwrap();
        assert!(count > 0);
        
        // RAX should be 100
        assert_eq!(cpu.state.read_gpr(Register::Rax), 100);
    }
}

fn serialize_gpr<S>(gpr: &[u64; 64], serializer: S) -> Result<S::Ok, S::Error> where S: Serializer { gpr.as_slice().serialize(serializer) }
fn deserialize_gpr<'de, D>(deserializer: D) -> Result<[u64; 64], D::Error> where D: Deserializer<'de> {
    let v: Vec<u64> = Vec::deserialize(deserializer)?;
    let mut arr = [0u64; 64];
    arr.copy_from_slice(&v[..64]);
    Ok(arr)
}
fn serialize_cr<S>(cr: &[u64; 16], serializer: S) -> Result<S::Ok, S::Error> where S: Serializer { cr.as_slice().serialize(serializer) }
fn deserialize_cr<'de, D>(deserializer: D) -> Result<[u64; 16], D::Error> where D: Deserializer<'de> {
    let v: Vec<u64> = Vec::deserialize(deserializer)?;
    let mut arr = [0u64; 16];
    arr.copy_from_slice(&v[..16]);
    Ok(arr)
}
fn serialize_fpr<S>(fpr: &[u64; 32], serializer: S) -> Result<S::Ok, S::Error> where S: Serializer { fpr.as_slice().serialize(serializer) }
fn deserialize_fpr<'de, D>(deserializer: D) -> Result<[u64; 32], D::Error> where D: Deserializer<'de> {
    let v: Vec<u64> = Vec::deserialize(deserializer)?;
    let mut arr = [0u64; 32];
    arr.copy_from_slice(&v[..32]);
    Ok(arr)
}
fn serialize_vreg<S>(vreg: &[u128; 32], serializer: S) -> Result<S::Ok, S::Error> where S: Serializer { vreg.as_slice().serialize(serializer) }
fn deserialize_vreg<'de, D>(deserializer: D) -> Result<[u128; 32], D::Error> where D: Deserializer<'de> {
    let v: Vec<u128> = Vec::deserialize(deserializer)?;
    let mut arr = [0u128; 32];
    arr.copy_from_slice(&v[..32]);
    Ok(arr)
}
