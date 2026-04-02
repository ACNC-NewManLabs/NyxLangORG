; SPDX-License-Identifier: MIT
; Nyx BIOS — https://github.com/surya/nyx-bios
; Copyright (c) 2026 Surya

; ── src/keyboard.asm — PS/2 Keyboard, IRQ1, INT 16h ──────────────────────

; BDA defines (DS is assumed to be 0)
%define BDA_KBD_FLAGS1      0x0417
%define BDA_KBD_FLAGS2      0x0418
%define BDA_KBD_ALT_KEYPAD  0x0419
%define BDA_KBD_BUF_HEAD    0x041A
%define BDA_KBD_BUF_TAIL    0x041C
%define BDA_KBD_BUFFER      0x041E
%define KBD_BUFFER_SIZE     16 ; 16 words (scancode + char)

; Keyboard ports
%define KBD_DATA_PORT   0x60
%define KBD_STATUS_PORT 0x64

; Keyboard status flags (BDA 0x0417)
%define KBD_FLAG_RIGHT_SHIFT  (1 << 0)
%define KBD_FLAG_LEFT_SHIFT   (1 << 1)
%define KBD_FLAG_CTRL         (1 << 2)
%define KBD_FLAG_ALT          (1 << 3)
%define KBD_FLAG_SCROLL_LOCK  (1 << 4)
%define KBD_FLAG_NUM_LOCK     (1 << 5)
%define KBD_FLAG_CAPS_LOCK    (1 << 6)
%define KBD_FLAG_INSERT       (1 << 7)

; Scancode state variable in scratch RAM
%define NYX_KBD_STATE       (NYX_VAR_BASE + 0x30) ; byte (for E0 prefix)

; ── kbd_ctrl_wait_input ───────────────────────────────────────────────────
; Purpose : Busy-wait until PS/2 controller input buffer is empty (bit 1=0)
; Input   : None
; Output  : None
; Trashes : AX
kbd_ctrl_wait_input:
    push ax
    push cx
    mov cx, 0xFFFF
.loop:
    in al, KBD_STATUS_PORT
    test al, 0x02           ; input buffer full?
    jz .done
    loop .loop
.done:
    pop cx
    pop ax
    ret

; ── kbd_a20_enable ────────────────────────────────────────────────────────
; Purpose : Enable A20 gate via PS/2 keyboard controller (KBD Write Output Port)
; Input   : None
; Output  : None
; Trashes : AX
; Note    : Should be called in addition to fast-A20 (port 0x92) for safety.
kbd_a20_enable:
    push ax
    call kbd_ctrl_wait_input
    mov al, 0xD1            ; KBD cmd: Write Output Port
    out KBD_STATUS_PORT, al
    call kbd_ctrl_wait_input
    mov al, 0xDF            ; bit1=1 (A20 enabled), keep other output lines high
    out KBD_DATA_PORT, al
    call kbd_ctrl_wait_input
    pop ax
    ret

; ── kbd_init ─────────────────────────────────────────────────────────────
; Purpose : Initialize the PS/2 keyboard controller
; Input   : None
; Output  : None
; Trashes : AX
kbd_init:
    ; For QEMU, the keyboard is generally ready. A full reset sequence
    ; can be complex. We'll just ensure the buffer is clear.
    push ds
    xor ax, ax
    mov ds, ax
    mov word [BDA_KBD_BUF_HEAD], BDA_KBD_BUFFER
    mov word [BDA_KBD_BUF_TAIL], BDA_KBD_BUFFER
    mov byte [NYX_KBD_STATE], 0
    pop ds
    ret

; ── irq1_keyboard ────────────────────────────────────────────────────────
; Purpose : Handle IRQ1 from the keyboard controller
irq1_keyboard:
    pusha
    push ds
    push es

    xor ax, ax
    mov ds, ax
    mov es, ax

    ; Read scancode from keyboard
    in al, KBD_DATA_PORT
    mov bl, al ; Save scancode

    ; Check for E0 prefix (extended key)
    cmp al, 0xE0
    je .e0_prefix

    ; Not an E0 prefix, check if we were expecting one
    cmp byte [NYX_KBD_STATE], 1
    je .is_extended

.not_extended:
    ; Handle key release (scancode > 0x80)
    test bl, 0x80
    jnz .key_release

.key_press:
    mov si, scancode_set1_normal
    mov di, scancode_set1_shifted
    call .translate_and_buffer
    jmp .done

.is_extended:
    mov byte [NYX_KBD_STATE], 0 ; Clear E0 state
    ; Extended keys are not buffered, just update flags (e.g., Right Ctrl)
    cmp bl, 0x1D ; Right Ctrl press
    je .r_ctrl_press
    cmp bl, 0x9D ; Right Ctrl release
    je .r_ctrl_release
    jmp .done

.key_release:
    and bl, 0x7F ; Get press scancode
    cmp bl, 0x2A ; Left Shift
    je .l_shift_release
    cmp bl, 0x36 ; Right Shift
    je .r_shift_release
    cmp bl, 0x1D ; Ctrl
    je .ctrl_release
    cmp bl, 0x38 ; Alt
    je .alt_release
    jmp .done

.l_shift_release:
    and byte [BDA_KBD_FLAGS1], ~KBD_FLAG_LEFT_SHIFT
    jmp .done
.r_shift_release:
    and byte [BDA_KBD_FLAGS1], ~KBD_FLAG_RIGHT_SHIFT
    jmp .done
.ctrl_release:
    and byte [BDA_KBD_FLAGS1], ~KBD_FLAG_CTRL
    jmp .done
.alt_release:
    and byte [BDA_KBD_FLAGS1], ~KBD_FLAG_ALT
    jmp .done
.r_ctrl_press:
    or byte [BDA_KBD_FLAGS1], KBD_FLAG_CTRL
    jmp .done
.r_ctrl_release:
    and byte [BDA_KBD_FLAGS1], ~KBD_FLAG_CTRL
    jmp .done

.e0_prefix:
    mov byte [NYX_KBD_STATE], 1
    jmp .done

.translate_and_buffer:
    ; Input: BL=scancode, SI=normal table, DI=shifted table
    ; Check for special keys first
    cmp bl, 0x2A ; Left Shift
    je .l_shift_press
    cmp bl, 0x36 ; Right Shift
    je .r_shift_press
    cmp bl, 0x1D ; Ctrl
    je .ctrl_press
    cmp bl, 0x38 ; Alt
    je .alt_press

    ; Check shift/caps state to select table
    mov ch, [BDA_KBD_FLAGS1]
    mov cl, ch
    and ch, KBD_FLAG_LEFT_SHIFT | KBD_FLAG_RIGHT_SHIFT
    and cl, KBD_FLAG_CAPS_LOCK
    
    ; Get ASCII char
    movzx bx, bl
    mov al, [si+bx] ; Get normal char

    ; If it's a letter, check caps lock
    cmp al, 'a'
    jb .not_letter
    cmp al, 'z'
    ja .not_letter
    xor ch, cl ; If shift and caps are both on/off, use lowercase
    jz .not_shifted
    jmp .is_shifted
.not_letter:
    test ch, ch ; Is shift pressed?
    jz .not_shifted

.is_shifted:
    mov al, [di+bx] ; Get shifted char

.not_shifted:
    ; AL = ASCII char, BL = scancode
    mov ah, bl
    call kbd_buffer_add
    ret

.l_shift_press:
    or byte [BDA_KBD_FLAGS1], KBD_FLAG_LEFT_SHIFT
    ret
.r_shift_press:
    or byte [BDA_KBD_FLAGS1], KBD_FLAG_RIGHT_SHIFT
    ret
.ctrl_press:
    or byte [BDA_KBD_FLAGS1], KBD_FLAG_CTRL
    ret
.alt_press:
    or byte [BDA_KBD_FLAGS1], KBD_FLAG_ALT
    ret

.done:
    ; Send EOI to PIC
    mov al, 0x20
    out 0x20, al

    pop es
    pop ds
    popa
    iret

; ── kbd_buffer_add ───────────────────────────────────────────────────────
; Purpose : Add a character/scancode pair to the BDA circular buffer
; Input   : AX = (AH=scancode, AL=char)
; Output  : None
; Trashes : CX, DI
kbd_buffer_add:
    mov di, [BDA_KBD_BUF_TAIL]
    add di, 2
    cmp di, BDA_KBD_BUFFER + (KBD_BUFFER_SIZE * 2)
    jne .no_wrap
    mov di, BDA_KBD_BUFFER
.no_wrap:
    cmp di, [BDA_KBD_BUF_HEAD] ; Check if buffer is full
    je .full
    mov [es:di-2], ax
    mov [BDA_KBD_BUF_TAIL], di
.full:
    ret

; ── int16_handler ────────────────────────────────────────────────────────
; Purpose : Handle INT 16h BIOS keyboard services
int16_handler:
    cmp ah, 0x00 ; Get Keystroke
    je .get_key
    cmp ah, 0x10 ; Get Keystroke (extended)
    je .get_key
    cmp ah, 0x01 ; Check Keystroke
    je .check_key
    cmp ah, 0x11 ; Check Keystroke (extended)
    je .check_key
    cmp ah, 0x02 ; Get Shift Status
    je .get_shift

    ; Unsupported function
    stc
    iret

.get_key:
    call .check_key
    jz .get_key ; Loop until a key is available
    mov di, [BDA_KBD_BUF_HEAD]
    mov ax, [es:di]
    add di, 2
    cmp di, BDA_KBD_BUFFER + (KBD_BUFFER_SIZE * 2)
    jne .no_wrap_head
    mov di, BDA_KBD_BUFFER
.no_wrap_head:
    mov [BDA_KBD_BUF_HEAD], di
    clc
    iret

.check_key:
    mov di, [BDA_KBD_BUF_HEAD]
    cmp di, [BDA_KBD_BUF_TAIL]
    jne .key_available
    ; Buffer empty
    stc
    mov ah, 0
    or ah, 0x80 ; Set ZF=1 in flags
    pushf
    pop ax
    or ax, 0x40
    push ax
    popf
    ret
.key_available:
    clc
    mov di, [BDA_KBD_BUF_HEAD]
    mov ax, [es:di]
    ret

.get_shift:
    mov al, [BDA_KBD_FLAGS1]
    clc
    iret

; --- Scancode Tables (US Layout) ---
scancode_set1_normal:
    db 0, 27, '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '-', '=', 8, 9   ; 0x00-0x0F
    db 'q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p', '[', ']', 13, 0, 'a', 's' ; 0x10-0x1F
    db 'd', 'f', 'g', 'h', 'j', 'k', 'l', ';', "'", '`', 0, '\', 'z', 'x', 'c', 'v' ; 0x20-0x2F
    db 'b', 'n', 'm', ',', '.', '/', 0, '*', 0, ' ', 0, 0, 0, 0, 0, 0       ; 0x30-0x3F

scancode_set1_shifted:
    db 0, 27, '!', '@', '#', '$', '%', '^', '&', '*', '(', ')', '_', '+', 8, 9   ; 0x00-0x0F
    db 'Q', 'W', 'E', 'R', 'T', 'Y', 'U', 'I', 'O', 'P', '{', '}', 13, 0, 'A', 'S' ; 0x10-0x1F
    db 'D', 'F', 'G', 'H', 'J', 'K', 'L', ':', '"', '~', 0, '|', 'Z', 'X', 'C', 'V' ; 0x20-0x2F
    db 'B', 'N', 'M', '<', '>', '?', 0, '*', 0, ' ', 0, 0, 0, 0, 0, 0       ; 0x30-0x3F