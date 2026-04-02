; src/serialstudio.asm — SerialStudio telemetry

%define SS_ENABLED 1

ss_init:
    push ax
    push dx
    ; Capture boot start tick (BDA 0x046C)
    mov ax, [BDA_TICKCOUNT]
    mov dx, [BDA_TICKCOUNT+2]
    mov [NYX_SS_START_TICK_LO], ax
    mov [NYX_SS_START_TICK_HI], dx
    mov [NYX_SS_LAST_TICK_LO], ax
    mov [NYX_SS_LAST_TICK_HI], dx

    ; Emit start marker if enabled
    mov al, [NYX_SERIALSTUDIO_EN]
    test al, al
    jz .done
    mov si, str_ss_boot_start
    call serial_puts
    call serial_puthex32_dxax
    mov si, str_log_nl
    call serial_puts

.done:
    pop dx
    pop ax
    ret

ss_stage:
    ; AL = stage id
    push ax
    push bx
    push cx
    push dx
    push si

    mov bl, al
    mov al, [NYX_SERIALSTUDIO_EN]
    test al, al
    jz .done

    ; Read current tick
    mov ax, [BDA_TICKCOUNT]
    mov dx, [BDA_TICKCOUNT+2]

    ; Compute delta from start in DX:AX
    mov bx, [NYX_SS_START_TICK_LO]
    mov cx, [NYX_SS_START_TICK_HI]
    sub ax, bx
    sbb dx, cx

    call ss_stage_name

    mov si, str_ss_ticks
    call serial_puts
    call serial_puthex32_dxax

    mov si, str_log_nl
    call serial_puts

.done:
    pop si
    pop dx
    pop cx
    pop bx
    pop ax
    ret

ss_log_boot_device:
    ; DL = boot device
    push ax
    push si

    mov al, [NYX_SERIALSTUDIO_EN]
    test al, al
    jz .done

    cmp dl, 0x80
    je .hdd
    cmp dl, 0xE0
    je .cd
    cmp dl, 0x81
    je .net
    jmp .done

.hdd:
    mov si, str_ss_hdd
    jmp .emit
.cd:
    mov si, str_ss_cd
    jmp .emit
.net:
    mov si, str_ss_net
.emit:
    call serial_puts
.done:
    pop si
    pop ax
    ret

ss_stage_name:
    ; BL = stage id, emits "SS,stage=<name>"
    push ax
    push si

    mov si, str_ss_stage
    call serial_puts

    cmp bl, 0x01
    je .serial
    cmp bl, 0x02
    je .pic
    cmp bl, 0x03
    je .pit
    cmp bl, 0x04
    je .video
    cmp bl, 0x05
    je .bda
    cmp bl, 0x06
    je .mem
    cmp bl, 0x07
    je .ivt
    cmp bl, 0x08
    je .pci
    cmp bl, 0x09
    je .disk
    cmp bl, 0x0A
    je .kbd
    cmp bl, 0x0B
    je .acpi
    cmp bl, 0x0C
    je .smbios
    cmp bl, 0x0D
    je .dma
    cmp bl, 0x0E
    je .rtc
    cmp bl, 0x10
    je .menu_enter
    cmp bl, 0x11
    je .menu_exit
    cmp bl, 0x12
    je .handoff
    mov si, str_ss_unknown
    jmp .emit

.serial:    mov si, str_ss_serial
    jmp .emit
.pic:       mov si, str_ss_pic
    jmp .emit
.pit:       mov si, str_ss_pit
    jmp .emit
.video:     mov si, str_ss_video
    jmp .emit
.bda:       mov si, str_ss_bda
    jmp .emit
.mem:       mov si, str_ss_mem
    jmp .emit
.ivt:       mov si, str_ss_ivt
    jmp .emit
.pci:       mov si, str_ss_pci
    jmp .emit
.disk:      mov si, str_ss_disk
    jmp .emit
.kbd:       mov si, str_ss_kbd
    jmp .emit
.acpi:      mov si, str_ss_acpi
    jmp .emit
.smbios:    mov si, str_ss_smbios
    jmp .emit
.dma:       mov si, str_ss_dma
    jmp .emit
.rtc:       mov si, str_ss_rtc
    jmp .emit
.menu_enter: mov si, str_ss_menu_enter
    jmp .emit
.menu_exit:  mov si, str_ss_menu_exit
    jmp .emit
.handoff:    mov si, str_ss_handoff

.emit:
    call serial_puts
    pop si
    pop ax
    ret

serial_puthex32_dxax:
    ; DX:AX = value
    push ax
    push dx
    mov ax, dx
    call serial_puthex16
    pop dx
    pop ax
    call serial_puthex16
    ret

str_ss_boot_start: db 'SS,boot_start_ticks=0x', 0
str_ss_stage:      db 'SS,stage=', 0
str_ss_ticks:      db ',ticks=0x', 0
str_ss_hdd:        db 'SS,boot_device=hdd', 13, 10, 0
str_ss_cd:         db 'SS,boot_device=cdrom', 13, 10, 0
str_ss_net:        db 'SS,boot_device=pxe', 13, 10, 0

str_ss_serial:     db 'serial_init', 0
str_ss_pic:        db 'pic_init', 0
str_ss_pit:        db 'pit_init', 0
str_ss_video:      db 'video_init', 0
str_ss_bda:        db 'bda_init', 0
str_ss_mem:        db 'mem_detect', 0
str_ss_ivt:        db 'ivt_init', 0
str_ss_pci:        db 'pci_init', 0
str_ss_disk:       db 'disk_detect', 0
str_ss_kbd:        db 'kbd_init', 0
str_ss_acpi:       db 'acpi_tables', 0
str_ss_smbios:     db 'smbios_init', 0
str_ss_dma:        db 'dma_init', 0
str_ss_rtc:        db 'rtc_init', 0
str_ss_menu_enter: db 'boot_menu_enter', 0
str_ss_menu_exit:  db 'boot_menu_exit', 0
str_ss_handoff:    db 'boot_handoff', 0
str_ss_unknown:    db 'unknown', 0
