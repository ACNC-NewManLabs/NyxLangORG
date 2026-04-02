//! Virtual Machine Module
//! 
//! Provides the main Virtual Machine implementation that combines CPU emulation,
//! memory management, and device virtualization.

use super::cpu::{CpuEmulator, Architecture, CpuState, Register};
use super::memory::{VirtualMemory, GuestPhysicalAddr};
use super::devices::{DeviceManager, VirtualDevice, create_standard_devices, FB_BASE, FB_SIZE, FB_WIDTH, FB_HEIGHT};
use super::hypercall::HypercallHandler;
use super::kvm::KvmContext;
use super::apic::LocalApic;
use font8x8::UnicodeFonts;
use std::io::Read;
use std::fs::File;
#[cfg(feature = "jit")]
use super::jit::JitEngine;
use super::magic_ring::{MagicRingManager};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

/// VM Configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VmConfig {
    /// Number of CPUs
    pub num_cpus: usize,
    /// Memory in bytes
    pub memory: u64,
    /// Architecture to emulate
    pub architecture: Architecture,
    /// Kernel command line
    pub cmdline: String,
    /// Kernel image path
    pub kernel: Option<String>,
    /// ISO image path
    pub iso: Option<String>,
    /// Initrd image path
    pub initrd: Option<String>,
    /// Enable KVM acceleration
    pub accel: bool,
    /// Callback to be called after each instruction step
    #[serde(skip)]
    pub on_step: Option<fn(&mut CpuEmulator)>,
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            num_cpus: 1,
            memory: 512 * 1024 * 1024, // 512 MB
            architecture: Architecture::X86_64,
            cmdline: "nyxvm".to_string(),
            kernel: None,
            iso: None,
            initrd: None,
            accel: false,
            on_step: None,
        }
    }
}

/// VM State
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum VmState {
    /// VM is created but not started
    Created,
    /// VM is running
    Running,
    /// VM is paused
    Paused,
    /// VM has stopped
    Stopped,
    /// VM has crashed
    Crashed,
}

/// Virtual Machine
pub struct VirtualMachine {
    /// Configuration
    pub config: VmConfig,
    /// State
    state: VmState,
    /// CPUs
    cpus: Vec<Arc<Mutex<CpuEmulator>>>,
    /// Memory
    memory: VirtualMemory,
    /// Device manager
    devices: DeviceManager,
    /// Local APICs (one per vCPU)
    pub apics: Vec<LocalApic>,
    /// Hypercall handler
    pub hypercall_handler: HypercallHandler,
    /// KVM context
    pub kvm_context: Option<KvmContext>,
    /// JIT engine
    #[cfg(feature = "jit")]
    pub jit_engine: Option<JitEngine>,
    /// Magic Ring Manager
    pub magic_ring: MagicRingManager,
    /// Instruction count
    instruction_count: u64,
}

impl VirtualMachine {
    /// Create a new virtual machine
    pub fn new(config: VmConfig) -> Result<Self, String> {
        // Validate configuration
        if config.num_cpus == 0 || config.num_cpus > super::MAX_VM_CPUS {
            return Err("Invalid number of CPUs".to_string());
        }
        
        if config.memory == 0 || config.memory > super::MAX_VM_MEMORY {
            return Err("Invalid memory size".to_string());
        }
        
        // Create memory
        let memory = VirtualMemory::new(config.memory);
        
        // Create CPUs
        let mut cpus = Vec::with_capacity(config.num_cpus);
        let mut apics = Vec::with_capacity(config.num_cpus);
        for i in 0..config.num_cpus {
            let mut cpu = CpuEmulator::new(config.architecture, config.memory as usize);
            cpu.memory = Arc::clone(&memory.guest_memory);
            cpus.push(Arc::new(Mutex::new(cpu)));
            apics.push(LocalApic::new(i as u32));
        }
        
        let mut kvm_context = None;
        if config.accel {
            match KvmContext::new(config.memory, config.num_cpus) {
                Ok(mut ctx) => {
                    // Create VCPUs first
                    for i in 0..config.num_cpus {
                        if let Err(e) = ctx.create_vcpu(i as u8) {
                            eprintln!("Warning: Failed to create VCPU {}: {}", i, e);
                        }
                    }

                    // Set up CPUID for the BSP (VCPU 0)
                    if !ctx.vcpu_fds.is_empty() {
                        let cpuid = ctx.kvm.get_supported_cpuid(kvm_bindings::KVM_MAX_CPUID_ENTRIES)
                            .map_err(|e| format!("Failed to get supported CPUID: {}", e))?;
                        ctx.vcpu_fds[0].set_cpuid2(&cpuid).map_err(|e| format!("Failed to set CPUID: {}", e))?;
                    }

                    if let Err(e) = ctx.map_memory(0, 0, &memory.guest_memory) {
                        eprintln!("Warning: Failed to map memory to KVM: {}", e);
                    }
                    kvm_context = Some(ctx);
                }
                Err(e) => {
                    eprintln!("Warning: KVM initialization failed, falling back to software emulation: {}", e);
                }
            }
        }
        
        // Create device manager
        let devices = create_standard_devices(&config);
        
        #[cfg(feature = "jit")]
        let jit_engine = Some(JitEngine::new());
        
        let mut vm = Self {
            config: config.clone(),
            state: VmState::Created,
            memory,
            cpus,
            devices,
            apics,
            hypercall_handler: HypercallHandler::new(),
            kvm_context,
            #[cfg(feature = "jit")]
            jit_engine,
            magic_ring: MagicRingManager::new(GuestPhysicalAddr(config.memory - 2 * 1024 * 1024)), // Last 2MB
            instruction_count: 0,
        };
        
        // Initial Architectural State for BIOS-only boot
        if config.kernel.is_none() || config.kernel.as_ref().unwrap().is_empty() {
             let mut cpu = vm.cpus[0].lock().unwrap();
             cpu.state.write_gpr(Register::Cs, 0xF000);
             cpu.state.set_pc(0xFFF0);
             cpu.state.mode = super::cpu::CpuMode::Real;
             // Segment registers for reset vector (F000:FFF0)
             // These would be set in KVM or JIT context during run_cpu
        }
        
        // Sync RAM size to CMOS for BIOS discovery
        let mem_kb = config.memory / 1024;
        if let Some(cmos_boxed) = vm.devices.get_device_mut("cmos") {
             if let Some(cmos) = cmos_boxed.as_any_mut().downcast_mut::<super::cmos::CmosDevice>() {
                 cmos.set_memory_size(mem_kb);
             }
        }
        
        Ok(vm)
    }
    
    /// Update mouse state in guest memory
    pub fn update_mouse(&mut self, x: u32, y: u32, buttons: u32) {
        let base = 0x10200000;
        let _ = self.memory.write_phys(GuestPhysicalAddr(base), 4, x as u64);
        let _ = self.memory.write_phys(GuestPhysicalAddr(base + 4), 4, y as u64);
        let _ = self.memory.write_phys(GuestPhysicalAddr(base + 8), 4, buttons as u64);
    }
     
    /// Load kernel
    pub fn load_kernel(&mut self, data: &[u8], entry: u64) -> Result<(), String> {
        for cpu in &self.cpus {
            cpu.lock().map_err(|_| "Lock poisoned")?.load_elf(data, entry)?;
        }
        
        // Also load BIOS if on x86_64
        if self.config.architecture == Architecture::X86_64 {
            self.load_bios();
        }
        Ok(())
    }

    /// Load SeaBIOS / Firmware
    pub fn load_bios(&mut self) {
        let bios_path = "/home/surya/Nyx Programming Language/nyx-bios/bios.bin";
        let mut bios_file = match File::open(bios_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Warning: Failed to open Nyx BIOS at {}: {}", bios_path, e);
                return;
            }
        };

        let mut bios_data = Vec::new();
        if let Err(e) = bios_file.read_to_end(&mut bios_data) {
            eprintln!("Warning: Failed to read Nyx BIOS: {}", e);
            return;
        }

        // The bios.bin is 128KB (0x20000 bytes)
        // It should be mapped at the top of the first megabyte: 0xE0000 - 0xFFFFF
        let bios_base = 0xE0000usize;
        let mut mem = self.memory.guest_memory.lock().unwrap();
        
        if mem.len() >= bios_base + bios_data.len() {
            let len = bios_data.len();
            mem[bios_base..bios_base + len].copy_from_slice(&bios_data);
            println!("Nyx Hypervisor: Nyx BIOS loaded successfully ({} bytes at 0x{:x})", len, bios_base);
            
            // Mirror BIOS at the top of 4GB (common for x86_64 reset)
            // 4GB = 0x100000000. 128KB BIOS = 0xFFFE0000
            let bios_top_base = 0xFFFE0000usize;
            if mem.len() >= bios_top_base + len {
                mem[bios_top_base..bios_top_base + len].copy_from_slice(&bios_data);
                println!("Nyx Hypervisor: Nyx BIOS mirrored at 0x{:x}", bios_top_base);
            }
        } else {
            eprintln!("Warning: Guest memory too small to load 128KB BIOS (Size: {} bytes)", mem.len());
        }
    }
    
    /// Start the VM
    pub fn start(&mut self) -> Result<(), String> {
        if self.state != VmState::Created && self.state != VmState::Paused {
            return Err("VM cannot be started in current state".to_string());
        }
        
        self.state = VmState::Running;
        
        // Load BIOS for x86_64 BIOS-only boot
        if self.config.architecture == Architecture::X86_64 {
            self.load_bios();
        }

        // Initialize Magic Ring
        self.magic_ring.init(&mut self.memory)?;
        
        // Start all CPUs
        for cpu in &self.cpus {
            cpu.lock().map_err(|_| "Lock poisoned")?.start();
        }
        
        Ok(())
    }
    
    /// Pause the VM
    pub fn pause(&mut self) -> Result<(), String> {
        if self.state != VmState::Running {
            return Err("VM is not running".to_string());
        }
        
        self.state = VmState::Paused;
        
        for cpu in &self.cpus {
            cpu.lock().map_err(|_| "Lock poisoned")?.stop();
        }
        
        Ok(())
    }
    
    /// Resume the VM
    pub fn resume(&mut self) -> Result<(), String> {
        self.start()
    }
    
    /// Stop the VM
    pub fn stop(&mut self) -> Result<(), String> {
        self.state = VmState::Stopped;
        
        for cpu in &self.cpus {
            cpu.lock().map_err(|_| "Lock poisoned")?.stop();
        }
        
        Ok(())
    }
    
    /// Run the VM for a specified number of instructions
    pub fn run(&mut self, max_instructions: u64) -> Result<u64, String> {
        if self.state != VmState::Running && self.state != VmState::Created {
            return Err("VM is not in running state".to_string());
        }
        
        self.state = VmState::Running;
        
        // Run via KVM if available
        if self.kvm_context.is_some() {
            return self.run_kvm(max_instructions);
        }
        
        // Spawn threads for each CPU
        let mut handles = Vec::new();
        let num_cpus = self.cpus.len();
        
        // Spawn background UI sync thread (60 FPS)
        // We use a raw pointer to self since run() is synchronous and the VM 
        // state will be Stopped before this function returns.
        let vm_ptr = self as *mut Self as usize;
        let sync_handle = std::thread::spawn(move || {
            let vm = unsafe { &*(vm_ptr as *const VirtualMachine) };
            while vm.state == VmState::Running {
                vm.sync_framebuffer();
                std::thread::sleep(std::time::Duration::from_millis(16));
            }
        });
        handles.push(sync_handle);

        // For Phase 4, we will maintain the simplest multi-threading:
        // Run BSP on main thread, APs on background threads.
        
        for i in 1..num_cpus {
            let cpu = Arc::clone(&self.cpus[i]);
            let handle = std::thread::spawn(move || {
                // Secondary CPU run loop
                loop {
                    // Lock for a short burst of execution
                    let mut c = cpu.lock().unwrap();
                    if !c.running { break; }
                    
                    for _ in 0..1000 {
                        if let Err(_) = c.step() {
                            break;
                        }
                    }
                    // Yield to allow other threads (like UI or main) to take the lock
                    std::thread::yield_now();
                }
            });
            handles.push(handle);
        }

        // Run BSP (CPU 0) on current thread
        let result = self.run_cpu(0, max_instructions);
        
        result
    }

    /// Run a single CPU
    pub fn run_cpu(&mut self, cpu_idx: usize, max_instructions: u64) -> Result<u64, String> {
        let mut total_instructions = 0u64;
        
        while total_instructions < max_instructions && self.state == VmState::Running {
            // Check for hypercalls
            if self.check_hypercall(cpu_idx)? {
                continue;
            }
            
            // Check for I/O
            if self.handle_io(cpu_idx)? {
                continue;
            }
            
            let mut cpu = self.cpus[cpu_idx].lock().unwrap();

            // Execute instruction (via JIT if possible, but NOT in Real Mode)
            #[cfg(feature = "jit")]
            {
                let use_jit = cpu.state.mode != super::cpu::CpuMode::Real;
                if use_jit {
                    if let Some(jit) = &mut self.jit_engine {
                        let pc = cpu.state.get_pc();

                        // Check for hot-block re-optimization
                        if jit.is_hot(pc, 50000) {
                            jit.clear_cache(pc);
                        }

                        match jit.compile_block(&mut cpu, pc) {
                            Ok(code_ptr) => {
                                // Execute using raw function pointer to avoid double-borrow
                                if let Some(counter) = jit.hit_counters.get(&pc) {
                                    let counter_ptr = counter.as_ref() as *const _ as *const u64;
                                    type JitFunc = unsafe extern "C" fn(*mut super::cpu::CpuState, *const u64);
                                    let func: JitFunc = unsafe { std::mem::transmute(code_ptr) };
                                    unsafe { func(&mut cpu.state as *mut _, counter_ptr) };
                                }
                                total_instructions += 10;
                            }
                            Err(_) => {
                                cpu.step()?;
                                total_instructions += 1;
                            }
                        }
                    } else {
                        cpu.step()?;
                        total_instructions += 1;
                    }
                } else {
                    // Real Mode: always use interpreter
                    cpu.step()?;
                    total_instructions += 1;
                }
            }
            #[cfg(not(feature = "jit"))]
            {
                cpu.step()?;
                total_instructions += 1;
            }
            
            drop(cpu);
            
            if total_instructions % 1000 == 0 {
                self.magic_ring.process_requests(&self.memory);
            }
        }
        
        Ok(total_instructions)
    }
    
    /// Run the VM using KVM hardware acceleration
    fn run_kvm(&mut self, _max_instructions: u64) -> Result<u64, String> {
        #[cfg(target_os = "linux")]
        {
            let mut ctx_opt = self.kvm_context.take();
            let result = if let Some(ref mut ctx) = ctx_opt {
                // Sync registers to KVM before running
                {
                    let cpu = self.cpus[0].lock().unwrap();
                    ctx.sync_regs_to_kvm(0, &cpu.state)?;
                }
                
                let mut run_res = Ok(());
                loop {
                    match ctx.run_vcpu(0) {
                        Ok(kvm_ioctls::VcpuExit::IoIn(port, data)) => {
                            for i in 0..data.len() {
                                if let Ok(val) = self.devices.port_read(port, 1) {
                                    data[i] = val as u8;
                                }
                            }
                        }
                        Ok(kvm_ioctls::VcpuExit::IoOut(port, data)) => {
                            for &b in data {
                                if let Err(e) = self.devices.port_write(port, 1, b as u64) {
                                    run_res = Err(e.to_string());
                                    break;
                                }
                            }
                        }
                        Ok(kvm_ioctls::VcpuExit::MmioRead(addr, data)) => {
                            // Assume simplified MMIO for now
                            if let Ok(val) = self.memory.read_phys(GuestPhysicalAddr(addr), data.len()) {
                                let bytes = val.to_le_bytes();
                                data.copy_from_slice(&bytes[..data.len()]);
                            }
                        }
                        Ok(kvm_ioctls::VcpuExit::MmioWrite(addr, data)) => {
                            let mut val = 0u64;
                            for i in 0..data.len().min(8) {
                                val |= (data[i] as u64) << (i * 8);
                            }
                            let _ = self.memory.write_phys(GuestPhysicalAddr(addr), data.len(), val);
                            
                            // Check if drawing to framebuffer
                            if addr >= FB_BASE && addr < FB_BASE + FB_SIZE {
                                self.sync_framebuffer();
                            }
                        }
                        Ok(kvm_ioctls::VcpuExit::Hlt) => {
                            self.state = VmState::Paused;
                            break;
                        }
                        Ok(kvm_ioctls::VcpuExit::Shutdown) => {
                            self.state = VmState::Stopped;
                            break;
                        }
                        Ok(kvm_ioctls::VcpuExit::InternalError) => {
                            run_res = Err("KVM Internal Error".to_string());
                            break;
                        }
                        Ok(exit) => {
                            eprintln!("KVM Unhandled Exit: {:?}", exit);
                            break;
                        }
                        Err(e) => {
                            run_res = Err(e);
                            break;
                        }
                    }
                    if run_res.is_err() { break; }
                }
                
                // Sync registers back from KVM
                let mut cpu = self.cpus[0].lock().unwrap();
                if let Err(e) = ctx.sync_regs_from_kvm(0, &mut cpu.state) {
                    if run_res.is_ok() { run_res = Err(e); }
                }
                
                run_res
            } else {
                Err("KVM context missing".to_string())
            };
            
            // Restore KVM context
            self.kvm_context = ctx_opt;
            
            return result.map(|_| 1);
        }
        
        #[cfg(not(target_os = "linux"))]
        Err("KVM not supported on this platform".to_string())
    }
    /// Check for hypercall
    fn check_hypercall(&mut self, cpu_idx: usize) -> Result<bool, String> {
        let pc = self.cpus[cpu_idx].lock().unwrap().state.get_pc();
        
        // Check for hypercall (typically via special instruction)
        // For x86_64, we check for 0xF1 (hypercall opcode)
        // For ARM64, we check for HVC instruction
        // For RISC-V, we check for ECALL with specific arguments
        
        let (opcode, _rax) = {
            let mut cpu = self.cpus[cpu_idx].lock().unwrap();
            (cpu.read_memory(pc, 1).unwrap_or(0) as u8, cpu.state.read_gpr(Register::Rax))
        };
        
        match self.config.architecture {
            Architecture::X86_64 => {
                if opcode == 0xF1 {
                    let mut cpu = self.cpus[cpu_idx].lock().unwrap();
                    let hypercall_num = cpu.state.read_gpr(Register::Rax);
                    
                    if hypercall_num == 100 {
                        // GetFbPtr: Return guest physical address of framebuffer
                        println!("Nyx Hypervisor: Guest requested GetFbPtr -> 0x{:x}", FB_BASE);
                        cpu.state.write_gpr(Register::Rax, FB_BASE);
                        let pc = cpu.state.get_pc();
                        cpu.state.set_pc(pc + 1);
                        return Ok(true);
                    }
                    
                    if hypercall_num == 101 {
                        // GetInputPtr: Return guest physical address of input buffer
                        let input_ptr = 0x10200000;
                        println!("Nyx Hypervisor: Guest requested GetInputPtr -> 0x{:x}", input_ptr);
                        cpu.state.write_gpr(Register::Rax, input_ptr);
                        let pc = cpu.state.get_pc();
                        cpu.state.set_pc(pc + 1);
                        return Ok(true);
                    }
                    
                    // Fallback to standard handler
                    match self.hypercall_handler.handle(&mut cpu) {
                        Ok(_) => {
                            let pc = cpu.state.get_pc();
                            cpu.state.set_pc(pc + 1);
                            return Ok(true);
                        }
                        Err(e) => {
                            self.state = VmState::Crashed;
                            return Err(format!("Hypercall error: {}", e));
                        }
                    }
                } else {
                    return Ok(false);
                }
            }
            Architecture::AArch64 => {
                // Check for HVC (Hypervisor Call)
                if opcode == 0xD4 || opcode == 0x14 {
                    // Handle hypercall
                    let mut cpu = self.cpus[cpu_idx].lock().unwrap();
                    self.hypercall_handler.handle(&mut cpu).map_err(|e| e.to_string())?;
                    return Ok(true);
                }
            }
            Architecture::RiscV64 => {
                // Check for ECALL
                if opcode == 0x73 {
                    // Handle hypercall
                    let mut cpu = self.cpus[cpu_idx].lock().unwrap();
                    self.hypercall_handler.handle(&mut cpu).map_err(|e| e.to_string())?;
                    return Ok(true);
                }
            }
        }
        
        Ok(false)
    }
    
    /// Handle I/O operations
    fn handle_io(&mut self, cpu_idx: usize) -> Result<bool, String> {
        let pc = self.cpus[cpu_idx].lock().unwrap().state.get_pc();
        let opcode = self.memory.read_phys(GuestPhysicalAddr(pc), 1)?;
        
        // x86_64 IN/OUT instructions
        if opcode == 0xE4 || opcode == 0xE5 || // IN
           opcode == 0xE6 || opcode == 0xE7 || // OUT
           opcode == 0xEC || opcode == 0xED || // IN (dx)
           opcode == 0xEE || opcode == 0xEF {  // OUT (dx)
            
            // Full I/O Dispatch
            let mut cpu = self.cpus[cpu_idx].lock().unwrap();
            let state = &mut cpu.state;
            
            match opcode {
                0xE6 | 0xE7 | 0xEE | 0xEF => { // OUT
                    let port = if opcode >= 0xEE { state.read_gpr(super::cpu::Register::Rdx) as u16 } else { self.memory.read_phys(GuestPhysicalAddr(pc + 1), 1)? as u16 };
                    let val = state.read_gpr(super::cpu::Register::Rax);
                    let _ = self.devices.port_write(port, 1, val);
                    state.set_pc(pc + (if opcode >= 0xEE { 1 } else { 2 }));
                }
                0xE4 | 0xE5 | 0xEC | 0xED => { // IN
                    let port = if opcode >= 0xEC { state.read_gpr(super::cpu::Register::Rdx) as u16 } else { self.memory.read_phys(GuestPhysicalAddr(pc + 1), 1)? as u16 };
                    if let Ok(val) = self.devices.port_read(port, 1) {
                         state.write_gpr(super::cpu::Register::Rax, val);
                    }
                    state.set_pc(pc + (if opcode >= 0xEC { 1 } else { 2 }));
                }
                _ => {}
            }
            return Ok(true);
        }
        
        Ok(false)
    }
    
    /// Get VM state
    pub fn state(&self) -> VmState {
        self.state
    }
    
    /// Get instruction count
    pub fn instruction_count(&self) -> u64 {
        self.instruction_count
    }
    
    /// Get CPU state
    pub fn cpu_state(&self, cpu_id: usize) -> Option<CpuState> {
        self.cpus.get(cpu_id).map(|c| c.lock().unwrap().state.clone())
    }
    
    /// Get memory
    pub fn memory(&self) -> &VirtualMemory {
        &self.memory
    }
    
    /// Get device manager
    pub fn devices(&self) -> &DeviceManager {
        &self.devices
    }
    
    /// Get device manager (mutable)
    pub fn devices_mut(&mut self) -> &mut DeviceManager {
        &mut self.devices
    }
    
    /// Add a device
    pub fn add_device(&mut self, name: &str, device: Box<dyn VirtualDevice>) {
        self.devices.add_device(name, device);
    }
    
    /// Sync console framebuffer from physical memory
    pub fn sync_framebuffer(&self) {
        if let Some(fb_mutex) = self.devices.get_console_framebuffer() {
            let mut fb = fb_mutex.lock().unwrap();
            let mem_locked = self.memory.guest_memory.lock().unwrap();
            
            // 1. Try VBE/Linear Framebuffer (0xFD000000)
            let fb_base = FB_BASE as usize;
            let fb_size = FB_SIZE as usize;
            if fb_base + fb_size <= mem_locked.len() {
                let src_slice = &mem_locked[fb_base..fb_base + fb_size];
                for (i, chunk) in src_slice.chunks_exact(4).enumerate() {
                    if i < fb.len() {
                        fb[i] = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                    }
                }
                return;
            }

            // 2. Fallback: Standard BIOS Text Mode (0xB8000)
            // Render 80x25 text to 800x600 using 8x8 font (scaled 1x)
            let text_base = 0xB8000usize;
            if text_base + 4000 <= mem_locked.len() {
                let text_mem = &mem_locked[text_base..text_base + 4000];
                for row in 0..25 {
                    for col in 0..80 {
                        let idx = (row * 80 + col) * 2;
                        let char_code = text_mem[idx];
                        let attr = text_mem[idx + 1];
                        
                        // Simple 8x8 font to pixel rendering
                        self.draw_char_to_fb(&mut fb, col * 8 + 80, row * 12 + 100, char_code, attr);
                    }
                }
            }
        }
    }

    fn draw_char_to_fb(&self, fb: &mut Vec<u32>, x: usize, y: usize, char_code: u8, attr: u8) {
        let fg_colors = [
            0xFF000000, 0xFF0000AA, 0xFF00AA00, 0xFF00AAAA,
            0xFFAA0000, 0xFFAA00AA, 0xFFAA5500, 0xFFAAAAAA,
            0xFF555555, 0xFF5555FF, 0xFF55FF55, 0xFF55FFFF,
            0xFFFF5555, 0xFFFF55FF, 0xFFFFFF55, 0xFFFFFFFF,
        ];
        let fg = fg_colors[(attr & 0x0F) as usize];
        let bg = fg_colors[((attr & 0xF0) >> 4) as usize];

        if let Some(glyph) = font8x8::BASIC_FONTS.get(char_code as char) {
            for gy in 0..8 {
                for gx in 0..8 {
                    let pixel = if glyph[gy] & (1 << gx) != 0 { fg } else { bg };
                    let px = x + gx;
                    let py = y + gy;
                    if px < FB_WIDTH && py < FB_HEIGHT {
                        fb[py * FB_WIDTH + px] = pixel;
                    }
                }
            }
        }
    }

    /// Save VM snapshot
    pub fn save_snapshot(&self, path: &str) -> Result<(), String> {
        use std::fs::File;
        use flate2::write::GzEncoder;
        use flate2::Compression;

        let file = File::create(path).map_err(|e| e.to_string())?;
        let mut encoder = GzEncoder::new(file, Compression::default());
        
        // Collect CPU states
        let cpu_states: Vec<CpuState> = self.cpus.iter()
            .map(|c| c.lock().unwrap().state.clone())
            .collect();
            
        // Get memory dump
        let memory_dump = self.memory.guest_memory.lock().unwrap().clone();
        
        let snapshot = (
            &self.config,
            cpu_states,
            memory_dump,
        );
        
        bincode::serialize_into(&mut encoder, &snapshot)
            .map_err(|e| e.to_string())?;
            
        Ok(())
    }

    /// Load VM snapshot
    pub fn load_snapshot(path: &str) -> Result<Self, String> {
        use std::fs::File;
        use flate2::read::GzDecoder;

        let file = File::open(path).map_err(|e| e.to_string())?;
        let mut decoder = GzDecoder::new(file);
        
        let (config, cpu_states, memory_dump): (VmConfig, Vec<CpuState>, super::memory::PageAlignedBuffer) = 
            bincode::deserialize_from(&mut decoder)
            .map_err(|e| e.to_string())?;
            
        let vm = Self::new(config)?;
        
        // Restore memory
        *vm.memory.guest_memory.lock().unwrap() = memory_dump;
        
        // Restore CPUs
        for (i, state) in cpu_states.into_iter().enumerate() {
            if i < vm.cpus.len() {
                vm.cpus[i].lock().unwrap().state = state;
            }
        }
        
        Ok(vm)
    }

    /// Helper to execute a JIT block
    #[cfg(feature = "jit")]
    #[allow(dead_code)]
    fn execute_jit_block(&self, code_ptr: *const u8, jit: &super::jit::JitEngine, pc: u64, cpu: &mut super::cpu::CpuEmulator) {
        if let Some(counter) = jit.hit_counters.get(&pc) {
            let counter_ptr = counter.as_ref() as *const _ as *const u64;
            type JitFunc = unsafe extern "C" fn(*mut super::cpu::CpuState, *const u64);
            let func: JitFunc = unsafe { std::mem::transmute(code_ptr) };
            unsafe { func(&mut cpu.state as *mut _, counter_ptr) };
        }
    }

    /// Perform a high-speed micro-snapshot (Nyx-Freeze)
    pub fn freeze(&mut self) -> Result<Vec<u8>, String> {
        let mut snapshot_data = Vec::new();
        
        // 1. Pause all CPUs
        self.pause()?;
        
        // 2. Collect states
        let cpu_states: Vec<CpuState> = self.cpus.iter()
            .map(|c| c.lock().unwrap().state.clone())
            .collect();
            
        // 3. Get dirty memory if KVM is available
        let memory_delta = if let Some(ref ctx) = self.kvm_context {
            #[cfg(target_os = "linux")]
            {
                let dirty_log = ctx.get_dirty_log(0, self.config.memory)?;
                let mut delta = HashMap::new();
                let mem = self.memory.guest_memory.lock().unwrap();
                
                // Process dirty bitmap (each bit represents a 4KB page)
                for (i, &byte) in dirty_log.iter().enumerate() {
                    if byte != 0 {
                        for bit in 0..64 {
                            if (byte & (1 << bit)) != 0 {
                                let page_idx = i * 64 + bit;
                                let offset = page_idx * 4096;
                                if offset + 4096 <= mem.len() {
                                    delta.insert(offset as u64, mem[offset..offset + 4096].to_vec());
                                }
                            }
                        }
                    }
                }
                Some(delta)
            }
            #[cfg(not(target_os = "linux"))]
            None
        } else {
            None
        };
        
        let freeze_packet = (cpu_states, memory_delta);
        bincode::serialize_into(&mut snapshot_data, &freeze_packet)
            .map_err(|e| e.to_string())?;
            
        Ok(snapshot_data)
    }

    /// Restore from a micro-snapshot (Nyx-Thaw)
    pub fn thaw(&mut self, data: &[u8]) -> Result<(), String> {
        let (cpu_states, memory_delta): (Vec<CpuState>, Option<HashMap<u64, Vec<u8>>>) = 
            bincode::deserialize(data).map_err(|e| e.to_string())?;
            
        // Restore CPU states
        for (i, state) in cpu_states.into_iter().enumerate() {
            if i < self.cpus.len() {
                self.cpus[i].lock().unwrap().state = state;
            }
        }
        
        // Restore memory delta
        if let Some(delta) = memory_delta {
            let mut mem = self.memory.guest_memory.lock().unwrap();
            for (offset, page_data) in delta {
                let off = offset as usize;
                if off + page_data.len() <= mem.len() {
                    mem[off..off + page_data.len()].copy_from_slice(&page_data);
                }
            }
        }
        
        self.resume()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_creation() {
        let config = VmConfig::default();
        let _devices = create_standard_devices(&config);
        let vm = VirtualMachine::new(config);
        assert!(vm.is_ok());
        
        let vm = vm.unwrap();
        assert_eq!(vm.state(), VmState::Created);
    }

    #[test]
    fn test_vm_run() {
        let config = VmConfig::default();
        let mut vm = VirtualMachine::new(config).unwrap();
        
        // Write a simple program: HLT (0xF4)
        vm.memory.write_phys(GuestPhysicalAddr(0x1000), 1, 0xF4).unwrap();
        
        // Set PC to program location
        vm.cpus[0].lock().unwrap().state.set_pc(0x1000);
        
        // Run for a few instructions
        let count = vm.run(10).unwrap();
        
        assert!(count > 0);
    }
}

