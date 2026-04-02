; src/boot.asm — Boot device manager

%define BOOT_DRIVE_HD    0x80
%define BOOT_DRIVE_CDROM 0xE0
%define BOOT_DRIVE_NET   0x81

; ── boot_sequence ─────────────────────────────────────────────────────────────
; Shows the interactive boot menu, then boots the selected device.
; Falls back through remaining devices if the selection fails.
boot_sequence:
    POST 0x19
    mov si, str_boot_seq
    call serial_puts

    ; Show interactive boot menu — returns DL = selected device
    call boot_menu

    ; DL = device chosen by user (or timeout default)
    ; Try the chosen device first, then fall back to others.
    cmp dl, BOOT_DRIVE_HD
    je .try_hd_first
    cmp dl, BOOT_DRIVE_CDROM
    je .try_cd_first
    cmp dl, BOOT_DRIVE_NET
    je .try_net_first

    ; Unknown — default to HDD
    jmp .try_hd_first

.try_hd_first:
    POST 0x20
    mov si, str_try_hd
    call serial_puts
    call hd_boot
    jnc .done
    POST 0x21
    mov si, str_try_cdrom
    call serial_puts
    call cdrom_boot
    jnc .done
    jmp .no_boot

.try_cd_first:
    POST 0x20
    mov si, str_try_cdrom
    call serial_puts
    call cdrom_boot
    jnc .done
    POST 0x21
    mov si, str_try_hd
    call serial_puts
    call hd_boot
    jnc .done
    jmp .no_boot

.try_net_first:
    ; PXE stub — fall through to HDD
    POST 0x20
    mov si, str_try_pxe
    call serial_puts
    POST 0x21
    mov si, str_try_hd
    call serial_puts
    call hd_boot
    jnc .done
    call cdrom_boot
    jnc .done
    jmp .no_boot

.no_boot:
    POST 0x22
    mov si, str_no_boot
    call serial_puts
    stc
    ret

.done:
    clc
    ret

; ── hd_boot ───────────────────────────────────────────────────────────────────
; Read MBR from 0x80, validate 0xAA55, jump to 0000:7C00.
hd_boot:
    POST 0x40
    push ds
    push es
    xor ax, ax
    mov ds, ax
    mov es, ax

    ; Build DAP at 0x7000
    mov word [0x7000+0],  16    ; size of DAP
    mov word [0x7000+2],   1    ; sectors to read
    mov word [0x7000+4], 0x7C00 ; destination offset
    mov word [0x7000+6],   0    ; destination segment
    mov dword [0x7000+8],  0    ; LBA low  (sector 0 = MBR)
    mov dword [0x7000+12], 0    ; LBA high

    mov ah, 0x42
    mov dl, BOOT_DRIVE_HD
    mov si, 0x7000
    int 0x13
    jc .fail_int13

    ; Verify MBR signature
    cmp word [0x7DFE], 0xAA55
    jne .fail_sig

    POST 0x41
    mov si, str_mbr_ok
    call serial_puts

    mov dl, BOOT_DRIVE_HD
    pop es
    pop ds
    xor ax, ax
    mov ds, ax
    mov es, ax
    jmp 0x0000:0x7C00

.fail_int13:
    mov si, str_hd_int13_fail
    call serial_puts
    mov ax, ax              ; AH already has error code from INT 13h
    call serial_puthex16
    mov si, str_log_nl
    call serial_puts
    pop es
    pop ds
    stc
    ret

.fail_sig:
    mov si, str_hd_sig_fail
    call serial_puts
    mov ax, [0x7DFE]
    call serial_puthex16
    mov si, str_log_nl
    call serial_puts
    pop es
    pop ds
    stc
    ret

; ── Strings ───────────────────────────────────────────────────────────────────
str_boot_seq:      db '[  ] Boot sequence starting', 13, 10, 0
str_try_hd:        db '[  ] Trying hard disk...', 13, 10, 0
str_try_cdrom:     db '[  ] Trying CD-ROM...', 13, 10, 0
str_try_pxe:       db '[  ] Network boot (PXE) — not implemented, trying HDD', 13, 10, 0
str_mbr_ok:        db '[OK] MBR loaded, jumping to 0000:7C00', 13, 10, 0
str_no_boot:       db '[!!] No bootable device found!', 13, 10, 0
str_hd_int13_fail: db '[!!] HDD INT 13h failed, AH=', 0
str_hd_sig_fail:   db '[!!] MBR bad signature: ', 0
