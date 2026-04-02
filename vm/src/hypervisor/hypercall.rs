//! Hypercall Interface Module
//!
//! Provides hypercall (paravirtualization) interface between guest and hypervisor.
//! Hypercalls allow the guest OS to request services from the host.

use super::cpu::{CpuEmulator, Register};

/// Hypercall numbers
#[derive(Debug, Clone, Copy)]
pub enum HypercallNumber {
    /// Shutdown the VM
    Shutdown,
    /// Reboot the VM
    Reboot,
    /// Exit to host
    Exit,
    /// Get hypervisor info
    GetInfo,
    /// Allocate memory
    AllocMem,
    /// Free memory
    FreeMem,
    /// Console output
    ConsoleWrite,
    /// Console input
    ConsoleRead,
    /// Block read
    BlockRead,
    /// Block write
    BlockWrite,
    /// Network send
    NetSend,
    /// Network receive
    NetReceive,
    /// Set timer
    /// Set timer
    SetTimer,
    /// Interrupt inject
    InjectInterrupt,
    /// Get framebuffer pointer
    GetFbPtr,
    /// Get input buffer pointer
    GetInputPtr,
    /// Custom hypercall
    Custom(u32),
}

impl HypercallNumber {
    pub fn to_u32(&self) -> u32 {
        match self {
            Self::Shutdown => 0,
            Self::Reboot => 1,
            Self::Exit => 2,
            Self::GetInfo => 3,
            Self::AllocMem => 4,
            Self::FreeMem => 5,
            Self::ConsoleWrite => 10,
            Self::ConsoleRead => 11,
            Self::BlockRead => 20,
            Self::BlockWrite => 21,
            Self::NetSend => 30,
            Self::NetReceive => 31,
            Self::SetTimer => 40,
            Self::InjectInterrupt => 50,
            Self::GetFbPtr => 100,
            Self::GetInputPtr => 101,
            Self::Custom(n) => *n,
        }
    }
}

impl HypercallNumber {
    pub fn to_u64(&self) -> u64 {
        match self {
            HypercallNumber::Shutdown => 0,
            HypercallNumber::Reboot => 1,
            HypercallNumber::Exit => 2,
            HypercallNumber::GetInfo => 3,
            HypercallNumber::AllocMem => 4,
            HypercallNumber::FreeMem => 5,
            HypercallNumber::ConsoleWrite => 10,
            HypercallNumber::ConsoleRead => 11,
            HypercallNumber::BlockRead => 20,
            HypercallNumber::BlockWrite => 21,
            HypercallNumber::NetSend => 30,
            HypercallNumber::NetReceive => 31,
            HypercallNumber::SetTimer => 40,
            HypercallNumber::InjectInterrupt => 50,
            HypercallNumber::GetFbPtr => 100,
            HypercallNumber::GetInputPtr => 101,
            HypercallNumber::Custom(n) => *n as u64,
        }
    }
}

/// Hypercall result
pub type HypercallResult = Result<u64, HypercallError>;

/// Hypercall errors
#[derive(Debug)]
pub enum HypercallError {
    InvalidHypercall,
    InvalidParameter,
    NotSupported,
    OutOfMemory,
    IoError(String),
}

impl std::fmt::Display for HypercallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HypercallError::InvalidHypercall => write!(f, "Invalid hypercall"),
            HypercallError::InvalidParameter => write!(f, "Invalid parameter"),
            HypercallError::NotSupported => write!(f, "Operation not supported"),
            HypercallError::OutOfMemory => write!(f, "Out of memory"),
            HypercallError::IoError(msg) => write!(f, "I/O error: {}", msg),
        }
    }
}

impl std::error::Error for HypercallError {}

/// Hypercall handler
pub struct HypercallHandler {
    _console_buffer: String,
    timer_interval: u64,
}

impl HypercallHandler {
    /// Create a new hypercall handler
    pub fn new() -> Self {
        Self {
            _console_buffer: String::new(),
            timer_interval: 0,
        }
    }

    /// Handle a hypercall from a CPU
    pub fn handle(&mut self, cpu: &mut CpuEmulator) -> HypercallResult {
        // Get hypercall number from standard register
        // For x86_64: typically RAX
        // For ARM64: typically X0
        // For RISC-V: typically A0

        let hypercall_num = cpu.state.read_gpr(Register::Rax);

        match hypercall_num {
            0 => self.handle_shutdown(cpu),
            1 => self.handle_reboot(cpu),
            2 => self.handle_exit(cpu),
            3 => self.handle_get_info(cpu),
            4 => self.handle_alloc_mem(cpu),
            5 => self.handle_free_mem(cpu),
            10 => self.handle_console_write(cpu),
            11 => self.handle_console_read(cpu),
            20 => self.handle_block_read(cpu),
            21 => self.handle_block_write(cpu),
            30 => self.handle_net_send(cpu),
            31 => self.handle_net_receive(cpu),
            40 => self.handle_set_timer(cpu),
            50 => self.handle_inject_interrupt(cpu),
            _ => Err(HypercallError::InvalidHypercall),
        }
    }

    /// Handle shutdown hypercall
    fn handle_shutdown(&mut self, cpu: &mut CpuEmulator) -> HypercallResult {
        log::info!("VM shutdown requested");
        // Return success
        cpu.state.write_gpr(Register::Rax, 0);
        Ok(0)
    }

    /// Handle reboot hypercall
    fn handle_reboot(&mut self, cpu: &mut CpuEmulator) -> HypercallResult {
        log::info!("VM reboot requested");
        cpu.state.write_gpr(Register::Rax, 0);
        Ok(0)
    }

    /// Handle exit hypercall
    fn handle_exit(&mut self, cpu: &mut CpuEmulator) -> HypercallResult {
        // Get exit reason from RBX
        let reason = cpu.state.read_gpr(Register::Rbx);
        log::info!("VM exit requested: reason={}", reason);

        cpu.state.write_gpr(Register::Rax, 0);
        Ok(0)
    }

    /// Handle get info hypercall
    fn handle_get_info(&mut self, cpu: &mut CpuEmulator) -> HypercallResult {
        // Get info type from RBX
        let info_type = cpu.state.read_gpr(Register::Rbx);

        match info_type {
            0 => {
                // Hypervisor version
                cpu.state.write_gpr(Register::Rax, 1); // Version major
                cpu.state.write_gpr(Register::Rbx, 0); // Version minor
                Ok(0)
            }
            1 => {
                // Memory info
                cpu.state.write_gpr(Register::Rax, 512 * 1024 * 1024); // Total memory
                cpu.state.write_gpr(Register::Rbx, 256 * 1024 * 1024); // Free memory
                Ok(0)
            }
            2 => {
                // CPU info
                cpu.state.write_gpr(Register::Rax, 1); // Number of CPUs
                Ok(0)
            }
            _ => Err(HypercallError::InvalidParameter),
        }
    }

    /// Handle allocate memory hypercall
    fn handle_alloc_mem(&mut self, cpu: &mut CpuEmulator) -> HypercallResult {
        // RBX: size
        let size = cpu.state.read_gpr(Register::Rbx);

        // In a real implementation, this would allocate memory in the VM
        // For now, return a fake address
        let addr = 0x10000000 + size;

        cpu.state.write_gpr(Register::Rax, addr);
        Ok(addr)
    }

    /// Handle free memory hypercall
    fn handle_free_mem(&mut self, cpu: &mut CpuEmulator) -> HypercallResult {
        // RBX: address to free
        let _addr = cpu.state.read_gpr(Register::Rbx);

        // In a real implementation, this would free the memory
        cpu.state.write_gpr(Register::Rax, 0);
        Ok(0)
    }

    /// Handle console write hypercall
    fn handle_console_write(&mut self, cpu: &mut CpuEmulator) -> HypercallResult {
        // RBX: address of string
        // RCX: length
        let addr = cpu.state.read_gpr(Register::Rbx) as usize;
        let len = cpu.state.read_gpr(Register::Rcx) as usize;

        // Read string from memory (simplified - assumes direct access)
        // In reality, would use memory management system
        let message = format!("[hypercall console write: {} bytes at 0x{:x}]", len, addr);
        println!("{}", message);

        cpu.state.write_gpr(Register::Rax, len as u64);
        Ok(len as u64)
    }

    /// Handle console read hypercall
    fn handle_console_read(&mut self, cpu: &mut CpuEmulator) -> HypercallResult {
        // RBX: address to store string
        // RCX: max length
        let _addr = cpu.state.read_gpr(Register::Rbx) as usize;
        let _max_len = cpu.state.read_gpr(Register::Rcx) as usize;

        // In a real implementation, would read from console
        // For now, return 0 (no data)
        cpu.state.write_gpr(Register::Rax, 0);
        Ok(0)
    }

    /// Handle block read hypercall
    fn handle_block_read(&mut self, cpu: &mut CpuEmulator) -> HypercallResult {
        // RBX: device ID
        // RCX: sector number
        // RDX: address to store data
        // R8: number of sectors
        let device = cpu.state.read_gpr(Register::Rbx);
        let sector = cpu.state.read_gpr(Register::Rcx);
        let _addr = cpu.state.read_gpr(Register::Rdx);
        let count = cpu.state.read_gpr(Register::R8);

        log::debug!(
            "Block read: device={}, sector={}, count={}",
            device,
            sector,
            count
        );

        // Return number of sectors read
        cpu.state.write_gpr(Register::Rax, count);
        Ok(count)
    }

    /// Handle block write hypercall
    fn handle_block_write(&mut self, cpu: &mut CpuEmulator) -> HypercallResult {
        // RBX: device ID
        // RCX: sector number
        // RDX: address of data
        // R8: number of sectors
        let device = cpu.state.read_gpr(Register::Rbx);
        let sector = cpu.state.read_gpr(Register::Rcx);
        let _addr = cpu.state.read_gpr(Register::Rdx);
        let count = cpu.state.read_gpr(Register::R8);

        log::debug!(
            "Block write: device={}, sector={}, count={}",
            device,
            sector,
            count
        );

        cpu.state.write_gpr(Register::Rax, count);
        Ok(count)
    }

    /// Handle network send hypercall
    fn handle_net_send(&mut self, cpu: &mut CpuEmulator) -> HypercallResult {
        // RBX: address of data
        // RCX: length
        let _addr = cpu.state.read_gpr(Register::Rbx) as usize;
        let len = cpu.state.read_gpr(Register::Rcx) as usize;

        log::debug!("Network send: {} bytes", len);

        cpu.state.write_gpr(Register::Rax, len as u64);
        Ok(len as u64)
    }

    /// Handle network receive hypercall
    fn handle_net_receive(&mut self, cpu: &mut CpuEmulator) -> HypercallResult {
        // RBX: address to store data
        // RCX: max length
        let _addr = cpu.state.read_gpr(Register::Rbx) as usize;
        let _max_len = cpu.state.read_gpr(Register::Rcx) as usize;

        // Return 0 (no data available)
        cpu.state.write_gpr(Register::Rax, 0);
        Ok(0)
    }

    /// Handle set timer hypercall
    fn handle_set_timer(&mut self, cpu: &mut CpuEmulator) -> HypercallResult {
        // RBX: timer interval in microseconds
        let interval = cpu.state.read_gpr(Register::Rbx);

        self.timer_interval = interval;

        log::debug!("Timer set: {} microseconds", interval);

        cpu.state.write_gpr(Register::Rax, 0);
        Ok(0)
    }

    /// Handle inject interrupt hypercall
    fn handle_inject_interrupt(&mut self, cpu: &mut CpuEmulator) -> HypercallResult {
        // RBX: interrupt number
        let irq = cpu.state.read_gpr(Register::Rbx) as u8;

        log::debug!("Inject interrupt: {}", irq);

        cpu.state.write_gpr(Register::Rax, 0);
        Ok(0)
    }
}

impl Default for HypercallHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a hypercall
pub struct Hypercall;

impl Hypercall {
    /// Create a hypercall request
    pub fn request(number: HypercallNumber, _args: &[u64]) -> u64 {
        // In a real implementation, this would trigger a hypercall
        // For now, return the hypercall number
        number.to_u64()
    }

    /// Parse hypercall number from raw value
    pub fn from_raw(value: u32) -> HypercallNumber {
        match value {
            0 => HypercallNumber::Shutdown,
            1 => HypercallNumber::Reboot,
            2 => HypercallNumber::Exit,
            3 => HypercallNumber::GetInfo,
            4 => HypercallNumber::AllocMem,
            5 => HypercallNumber::FreeMem,
            10 => HypercallNumber::ConsoleWrite,
            11 => HypercallNumber::ConsoleRead,
            20 => HypercallNumber::BlockRead,
            21 => HypercallNumber::BlockWrite,
            30 => HypercallNumber::NetSend,
            31 => HypercallNumber::NetReceive,
            40 => HypercallNumber::SetTimer,
            50 => HypercallNumber::InjectInterrupt,
            100 => HypercallNumber::GetFbPtr,
            101 => HypercallNumber::GetInputPtr,
            n => HypercallNumber::Custom(n),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hypercall_handler() {
        let handler = HypercallHandler::new();
        assert_eq!(handler.timer_interval, 0);
    }

    #[test]
    fn test_hypercall_numbers() {
        assert_eq!(HypercallNumber::Shutdown.to_u32(), 0);
        assert_eq!(HypercallNumber::ConsoleWrite.to_u32(), 10);
    }
}
