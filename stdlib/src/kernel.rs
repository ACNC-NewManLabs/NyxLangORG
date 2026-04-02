//! NYX Kernel Development Standard Library [Layer 42]
//!
//! Pre-built modules for OS/kernel authors. Import these instead of
//! implementing hardware drivers yourself.
//!
//! ## Modules
//! - `keyboard`  — PS/2 & USB HID key codes, event decoding
//! - `mouse`     — Button state, delta movement, scroll
//! - `vga`       — VGA text-mode color constants & cell helpers  
//! - `serial`    — COM port constants for early kernel debug output
//! - `ports`     — x86 I/O port numbers (PIC, PIT, PS/2, COM, VGA)
//! - `memory`    — Physical page size constants & layout helpers
//! - `interrupts`— x86 IRQ / exception vector constants

/// Keyboard scan-code tables and key constants
pub mod keyboard {
    /// Standard HID / PS/2 key codes (USB HID Usage Page 0x07)
    pub struct KeyCode;
    impl KeyCode {
        // --- Control ---
        pub const ESCAPE: u8 = 0x01;
        pub const BACKSPACE: u8 = 0x0E;
        pub const TAB: u8 = 0x0F;
        pub const ENTER: u8 = 0x1C;
        pub const LCTRL: u8 = 0x1D;
        pub const LSHIFT: u8 = 0x2A;
        pub const RSHIFT: u8 = 0x36;
        pub const LALT: u8 = 0x38;
        pub const SPACE: u8 = 0x39;
        pub const CAPSLOCK: u8 = 0x3A;
        pub const NUMLOCK: u8 = 0x45;
        pub const SCROLLLOCK: u8 = 0x46;
        // --- Function keys ---
        pub const F1: u8 = 0x3B;
        pub const F2: u8 = 0x3C;
        pub const F3: u8 = 0x3D;
        pub const F4: u8 = 0x3E;
        pub const F5: u8 = 0x3F;
        pub const F6: u8 = 0x40;
        pub const F7: u8 = 0x41;
        pub const F8: u8 = 0x42;
        pub const F9: u8 = 0x43;
        pub const F10: u8 = 0x44;
        pub const F11: u8 = 0x57;
        pub const F12: u8 = 0x58;
        // --- Arrow keys (extended) ---
        pub const UP: u8 = 0x48;
        pub const DOWN: u8 = 0x50;
        pub const LEFT: u8 = 0x4B;
        pub const RIGHT: u8 = 0x4D;
        // --- Alphanumeric ---
        pub const A: u8 = 0x1E;
        pub const B: u8 = 0x30;
        pub const C: u8 = 0x2E;
        pub const D: u8 = 0x20;
        pub const E: u8 = 0x12;
        pub const F: u8 = 0x21;
        pub const G: u8 = 0x22;
        pub const H: u8 = 0x23;
        pub const I: u8 = 0x17;
        pub const J: u8 = 0x24;
        pub const K: u8 = 0x25;
        pub const L: u8 = 0x26;
        pub const M: u8 = 0x32;
        pub const N: u8 = 0x31;
        pub const O: u8 = 0x18;
        pub const P: u8 = 0x19;
        pub const Q: u8 = 0x10;
        pub const R: u8 = 0x13;
        pub const S: u8 = 0x1F;
        pub const T: u8 = 0x14;
        pub const U: u8 = 0x16;
        pub const V: u8 = 0x2F;
        pub const W: u8 = 0x11;
        pub const X: u8 = 0x2D;
        pub const Y: u8 = 0x15;
        pub const Z: u8 = 0x2C;
        pub const N0: u8 = 0x0B;
        pub const N1: u8 = 0x02;
        pub const N2: u8 = 0x03;
        pub const N3: u8 = 0x04;
        pub const N4: u8 = 0x05;
        pub const N5: u8 = 0x06;
        pub const N6: u8 = 0x07;
        pub const N7: u8 = 0x08;
        pub const N8: u8 = 0x09;
        pub const N9: u8 = 0x0A;
    }

    /// Map a PS/2 scan code (Set 1) to an ASCII character (uppercase).
    /// Returns `None` for non-printable keys.
    pub fn scancode_to_ascii(sc: u8) -> Option<char> {
        let table: &[(u8, char)] = &[
            (0x02, '1'),
            (0x03, '2'),
            (0x04, '3'),
            (0x05, '4'),
            (0x06, '5'),
            (0x07, '6'),
            (0x08, '7'),
            (0x09, '8'),
            (0x0A, '9'),
            (0x0B, '0'),
            (0x10, 'q'),
            (0x11, 'w'),
            (0x12, 'e'),
            (0x13, 'r'),
            (0x14, 't'),
            (0x15, 'y'),
            (0x16, 'u'),
            (0x17, 'i'),
            (0x18, 'o'),
            (0x19, 'p'),
            (0x1A, '['),
            (0x1B, ']'),
            (0x1C, '\n'),
            (0x1E, 'a'),
            (0x1F, 's'),
            (0x20, 'd'),
            (0x21, 'f'),
            (0x22, 'g'),
            (0x23, 'h'),
            (0x24, 'j'),
            (0x25, 'k'),
            (0x26, 'l'),
            (0x27, ';'),
            (0x28, '\''),
            (0x29, '`'),
            (0x2B, '\\'),
            (0x2C, 'z'),
            (0x2D, 'x'),
            (0x2E, 'c'),
            (0x2F, 'v'),
            (0x30, 'b'),
            (0x31, 'n'),
            (0x32, 'm'),
            (0x33, ','),
            (0x34, '.'),
            (0x35, '/'),
            (0x39, ' '),
            (0x0C, '-'),
            (0x0D, '='),
        ];
        table
            .iter()
            .find(|(code, _)| *code == sc)
            .map(|(_, ch)| *ch)
    }

    /// A keyboard event record
    #[derive(Debug, Clone)]
    pub struct KeyEvent {
        pub scancode: u8,
        pub pressed: bool,
        pub ascii: Option<char>,
    }

    impl KeyEvent {
        pub fn new(scancode: u8, pressed: bool) -> Self {
            let ascii = scancode_to_ascii(scancode);
            Self {
                scancode,
                pressed,
                ascii,
            }
        }
    }
}

/// Mouse state and button constants
pub mod mouse {
    #[derive(Debug, Clone, Default)]
    pub struct MouseState {
        pub delta_x: i16,
        pub delta_y: i16,
        pub delta_wheel: i8,
        pub left_button: bool,
        pub right_button: bool,
        pub middle_button: bool,
    }

    pub struct MouseButton;
    impl MouseButton {
        pub const LEFT: u8 = 0;
        pub const RIGHT: u8 = 1;
        pub const MIDDLE: u8 = 2;
    }

    impl MouseState {
        /// Decode a 3-byte PS/2 mouse data packet
        pub fn from_ps2_packet(b0: u8, b1: u8, b2: u8) -> Self {
            let dx = b1 as i16 - (((b0 as i16) << 4) & 0x100);
            let dy = b2 as i16 - (((b0 as i16) << 3) & 0x100);
            Self {
                delta_x: dx,
                delta_y: dy,
                delta_wheel: 0,
                left_button: (b0 & 0x01) != 0,
                right_button: (b0 & 0x02) != 0,
                middle_button: (b0 & 0x04) != 0,
            }
        }
    }
}

/// VGA text-mode color constants and cell helpers
pub mod vga {
    #[repr(u8)]
    #[derive(Copy, Clone, Debug)]
    pub enum Color {
        Black = 0,
        Blue = 1,
        Green = 2,
        Cyan = 3,
        Red = 4,
        Magenta = 5,
        Brown = 6,
        LightGray = 7,
        DarkGray = 8,
        LightBlue = 9,
        LightGreen = 10,
        LightCyan = 11,
        LightRed = 12,
        Pink = 13,
        Yellow = 14,
        White = 15,
    }

    /// Create a VGA color attribute byte (foreground | background<<4)
    pub fn color_code(fg: Color, bg: Color) -> u8 {
        (fg as u8) | ((bg as u8) << 4)
    }

    /// Create a VGA text-mode cell (char | color_byte<<8)
    pub fn make_cell(ch: u8, fg: Color, bg: Color) -> u16 {
        (ch as u16) | ((color_code(fg, bg) as u16) << 8)
    }

    pub const VGA_BUFFER_ADDR: usize = 0xB8000;
    pub const VGA_WIDTH: usize = 80;
    pub const VGA_HEIGHT: usize = 25;
}

/// Standard serial/UART COM port addresses
pub mod serial {
    pub const COM1: u16 = 0x3F8;
    pub const COM2: u16 = 0x2F8;
    pub const COM3: u16 = 0x3E8;
    pub const COM4: u16 = 0x2E8;

    // UART register offsets
    pub const UART_DATA: u16 = 0; // Data register
    pub const UART_IER: u16 = 1; // Interrupt enable
    pub const UART_FCR: u16 = 2; // FIFO control
    pub const UART_LCR: u16 = 3; // Line control
    pub const UART_MCR: u16 = 4; // Modem control
    pub const UART_LSR: u16 = 5; // Line status
    pub const LSR_TX_IDLE: u8 = 0x20;
    pub const LSR_RX_READY: u8 = 0x01;
}

/// x86 I/O port numbers (PIC, PIT, PS/2, COM, VGA, PCI)
pub mod ports {
    // PIC (Programmable Interrupt Controller)
    pub const PIC1_CMD: u16 = 0x0020;
    pub const PIC1_DATA: u16 = 0x0021;
    pub const PIC2_CMD: u16 = 0x00A0;
    pub const PIC2_DATA: u16 = 0x00A1;
    pub const PIC_EOI: u8 = 0x20;

    // PIT (Programmable Interval Timer)
    pub const PIT_CHANNEL0: u16 = 0x0040;
    pub const PIT_CHANNEL1: u16 = 0x0041;
    pub const PIT_CHANNEL2: u16 = 0x0042;
    pub const PIT_CMD: u16 = 0x0043;
    pub const PIT_BASE_HZ: u32 = 1_193_182;

    // PS/2 Controller
    pub const PS2_DATA: u16 = 0x0060;
    pub const PS2_CMD: u16 = 0x0064;
    pub const PS2_STATUS: u16 = 0x0064;
    pub const PS2_OUTPUT_FULL: u8 = 0x01;
    pub const PS2_INPUT_FULL: u8 = 0x02;
    pub const PS2_CMD_DISABLE_PORT1: u8 = 0xAD;
    pub const PS2_CMD_ENABLE_PORT1: u8 = 0xAE;
    pub const PS2_CMD_DISABLE_PORT2: u8 = 0xA7;
    pub const PS2_CMD_ENABLE_PORT2: u8 = 0xA8;
    pub const PS2_CMD_TEST_CTRL: u8 = 0xAA;
    pub const PS2_CMD_TEST_PORT1: u8 = 0xAB;

    // VGA registers
    pub const VGA_SEQ_INDEX: u16 = 0x03C4;
    pub const VGA_SEQ_DATA: u16 = 0x03C5;
    pub const VGA_MISC_WRITE: u16 = 0x03C2;
    pub const VGA_CRTC_INDEX: u16 = 0x03D4;
    pub const VGA_CRTC_DATA: u16 = 0x03D5;

    // CMOS / RTC
    pub const CMOS_CMD: u16 = 0x0070;
    pub const CMOS_DATA: u16 = 0x0071;
}

/// Physical memory layout constants
pub mod memory {
    pub const PAGE_SIZE_4K: usize = 4096;
    pub const PAGE_SIZE_2M: usize = 2 * 1024 * 1024;
    pub const PAGE_SIZE_1G: usize = 1024 * 1024 * 1024;

    /// Align an address up to the nearest page boundary
    pub fn align_up(addr: usize, align: usize) -> usize {
        (addr + align - 1) & !(align - 1)
    }

    /// Align an address down to a page boundary
    pub fn align_down(addr: usize, align: usize) -> usize {
        addr & !(align - 1)
    }

    /// Return how many 4K pages are needed to cover `bytes`
    pub fn pages_needed(bytes: usize) -> usize {
        align_up(bytes, PAGE_SIZE_4K) / PAGE_SIZE_4K
    }

    // Well-known physical memory regions (classic x86)
    pub const REAL_MODE_IVT: usize = 0x0000_0000; // 0x000 - 0x3FF  Interrupt Vector Table
    pub const BIOS_DATA_AREA: usize = 0x0000_0400; // 0x400 - 0x4FF
    pub const CONVENTIONAL_RAM: usize = 0x0000_0500; // 0x500 - 0x7BFF
    pub const VGA_TEXT_BUFFER: usize = 0x000B_8000; // Text mode framebuffer
    pub const BIOS_ROM: usize = 0x000C_0000; // ROM extension area
    pub const HIGH_MEMORY_START: usize = 0x0010_0000; // 1MB+ usable RAM
}

/// x86 IRQ and exception vector numbers
pub mod interrupts {
    // CPU exception vectors
    pub const EX_DIVIDE_BY_ZERO: u8 = 0;
    pub const EX_DEBUG: u8 = 1;
    pub const EX_NMI: u8 = 2;
    pub const EX_BREAKPOINT: u8 = 3;
    pub const EX_OVERFLOW: u8 = 4;
    pub const EX_BOUND_RANGE: u8 = 5;
    pub const EX_INVALID_OPCODE: u8 = 6;
    pub const EX_DEVICE_NOT_AVAILABLE: u8 = 7;
    pub const EX_DOUBLE_FAULT: u8 = 8;
    pub const EX_INVALID_TSS: u8 = 10;
    pub const EX_SEGMENT_NOT_PRESENT: u8 = 11;
    pub const EX_STACK_FAULT: u8 = 12;
    pub const EX_GENERAL_PROTECTION: u8 = 13;
    pub const EX_PAGE_FAULT: u8 = 14;
    pub const EX_FPU_EXCEPTION: u8 = 16;
    pub const EX_ALIGNMENT_CHECK: u8 = 17;
    pub const EX_MACHINE_CHECK: u8 = 18;
    pub const EX_SIMD_EXCEPTION: u8 = 19;

    // Hardware IRQ lines (after PIC remapping to 0x20+)
    pub const IRQ0_TIMER: u8 = 32; // PIT
    pub const IRQ1_KEYBOARD: u8 = 33; // PS/2 Keyboard
    pub const IRQ3_COM2: u8 = 35;
    pub const IRQ4_COM1: u8 = 36;
    pub const IRQ8_RTC: u8 = 40; // Real-Time Clock
    pub const IRQ12_MOUSE: u8 = 44; // PS/2 Mouse
    pub const IRQ14_ATA0: u8 = 46; // Primary ATA
    pub const IRQ15_ATA1: u8 = 47; // Secondary ATA
}

/// Global Descriptor Table (GDT) and Segment Selectors
pub mod gdt {
    pub const ACCESS_PR: u8 = 0b10000000; // Present
    pub const ACCESS_PRIV_RING0: u8 = 0b00000000;
    pub const ACCESS_PRIV_RING3: u8 = 0b01100000;
    pub const ACCESS_EX: u8 = 0b00001000; // Executable
    pub const ACCESS_DC: u8 = 0b00000100; // Direction/Conforming
    pub const ACCESS_RW: u8 = 0b00000010; // Read/Write
    pub const ACCESS_AC: u8 = 0b00000001; // Accessed

    // x86_64 segment flags
    pub const FLAG_GR: u8 = 0b10000000; // Page granularity
    pub const FLAG_SZ: u8 = 0b01000000; // Size (0 for 64-bit)
    pub const FLAG_L: u8 = 0b00100000; // Long mode (64-bit)

    /// Build a 64-bit GDT entry
    pub fn build_entry(base: u32, limit: u32, access: u8, flags: u8) -> u64 {
        let mut entry: u64 = 0;
        entry |= (limit as u64) & 0xFFFF;
        entry |= ((base as u64) & 0xFFFFFF) << 16;
        entry |= (access as u64) << 40;
        entry |= (((limit as u64) >> 16) & 0x0F) << 48;
        entry |= (flags as u64) << 52;
        entry |= (((base as u64) >> 24) & 0xFF) << 56;
        entry
    }
}

/// Interrupt Descriptor Table (IDT) Gate Types
pub mod idt {
    pub const ATTR_PRESENT: u8 = 0b1000_0000;
    pub const ATTR_RING0: u8 = 0b0000_0000;
    pub const ATTR_RING3: u8 = 0b0110_0000;
    pub const ATTR_INT_GATE: u8 = 0b0000_1110; // 32-bit/64-bit Interrupt gate
    pub const ATTR_TRAP_GATE: u8 = 0b0000_1111; // 32-bit/64-bit Trap gate
}

/// x86_64 Paging Structures (PML4, PDPT, PD, PT)
pub mod paging {
    pub const FLAG_PRESENT: u64 = 1 << 0;
    pub const FLAG_WRITABLE: u64 = 1 << 1;
    pub const FLAG_USER: u64 = 1 << 2;
    pub const FLAG_WRITE_THROUGH: u64 = 1 << 3;
    pub const FLAG_NO_CACHE: u64 = 1 << 4;
    pub const FLAG_ACCESSED: u64 = 1 << 5;
    pub const FLAG_DIRTY: u64 = 1 << 6;
    pub const FLAG_HUGE_PAGE: u64 = 1 << 7;
    pub const FLAG_GLOBAL: u64 = 1 << 8;
    pub const FLAG_NO_EXECUTE: u64 = 1 << 63;

    pub const PAGE_MASK_4K: u64 = 0x000F_FFFF_FFFF_F000;
}

/// Peripheral Component Interconnect (PCI) Configuration
pub mod pci {
    pub const CONFIG_ADDRESS: u16 = 0xCF8;
    pub const CONFIG_DATA: u16 = 0xCFC;

    // Offsets within PCI config space
    pub const OFFSET_VENDOR_ID: u8 = 0x00; // 16-bit
    pub const OFFSET_DEVICE_ID: u8 = 0x02; // 16-bit
    pub const OFFSET_COMMAND: u8 = 0x04; // 16-bit
    pub const OFFSET_STATUS: u8 = 0x06; // 16-bit
    pub const OFFSET_REVISION: u8 = 0x08; // 8-bit
    pub const OFFSET_PROG_IF: u8 = 0x09; // 8-bit
    pub const OFFSET_SUBCLASS: u8 = 0x0A; // 8-bit
    pub const OFFSET_CLASS: u8 = 0x0B; // 8-bit
    pub const OFFSET_HEADER_TYPE: u8 = 0x0E; // 8-bit
    pub const OFFSET_BAR0: u8 = 0x10; // 32-bit
    pub const OFFSET_BAR1: u8 = 0x14; // 32-bit
    pub const OFFSET_INTR_LINE: u8 = 0x3C; // 8-bit

    /// Build a 32-bit PCI CONFIG_ADDRESS
    pub fn build_address(bus: u8, slot: u8, func: u8, offset: u8) -> u32 {
        let bus = bus as u32;
        let slot = slot as u32;
        let func = func as u32;
        let offset = (offset & 0xFC) as u32;

        0x80000000 | (bus << 16) | ((slot & 0x1F) << 11) | ((func & 0x07) << 8) | offset
    }
}

/// ACPI (Advanced Configuration and Power Interface)
pub mod acpi {
    // Standard table signatures
    pub const SIG_RSDP: &str = "RSD PTR ";
    pub const SIG_RSDT: &str = "RSDT";
    pub const SIG_XSDT: &str = "XSDT";
    pub const SIG_FADT: &str = "FACP";
    pub const SIG_MADT: &str = "APIC";
    pub const SIG_HPET: &str = "HPET";
    pub const SIG_MCFG: &str = "MCFG";
}

/// Multiboot2 Bootloader Standard
pub mod multiboot2 {
    pub const MAGIC_BOOTLOADER: u32 = 0x36d76289;

    pub const TAG_TYPE_END: u32 = 0;
    pub const TAG_TYPE_CMDLINE: u32 = 1;
    pub const TAG_TYPE_BOOT_LOADER_NAME: u32 = 2;
    pub const TAG_TYPE_MODULE: u32 = 3;
    pub const TAG_TYPE_BASIC_MEMINFO: u32 = 4;
    pub const TAG_TYPE_BOOTDEV: u32 = 5;
    pub const TAG_TYPE_MMAP: u32 = 6;
    pub const TAG_TYPE_VBE: u32 = 7;
    pub const TAG_TYPE_FRAMEBUFFER: u32 = 8;
    pub const TAG_TYPE_ELF_SECTIONS: u32 = 9;
    pub const TAG_TYPE_APM: u32 = 10;
    pub const TAG_TYPE_EFI32: u32 = 11;
    pub const TAG_TYPE_EFI64: u32 = 12;
    pub const TAG_TYPE_SMBIOS: u32 = 13;
    pub const TAG_TYPE_ACPI_OLD: u32 = 14;
    pub const TAG_TYPE_ACPI_NEW: u32 = 15;
    pub const TAG_TYPE_NETWORK: u32 = 16;
}

/// APIC (Advanced Programmable Interrupt Controller)
pub mod apic {
    pub const BASE_MSR: u32 = 0x1B;
    pub const BASE_MSR_ENABLE: u64 = 0x800;

    // MMIO offsets from the APIC Base Address
    pub const REG_ID: u32 = 0x0020;
    pub const REG_VERSION: u32 = 0x0030;
    pub const REG_TPR: u32 = 0x0080; // Task Priority
    pub const REG_EOI: u32 = 0x00B0; // End of Interrupt
    pub const REG_SIV: u32 = 0x00F0; // Spurious Interrupt Vector
    pub const REG_ICR_LOW: u32 = 0x0300; // Interrupt Command Register (Low)
    pub const REG_ICR_HIGH: u32 = 0x0310; // Interrupt Command Register (High)
    pub const REG_LVT_TIMER: u32 = 0x0320;
    pub const REG_TIMER_INIT_COUNT: u32 = 0x0380;
    pub const REG_TIMER_CURR_COUNT: u32 = 0x0390;
    pub const REG_TIMER_DIVIDE: u32 = 0x03E0;

    // Spurious Interrupt Vector flags
    pub const SIV_ENABLE: u32 = 0x100;
}

/// CPUID feature constants and execution macros
pub mod cpuid {
    pub const LEAF_VENDOR_ID: u32 = 0x0000_0000;
    pub const LEAF_FEATURES: u32 = 0x0000_0001;
    pub const LEAF_EXT_FEATURES: u32 = 0x8000_0001;

    // Feature mask bits in EAX / ECX / EDX
    pub const FEAT_EDX_FPU: u32 = 1 << 0;
    pub const FEAT_EDX_APIC: u32 = 1 << 9;
    pub const FEAT_EDX_SSE: u32 = 1 << 25;
    pub const FEAT_EDX_SSE2: u32 = 1 << 26;
    pub const FEAT_ECX_SSE3: u32 = 1 << 0;
    pub const FEAT_ECX_AVX: u32 = 1 << 28;

    pub const EXT_EDX_SYSCALL: u32 = 1 << 11;
    pub const EXT_EDX_NX: u32 = 1 << 20; // No-Execute
    pub const EXT_EDX_LONG_MODE: u32 = 1 << 29; // 64-bit Long Mode
}
