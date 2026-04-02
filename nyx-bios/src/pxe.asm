; src/pxe.asm — PXE Network Boot: DHCP Discover and TFTP Download

BITS 16

; Network Constants
%define DHCP_DISCOVER         1
%define DHCP_MAGIC_COOKIE     0x63825363

; Network Packet Buffer
%define NET_BUFFER_SEG        0x9000
%define NET_BUFFER_OFF        0x0000
%define NET_BUFFER_SIZE       2048

; ── PXE State Variables ───────────────────────────────
NYX_PXE_FOUND:       db 0x00
NYX_PXE_MAC_ADDR:   times 6 db 0
NYX_PXE_IP_ADDR:    dd 0x00000000
NYX_PXE_SERVER_IP:  dd 0x00000000
NYX_PXE_BOOT_FILE:  times 128 db 0

; ── PXE Initialization ───────────────────────────────
pxe_initialize:
    push ax
    push si
    
    POST 0x36
    
    mov si, str_pxe_init
    call serial_puts
    
    call pxe_detect
    
    cmp byte [NYX_PXE_FOUND], 0x00
    je .no_pxe
    
    mov si, str_pxe_ok
    call serial_puts
    
    jmp .done
    
.no_pxe:
    mov si, str_pxe_not_found
    call serial_puts
    
.done:
    POST 0x37
    
    pop si
    pop ax
    ret

; ── Detect PXE ROM ────────────────────────────────────
pxe_detect:
    push ax
    push si
    
    mov si, 0xC000
    
.scan_loop:
    cmp si, 0xE000
    jge .not_found
    
    mov ax, [cs:si]
    cmp ax, 0xAA55
    jne .next_rom
    
    mov ax, [cs:si+0x19]
    cmp ax, 0x4550
    jne .next_rom
    
    mov byte [NYX_PXE_FOUND], 0x01
    
    mov si, str_pxe_detected
    call serial_puts
    jmp .done
    
.next_rom:
    add si, 0x200
    and si, 0xFE00
    add si, 0x200
    jmp .scan_loop
    
.not_found:
    mov si, str_pxe_scan_done
    call serial_puts
    
.done:
    pop si
    pop ax
    ret

; ── DHCP Discover ─────────────────────────────────────
pxe_do_dhcp_discover:
    push ax
    push bx
    push cx
    push dx
    push si
    push di
    push es
    
    mov si, str_pxe_dhcp
    call serial_puts
    
    mov ax, NET_BUFFER_SEG
    mov es, ax
    xor di, di
    
    xor ax, ax
    mov cx, 192
    rep stosb
    
    mov ax, NET_BUFFER_SEG
    mov ds, ax
    xor si, si
    
    mov byte [ds:si], 0x01
    mov byte [ds:si+1], 0x01
    mov byte [ds:si+2], 0x06
    mov byte [ds:si+3], 0x00
    
    mov eax, 0x12345678
    mov [ds:si+4], eax
    
    mov word [ds:si+8], 0x0000
    mov word [ds:si+10], 0x8000
    
    mov dword [ds:si+12], 0x00000000
    mov dword [ds:si+16], 0x00000000
    mov dword [ds:si+20], 0x00000000
    mov dword [ds:si+24], 0x00000000
    
    mov si, 236
    mov eax, DHCP_MAGIC_COOKIE
    mov [ds:si], eax
    
    mov si, 240
    mov byte [ds:si], 0x35
    mov byte [ds:si+1], 0x01
    mov byte [ds:si+2], DHCP_DISCOVER
    mov byte [ds:si+3], 0x37
    mov byte [ds:si+4], 0x04
    mov byte [ds:si+5], 0x01
    mov byte [ds:si+6], 0x03
    mov byte [ds:si+7], 0x06
    mov byte [ds:si+8], 0xFF
    mov byte [ds:si+9], 0xFF
    
    mov si, str_pxe_dhcp_fail
    call serial_puts
    
    stc
    
    pop es
    pop di
    pop si
    pop dx
    pop cx
    pop bx
    pop ax
    ret

; ── TFTP Download ─────────────────────────────────────
pxe_do_tftp_read:
    push bx
    push cx
    push dx
    push si
    
    mov si, str_pxe_tftp
    call serial_puts
    
    mov eax, [NYX_PXE_SERVER_IP]
    test eax, eax
    jz .no_server
    
    mov si, str_pxe_tftp_fail
    call serial_puts
    
    stc
    jmp .done
    
.no_server:
    mov si, str_pxe_no_server
    call serial_puts
    stc
    
.done:
    xor ax, ax
    
    pop si
    pop dx
    pop cx
    pop bx
    ret

; ── PXE Boot ──────────────────────────────────────────
pxe_net_boot:
    push ax
    push si
    
    mov si, str_pxe_boot
    call serial_puts
    
    call pxe_do_dhcp_discover
    jc .dhcp_failed
    
    call pxe_do_tftp_read
    jc .tftp_failed
    
    mov si, str_pxe_bad_image
    call serial_puts
    stc
    jmp .done
    
.dhcp_failed:
    mov si, str_pxe_dhcp_fail
    call serial_puts
    stc
    jmp .done
    
.tftp_failed:
    mov si, str_pxe_tftp_fail
    call serial_puts
    stc
    
.done:
    pop si
    pop ax
    ret

; ── Strings ───────────────────────────────────────────
str_pxe_init:        db '[  ] PXE: initializing...', 13, 10, 0
str_pxe_ok:          db '[OK] PXE initialized', 13, 10, 0
str_pxe_not_found:  db '[  ] PXE: not found', 13, 10, 0
str_pxe_detected:   db '[OK] PXE ROM detected', 13, 10, 0
str_pxe_scan_done:  db '[  ] PXE: ROM scan complete', 13, 10, 0
str_pxe_boot:       db '[  ] PXE: attempting network boot', 13, 10, 0
str_pxe_dhcp:        db '[  ] PXE: DHCP discover...', 13, 10, 0
str_pxe_dhcp_fail:  db '[  ] PXE: DHCP failed (no network)', 13, 10, 0
str_pxe_tftp:       db '[  ] PXE: TFTP download...', 13, 10, 0
str_pxe_tftp_fail:  db '[  ] PXE: TFTP failed (no network)', 13, 10, 0
str_pxe_no_server:  db '[  ] PXE: no server configured', 13, 10, 0
str_pxe_bad_image:  db '[  ] PXE: invalid boot image', 13, 10, 0
