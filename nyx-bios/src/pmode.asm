; src/pmode.asm — Protected mode switching
; Used by bootloaders that call BIOS 
; then switch to pmode

%define GDT_ADDR  0x7E00

; GDT entries
gdt_start:
    ; Null descriptor
    dq 0

gdt_code16:
    ; 16-bit code segment
    dw 0xFFFF, 0x0000
    db 0x00, 0x9A, 0x00, 0x00

gdt_data16:
    ; 16-bit data segment
    dw 0xFFFF, 0x0000
    db 0x00, 0x92, 0x00, 0x00

gdt_code32:
    ; 32-bit code segment (flat)
    dw 0xFFFF, 0x0000
    db 0x00, 0x9A, 0xCF, 0x00

gdt_data32:
    ; 32-bit data segment (flat)
    dw 0xFFFF, 0x0000
    db 0x00, 0x92, 0xCF, 0x00

gdt_code64:
    ; 64-bit code segment
    dw 0x0000, 0x0000
    db 0x00, 0x9A, 0x20, 0x00

gdt_data64:
    ; 64-bit data segment
    dw 0x0000, 0x0000
    db 0x00, 0x92, 0x00, 0x00

gdt_end:

gdt_ptr:
    dw gdt_end - gdt_start - 1
    dd GDT_ADDR

setup_gdt:
    ; Copy GDT to 0x7E00
    push es
    push di
    push si
    push cx
    mov ax, 0x0000
    mov es, ax
    mov di, GDT_ADDR
    mov si, gdt_start
    mov cx, gdt_end - gdt_start
    rep movsb
    ; Load GDT
    lgdt [gdt_ptr]
    pop cx
    pop si
    pop di
    pop es
    ret

enter_protected_mode:
    cli
    call setup_gdt
    ; Enable A20
    call a20_enable
    ; Set PE bit in CR0
    mov eax, cr0
    or eax, 0x01
    mov cr0, eax
    ; Far jump to flush pipeline
    jmp 0x18:pmode_entry    ; selector 0x18 = code32

BITS 32
pmode_entry:
    ; Setup data segments
    mov ax, 0x20            ; data32 selector
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    mov esp, 0x90000
    ret

BITS 16