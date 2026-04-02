; tools/mbr.asm - minimal MBR/bootsector for testing Nyx BIOS disk boot.
BITS 16
ORG 0x7C00

start:
    cli
    xor ax, ax
    mov ds, ax
    mov es, ax
    sti

    ; Also print to COM1 so it shows up in `-serial stdio` runs.
    mov si, msg
    call serial_puts

    mov si, msg
.loop:
    lodsb
    test al, al
    jz .halt
    mov ah, 0x0E
    mov bh, 0x00
    mov bl, 0x07
    int 0x10
    jmp .loop

.halt:
    cli
.hlt:
    hlt
    jmp .hlt

msg db "Nyx BIOS HDD boot OK", 13, 10, 0

; ── Minimal COM1 serial ──────────────────────────────────────────────────────
%define COM1_BASE 0x3F8

serial_putc:
    push dx
.wait:
    mov dx, COM1_BASE + 5       ; LSR
    in al, dx
    test al, 0x20               ; THR empty
    jz .wait
    mov dx, COM1_BASE + 0       ; THR
    out dx, al
    pop dx
    ret

serial_puts:
    push ax
.next:
    lodsb
    test al, al
    jz .done
    call serial_putc
    jmp .next
.done:
    pop ax
    ret

times 510-($-$$) db 0
dw 0xAA55
