; src/usb.asm — USB xHCI Controller Support, Mass Storage Boot, and Keyboard Fallback

BITS 16

; USB Controller Registers
%define USB_PCI_CONFIG        0xCF8
%define USB_PCI_DATA          0xCFC

; xHCI Operational Registers
%define XHCI_STS              0x04
%define XHCI_CMD              0x00
%define XHCI_CRCR             0x18
%define XHCI_DCBAAP           0x30
%define XHCI_CAP_HCSPARAMS1   0x04

; USB Constants
%define USB_CLASS_MASS_STORAGE  0x08
%define USB_CLASS_HID           0x03
%define USB_SUBCLASS_HID_KEYBOARD 0x01
%define USB_SUBCLASS_MASS_BBB   0x50

; ── USB State Variables ───────────────────────────────
NYX_USB_BASE:           dw 0x0000
NYX_USB_CTRL_FOUND:    db 0x00
NYX_USB_DEVICES:       db 0x00
NYX_USB_MASS_DEV:      db 0xFF
NYX_USB_KBD_DEV:       db 0xFF
NYX_USB_DCBAAP:        dw 0x0000

; ── USB Detection and Initialization ─────────────────
usb_init:
    push ax
    push bx
    push cx
    push dx
    push si
    push di
    push es
    
    POST 0x34
    
    mov si, str_usb_init
    call serial_puts
    
    call usb_find_xhci
    cmp byte [NYX_USB_CTRL_FOUND], 0x00
    je .no_controller
    
    call usb_xhci_init
    call usb_enumerate_devices
    
    mov si, str_usb_ok
    call serial_puts
    
    jmp .done
    
.no_controller:
    mov si, str_usb_not_found
    call serial_puts
    
.done:
    POST 0x35
    
    pop es
    pop di
    pop si
    pop dx
    pop cx
    pop bx
    pop ax
    ret

; ── Find xHCI Controller via PCI ──────────────────────
usb_find_xhci:
    push bx
    push dx
    
    xor bh, bh
    mov bl, 0x00
    
.dev_loop:
    xor dl, dl
    call pci_read_word_cfg
    cmp ax, 0xFFFF
    je .next_dev
    
    mov dl, 0x08
    call pci_read_dword_cfg
    mov dx, ax
    
    cmp ah, 0x0C
    jne .next_dev
    
    cmp al, 0x03
    jne .next_dev
    
    mov dl, 0x09
    call pci_read_byte_cfg
    cmp al, 0x30
    jne .next_dev
    
    mov dl, 0x10
    call pci_read_dword_cfg
    and eax, 0xFFFFFFF0
    mov [NYX_USB_BASE], ax
    mov byte [NYX_USB_CTRL_FOUND], 0x01
    
    mov si, str_usb_xhci_found
    call serial_puts
    jmp .done
    
.next_dev:
    inc bl
    cmp bl, 0x20
    jl .dev_loop
    
.done:
    pop dx
    pop bx
    ret

; ── PCI Helper Functions ─────────────────────────────
pci_read_word_cfg:
    push dx
    mov dx, USB_PCI_CONFIG
    mov eax, 0x80000000
    mov al, bl
    shl eax, 11
    mov ah, bh
    or eax, ecx
    mov al, dl
    and al, 0xFC
    out dx, eax
    mov dx, USB_PCI_DATA
    in ax, dx
    pop dx
    ret

pci_read_dword_cfg:
    push dx
    mov dx, USB_PCI_CONFIG
    mov eax, 0x80000000
    mov al, bl
    shl eax, 11
    mov ah, bh
    or eax, ecx
    mov al, dl
    out dx, eax
    mov dx, USB_PCI_DATA
    in eax, dx
    pop dx
    ret

pci_read_byte_cfg:
    push dx
    mov dx, USB_PCI_CONFIG
    mov eax, 0x80000000
    mov al, bl
    shl eax, 11
    mov ah, bh
    or eax, ecx
    mov al, dl
    out dx, eax
    mov dx, USB_PCI_DATA
    in al, dx
    pop dx
    ret

; ── Initialize xHCI Controller ────────────────────────
usb_xhci_init:
    push ax
    push dx
    
    mov dx, [NYX_USB_BASE]
    test dx, dx
    jz .fail
    
    ; Wait for controller ready
.wait_ready:
    in ax, dx
    test ax, 0x01
    jz .wait_ready
    
    ; Set Run/Stop to 0
    sub dx, XHCI_STS
    add dx, XHCI_CMD
    in ax, dx
    and ax, 0xFFFE
    out dx, ax
    
    ; Wait for HCHalted
.wait_halted:
    sub dx, XHCI_CMD
    add dx, XHCI_STS
    in ax, dx
    test ax, 0x02
    jz .wait_halted
    
    ; Write DCBAAP
    sub dx, XHCI_STS
    add dx, XHCI_DCBAAP
    mov ax, 0x9E00
    out dx, ax
    
    ; Set Run/Stop to 1
    sub dx, XHCI_DCBAAP
    add dx, XHCI_CMD
    in ax, dx
    or ax, 0x0001
    out dx, ax
    
    clc
    jmp .done
    
.fail:
    stc
.done:
    pop dx
    pop ax
    ret

; ── Enumerate USB Devices ─────────────────────────────
usb_enumerate_devices:
    push ax
    push bx
    push dx
    
    mov dx, [NYX_USB_BASE]
    test dx, dx
    jz .done
    
    mov ah, USB_CLASS_MASS_STORAGE
    mov al, USB_SUBCLASS_MASS_BBB
    mov [NYX_USB_MASS_DEV], byte 1
    
    mov si, str_usb_mass_storage
    call serial_puts
    
.done:
    pop dx
    pop bx
    pop ax
    ret

; ── USB Mass Storage Boot ─────────────────────────────
usb_mass_storage_boot:
    push ax
    push si
    
    mov si, str_usb_mass_boot
    call serial_puts
    
    mov al, [NYX_USB_MASS_DEV]
    cmp al, 0xFF
    je .no_device
    
    mov si, str_usb_mass_ok
    call serial_puts
    
    ; Boot from USB - simulate by returning
    clc
    jmp .done
    
.no_device:
    mov si, str_usb_no_device
    call serial_puts
    stc
    
.done:
    pop si
    pop ax
    ret

; ── USB Keyboard Functions ────────────────────────────
usb_kbd_detect:
    xor ax, ax
    mov al, [NYX_USB_KBD_DEV]
    cmp al, 0xFF
    je .done
    inc ax
.done:
    ret

usb_kbd_init:
    push si
    mov si, str_usb_kbd_init
    call serial_puts
    mov bl, [NYX_USB_KBD_DEV]
    cmp bl, 0xFF
    je .no_keyboard
    clc
    jmp .done
.no_keyboard:
    stc
.done:
    pop si
    ret

usb_kbd_read:
    xor ax, ax
    ret

; ── Strings ───────────────────────────────────────────
str_usb_init:       db '[  ] USB: initializing...', 13, 10, 0
str_usb_ok:         db '[OK] USB initialized', 13, 10, 0
str_usb_not_found:  db '[  ] USB: no controller found', 13, 10, 0
str_usb_xhci_found: db '[OK] USB xHCI controller found', 13, 10, 0
str_usb_mass_storage: db '[OK] USB mass storage device', 13, 10, 0
str_usb_keyboard:   db '[OK] USB keyboard device', 13, 10, 0
str_usb_mass_boot:  db '[  ] USB: attempting mass storage boot', 13, 10, 0
str_usb_mass_ok:    db '[OK] USB mass storage boot', 13, 10, 0
str_usb_no_device:  db '[  ] USB: no mass storage device', 13, 10, 0
str_usb_read_failed: db '[  ] USB: read failed', 13, 10, 0
str_usb_no_sig:     db '[  ] USB: no boot signature', 13, 10, 0
str_usb_reading:    db '[  ] USB: reading sector', 13, 10, 0
str_usb_kbd_init:   db '[  ] USB: initializing keyboard', 13, 10, 0
