; SPDX-License-Identifier: MIT
; Nyx BIOS — https://github.com/surya/nyx-bios
; Copyright (c) 2026 Surya

; ── src/acpi.asm — Advanced Configuration and Power Interface ────────────

%define ACPI_BASE       0x9E000         ; physical 32-bit address (for table pointers)
%define ACPI_SEG        0x9E00          ; segment for 16-bit access
%define ACPI_RSDP_PHYS  0x9FC10        ; physical address of RSDP
%define ACPI_RSDP_SEG   0x9FC0         ; segment of EBDA / RSDP
%define ACPI_RSDP_OFF   0x0010         ; offset of RSDP within EBDA
%define ACPI_RSDP_ADDR  ACPI_RSDP_PHYS ; kept for compatibility

; ── acpi_init ────────────────────────────────────────────────────────────
; Purpose : Initialize ACPI tables in RAM (0x9E000) and RSDP in EBDA
; Input   : None
; Output  : None
; Trashes : AX, CX, SI, DI, DS, ES
acpi_init:
    pusha
    push ds
    push es

    ; ── Copy ACPI templates from ROM (F000:si) to RAM (9E00:0000) ──────────
    mov ax, ACPI_SEG
    mov es, ax              ; ES = 0x9E00 → physical 0x9E000
    mov ax, 0xF000
    mov ds, ax              ; DS = ROM segment
    mov si, acpi_templates  ; SI = ROM offset of templates
    xor di, di              ; DI = 0 → ES:DI = 0x9E000
    mov cx, acpi_templates_end - acpi_templates
    rep movsb

    ; ── Checksum each table (DS = ACPI RAM segment, SI = table offset) ────
    mov ax, ACPI_SEG
    mov ds, ax

    mov si, dsdt_start - acpi_templates
    call acpi_checksum_table

    mov si, hpet_start - acpi_templates
    call acpi_checksum_table

    mov si, madt_start - acpi_templates
    call acpi_checksum_table

    mov si, mcfg_start - acpi_templates
    call acpi_checksum_table

    mov si, fadt_start - acpi_templates
    call acpi_checksum_table

    mov si, rsdt_start - acpi_templates
    call acpi_checksum_table

    ; ── Copy RSDP to EBDA (9FC0:0010) ──────────────────────────────────────
    mov ax, 0xF000
    mov ds, ax              ; DS back to ROM
    mov si, rsdp_template
    mov ax, ACPI_RSDP_SEG
    mov es, ax              ; ES = 0x9FC0 → physical 0x9FC00
    mov di, ACPI_RSDP_OFF   ; DI = 0x10  → physical 0x9FC10
    mov cx, 20
    rep movsb

    ; ── Patch RSDP checksum ────────────────────────────────────────────────
    mov ax, ACPI_RSDP_SEG
    mov ds, ax
    mov si, ACPI_RSDP_OFF   ; DS:SI = 0x9FC0:0x10
    mov cx, 20
    call acpi_checksum
    mov byte [ds:ACPI_RSDP_OFF + 8], al    ; store checksum at RSDP+8

    pop es
    pop ds
    popa
    ret

; ── acpi_checksum_table ──────────────────────────────────────────────────
; Input: DS:SI = pointer to ACPI table in RAM. Uses low 16 bits of length.
acpi_checksum_table:
    pusha
    mov cx, [si+4]          ; table length (low 16 bits; all our tables < 64KB)
    mov byte [si+9], 0      ; zero out the checksum field first
    call acpi_checksum      ; compute two's complement checksum over CX bytes at DS:SI
    mov [si+9], al          ; write checksum
    popa
    ret

; ── acpi_checksum ────────────────────────────────────────────────────────
acpi_checksum:
    push cx
    push si
    xor ah, ah
.loop:
    lodsb
    add ah, al
    loop .loop
    neg ah
    mov al, ah
    pop si
    pop cx
    ret

; ── ACPI Templates (ROM) ─────────────────────────────────────────────────
ALIGN 4
acpi_templates:

facs_start:
    db 'FACS'
    dd 64
    dd 0, 0, 0, 0
    times 40 db 0
facs_end:

rsdt_start:
    db 'RSDT'
    dd rsdt_end - rsdt_start
    db 1, 0
    db 'NYX   ', 'NYXBIOS '
    dd 1, 'NYX ', 1
    dd ACPI_BASE + (fadt_start - acpi_templates)
    dd ACPI_BASE + (madt_start - acpi_templates)
    dd ACPI_BASE + (hpet_start - acpi_templates)
    dd ACPI_BASE + (mcfg_start - acpi_templates)
rsdt_end:

fadt_start:
    db 'FACP'
    dd 244  ; ACPI 2.0 format length
    db 3, 0 ; Revision 3
    db 'NYX   ', 'NYXBIOS '
    dd 1, 'NYX ', 1
    dd ACPI_BASE + (facs_start - acpi_templates) ; FIRMWARE_CTRL
    dd ACPI_BASE + (dsdt_start - acpi_templates) ; DSDT
    db 0, 0 ; Reserved, Preferred_PM_Profile
    dw 9 ; SCI_INT (Standard ACPI IRQ)
    dd 0x000000B2 ; SMI_CMD
    db 0x02, 0x03, 0, 0 ; ACPI_ENABLE, ACPI_DISABLE, S4, PSTATE
    dd 0x0000B000, 0 ; PM1a_EVT_BLK, PM1b
    dd 0x0000B004, 0 ; PM1a_CNT_BLK, PM1b
    dd 0, 0x0000B008 ; PM2_CNT_BLK, PM_TMR_BLK
    dd 0x0000B020, 0 ; GPE0_BLK, GPE1_BLK
    db 4, 2, 0, 4, 16, 0 ; Block Lengths
    db 0, 0 ; GPE1_BASE, CST_CNT
    dw 0, 0, 0, 0 ; Latency / Flush settings
    db 0, 0, 0, 0 ; Duty / Alarm
    db 0x32 ; CENTURY (CMOS offset)
    dw 2 ; IAPC_BOOT_ARCH (VGA + 8042 standard)
    db 0 ; Reserved
    dd 0x000000A5 ; Flags (WBINVD, C1, SLP_BUTTON, RTC_S4)
    ; RESET_REG (Port 0xCF9 hard reset)
    db 1, 8, 0, 1 ; GAS (I/O, 8-bit, 0-offset, byte-access)
    dq 0x0000000000000CF9
    db 0x06 ; RESET_VALUE
    db 0, 0, 0 ; Reserved
    dq 0, 0 ; X_FIRMWARE_CTRL, X_DSDT
    times 96 db 0 ; X_ pointer blocks (unused, falling back to 32-bit pointers)
fadt_end:

madt_start:
    db 'APIC'
    dd madt_end - madt_start
    db 1, 0
    db 'NYX   ', 'NYXBIOS '
    dd 1, 'NYX ', 1
    dd 0xFEE00000 ; Local APIC Address
    dd 1 ; Flags (PCAT_COMPAT)
    db 0, 8, 0, 0, 1, 0, 0, 0  ; Type 0: Processor Local APIC
    db 1, 12, 0, 0, 0, 0xC0, 0xFE, 0x00, 0, 0, 0, 0 ; Type 1: I/O APIC
    db 2, 10, 0, 0, 2, 0, 0, 0, 0, 0 ; Type 2: IRQ0 -> GSI2
    db 2, 10, 0, 9, 9, 0, 0x0D, 0x00, 0, 0 ; Type 2: IRQ9 -> GSI9 (Level, Active High)
madt_end:

hpet_start:
    db 'HPET'
    dd hpet_end - hpet_start
    db 1, 0
    db 'NYX   ', 'NYXBIOS '
    dd 1, 'NYX ', 1
    dd 0x8086A201 ; Event Timer Block ID
    db 0, 64, 0, 0 ; GAS (Memory, 64-bit)
    dq 0xFED00000 ; Address
    db 0, 0, 0, 0 ; HPET Number, Min Tick, Page Protection
hpet_end:

mcfg_start:
    db 'MCFG'
    dd mcfg_end - mcfg_start
    db 1, 0
    db 'NYX   ', 'NYXBIOS '
    dd 1, 'NYX ', 1
    dq 0 ; Reserved
    dq 0xE0000000 ; Base Address
    dw 0 ; Segment Group
    db 0 ; Start Bus
    db 255 ; End Bus
    dd 0 ; Reserved
mcfg_end:

dsdt_start:
    db 'DSDT'
    dd dsdt_end - dsdt_start
    db 1, 0
    db 'NYX   ', 'NYXBIOS '
    dd 1, 'NYX ', 1
    ; Minimal AML: \_S5 Package for graceful ACPI Shutdown
    db 0x08, '_', 'S', '5', '_'
    db 0x12, 0x06, 0x04, 0x00, 0x00, 0x00, 0x00
dsdt_end:
acpi_templates_end:

rsdp_template:
    db 'RSD PTR '
    db 0 ; Checksum
    db 'NYX   ' ; OEMID
    db 0 ; Revision
    dd ACPI_BASE + (rsdt_start - acpi_templates) ; RsdtAddress
    dd 0 ; Length
    dq 0 ; XsdtAddress
    db 0 ; Ext Checksum
    db 0, 0, 0 ; Reserved