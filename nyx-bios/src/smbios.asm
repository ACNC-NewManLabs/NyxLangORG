; src/smbios.asm — SMBIOS 2.8 tables for Windows/macOS boot

%define SMBIOS_BEGIN     0xF0000
%define SMBIOS_ENTRY     0xF0000

; SMBIOS structures
%define SMBIOS_TYPE_BIOS 0
%define SMBIOS_TYPE_SYSTEM 1
%define SMBIOS_TYPE_BASEBOARD 2
%define SMBIOS_TYPE_CHASSIS 3
%define SMBIOS_TYPE_PROCESSOR 4
%define SMBIOS_TYPE_CACHE 7
%define SMBIOS_TYPE_PORT_CONNECTOR 8
%define SMBIOS_TYPE_SYSTEM_SLOT 9
%define SMBIOS_TYPE_MEMORY_ARRAY 16
%define SMBIOS_TYPE_MEMORY_DEVICE 17
%define SMBIOS_TYPE_MEMORY_MAPPED_ADDR 19

; ── SMBIOS Entry Point ───────────────────────────
smbios_entry:
    ; Anchor string "_SM_\0"
    db '_SM_'
    ; Length (0x1F = 31 bytes for entry point)
    db 0x1F
    ; Major version (2.8)
    db 0x02
    ; Minor version (0x08)
    db 0x08
    ; Max structure size (we'll use 0x100)
    dw 0x0100
    ; Entry point revision
    db 0x00
    ; Formatted area
    db 0x00, 0x00, 0x00, 0x00, 0x00
    ; Intermediate anchor string "_DMI_"
    db '_DMI_'
    ; Structure table length
    dw smbios_table_end - smbios_table
    ; Structure table address (32-bit)
    dd smbios_table
    ; Number of structures
    dw smbios_struct_count
    ; SMBIOS BCD revision (0x0280 for 2.8)
    dw 0x0280

; Checksum for entry point (bytes 0-15)
db 0x00    ; To be computed

; ── SMBIOS Structure Table ───────────────────────
smbios_table:

; Type 0: BIOS Information
smbios_bios_info:
    db SMBIOS_TYPE_BIOS    ; Type 0
    db bios_info_end - smbios_bios_info  ; Length
    dw 0x0000              ; Handle
    db 0x00                ; Vendor (use string)
    db 0x01                ; BIOS version
    db 0x00                ; BIOS segment
    dw 0xE800              ; BIOS release (segment)
    db 0x00                ; ROM size (64KB = 0x40 * 10h = 0x40000)
    db 0x40                ; 
    ; BIOS characteristics
    db 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
    ; BIOS characteristics extension bytes
    db 0x81, 0x08          ; ACPI, USB, boot from CD, BIOS serial
    db 0x00                ; 
    db 0x00                ; Major release
    db 0x00                ; Minor release
bios_info_end:

; Type 1: System Information
smbios_system_info:
    db SMBIOS_TYPE_SYSTEM  ; Type 1
    db system_info_end - smbios_system_info  ; Length
    dw 0x0001              ; Handle
    db 0x01                ; Manufacturer (string 1)
    db 0x02                ; Product name (string 2)
    db 0x03                ; Version (string 3)
    db 0x04                ; Serial number (string 4)
    db 0x00                ; UUID (all zeros)
    times 16 db 0x00
    db 0x00                ; Wakeup type (power switch)
    db 0x00                ; SKUNumber (string)
    db 0x00                ; Family (string)
system_info_end:

; Type 2: Baseboard Information
smbios_baseboard:
    db SMBIOS_TYPE_BASEBOARD  ; Type 2
    db baseboard_end - smbios_baseboard  ; Length
    dw 0x0002              ; Handle
    db 0x01                ; Manufacturer
    db 0x02                ; Product
    db 0x03                ; Version
    db 0x04                ; Serial number
    db 0x00                ; Asset tag
    db 0x00                ; Feature flags
    db 0x00                ; Location in chassis
    dw 0x0003              ; Chassis handle
    db 0x00                ; Board type (motherboard)
baseboard_end:

; Type 3: Chassis Information
smbios_chassis:
    db SMBIOS_TYPE_CHASSIS  ; Type 3
    db chassis_end - smbios_chassis  ; Length
    dw 0x0003              ; Handle
    db 0x01                ; Manufacturer
    db 0x00                ; Element 1
    db 0x00                ; Element 2
    db 0x00                ; Element 3
    db 0x00                ; Element 4
    db 0x00                ; Element 5
    db 0x00                ; Element 6
    db 0x00                ; Element 7
    db 0x00                ; Element 8
    db 0x00                ; Element 9
    db 0x00                ; Element 10
    db 0x00                ; Element 11
    db 0x00                ; Element 12
    db 0x00                ; Element 13
    db 0x00                ; Element 14
    db 0x00                ; Element 15
    db 0xA1                ; Chassis type (desktop)
    db 0x00                ; Bootup state
    db 0x00                ; Power supply state
    db 0x00                ; Thermal state
    db 0x00                ; Security status
    dq 0x0000000000000000  ; OEM-defined
    db 0x00                ; Height (1U)
    db 0x00                ; Power cord count
chassis_end:

; Type 4: Processor Information
smbios_processor:
    db SMBIOS_TYPE_PROCESSOR  ; Type 4
    db processor_end - smbios_processor  ; Length
    dw 0x0004              ; Handle
    db 0x01                ; Processor socket
    db 0x03                ; Processor type (CPU)
    db 0x03                ; Processor family
    db 0x02                ; Processor manufacturer
    db 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00  ; Processor ID
    db 0x01                ; Processor version
    db 0x00, 0x00          ; Voltage
    dw 0x0000              ; External clock
    dw 0x0000              ; Max speed
    dw 0x0000              ; Current speed
    db 0x00                ; Status
    db 0x00                ; Processor upgrade
    dw 0xFFFF              ; L1 cache handle
    dw 0xFFFF              ; L2 cache handle
    dw 0xFFFF              ; L3 cache handle
    db 0x00                ; Core count
    db 0x00                ; Thread count
    dw 0x0000              ; Processor characteristics
    dw 0x0000              ; Processor family 2
processor_end:

; Type 7: Cache Information (L1)
smbios_cache_l1:
    db SMBIOS_TYPE_CACHE    ; Type 7
    db cache_l1_end - smbios_cache_l1  ; Length
    dw 0x0007              ; Handle
    db 0x01                ; Socket designation
    db 0x00                ; Cache configuration
    dw 0x0000              ; Maximum cache size
    dw 0x0000              ; Installated cache size
    db 0x00                ; Cache supported
cache_l1_end:

; Type 8: Port Connector Information
smbios_port:
    db SMBIOS_TYPE_PORT_CONNECTOR  ; Type 8
    db port_end - smbios_port  ; Length
    dw 0x0008              ; Handle
    db 0x01                ; Port type (keyboard)
    db 0x01                ; Port connector type
    db 0x00                ; Port type 2
    db 0x00                ; Port connector 2
port_end:

; Type 9: System Slot Information
smbios_slot:
    db SMBIOS_TYPE_SYSTEM_SLOT  ; Type 9
    db slot_end - smbios_slot  ; Length
    dw 0x0009              ; Handle
    db 0x01                ; Slot designation
    db 0x00                ; Slot type (other)
    db 0x00                ; Slot data bus width
    db 0x00                ; Current usage
    db 0x00                ; Slot length
    db 0x00                ; Slot ID
    db 0x00                ; Slot characteristics 1
    db 0x00                ; Slot characteristics 2
    dw 0x0000              ; Segment group
    db 0x00                ; Bus number
    db 0x00                ; Device/function
slot_end:

; Type 16: Physical Memory Array
smbios_mem_array:
    db SMBIOS_TYPE_MEMORY_ARRAY  ; Type 16
    db mem_array_end - smbios_mem_array  ; Length
    dw 0x0010              ; Handle
    db 0x03                ; Location (system board)
    db 0x01                ; Use (system memory)
    db 0x02                ; Memory error correction
    dw 0x0000              ; Maximum capacity (64MB)
    dw 0xFFFF              ; Memory error handle
    db 0x01                ; Number of memory devices
    dw 0x0000              ; Extended maximum capacity
mem_array_end:

; Type 17: Memory Device
smbios_mem_device:
    db SMBIOS_TYPE_MEMORY_DEVICE  ; Type 17
    db mem_device_end - smbios_mem_device  ; Length
    dw 0x0011              ; Handle
    dw 0x0010              ; Physical memory array
    dw 0x0000              ; Memory error handle
    db 0x01                ; Total width (32 bits)
    db 0x01                ; Data width (32 bits)
    dw 0x0080              ; Size (128MB = 128 * 1024)
    db 0x00                ; Form factor (DIMM)
    db 0x00                ; Device set
    db 0x01                ; Device locator
    db 0x01                ; Bank locator
    db 0x00                ; Memory type (DDR)
    dw 0x0000              ; Type detail
mem_device_end:

; Type 19: Memory Mapped Address
smbios_mem_mapped:
    db SMBIOS_TYPE_MEMORY_MAPPED_ADDR  ; Type 19
    db mem_mapped_end - smbios_mem_mapped  ; Length
    dw 0x0013              ; Handle
    dw 0x0010              ; Memory array handle
    dq 0x000000000          ; Starting address
    dq 0x07FFFFFF          ; Ending address (128MB - 1)
    dw 0x0011              ; Memory device handle
    dw 0x0001              ; Partition row position
    db 0x00                ; Interleave position
    db 0x00                ; Interleave data depth
mem_mapped_end:

smbios_table_end:

; Calculate structure count
; 11 structures defined
smbios_struct_count:
    dw 11

; String area for SMBIOS
smbios_strings:
    db 'Nyx BIOS', 0
    db 'Nyx Computer', 0
    db 'Default System', 0
    db '000000000000', 0
    db 0

; ── SMBIOS Init ──────────────────────────────────
smbios_init:
    ; Copy SMBIOS entry to 0xF0000
    push es
    push si
    push di
    push cx
    
    mov ax, 0xF000
    mov es, ax
    mov si, smbios_entry
    xor di, di
    mov cx, 32
    rep movsb
    
    ; Compute and set checksum
    xor ax, ax
    mov si, smbios_entry
    mov cx, 16
.checksum_loop:
    lodsb
    add al, ah
    mov ah, al
    loop .checksum_loop
    neg ah
    mov [es:0x10], ah
    
    pop cx
    pop di
    pop si
    pop es
    ret
