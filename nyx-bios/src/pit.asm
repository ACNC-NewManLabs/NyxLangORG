; src/pit.asm — 8254 PIT timer

%define PIT_CH0   0x40
%define PIT_CH1   0x41
%define PIT_CH2   0x42
%define PIT_CMD   0x43

pit_init:
    push ax
    ; Channel 0: rate generator, 18.2 Hz
    ; Divisor 0x10000 = 65536 → 18.2065 Hz
    mov al, 0x36        ; CH0, lobyte/hibyte, mode 3
    out PIT_CMD, al
    ; Divisor low byte
    mov al, 0x00
    out PIT_CH0, al
    ; Divisor high byte
    mov al, 0x00
    out PIT_CH0, al
    pop ax
    ret

pit_wait_ms:
    ; Wait approximately CX milliseconds
    push cx
    push ax
.outer:
    mov ax, 1193        ; ~1ms at 1.193MHz
.inner:
    dec ax
    jnz .inner
    loop .outer
    pop ax
    pop cx
    ret