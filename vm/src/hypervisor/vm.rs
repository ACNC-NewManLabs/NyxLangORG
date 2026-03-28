//! Virtual Machine Module
//! 
//! Provides the main Virtual Machine implementation that combines CPU emulation,
//! memory management, and device virtualization.

use super::cpu::{CpuEmulator, Architecture, CpuState};
use super::memory::{VirtualMemory, GuestPhysicalAddr, PteFlags};
use super::devices::{DeviceManager, create_standard_devices, VirtualDevice};
use super::hypercall::{Hypercall, HypercallHandler};

/// VM Configuration
#[derive(Debug, Clone)]
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
    /// Initrd image path
    pub initrd: Option<String>,
    /// Enable KVM acceleration
    pub accel: bool,
    /// Callback to be called after each instruction step
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
            initrd: None,
            accel: false,
            on_step: None,
        }
    }
}

/// VM State
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    config: VmConfig,
    /// State
    state: VmState,
    /// CPUs
    cpus: Vec<CpuEmulator>,
    /// Memory
    memory: VirtualMemory,
    /// Device manager
    devices: DeviceManager,
    /// Hypercall handler
    hypercall_handler: HypercallHandler,
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
        let mut cpus = Vec::new();
        for _ in 0..config.num_cpus {
            cpus.push(CpuEmulator::new(config.architecture, config.memory as usize));
        }
        
        // Create device manager
        let devices = create_standard_devices();
        
        // Create hypercall handler
        let hypercall_handler = HypercallHandler::new();
        
        Ok(Self {
            config,
            state: VmState::Created,
            cpus,
            memory,
            devices,
            hypercall_handler,
            instruction_count: 0,
        })
    }
    
    /// Load kernel
    pub fn load_kernel(&mut self, data: &[u8], entry: u64) -> Result<(), String> {
        for cpu in &mut self.cpus {
            cpu.load_elf(data, entry)?;
        }
        Ok(())
    }
    
    /// Start the VM
    pub fn start(&mut self) -> Result<(), String> {
        if self.state != VmState::Created && self.state != VmState::Paused {
            return Err("VM cannot be started in current state".to_string());
        }
        
        self.state = VmState::Running;
        
        // Start all CPUs
        for cpu in &mut self.cpus {
            cpu.start();
        }
        
        Ok(())
    }
    
    /// Pause the VM
    pub fn pause(&mut self) -> Result<(), String> {
        if self.state != VmState::Running {
            return Err("VM is not running".to_string());
        }
        
        self.state = VmState::Paused;
        
        for cpu in &mut self.cpus {
            cpu.stop();
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
        
        for cpu in &mut self.cpus {
            cpu.stop();
        }
        
        Ok(())
    }
    
    /// Run the VM for a specified number of instructions
    pub fn run(&mut self, max_instructions: u64) -> Result<u64, String> {
        if self.state != VmState::Running && self.state != VmState::Created {
            return Err("VM is not in running state".to_string());
        }
        
        self.state = VmState::Running;
        
        let mut total_instructions = 0u64;
        let mut first_cpu = true;
        
        // Run on first CPU (BSP)
        while total_instructions < max_instructions && self.state == VmState::Running {
            // Check for hypercalls
            let pc = self.cpus[0].state.get_pc();
            if self.check_hypercall(&mut self.cpus[0])? {
                continue;
            }
            
            // Check for I/O
            if self.handle_io(&mut self.cpus[0])? {
                continue;
            }
            
            // Execute instruction
            self.cpus[0].step()?;
            total_instructions += 1;
            
            if first_cpu {
                self.instruction_count = total_instructions;
                first_cpu = false;
            }
        }
        
        if self.state == VmState::Running && total_instructions >= max_instructions {
            self.state = VmState::Paused;
        }
        
        Ok(total_instructions)
    }
    
    /// Check for hypercall
    fn check_hypercall(&mut self, cpu: &mut CpuEmulator) -> Result<bool, String> {
        let pc = cpu.state.get_pc();
        
        // Check for hypercall (typically via special instruction)
        // For x86_64, we check for 0xF1 (hypercall opcode)
        // For ARM64, we check for HVC instruction
        // For RISC-V, we check for ECALL with specific arguments
        
        let opcode = self.memory.read_phys(GuestPhysicalAddr(pc), 1)?;
        
        match self.config.architecture {
            Architecture::X86_64 => {
                if opcode == 0xF1 {
                    // Handle hypercall
                    let result = self.hypercall_handler.handle(cpu);
                    match result {
                        Ok(_) => {
                            cpu.state.set_pc(cpu.state.get_pc() + 1);
                            return Ok(true);
                        }
                        Err(e) => {
                            self.state = VmState::Crashed;
                            return Err(format!("Hypercall error: {}", e));
                        }
                    }
                }
            }
            Architecture::AArch64 => {
                // Check for HVC (Hypervisor Call)
                if opcode == 0xD4 || opcode == 0x14 {
                    // Handle hypercall
                    self.hypercall_handler.handle(cpu)?;
                    return Ok(true);
                }
            }
            Architecture::RiscV64 => {
                // Check for ECALL
                if opcode == 0x73 {
                    // Handle hypercall
                    self.hypercall_handler.handle(cpu)?;
                    return Ok(true);
                }
            }
        }
        
        Ok(false)
    }
    
    /// Handle I/O operations
    fn handle_io(&mut self, cpu: &mut CpuEmulator) -> Result<bool, String> {
        // Check for I/O port access (simplified)
        // In a real implementation, this would intercept IN/OUT instructions
        
        let pc = cpu.state.get_pc();
        let opcode = self.memory.read_phys(GuestPhysicalAddr(pc), 1)?;
        
        // x86_64 IN/OUT instructions
        if opcode == 0xE4 || opcode == 0xE5 || // IN
           opcode == 0xE6 || opcode == 0xE7 || // OUT
           opcode == 0xEC || opcode == 0xED || // IN (dx)
           opcode == 0xEE || opcode == 0xEF {  // OUT (dx)
            
            // Simplified I/O handling
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
    pub fn cpu_state(&self, cpu_id: usize) -> Option<&CpuState> {
        self.cpus.get(cpu_id).map(|c| &c.state)
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_creation() {
        let config = VmConfig::default();
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
        vm.cpus[0].state.set_pc(0x1000);
        
        // Run for a few instructions
        let count = vm.run(10).unwrap();
        
        assert!(count > 0);
    }
}

