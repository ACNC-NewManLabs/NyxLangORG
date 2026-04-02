; src/serial.asm — Full 16550 UART driver

%define COM1 0x3F8
%define COM2 0x2F8

serial_init:
    push ax
    push dx
    ; Disable interrupts
    mov dx, COM1 + 1
    mov al, 0x00
    out dx, al
    ; Enable DLAB
    mov dx, COM1 + 3
    mov al, 0x80
    out dx, al
    ; Set 115200 baud (divisor = 1)
    mov dx, COM1 + 0
    mov al, 0x01
    out dx, al
    mov dx, COM1 + 1
    mov al, 0x00
    out dx, al
    ; 8N1
    mov dx, COM1 + 3
    mov al, 0x03
    out dx, al
    ; Enable FIFO, clear, 14-byte threshold
    mov dx, COM1 + 2
    mov al, 0xC7
    out dx, al
    ; RTS/DSR
    mov dx, COM1 + 4
    mov al, 0x0B
    out dx, al
    pop dx
    pop ax
    ret

serial_putchar:
    push dx
    push ax
    mov dx, COM1 + 5
.wait:
    in al, dx
    test al, 0x20
    jz .wait
    pop ax
    mov dx, COM1
    out dx, al
    pop dx
    ret

serial_puts:
    push ax
    push si
.loop:
    ; BIOS keeps DS=0 for BDA/low-memory access; strings live in the code segment.
    cs lodsb
    test al, al
    jz .done
    call serial_putchar
    jmp .loop
.done:
    pop si
    pop ax
    ret

serial_puthex16:
    ; Print AX as 4-digit hex
    push cx
    push ax
    mov cx, 4
.loop:
    rol ax, 4
    push ax
    and al, 0x0F
    add al, '0'
    cmp al, '9'
    jle .ok
    add al, 7
.ok:
    call serial_putchar
    pop ax
    loop .loop
    pop ax
    pop cx
    ret

serial_puthex32:
    ; Print EAX as 8-digit hex
    push eax
    shr eax, 16
    call serial_puthex16
    pop eax
    call serial_puthex16
    ret
