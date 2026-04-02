; src/main.asm
; Nyx BIOS v1.0 — Advanced BIOS
; Supports: Linux, Windows, macOS, any ISO

; PIC defines
%define PIC1_CMD        0x20
%define PIC_EOI         0x20

BITS 16

; ── POST codes (port 0x80) ──────────────────────
; Keep early bring-up debuggable in emulators and with POST card.
%macro POST 1
    push ax
    mov al, %1
    out 0x80, al
    pop ax
%endmacro

; ROM layout (128KiB image):
; - Physical mapping: 0xE0000-0xFFFFF
; - Reset vector fetch: CS=0xF000, IP=0xFFF0 (physical 0xFFFF0)
; This image keeps the lower 64KiB (0xE0000-0xEFFFF) as 0xFF padding and
; places the executable BIOS code in the upper 64KiB (0xF0000-0xFFFFF).

%define ROM_SIZE       0x20000          ; 128KiB
%define ROM_PAD_SIZE   0x10000          ; 64KiB (E0000-EFFFF)
%define ROM_BIOS_OFF   ROM_PAD_SIZE     ; file offset where F000:0000 starts
%define ROM_RESET_OFF  (ROM_SIZE - 0x10)

; ── BIOS Data Area (BDA) at 0x0400 ──────────────
%define BDA_COM1        0x0400  ; COM1 port addr
%define BDA_COM2        0x0402  ; COM2 port addr
%define BDA_EQUIPMENT   0x0410  ; equipment list
%define BDA_MEMSIZE     0x0413  ; base memory KB
%define BDA_KBDFLAG     0x0417  ; keyboard flags
%define BDA_CURPOS      0x0450  ; cursor positions
%define BDA_CURTYPE     0x0460  ; cursor type
%define BDA_VIDEOMODE   0x0449  ; current video mode
%define BDA_COLUMNS     0x044A  ; screen columns
%define BDA_VIDEOSEG    0x044E  ; video page offset
%define BDA_ROWS        0x0484  ; screen rows - 1
%define BDA_HDCOUNT     0x0475  ; hard disk count
%define BDA_BOOTDEV     0x0476  ; boot device
%define BDA_TIMER_LOW   0x046C
%define BDA_TIMER_OVERFLOW 0x0471

; ── EBDA at 0x9FC00 ─────────────────────────────
%define EBDA_SEG        0x9FC0
%define EBDA_SIZE       0x9FC0  ; 1KB

; ── Nyx Scratch RAM (must be writable) ───────────
; Keep small runtime state here (DS=0 physical addresses).
; Avoid 0x0500..0x0510 (used by A20 check), and avoid the BDA (0x0400..).
%define NYX_VAR_BASE                 0x0600
%define NYX_ATAPI2_LAST_STAGE        (NYX_VAR_BASE + 0x00)
%define NYX_ATAPI2_LAST_STATUS       (NYX_VAR_BASE + 0x01)
%define NYX_ATAPI2_LAST_ERROR        (NYX_VAR_BASE + 0x02)
%define NYX_ATAPI1_LAST_STAGE        (NYX_VAR_BASE + 0x03)
%define NYX_ATAPI1_LAST_STATUS       (NYX_VAR_BASE + 0x04)
%define NYX_ATAPI1_LAST_ERROR        (NYX_VAR_BASE + 0x05)
%define NYX_ATAPI2_INIT_DONE         (NYX_VAR_BASE + 0x06)

%define NYX_CDROM_BOOT_CATALOG_LBA   (NYX_VAR_BASE + 0x10) ; dd
%define NYX_CDROM_BOOT_IMAGE_LBA     (NYX_VAR_BASE + 0x14) ; dd
%define NYX_CDROM_BOOT_LOAD_SEG      (NYX_VAR_BASE + 0x18) ; dw
%define NYX_CDROM_BOOT_SECTOR_COUNT  (NYX_VAR_BASE + 0x1A) ; dw
%define NYX_CDROM_BOOT_MEDIA_TYPE    (NYX_VAR_BASE + 0x1C) ; db
%define NYX_BOOT_SELECTED            (NYX_VAR_BASE + 0x1D) ; db: 0=HDD,1=CD,2=Net
%define NYX_BOOT_LAST                (NYX_VAR_BASE + 0x1E) ; db: last booted device
%define NYX_THEME_IDX                (NYX_VAR_BASE + 0x1F) ; db: splash theme 0-2
%define NYX_SERIALSTUDIO_EN          (NYX_VAR_BASE + 0x20) ; db: serial telemetry enable

; Lower half of ROM is unused padding (typical EPROM contents are 0xFF).
SECTION .pad start=0 vstart=0
times ROM_PAD_SIZE db 0xFF

; Main BIOS code lives at file offset 0x10000, but is addressed as F000:0000.
SECTION .bios start=ROM_BIOS_OFF vstart=0x0000

%include "src/serial.asm"
%include "src/pic.asm"
%include "src/pit.asm"
%include "src/keyboard.asm"
%include "src/bda.asm"
%include "src/memory.asm"
%include "src/video.asm"
%include "src/pci.asm"
%include "src/disk.asm"
%include "src/disk_addons.asm"
%include "src/cdrom.asm"
%include "src/rtc.asm"
%include "src/acpi.asm"
%include "src/smbios.asm"
%include "src/dma.asm"
%include "src/boot_menu.asm"
%include "src/cpuid.asm"
%include "src/longmode.asm"
%include "src/option_rom.asm"
%include "src/beep.asm"
%include "src/splash.asm"
; %include "src/pmode.asm"
%include "src/boot.asm"
%include "src/usb.asm"
%include "src/pxe.asm"
%include "src/smp.asm"
%include "src/disk_strings.asm"
%include "src/cdrom_strings.asm"

; ── Entry point ─────────────────────────────────
bios_entry:
    ; Ensure real mode CR0 (hypervisor safety)
    ; Skip CR0 setup (hypervisor handles)
    nop
    
    cli
    cld
    POST 0x01

    ; Setup segments
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    mov sp, 0x7000
    POST 0x02

    ; Initialize serial first for debugging
    POST 0x10
    call serial_init
    mov si, str_banner
    call serial_puts

    ; Initialize PIC (remap IRQs)
    POST 0x11
    call pic_init
    mov si, str_pic_ok
    call serial_puts

    ; PIT stubbed
    POST 0x12
    call pit_init

    ; VGA text mode - brings up display for early debug
    POST 0x13
    call video_init

    ; Setup BIOS Data Area
    POST 0x14
    call bda_ebda_init

    ; Memory map (E820)
    POST 0x15
    call memory_detect

    ; IVT must come before any INT calls
    POST 0x16
    call ivt_init
    mov si, str_ivt_ok
    call serial_puts

    ; Animated splash screen (uses INT 10h, so must be after ivt_init)
    mov si, str_splash_start
    call serial_puts
    call splash_init
    mov si, str_splash_done
    call serial_puts

    ; PCI + ATA disk detect
    POST 0x16
    call pci_init
    mov si, str_ata_detect
    call serial_puts
    call disk_detect
    mov si, str_ata_detect_ok
    call serial_puts
    mov al, 0x08
    call splash_stage
    mov al, 0x09
    call splash_stage
    mov al, 40
    call splash_progress

    ; Keyboard
    POST 0x17
    call kbd_init
    mov si, str_kbd_ok
    call serial_puts
    mov al, 0x0A
    call splash_stage
    mov al, 55
    call splash_progress

    ; ACPI tables
    POST 0x18
    call acpi_init
    mov si, str_acpi_ok
    call serial_puts
    mov al, 0x0B
    call splash_stage
    mov al, 70
    call splash_progress

    ; SMBIOS
    POST 0x19
    call smbios_init
    mov si, str_smbios_ok
    call serial_puts
    mov al, 0x0C
    call splash_stage
    mov al, 82
    call splash_progress

    ; DMA
    POST 0x1A
    call dma_init
    mov si, str_dma_ok
    call serial_puts
    mov al, 0x0D
    call splash_stage
    mov al, 92
    call splash_progress

    ; RTC
    POST 0x1B
    call rtc_init
    mov si, str_rtc_ok
    call serial_puts
    mov al, 0x0E
    call splash_stage
    mov al, 100
    call splash_progress

    ; 2 short beeps: POST complete
    mov al, 0x00
    mov ah, 5
    call beep
    call beep

    ; Transition splash → text mode before boot menu
    call splash_end
    mov si, str_splash_end
    call serial_puts
    
    ; SMBIOS stubbed (removed)

    ; Enable interrupts
    sti

    ; Start boot sequence
    POST 0x18
    mov si, str_boot
    call serial_puts
    call boot_sequence

    ; Should never reach here
    mov si, str_halt
    call serial_puts
    cli
.halt:
    hlt
    jmp .halt

; ── IVT Initialization ──────────────────────────
ivt_init:
    ; Set only needed IVT entries (others default to dummy_isr at F000:dummy_isr after boot)

    ; INT 0x08 — IRQ0 timer
    mov word [0x08*4],   irq0_timer
    mov word [0x08*4+2], 0xF000

    ; INT 0x09 — IRQ1 keyboard
    mov word [0x09*4],   irq1_keyboard
    mov word [0x09*4+2], 0xF000

    ; INT 0x10 — video services
    mov word [0x10*4],   int10_handler
    mov word [0x10*4+2], 0xF000

    ; INT 0x11 — equipment list
    mov word [0x11*4],   int11_handler
    mov word [0x11*4+2], 0xF000

    ; INT 0x12 — memory size
    mov word [0x12*4],   int12_handler
    mov word [0x12*4+2], 0xF000

    ; INT 0x13 — disk services
    mov word [0x13*4],   int13_handler
    mov word [0x13*4+2], 0xF000

    ; INT 0x14 — serial services
    mov word [0x14*4],   int14_handler
    mov word [0x14*4+2], 0xF000

    ; INT 0x15 — misc/E820/A20
    mov word [0x15*4],   int15_handler
    mov word [0x15*4+2], 0xF000

    ; INT 0x16 — keyboard services
    mov word [0x16*4],   int16_handler
    mov word [0x16*4+2], 0xF000

    ; INT 0x17 — printer services
    mov word [0x17*4],   int17_handler
    mov word [0x17*4+2], 0xF000

    ; INT 0x18 — ROM BASIC (boot fail)
    mov word [0x18*4],   int18_handler
    mov word [0x18*4+2], 0xF000

    ; INT 0x19 — bootstrap loader
    mov word [0x19*4],   int19_handler
    mov word [0x19*4+2], 0xF000

    ; INT 0x1A — time/date/PCI
    mov word [0x1A*4],   int1a_handler
    mov word [0x1A*4+2], 0xF000
    
    ; INT 0x77 — Nyx hypervisor extension
    mov word [0x77*4],   int77_handler
    mov word [0x77*4+2], 0xF000

    ; INT 0x1C — user timer hook (called by IRQ0; must point to valid handler)
    mov word [0x1C*4],   dummy_isr
    mov word [0x1C*4+2], 0xF000

    ; INT 0x1D, 0x1E, 0x1F — video/disk parameter tables (safe dummy)
    mov word [0x1D*4],   dummy_isr
    mov word [0x1D*4+2], 0xF000
    mov word [0x1E*4],   dummy_isr
    mov word [0x1E*4+2], 0xF000
    mov word [0x1F*4],   dummy_isr
    mov word [0x1F*4+2], 0xF000

    ; Fill all uninitialized IRQ vectors (0x20-0x70) with dummy_isr
    ; to prevent crashes if unexpected interrupts fire
    mov cx, 0x70 - 0x20
    mov bx, 0x20 * 4
.fill_irq:
    mov word [bx],   dummy_isr
    mov word [bx+2], 0xF000
    add bx, 4
    loop .fill_irq

    ret

; ── Dummy ISR ───────────────────────────────────
dummy_isr:
    iret

; ── IRQ0 Timer ──────────────────────────────────
irq0_timer:
    ; This is the 18.2Hz timer tick interrupt handler.
    ; It increments the BDA tick counter and calls the user timer hook INT 1Ch.
    pushf ; Save flags, as INT 1Ch might modify them
    pusha ; Save all general purpose registers

    ; The BDA is at 0000:0400. We can access it directly with DS=0.
    ; Atomically increment the 32-bit timer tick count at 0x046C
    add dword [BDA_TIMER_LOW], 1
    jnc .no_overflow

    ; On overflow (after ~24 hours), set the overflow flag at 0x0471
    or byte [BDA_TIMER_OVERFLOW], 0x01

.no_overflow:
    ; Call user-definable timer tick interrupt
    int 0x1C

    ; Send End-Of-Interrupt to the PIC
    mov al, PIC_EOI
    out PIC1_CMD, al

    popa
    popf
    iret

; ── INT 0x11 — Equipment list ───────────────────
int11_handler:
    mov ax, [BDA_EQUIPMENT]
    iret

; ── INT 0x12 — Memory size ──────────────────────
int12_handler:
    mov ax, [BDA_MEMSIZE]
    iret

; ── INT 0x14 — Serial services ──────────────────
int14_handler:
    cmp ah, 0x00
    je .init
    cmp ah, 0x01
    je .write
    cmp ah, 0x02
    je .read
    cmp ah, 0x03
    je .status
    iret
.init:
    call serial_init
    xor ah, ah
    iret
.write:
    call serial_putchar
    xor ah, ah
    iret
.read:
    ; Wait for char
    mov dx, 0x3F8 + 5
.wait:
    in al, dx
    test al, 0x01
    jz .wait
    mov dx, 0x3F8
    in al, dx
    xor ah, ah
    iret
.status:
    mov dx, 0x3F8 + 5
    in al, dx
    mov ah, al
    xor al, al
    iret

; ── INT 0x17 — Printer (stub) ───────────────────
int17_handler:
    mov ah, 0x00
    iret

; ── INT 0x18 — ROM BASIC / Boot fail ────────────
int18_handler:
    mov si, str_no_boot_main
    call serial_puts
    ; call video_print_str
    cli
.halt:
    hlt
    jmp .halt

; ── INT 0x19 — Bootstrap loader ─────────────────
int19_handler:
    call boot_sequence
    iret

; ── INT 0x1A — Time/Date/PCI ────────────────────
int1a_handler:
    cmp ah, 0x00
    je .get_ticks
    cmp ah, 0x01
    je .set_ticks
    ; RTC / PCI functions — dispatch to rtc.asm handler
    jmp int1a_rtc_handler
.get_ticks:
    ; Returns 32-bit BDA tick count in CX:DX; AL=overflow flag
    push bx
    xor bx, bx
    mov ds, bx
    mov cx, [0x046E]
    mov dx, [0x046C]
    mov al, [0x0470]        ; overflow byte
    mov byte [0x0470], 0    ; clear overflow
    pop bx
    iret
.set_ticks:
    push bx
    xor bx, bx
    mov ds, bx
    mov [0x046E], cx
    mov [0x046C], dx
    pop bx
    iret

; ── Strings ─────────────────────────────────────
str_banner:
    db 'Nyx BIOS v1.0', 13, 10, 0
str_pic_ok:   db '[OK] PIC initialized', 13, 10, 0
str_pit_ok:   db '[OK] PIT OK', 13, 10, 0
; str_video_ok: 
; str_bda_ok:   
; str_mem_ok:   
str_ivt_ok:   db '[OK] IVT OK', 13, 10, 0
str_pci_ok_main:   db '[OK] PCI stub', 13, 10, 0
	str_kbd_ok:   db '[OK] Keyboard initialized', 13, 10, 0
	str_acpi_ok:  db '[OK] ACPI initialized', 13, 10, 0
	str_ata_detect: db '[  ] ATA detect...', 13, 10, 0
	str_ata_detect_ok: db '[OK] ATA detect done', 13, 10, 0
	str_smbios_ok: db '[OK] SMBIOS initialized', 13, 10, 0
	str_dma_ok:    db '[OK] DMA initialized', 13, 10, 0
	str_rtc_ok:    db '[OK] RTC initialized', 13, 10, 0
	str_splash_start: db '[  ] Splash init...', 13, 10, 0
	str_splash_done:  db '[OK] Splash init done', 13, 10, 0
	str_splash_end:   db '[OK] Splash end done', 13, 10, 0
	str_boot:     db '[  ] Starting boot...', 13, 10, 0
	str_halt:     db '[!!] System halted', 13, 10, 0
	str_no_boot_main:  db '[!!] No bootable device found!', 13, 10, 0

; Pad to the reset vector (F000:FFF0).
times (0xFFF0 - ($ - $$)) db 0xFF

; Reset vector must be located at physical 0xFFFF0 (F000:FFF0), which is the
; last 16 bytes of a 128KiB image.
SECTION .reset start=ROM_RESET_OFF vstart=0xFFF0
reset_vector:
    cli
    jmp 0xF000:bios_entry
    times (0x10 - ($ - $$)) db 0xFF
