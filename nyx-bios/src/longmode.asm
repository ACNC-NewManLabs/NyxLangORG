; src/longmode.asm — 64-bit GDT and page tables for UEFI/macOS

; ── 64-bit GDT ───────────────────────────────────
gdt64_start:

; Null descriptor (required)
gdt64_null:
    dq 0x0000000000000000

; Code segment (64-bit)
gdt64_code:
    dw 0xFFFF       ; Limit (4GB, covered by granularity)
    dw 0x0000       ; Base (0)
    db 0x00         ; Base (middle)
    db 0x9A         ; Access: present, executable, readable
    db 0xAF         ; Flags: 64-bit, 4KB granularity, limit = FFFF
    db 0x00         ; Base (high)

; Data segment
gdt64_data:
    dw 0xFFFF
    dw 0x0000
    db 0x00
    db 0x92         ; Access: present, writable
    db 0xCF         ; Flags: 4KB granularity
    db 0x00

; 16-bit code segment (for BIOS compatibility)
gdt64_code16:
    dw 0xFFFF
    dw 0x0000
    db 0x00
    db 0x9A
    db 0x0F         ; 16-bit
    db 0x00

; 16-bit data segment
gdt64_data16:
    dw 0xFFFF
    dw 0x0000
    db 0x00
    db 0x92
    db 0x0F
    db 0x00

gdt64_end:

; GDT descriptor
gdt_descriptor:
    dw gdt64_end - gdt64_start - 1
    dd gdt64_start

; ── Page Tables for Identity Mapping ───────────
; We'll set up a simple 2MB identity mapping for boot
%define PML4_ADDR    0x1000    ; Page Map Level 4
%define PDPT_ADDR    0x2000    ; Page Directory Pointer Table
%define PD_ADDR      0x3000    ; Page Directory

setup_page_tables:
    push ax
    push cx
    push di
    push es
    
    ; Clear page tables (set to 0)
    xor ax, ax
    mov es, ax
    
    ; Clear PML4 (4KB)
    mov di, PML4_ADDR
    mov cx, 1024
    rep stosd
    
    ; Clear PDPT (4KB)
    mov di, PDPT_ADDR
    mov cx, 1024
    rep stosd
    
    ; Clear PD (4KB)
    mov di, PD_ADDR
    mov cx, 1024
    rep stosd
    
    ; Set up PML4 (Page Map Level 4)
    ; PML4[0] = PDPT_ADDR | present | writable
    mov di, PML4_ADDR
    mov eax, PDPT_ADDR | 0x03    ; Present + Writable
    stosd
    xor eax, eax
    stosd
    stosd
    stosd
    
    ; Set up PDPT
    ; PDPT[0] = PD_ADDR | present | writable | large (1GB pages)
    mov di, PDPT_ADDR
    mov eax, PD_ADDR | 0x03 | 0x80   ; Present + Writable + PS (1GB)
    stosd
    xor eax, eax
    stosd
    stosd
    stosd
    
    ; Set up Page Directory
    ; Each entry maps 2MB, we'll set up entries for identity mapping
    ; PD[0] = 0x000 | present | writable
    ; PD[1] = 0x200000 | present | writable
    ; etc.
    mov di, PD_ADDR
    mov eax, 0x000 | 0x83    ; Present + Writable + PS (2MB)
    stosd
    mov eax, 0x200000 | 0x83
    stosd
    mov eax, 0x400000 | 0x83
    stosd
    mov eax, 0x600000 | 0x83
    stosd
    mov eax, 0x800000 | 0x83
    stosd
    mov eax, 0xA00000 | 0x83
    stosd
    mov eax, 0xC00000 | 0x83
    stosd
    mov eax, 0xE00000 | 0x83
    stosd
    
    ; Fill rest with zeros
    mov cx, 256 - 8
    xor eax, eax
    rep stosd
    
    pop es
    pop di
    pop cx
    pop ax
    ret

; ── Switch to Long Mode (64-bit) ───────────────
; This would be called to switch to 64-bit mode
switch_to_long_mode:
    ; Already in protected mode at this point
    
    ; Load the page table base
    mov eax, PML4_ADDR
    mov cr3, eax
    
    ; Enable PAE
    mov eax, cr4
    or eax, 0x20
    mov cr4, eax
    
    ; Enable long mode (EFER MSR, MSR 0xC0000080)
    mov ecx, 0xC0000080
    rdmsr
    or eax, 0x100    ; LME (Long Mode Enable)
    wrmsr
    
    ; Enable paging
    mov eax, cr0
    or eax, 0x80000000    ; PG (Paging)
    mov cr0, eax
    
    ; Load GDT
    lgdt [gdt_descriptor]
    
    ; Far jump to 64-bit code
    jmp 0x08:long_mode_start

; This would be in a 64-bit section
BITS 64
long_mode_start:
    ; We're now in 64-bit mode
    ; Set up segment registers
    mov ax, 0x10    ; Data segment selector
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    
    ; Jump to OS boot loader
    ; (This would hand off to a 64-bit kernel)
    
    ; For now, halt
    jmp $

BITS 16

; ── EFI CSM Stub ─────────────────────────────────
; TIER 3: EFI CSM stub for EFI handoff
efi_csm_entry:
    ; This would be called when EFI wants to boot via CSM
    ; Check for EFI boot services
    ; For now, just return to legacy boot
    
    ret

; ── MP Table (Multiprocessor Specification) ──────
mp_table:
    ; Signature "_MP_"
    db '_MP_'
    
    ; Length
    db 0x10
    
    ; Revision
    db 0x04
    
    ; Checksum
    db 0x00
    
    ; OEM type
    db 'Nyx BIOS  '
    
    ; Product ID
    db 'Nyx v1.0     '
    
    ; OEM table pointer
    dd 0x00000000
    
    ; OEM table size
    dd 0x00000000
    
    ; Entry count
    db 0x02
    
    ; Local APIC address
    dd 0xFEE00000
    
    ; Extended table length
    db 0x00
    
    ; Extended table checksum
    db 0x00
    
; MP floating pointer
mp_float:
    ; Signature "_MP_"
    db '_MP_'
    
    ; Physical address of MP table
    dd mp_table
    
    ; Length
    db 0x01
    
    ; Revision
    db 0x04
    
    ; Checksum
    db 0x00
    
    ; Type
    db 0x00    ; Default

; ── APM (Advanced Power Management) ──────────────
apm_handler:
    ; INT 0x15, AH = 0x53
    cmp al, 0x00
    je .apm_installed
    cmp al, 0x01
    je .apm_connect
    cmp al, 0x02
    je .apm_disconnect
    cmp al, 0x03
    je .apm_cpu_idle
    cmp al, 0x04
    je .apm_cpu_busy
    cmp al, 0x05
    je .apm_set_state
    cmp al, 0x06
    je .apm_enable
    cmp al, 0x07
    je .apm_disable
    cmp al, 0x08
    je .apm_status
    
    ; Unknown
    stc
    iret

.apm_installed:
    ; Check if APM installed
    ; Return AX = 0x504D ('PM') if present
    mov ax, 0x504D
    clc
    iret

.apm_connect:
    ; Connect to APM
    mov ax, 0x0000
    clc
    iret

.apm_disconnect:
    mov ax, 0x0000
    clc
    iret

.apm_cpu_idle:
    ; CPU idle
    hlt
    mov ax, 0x0000
    clc
    iret

.apm_cpu_busy:
    ; CPU busy
    mov ax, 0x0000
    clc
    iret

.apm_set_state:
    ; Set power state
    mov ax, 0x0000
    clc
    iret

.apm_enable:
    mov ax, 0x0000
    clc
    iret

.apm_disable:
    mov ax, 0x0000
    clc
    iret

.apm_status:
    ; Get APM status
    mov ah, 0x00
    mov al, 0x01    ; Enabled
    clc
    iret
