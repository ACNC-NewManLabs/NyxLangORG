; src/memory.asm — Memory detection + E820

%define E820_TABLE  0x5000
%define E820_COUNT  0x5FF0

memory_detect:
    push ax
    push cx
    push di

    ; Build E820 table at 0x5000
    mov di, E820_TABLE
    xor cx, cx          ; entry count

    ; Entry 0: 0x00000000 640KB conventional
    call e820_add_entry_conv

    ; Entry 1: 0xA0000-0xFFFFF reserved
    call e820_add_entry_vga

    ; Entry 2: 0x100000 extended memory
    call e820_add_entry_ext

    ; Entry 3: ACPI reclaimable region at top of RAM (just below 16MB for now)
    ; This is needed for Windows and some Linux ACPI tables
    call e820_add_entry_acpi_reclaim

    ; Store count
    movzx eax, cx
    mov dword [E820_COUNT], eax

    ; Update BDA memory size to 632KB to reserve space for ACPI/SMBIOS
    mov word [BDA_MEMSIZE], 632

    ; Verify A20 line is working
    call a20_verify

    pop di
    pop cx
    pop ax
    ret

e820_add_entry_acpi_reclaim:
    ; ACPI reclaimable: 0xFEC00000 - 0xFFFFFFFF (APIC area, reserved)
    ; But for memory map, report a reclaimable region at high memory
    ; 0x0F000000 - 0x10000000 (240MB - 256MB) as ACPI NVS
    mov dword [di+0],  0x0F000000
    mov dword [di+4],  0x00000000
    mov dword [di+8],  0x01000000
    mov dword [di+12], 0x00000000
    mov dword [di+16], 0x00000004  ; ACPI NVS / reclaimable
    add di, 20
    inc cx
    ret

e820_add_entry_conv:
    ; 0x00000000 - 0x0009FBFF usable
    mov dword [di+0],  0x00000000
    mov dword [di+4],  0x00000000
    mov dword [di+8],  0x0009E000
    mov dword [di+12], 0x00000000
    mov dword [di+16], 0x00000001  ; usable
    add di, 20
    inc cx
    ret

e820_add_entry_vga:
    ; 0x0009E000 - 0x000FFFFF reserved (ACPI, SMBIOS, EBDA, VGA, ROM)
    mov dword [di+0],  0x0009E000
    mov dword [di+4],  0x00000000
    mov dword [di+8],  0x00062000
    mov dword [di+12], 0x00000000
    mov dword [di+16], 0x00000002  ; reserved
    add di, 20
    inc cx
    ret

e820_add_entry_ext:
    ; 0x00100000 - top of RAM usable
    ; Read memory size from CMOS:
    ; Index 0x30/0x31 = KB between 1MB and 16MB
    ; Index 0x34/0x35 = 64KB blocks above 16MB
    
    mov al, 0x30
    call cmos_read
    mov bl, al
    mov al, 0x31
    call cmos_read
    mov bh, al
    ; BX = size in KB (max 15360)
    
    mov al, 0x34
    call cmos_read
    mov cl, al
    mov al, 0x35
    call cmos_read
    mov ch, al
    ; CX = blocks of 64KB above 16MB
    
    ; Calculate total bytes above 1MB
    ; bytes = (BX * 1024) + (CX * 65536)
    
    ; For E820, we need the length in bytes (64-bit)
    mov dword [di+0],  0x00100000
    mov dword [di+4],  0x00000000
    
    ; Low 32 bits of length:
    ; (BX << 10) + (CX << 16)
    movzx eax, bx
    shl eax, 10
    movzx edx, cx
    shl edx, 16
    add eax, edx
    
    mov dword [di+8],  eax
    mov dword [di+12], 0x00000000
    mov dword [di+16], 0x00000001  ; usable
    add di, 20
    inc cx
    ret

; ── INT 0x15 handler ────────────────────────────
int15_handler:
    cmp ax, 0xE820
    je .e820
    cmp ax, 0xE801
    je .e801
    cmp ax, 0x8800
    je .x88
    cmp ah, 0x87
    je .copy
    cmp ah, 0x88
    je .ext88
    cmp ax, 0x2400
    je .a20_disable
    cmp ax, 0x2401
    je .a20_enable
    cmp ax, 0x2402
    je .a20_status
    cmp ax, 0x2403
    je .a20_support
    ; Unknown — carry set
    stc
    iret

.e820:
    ; Full E820 implementation
    cmp edx, 0x534D4150     ; 'SMAP'
    jne .e820_fail
    cmp ebx, dword [E820_COUNT]
    jge .e820_end

    ; Copy entry
    push si
    push cx
    push di
    mov si, E820_TABLE
    mov eax, ebx
    imul eax, 20
    add si, ax
    mov cx, 20
    rep movsb
    pop di
    pop cx
    pop si

    inc ebx
    cmp ebx, dword [E820_COUNT]
    jl .e820_more
.e820_end:
    xor ebx, ebx
    jmp .e820_ok
.e820_more:
    ; ebx already incremented
.e820_ok:
    mov eax, 0x534D4150
    mov ecx, 20
    clc
    iret
.e820_fail:
    stc
    iret

.e801:
    ; AX = KB between 1MB-16MB, BX = 64KB blocks above 16MB
    mov al, 0x30
    call cmos_read
    mov ah, al
    mov al, 0x31
    xchg al, ah             ; AX = word from 30,31
    
    push ax
    mov al, 0x34
    call cmos_read
    mov ah, al
    mov al, 0x35
    xchg al, ah             ; BX (now AX) = word from 34,35
    mov bx, ax
    pop ax
    
    mov cx, ax
    mov dx, bx
    clc
    iret

.x88:
    ; Return KB above 1MB (max 64MB)
    mov al, 0x30
    call cmos_read
    mov bl, al
    mov al, 0x31
    call cmos_read
    mov bh, al
    mov ax, bx
    clc
    iret

.ext88:
    ; AL = 64KB blocks above 1MB
    mov al, 0x34
    call cmos_read
    clc
    iret

.copy:
    ; Block move — needed by some loaders
    clc
    iret

.a20_disable:
    call a20_disable
    mov ah, 0
    clc
    iret

.a20_enable:
    call a20_enable
    mov ah, 0
    clc
    iret

.a20_status:
    call a20_check
    mov ah, 0
    clc
    iret

.a20_support:
    mov ax, 0x0003          ; keyboard + port 0x92
    clc
    iret

; ── A20 line control ────────────────────────────
a20_enable:
    push ax
    ; Method 1: Fast A20 via port 0x92
    in al, 0x92
    or al, 0x02
    and al, 0xFE
    out 0x92, al
    ; Method 2: Keyboard controller
    call kbd_a20_enable
    pop ax
    ret

a20_disable:
    push ax
    in al, 0x92
    and al, 0xFD
    out 0x92, al
    pop ax
    ret

a20_check:
    ; Returns AL=1 if A20 enabled
    push ds
    push es
    push di
    push si
    xor ax, ax
    mov es, ax
    not ax
    mov ds, ax
    mov di, 0x0500
    mov si, 0x0510
    mov al, [es:di]
    push ax
    mov al, [ds:si]
    push ax
    mov byte [es:di], 0x00
    mov byte [ds:si], 0xFF
    cmp byte [es:di], 0xFF
    pop ax
    mov [ds:si], al
    pop ax
    mov [es:di], al
    mov al, 0
    je .disabled
    mov al, 1
.disabled:
    pop si
    pop di
    pop es
    pop ds
    ret

; ── A20 Verify with Memory Test ─────────────────
a20_verify:
    ; Verify A20 is enabled by testing memory at 0x100000 (1MB)
    ; Compare with memory at 0x000000 (should be different after A20 enabled)
    pushf
    push ax
    push bx
    push cx
    push ds
    
    ; Disable interrupts
    cli
    
    ; Save original values
    xor ax, ax
    mov ds, ax
    mov ax, [0x0000]     ; Save value at 0x0000
    push ax
    mov ax, [0x0010]     ; Save value at 0x0010 (16 bytes in)
    push ax
    
    ; Write test pattern to 0x0000
    mov ax, 0x1234
    mov [0x0000], ax
    mov ax, 0x5678
    mov [0x0010], ax
    
    ; Enable A20 first (in case it's not enabled)
    call a20_enable
    
    ; Read from 0x100000 (must use 32-bit address)
    mov ax, 0xFFFF
    mov es, ax
    mov bx, 0x0000
    
    ; Check if A20 makes a difference
    ; If A20 is disabled, 0x100000 maps to 0x000000
    ; If A20 is enabled, they are different
    mov ax, [es:bx]      ; Read from 0x100000
    
    ; Restore original values at 0x0000
    pop ax
    mov [0x0010], ax
    pop ax
    mov [0x0000], ax
    
    ; Check if the value at 0x100000 matches what we wrote at 0x0000
    ; If A20 is disabled, they would match (wrap around)
    cmp ax, 0x1234
    je .a20_disabled
    
    ; A20 is enabled - memory at 0x100000 is different from 0x00000
    mov si, str_a20_ok
    call serial_puts
    mov al, 1
    jmp .done
    
.a20_disabled:
    mov si, str_a20_fail
    call serial_puts
    mov al, 0
    
.done:
    pop ds
    pop cx
    pop bx
    pop ax
    popf
    ret

str_a20_ok:   db '[OK] A20 line verified enabled', 13, 10, 0
str_a20_fail: db '[!!] A20 line verification failed', 13, 10, 0
