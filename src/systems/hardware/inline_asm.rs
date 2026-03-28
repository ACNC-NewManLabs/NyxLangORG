//! Inline Assembly Module - For Nyx compilation pipeline
//!
//! This module provides inline assembly support for the Nyx compiler.
//! These functions are evaluated at IR generation phase, not host execution.
//! The actual assembly is embedded into compiled code during compilation.

#[macro_export]
macro_rules! asm {
    ($($tokens:tt)*) => {
        // This macro is handled at compile-time by the Nyx compiler
        // It extracts the assembly string and embeds it into the generated code
        // compile_error!("Inline assembly must be processed by the Nyx compiler, not the runtime")
    };
}

pub mod reg {
    /// Marker trait for input registers
    pub trait InputReg: Copy + 'static {}
    /// Marker trait for output registers
    pub trait OutputReg: Copy + 'static {}
    
    // Common register types
    #[derive(Debug, Clone, Copy)]
    pub struct Rax;
    #[derive(Debug, Clone, Copy)]
    pub struct Rbx;
    #[derive(Debug, Clone, Copy)]
    pub struct Rcx;
    #[derive(Debug, Clone, Copy)]
    pub struct Rdx;
    #[derive(Debug, Clone, Copy)]
    pub struct Rsi;
    #[derive(Debug, Clone, Copy)]
    pub struct Rdi;
    #[derive(Debug, Clone, Copy)]
    pub struct Rbp;
    #[derive(Debug, Clone, Copy)]
    pub struct Rsp;
    #[derive(Debug, Clone, Copy)]
    pub struct R8;
    #[derive(Debug, Clone, Copy)]
    pub struct R9;
    #[derive(Debug, Clone, Copy)]
    pub struct R10;
    #[derive(Debug, Clone, Copy)]
    pub struct R11;
    #[derive(Debug, Clone, Copy)]
    pub struct R12;
    #[derive(Debug, Clone, Copy)]
    pub struct R13;
    #[derive(Debug, Clone, Copy)]
    pub struct R14;
    #[derive(Debug, Clone, Copy)]
    pub struct R15;
    
    impl InputReg for Rax {}
    impl InputReg for Rbx {}
    impl InputReg for Rcx {}
    impl InputReg for Rdx {}
    impl InputReg for Rsi {}
    impl InputReg for Rdi {}
    impl InputReg for Rbp {}
    impl InputReg for Rsp {}
    impl InputReg for R8 {}
    impl InputReg for R9 {}
    impl InputReg for R10 {}
    impl InputReg for R11 {}
    impl InputReg for R12 {}
    impl InputReg for R13 {}
    impl InputReg for R14 {}
    impl InputReg for R15 {}
    
    impl OutputReg for Rax {}
    impl OutputReg for Rbx {}
    impl OutputReg for Rcx {}
    impl OutputReg for Rdx {}
    impl OutputReg for Rsi {}
    impl OutputReg for Rdi {}
    impl OutputReg for Rbp {}
    impl OutputReg for Rsp {}
    impl OutputReg for R8 {}
    impl OutputReg for R9 {}
    impl OutputReg for R10 {}
    impl OutputReg for R11 {}
    impl OutputReg for R12 {}
    impl OutputReg for R13 {}
    impl OutputReg for R14 {}
    impl OutputReg for R15 {}
}

/// Assembly options for inline assembly
#[derive(Debug, Clone, Copy, Default)]
pub struct AsmOptions {
    pub volatile: bool,
    pub pure: bool,
    pub noreturn: bool,
    pub align_stack: bool,
}

impl AsmOptions {
    pub fn volatile(mut self) -> Self { self.volatile = true; self }
    pub fn pure(mut self) -> Self { self.pure = true; self }
    pub fn noreturn(mut self) -> Self { self.noreturn = true; self }
    pub fn align_stack(mut self) -> Self { self.align_stack = true; self }
}

pub mod routines {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    use core::arch;
    
    /// Disable CPU interrupts (privileged operation)
    /// # Safety
    /// This is a privileged CPU operation that requires ring 0 access
    pub unsafe fn disable_interrupts() { 
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            arch::asm!("cli", options(nomem, nostack, preserves_flags));
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        {
            // On non-x86 targets there is no standard user-mode interrupt flag control.
        }
    }
    
    /// Enable CPU interrupts (privileged operation)
    /// # Safety
    /// This is a privileged CPU operation that requires ring 0 access
    pub unsafe fn enable_interrupts() { 
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            arch::asm!("sti", options(nomem, nostack, preserves_flags));
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        {
            // No-op on non-x86 targets.
        }
    }
    
    /// Halt the CPU (privileged operation)
    /// # Safety
    /// This is a privileged CPU operation that requires ring 0 access
    pub unsafe fn halt() { 
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            arch::asm!("hlt", options(nomem, nostack, preserves_flags));
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        {
            std::thread::park();
        }
    }
    
    /// Get instruction pointer
    /// # Safety
    /// Reading IP in userspace is not directly possible
    pub unsafe fn get_ip() -> usize { 
        #[cfg(target_arch = "x86_64")]
        {
            let rip: usize;
            arch::asm!("lea {0}, [rip + 0]", out(reg) rip, options(nomem, nostack, preserves_flags));
            return rip;
        }
        #[cfg(target_arch = "x86")]
        {
            let eip: usize;
            arch::asm!(
                "call 1f",
                "1: pop {0}",
                out(reg) eip,
                options(nomem, preserves_flags)
            );
            return eip;
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        {
            get_ip as usize
        }
    }
    
    /// Get stack pointer
    pub fn get_sp() -> usize {
        #[cfg(target_arch = "x86_64")]
        {
            let sp: usize;
            unsafe { arch::asm!("mov {0}, rsp", out(reg) sp, options(nomem, preserves_flags)); }
            return sp;
        }
        #[cfg(target_arch = "x86")]
        {
            let sp: usize;
            unsafe { arch::asm!("mov {0}, esp", out(reg) sp, options(nomem, preserves_flags)); }
            return sp;
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        {
            let local = 0u8;
            &local as *const u8 as usize
        }
    }
    
    /// Get frame pointer
    pub fn get_fp() -> usize {
        #[cfg(target_arch = "x86_64")]
        {
            let fp: usize;
            unsafe { arch::asm!("mov {0}, rbp", out(reg) fp, options(nomem, preserves_flags)); }
            return fp;
        }
        #[cfg(target_arch = "x86")]
        {
            let fp: usize;
            unsafe { arch::asm!("mov {0}, ebp", out(reg) fp, options(nomem, preserves_flags)); }
            return fp;
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        {
            let local = 0u8;
            &local as *const u8 as usize
        }
    }
    
    /// CPU pause instruction (for spin-wait loops)
    pub fn pause() {
        std::hint::spin_loop();
    }
    
    /// Memory fence - load
    pub fn mfence() {
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
    }
    
    /// Memory fence - read
    pub fn rfence() {
        // On x86, read fence is a no-op since x86 has strong memory ordering
        // But we provide the instruction for code clarity
        core::sync::atomic::fence(core::sync::atomic::Ordering::Acquire);
    }
    
    /// Memory fence - write
    pub fn wfence() {
        core::sync::atomic::fence(core::sync::atomic::Ordering::Release);
    }
    
    /// Get CPU identifier string
    pub fn cpu_id() -> String { 
        #[cfg(target_arch = "x86")]
        {
            use std::arch::x86::__cpuid;
            let r = unsafe { __cpuid(0) };
            let bytes = [
                r.ebx.to_le_bytes(),
                r.edx.to_le_bytes(),
                r.ecx.to_le_bytes(),
            ]
            .concat();
            let vendor = String::from_utf8_lossy(&bytes).trim_matches('\0').to_string();
            return if vendor.is_empty() { "unknown".to_string() } else { vendor };
        }

        #[cfg(target_arch = "x86_64")]
        {
            use std::arch::x86_64::__cpuid;
            let r = unsafe { __cpuid(0) };
            let bytes = [
                r.ebx.to_le_bytes(),
                r.edx.to_le_bytes(),
                r.ecx.to_le_bytes(),
            ]
            .concat();
            let vendor = String::from_utf8_lossy(&bytes).trim_matches('\0').to_string();
            return if vendor.is_empty() { "unknown".to_string() } else { vendor };
        }

        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        {
            "unknown".to_string()
        }
    }
    
    /// Check if running in 64-bit mode
    pub fn is_64bit() -> bool { 
        cfg!(target_pointer_width = "64")
    }
}

pub mod bit_ops {
    /// Count leading zeros
    /// # Safety
    /// Requires valid 32-bit input
    pub unsafe fn clz(x: u32) -> u32 { 
        x.leading_zeros() 
    }
    
    /// Count trailing zeros
    /// # Safety
    /// Requires valid 32-bit input
    pub unsafe fn ctz(x: u32) -> u32 { 
        x.trailing_zeros() 
    }
    
    /// Population count (count set bits)
    /// # Safety
    /// Requires valid 32-bit input
    pub unsafe fn popcnt(x: u32) -> u32 { 
        x.count_ones() 
    }
    
    /// Byte swap (reverse endianness)
    /// # Safety
    /// Requires valid 32-bit input
    pub unsafe fn bswap(x: u32) -> u32 { 
        x.swap_bytes() 
    }
}

pub mod io {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    use core::arch;

    /// Read from input port
    /// # Safety
    /// This is a privileged I/O operation requiring port I/O permissions
    pub unsafe fn inb(port: u16) -> u8 { 
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            let value: u8;
            arch::asm!(
                "in al, dx",
                out("al") value,
                in("dx") port,
                options(nomem, nostack, preserves_flags)
            );
            return value;
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        {
            let _ = port;
            0
        }
    }
    
    /// Write to output port
    /// # Safety
    /// This is a privileged I/O operation requiring port I/O permissions
    pub unsafe fn outb(port: u16, value: u8) { 
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            arch::asm!(
                "out dx, al",
                in("dx") port,
                in("al") value,
                options(nomem, nostack, preserves_flags)
            );
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        {
            let _ = (port, value);
        }
    }
    
    /// Read 16-bit from input port
    /// # Safety
    /// This is a privileged I/O operation requiring port I/O permissions
    pub unsafe fn inw(port: u16) -> u16 { 
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            let value: u16;
            arch::asm!(
                "in ax, dx",
                out("ax") value,
                in("dx") port,
                options(nomem, nostack, preserves_flags)
            );
            return value;
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        {
            let _ = port;
            0
        }
    }
    
    /// Write 16-bit to output port
    /// # Safety
    /// This is a privileged I/O operation requiring port I/O permissions
    pub unsafe fn outw(port: u16, value: u16) { 
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            arch::asm!(
                "out dx, ax",
                in("dx") port,
                in("ax") value,
                options(nomem, nostack, preserves_flags)
            );
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        {
            let _ = (port, value);
        }
    }
    
    /// Read 32-bit from input port
    /// # Safety
    /// This is a privileged I/O operation requiring port I/O permissions
    pub unsafe fn inl(port: u16) -> u32 { 
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            let value: u32;
            arch::asm!(
                "in eax, dx",
                out("eax") value,
                in("dx") port,
                options(nomem, nostack, preserves_flags)
            );
            return value;
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        {
            let _ = port;
            0
        }
    }
    
    /// Write 32-bit to output port
    /// # Safety
    /// This is a privileged I/O operation requiring port I/O permissions
    pub unsafe fn outl(port: u16, value: u32) { 
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            arch::asm!(
                "out dx, eax",
                in("dx") port,
                in("eax") value,
                options(nomem, nostack, preserves_flags)
            );
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        {
            let _ = (port, value);
        }
    }
}
