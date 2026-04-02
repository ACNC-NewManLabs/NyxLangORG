; src/beep.asm — Beep codes and additional TIER 4 features

; ── PC Speaker Beep ─────────────────────────────
%define PIT_PORT  0x40
%define PIT_CMD   0x43
%define PIT_CH2   0x42
%define SPEAKER_PORT  0x61

; ── Beep Function ────────────────────────────────
; AL = frequency divisor, AH = duration (in 1/18 sec)
beep:
    push ax
    push cx
    push dx
    
    ; Set up PIT channel 2 for square wave
    mov al, 0xB6        ; Channel 2, lobyte/hibyte, square wave
    out PIT_CMD, al
    
    ; Set frequency
    mov al, ah          ; Use AH as frequency divisor
    out PIT_CH2, al
    mov al, 0x00
    out PIT_CH2, al
    
    ; Enable speaker
    in al, SPEAKER_PORT
    or al, 0x03
    out SPEAKER_PORT, al
    
    ; Wait for duration
    mov cx, ax          ; CX = duration
.wait_beep:
    loop .wait_beep
    
    ; Disable speaker
    in al, SPEAKER_PORT
    and al, 0xFC
    out SPEAKER_PORT, al
    
    pop dx
    pop cx
    pop ax
    ret

; ── POST Beep Codes ──────────────────────────────
; Standard POST beep codes for BIOS errors

post_beep:
    ; AX = beep code
    ; Each code has a specific meaning
    
    cmp ax, 0x0101
    je .beep_1_short
    cmp ax, 0x0102
    je .beep_2_short
    cmp ax, 0x0103
    je .beep_3_short
    cmp ax, 0x0201
    je .beep_1_long
    cmp ax, 0x0202
    je .beep_2_long
    
    ; Default beep
    mov al, 0x00        ; Low frequency
    mov ah, 20          ; Duration
    call beep
    ret

.beep_1_short:
    ; 1 short beep - BIOS ROM checksum error
    mov al, 0x00
    mov ah, 5
    call beep
    ret

.beep_2_short:
    ; 2 short beeps - POST memory test error
    mov al, 0x00
    mov ah, 5
    call beep
    call beep
    ret

.beep_3_short:
    ; 3 short beeps - Keyboard controller error
    mov al, 0x00
    mov ah, 5
    call beep
    call beep
    call beep
    ret

.beep_1_long:
    ; 1 long beep - Video error
    mov al, 0x00
    mov ah, 30
    call beep
    ret

.beep_2_long:
    ; 2 long beeps - Hard disk error
    mov al, 0x00
    mov ah, 30
    call beep
    call beep
    ret

; ── ATA Security Feature Set (Password) ──────────
ata_security_init:
    ; Initialize ATA security - stub
    ; Would implement password handling
    ret

ata_security_unlock:
    ; AL = password mode (user/master), DS:SI = password
    ; Returns: CF clear if success
    stc
    ret

ata_security_set_password:
    ; AL = password mode, DS:SI = password
    stc
    ret

ata_security_disable:
    ; AL = password mode, DS:SI = password
    stc
    ret

; ── SMP Secondary CPU Startup (compatibility) ───
smp_init:
    jmp smp_initialize

; ── Enhanced USB Mass Storage Boot (compatibility) ────
usb_storage_detect:
    jmp usb_init

usb_storage_read:
    jmp usb_mass_storage_boot

usb_storage_attach:
    ret

; ── Enhanced USB Keyboard (compatibility) ────────────
usb_keyboard_detect:
    jmp usb_kbd_detect

usb_keyboard_init_full:
    jmp usb_kbd_init

usb_keyboard_read:
    jmp usb_kbd_read

; ── VBE EDID Passthrough ────────────────────────────
vbe_get_edid:
    ; Read monitor EDID
    ; ES:DI = buffer for EDID data (128 bytes)
    ; Returns: AX = 0x004F if success
    push es
    push di
    push cx
    
    ; Try to read EDID via DDC
    ; Use port 0xA0/0xA1 for DDC1
    ; or port 0x6E/0x6F for DDC2
    
    mov cx, 128
    xor al, al
    
    ; Try reading 128 bytes of EDID
    ; This is a simplified version
    
    pop cx
    pop di
    pop es
    
    mov ax, 0x014F    ; Not supported
    stc
    iret

; ── PXE DHCP Implementation (calls full implementation) ──
pxe_dhcp_discover:
    jmp pxe_do_dhcp_discover

pxe_tftp_read:
    jmp pxe_do_tftp_read

pxe_boot_info:
    ret
