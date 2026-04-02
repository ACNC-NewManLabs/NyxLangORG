; src/option_rom.asm — Option ROM and Shadow RAM support

%define OPTION_ROM_START  0xC0000
%define OPTION_ROM_END    0xE0000

; ── Shadow RAM Control ────────────────────────────
; Shadow RAM is memory at 0xA0000-0xFFFFF that can be mapped to ROM
; or made writable for BIOS use

shadow_ram_init:
    push ax
    push dx
    
    POST 0x30
    
    ; Shadow RAM at 0xA0000-0xBFFFF (VGA)
    ; Typically controlled by bits in 0x3C3 or through chipset registers
    
    ; Enable shadow RAM for 0xA0000-0xBFFFF
    ; This is chipset-specific, but common approach:
    mov dx, 0x03C3
    in al, dx
    or al, 0x01        ; Enable shadow RAM
    out dx, al
    
    ; For 0xC0000-0xFFFFF (BIOS area)
    ; Usually controlled via port 0x22/0x24 or similar
    ; For now, leave as ROM (read-only)
    
    POST 0x31
    
    pop dx
    pop ax
    ret

shadow_ram_enable_write:
    ; Enable write to shadow RAM at given address
    ; AX = segment (0xA000-0xFFFF)
    push ax
    push dx
    
    cmp ax, 0xA000
    jb .done
    cmp ax, 0xFFFF
    ja .done
    
    ; Enable write via chipset registers
    ; This varies by chipset
    ; For now, just return
.done:
    pop dx
    pop ax
    ret

shadow_ram_disable_write:
    ; Disable write (make ROM again)
    push ax
    push dx
    
    ; Disable shadow RAM write
    mov dx, 0x03C3
    in al, dx
    and al, 0xFE
    out dx, al
    
    pop dx
    pop ax
    ret

; ── Option ROM Scanning ──────────────────────────
; Scan for expansion ROMs at 0xC0000-0xE0000

option_rom_scan:
    push ax
    push bx
    push cx
    push si
    push di
    push es
    
    POST 0x32
    
    mov bx, 0xC000          ; ES = current option ROM segment
.scan_loop:
    cmp bx, 0xE000
    jge .scan_done

    mov es, bx
    xor di, di
    
    ; Check for ROM signature (0x55AA)
    mov ax, [es:di]
    cmp ax, 0xAA55
    jne .next_rom
    
    ; Found ROM header at ES:0
    ; Get ROM length (in 512-byte blocks)
    mov al, [es:di+2]
    test al, al
    jz .next_rom

    ; Far-call ROM init entry at ES:0003.
    push cs
    push word .after_init
    push es
    push word 0x0003
    retf
.after_init:

    ; Advance by ROM size in paragraphs: blocks * 512 / 16 = blocks * 32 = blocks * 0x20.
    xor ah, ah
    mov cl, 5
    shl ax, cl              ; AX = blocks * 32 (paragraphs)
    add bx, ax
    jmp .scan_loop
    
.next_rom:
    ; Skip to next 2KiB boundary (common BIOS scan granularity).
    add bx, 0x0080
    jmp .scan_loop
    
.scan_done:
    POST 0x33
    
    pop es
    pop di
    pop si
    pop cx
    pop bx
    pop ax
    ret

; ── PCI Option ROM Scanning ──────────────────────
pci_option_rom_scan:
    ; TODO: PCI Option ROM discovery by walking PCI config space.
    ; For now, the generic C0000-E0000 scan covers typical emulators.
    jmp option_rom_scan

; ── USB xHCI Controller Detection Stub ──────────
usb_detect:
    push ax
    push bx
    push cx
    push dx
    
    POST 0x34
    
    ; xHCI controllers are PCI devices
    ; Class 0x0C, subclass 0x03, prog IF 0x30
    ; Scan PCI bus for USB controllers
    
    ; Check for xHCI at common locations
    ; Device 0x00-0x1F on bus 0
    xor bh, bh
    mov bl, 0x00        ; Start at devfn 0
    xor cx, cx          ; Count found controllers
    
.usb_scan_loop:
    ; Read class code
    mov dx, 0xCF8
    mov eax, 0x80000000
    mov al, bh
    shl eax, 16
    or eax, 0x00000800  ; Register 8
    out dx, eax
    mov dx, 0xCFC
    in eax, dx
    
    ; Check if USB controller (class 0x0C)
    cmp ah, 0x0C
    jne .not_usb
    
    ; Check subclass
    cmp al, 0x03
    jne .not_usb
    
    ; Found USB controller
    inc cx
    
.not_usb:
    inc bl
    cmp bl, 0x20        ; Scanned 32 devices
    jl .usb_scan_loop
    
    ; Return count in CX
    mov ax, cx
    
    POST 0x35
    
    pop dx
    pop cx
    pop bx
    pop ax
    ret

; ── USB Mass Storage Boot (compatibility) ───────────
usb_storage_boot:
    jmp usb_mass_storage_boot

; ── USB Keyboard (compatibility) ─────────────────────
usb_keyboard_init:
    jmp usb_kbd_init

; ── PXE Network Boot (compatibility) ─────────────────
pxe_init:
    jmp pxe_initialize

pxe_boot:
    jmp pxe_net_boot

; ── Boot Menu Remember Last Choice ───────────────
boot_menu_remember:
    ; Save last boot device to CMOS/NVRAM
    ; AL = device to remember
    push ax
    
    ; Write to CMOS at port 0x70/0x71
    mov al, 0x3E        ; NVRAM location for boot device
    out 0x70, al
    mov al, ah
    out 0x71, al
    
    pop ax
    ret

boot_menu_recall:
    ; Read last boot device from CMOS
    push ax
    
    mov al, 0x3E
    out 0x70, al
    in al, 0x71
    
    pop ax
    ret

; ── Live Reload Signal ────────────────────────────
; Signal hypervisor on halt for live reload
live_reload_signal:
    ; Write to port to signal hypervisor
    mov al, 0x00
    out 0xB0, al        ; Custom port for hypervisor signaling
    ret
