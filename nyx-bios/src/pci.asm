%define PCI_ADDR  0xCF8
%define PCI_DATA  0xCFC

pci_init:
    mov si, str_pci_scan
    call serial_puts

    ; Minimal PCI init: enable I/O decoding + bus mastering on the IDE controller.
    ; This is required on some QEMU machine configs for legacy IDE ports (1F0/3F6).
    call pci_enable_ide_io

    mov si, str_pci_ok_stub
    call serial_puts
    ret

; ── pci_cfg_set_addr ────────────────────────────
; Input: BH=bus, BL=device, CL=function, DL=reg (byte offset)
; Trashes: AX,DX,EAX
pci_cfg_set_addr:
    push bx
    push cx
    push dx
    push bp
    mov bp, sp

    mov eax, 0x80000000

    ; Stack layout (16-bit pushes):
    ; [SS:SP+0]  = original DX (DL=reg)
    ; [SS:SP+2]  = original CX (CL=function)
    ; [SS:SP+4]  = original BX (BH=bus, BL=device)
    ; (use BP because SP-relative addressing is not encodable in 16-bit mode)

    ; bus
    mov bx, [ss:bp+6]
    xor ecx, ecx
    mov cl, bh
    shl ecx, 16
    or eax, ecx

    ; device
    mov bx, [ss:bp+6]
    xor ecx, ecx
    mov cl, bl
    shl ecx, 11
    or eax, ecx

    ; function
    mov cx, [ss:bp+4]
    xor ebx, ebx
    mov bl, cl
    shl ebx, 8
    or eax, ebx

    ; reg (dword aligned)
    mov dx, [ss:bp+2]
    xor ebx, ebx
    mov bl, dl
    and bl, 0xFC
    or eax, ebx

    mov dx, PCI_ADDR
    out dx, eax

    pop bp
    pop dx
    pop cx
    pop bx
    ret

; ── pci_cfg_read_dword ──────────────────────────
; Input: BH=bus, BL=device, CL=function, DL=reg
; Output: EAX=dword
; Trashes: DX
pci_cfg_read_dword:
    call pci_cfg_set_addr
    mov dx, PCI_DATA
    in eax, dx
    ret

; ── pci_cfg_read_word ───────────────────────────
; Input: BH=bus, BL=device, CL=function, DL=reg
; Output: AX=word
; Trashes: BX,DX
pci_cfg_read_word:
    call pci_cfg_set_addr
    xor bx, bx
    mov bl, dl
    and bl, 0x02
    mov dx, PCI_DATA
    add dx, bx
    in ax, dx
    ret

; ── pci_cfg_write_word ──────────────────────────
; Input: BH=bus, BL=device, CL=function, DL=reg, AX=value
; Trashes: BX,DX
pci_cfg_write_word:
    call pci_cfg_set_addr
    xor bx, bx
    mov bl, dl
    and bl, 0x02
    mov dx, PCI_DATA
    add dx, bx
    out dx, ax
    ret

; ── pci_enable_ide_io ───────────────────────────
; Purpose: Find an IDE controller and set PCI CMD.IO + PCI CMD.BUSMASTER.
; Trashes: AX,BX,CX,DX,SI,DI,EAX
pci_enable_ide_io:
    xor di, di              ; device 0..31
.dev_loop:
    xor si, si              ; function 0..7
.fn_loop:
    mov bx, di              ; BL=device
    mov cx, si              ; CL=function
    xor bh, bh              ; bus 0
    mov dl, 0x00
    call pci_cfg_read_word
    cmp ax, 0xFFFF
    je .next_fn             ; no device

    ; Class code at 0x08: [31:24]=class, [23:16]=subclass, [15:8]=prog-if.
    mov dl, 0x08
    call pci_cfg_read_dword
    mov ebx, eax
    shr ebx, 16
    cmp bl, 0x01            ; subclass = IDE
    jne .next_fn
    shr eax, 24
    cmp al, 0x01            ; class = mass storage
    jne .next_fn

    ; Enable command bits in 0x04.
    mov dl, 0x04
    call pci_cfg_read_word
    or ax, 0x0005           ; I/O space + bus master
    mov dl, 0x04
    call pci_cfg_write_word
    jmp .done

.next_fn:
    inc si
    cmp si, 8
    jb .fn_loop
    inc di
    cmp di, 32
    jb .dev_loop
.done:
    ret

pci_int1a:
    cmp al, 0x01
    je .pci_present
    cmp al, 0x02
    je .find_device
    cmp al, 0x03
    je .find_class
    cmp al, 0x08
    je .read_config_byte
    cmp al, 0x09
    je .read_config_word
    cmp al, 0x0A
    je .read_config_dword
    
    mov ah, 0x81            ; Unsupported function
    stc
    ret

.pci_present:
    ; AH=B101h: Installation check
    ; Returns: EDX='PCI ', AL=ver, BX=lastbus, CL=config-mech
    mov edx, 0x20494350     ; 'PCI '
    mov ax, 0x0210          ; Ver 2.1
    mov bx, 0x0000          ; Last bus 0
    mov cl, 0x01            ; Config mechanism 1 (0xCF8/0xCFC)
    mov ah, 0x00            ; Success
    clc
    ret

.find_device:
    ; AH=B102h: Find PCI device
    ; DX=vendor, CX=device, SI=index
    ; Returns: BH=bus, BL=devfn
    push bp
    mov bp, sp
    push dx                 ; [bp-2] = Target Vendor
    push cx                 ; [bp-4] = Target Device
    
    xor di, di              ; current index tracker
.fd_bus:
    xor bx, bx              ; current bus (BH), current devfn (BL)
.fd_dev:
    ; Read Vendor ID (reg 0)
    push bx
    mov cl, bl
    and cl, 0x07            ; function
    shr bl, 3               ; device
    mov bh, 0               ; bus 0 (stub)
    mov dl, 0x00            ; reg 0
    call pci_cfg_read_word
    pop bx
    
    mov dx, [bp-2]          ; target vendor
    cmp ax, dx
    jne .fd_next
    
    ; Read Device ID (reg 2)
    push bx
    mov cl, bl
    and cl, 0x07
    shr bl, 3
    mov bh, 0
    mov dl, 0x02            ; reg 2
    call pci_cfg_read_word
    pop bx
    
    mov dx, [bp-4]          ; target device
    cmp ax, dx
    jne .fd_next
    
    ; Match! Is it the N-th instance (SI)?
    cmp di, si
    je .fd_found
    inc di
    
.fd_next:
    inc bl
    jnz .fd_dev
    ; No device found on this bus
    mov ah, 0x86
    stc
    jmp .fd_done

.fd_found:
    ; BH=bus, BL=devfn already in BX
    mov ah, 0x00
    clc

.fd_done:
    mov sp, bp
    pop bp
    ret

.find_class:
    ; AH=B103h: Find class (stub)
    mov ah, 0x86
    stc
    ret

.read_config_byte:
    ; AH=B108h: Read byte
    ; BH=bus, BL=devfn, DI=reg
    push dx
    mov cl, bl
    and cl, 0x07            ; function
    shr bl, 3               ; device
    mov dx, di              ; reg
    call pci_cfg_read_dword ; read dword first
    mov cx, di
    and cx, 3
    shl cx, 3
    shr eax, cl
    pop dx
    mov ah, 0x00
    clc
    ret

.read_config_word:
    ; AH=B109h: Read word
    push dx
    mov cl, bl
    and cl, 0x07
    shr bl, 3
    mov dx, di
    call pci_cfg_read_word
    pop dx
    mov ah, 0x00
    clc
    ret

.read_config_dword:
    ; AH=B10Ah: Read dword
    push dx
    mov cl, bl
    and cl, 0x07
    shr bl, 3
    mov dx, di
    call pci_cfg_read_dword
    ; Move EAX to ECX safely in 16-bit
    db 0x66, 0x89, 0xC1     ; mov ecx, eax
    pop dx
    mov ah, 0x00
    clc
    ret

str_pci_scan: db 'PCI: scanning stub', 13,10,0
str_pci_ok_stub: db '[OK] PCI stub OK',13,10,0
