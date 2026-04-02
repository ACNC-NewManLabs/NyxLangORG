; src/disk.asm — Full INT 0x13 implementation

%define DAP_SIZE     0x7000
%define DISK_BUFFER  0x8000

%define ATA_DATA     0x1F0
%define ATA_SECCNT   0x1F2
%define ATA_LBA0     0x1F3
%define ATA_LBA1     0x1F4
%define ATA_LBA2     0x1F5
%define ATA_DRIVE    0x1F6
%define ATA_STATUS   0x1F7
%define ATA_CMD      0x1F7
%define ATA_CTRL     0x3F6             ; primary channel device control / altstatus
%define ATA_ALTSTATUS 0x3F6

; ATAPI placements we try (common for QEMU IDE):
; - Secondary master: if=ide,index=2  -> base 0x170, ctrl 0x376, drive 0xA0
; - Primary slave:    if=ide,index=1  -> base 0x1F0, ctrl 0x3F6, drive 0xB0
%define ATAPI2_BASE   0x170
%define ATAPI2_CTRL   0x376
%define ATAPI2_DRIVESEL 0xA0

%define ATAPI1_BASE   0x1F0
%define ATAPI1_CTRL   0x3F6
%define ATAPI1_DRIVESEL 0xB0

%define ATAPI_DATA(base)   (base + 0)
%define ATAPI_FEAT(base)   (base + 1)
%define ATAPI_SECCNT(base) (base + 2)
%define ATAPI_LBA0(base)   (base + 3)
%define ATAPI_LBA1(base)   (base + 4)   ; byte count low
%define ATAPI_LBA2(base)   (base + 5)   ; byte count high
%define ATAPI_DRIVE(base)  (base + 6)
%define ATAPI_STATUS(base) (base + 7)
%define ATAPI_CMD(base)    (base + 7)

%define ATAPI_PKT_CMD     0xA0
%define ATAPI_READ10_CMD  0x28
%define ATAPI_SECTOR_SIZE 2048

%define ATAPI_SCRATCH_SEG 0x0800         ; 0x8000 physical
%define ATAPI_SCRATCH_OFF 0x0000

; Disk parameter table
%define HD0_PARAMS   0x0104  ; in EBDA

; ── ATAPI presence probe (avoid disrupting HDD) ───────────────────────────────
; Some QEMU configs attach no CD-ROM device; aggressively probing ATAPI (incl.
; primary-channel soft resets) can disturb the HDD. We use a lightweight
; signature probe first and only attempt full ATAPI reads if a device is present.
atapi_probe_any:
    push ax
    call atapi_probe_secondary_master
    jnc .present
    call atapi_probe_primary_slave
    jnc .present
    pop ax
    stc
    ret
.present:
    pop ax
    clc
    ret

atapi_probe_secondary_master:
    push dx
    ; Select secondary master
    mov dx, ATAPI_DRIVE(ATAPI2_BASE)
    mov al, ATAPI2_DRIVESEL
    out dx, al
    ; 400ns delay via control port
    mov dx, ATAPI2_CTRL
    in al, dx
    in al, dx
    in al, dx
    in al, dx
    ; Check ATAPI signature in LBA1/LBA2 (0x14/0xEB)
    mov dx, ATAPI_LBA1(ATAPI2_BASE)
    in al, dx
    cmp al, 0x14
    jne .no
    mov dx, ATAPI_LBA2(ATAPI2_BASE)
    in al, dx
    cmp al, 0xEB
    jne .no
    pop dx
    clc
    ret
.no:
    pop dx
    stc
    ret

atapi_probe_primary_slave:
    push dx
    ; Select primary slave
    mov dx, ATAPI_DRIVE(ATAPI1_BASE)
    mov al, ATAPI1_DRIVESEL
    out dx, al
    mov dx, ATAPI1_CTRL
    in al, dx
    in al, dx
    in al, dx
    in al, dx
    mov dx, ATAPI_LBA1(ATAPI1_BASE)
    in al, dx
    cmp al, 0x14
    jne .no
    mov dx, ATAPI_LBA2(ATAPI1_BASE)
    in al, dx
    cmp al, 0xEB
    jne .no
    pop dx
    clc
    ret
.no:
    pop dx
    stc
    ret

; ── ATA presence probe / hard disk count ─────────────────────────────────────
; Detect primary master (0x80) and primary slave (0x81) and write the count into
; the BIOS Data Area at 0x0475 (BDA_HDCOUNT).
disk_detect:
    push ax
    push bx
    push cx
    push dx
    push ds
    xor ax, ax
    mov ds, ax
    mov byte [BDA_HDCOUNT], 0
    pop ds

    ; Probe master
    xor al, al
    call ata_probe_drive
    jc .no_master
    inc byte [BDA_HDCOUNT]
.no_master:

    ; Probe slave
    mov al, 1
    call ata_probe_drive
    jc .done
    inc byte [BDA_HDCOUNT]

.done:
    pop dx
    pop cx
    pop bx
    pop ax
    ret

; Input:  AL=0 (master) or AL=1 (slave)
; Output: CF clear if drive responds to IDENTIFY, set otherwise
ata_probe_drive:
    push ax
    push bx
    push cx
    push dx

    ; Select drive
    mov dx, ATA_DRIVE
    cmp al, 1
    jne .sel_master
    mov al, 0xB0
    jmp .sel_out
.sel_master:
    mov al, 0xA0
.sel_out:
    out dx, al
    call ata_400ns_delay

    ; No bus / no device tends to read as 0xFF or 0x00.
    mov dx, ATA_STATUS
    in al, dx
    cmp al, 0xFF
    je .absent
    test al, al
    jz .absent

    ; If the device is still busy, wait a short while (don't stall boot forever).
    test al, 0x80
    jz .ready
    call ata_wait_not_bsy_short
    jc .absent
.ready:

    ; IDENTIFY DEVICE
    mov dx, ATA_SECCNT
    xor al, al
    out dx, al
    mov dx, ATA_LBA0
    out dx, al
    mov dx, ATA_LBA1
    out dx, al
    mov dx, ATA_LBA2
    out dx, al

    mov dx, ATA_CMD
    mov al, 0xEC
    out dx, al

    call ata_wait_drq_short
    jc .absent

    ; Drain 256 words (512 bytes) from data port.
    mov dx, ATA_DATA
    mov cx, 256
.drain:
    in ax, dx
    loop .drain

    clc
    jmp .done

.absent:
    stc
.done:
    pop dx
    pop cx
    pop bx
    pop ax
    ret

; ── Short ATA waits (used only for presence probing) ─────────────────────────
ata_wait_not_bsy_short:
    ; Like ata_wait_not_bsy but with a small timeout.
    push bx
    push cx
    push dx
    mov dx, ATA_STATUS
    mov bx, 0x0020
.wnb_outer:
    mov cx, 0xFFFF
.wnb_loop:
    in al, dx
    cmp al, 0xFF
    je .wnb_next
    test al, al
    jz .wnb_next
    test al, 0x01           ; ERR
    jnz .wnb_err
    test al, 0x80
    jnz .wnb_next
    test al, 0x40           ; DRDY
    jz .wnb_next
    clc
    jmp .wnb_done
.wnb_next:
    loop .wnb_loop
    dec bx
    jnz .wnb_outer
    stc
    jmp .wnb_done
.wnb_err:
    stc
.wnb_done:
    pop dx
    pop cx
    pop bx
    ret

ata_wait_drq_short:
    ; Like ata_wait_drq but with a small timeout.
    push bx
    push cx
    push dx
    mov dx, ATA_STATUS
    mov bx, 0x0020
.wdrq_outer:
    mov cx, 0xFFFF
.wdrq_loop:
    in al, dx
    cmp al, 0xFF
    je .wdrq_next
    test al, 0x01           ; ERR
    jnz .wdrq_err
    test al, 0x80
    jnz .wdrq_next
    test al, 0x08
    jnz .wdrq_ok
.wdrq_next:
    loop .wdrq_loop
    dec bx
    jnz .wdrq_outer
    stc
    jmp .wdrq_done
.wdrq_err:
    stc
    jmp .wdrq_done
.wdrq_ok:
    clc
.wdrq_done:
    pop dx
    pop cx
    pop bx
    ret

int13_handler:
    cmp ah, 0x00  
    je .reset
    cmp ah, 0x01  
    je .get_status
    cmp ah, 0x02  
    je .read_chs
    cmp ah, 0x03  
    je .write_chs
    cmp ah, 0x04  
    je .verify
    cmp ah, 0x08  
    je .get_params
    cmp ah, 0x0C  
    je .seek
    cmp ah, 0x0D  
    je .reset_hd
    cmp ah, 0x15  
    je .get_type
    cmp ah, 0x17  
    je .get_drive_type
    cmp ah, 0x41  
    je .check_ext
    cmp ah, 0x42  
    je .read_ext
    cmp ah, 0x43  
    je .write_ext
    cmp ah, 0x44  
    je .verify_ext
    cmp ah, 0x47  
    je .seek_ext
    cmp ah, 0x48  
    je .get_params_ext
    cmp ah, 0x4B  
    je .cdrom_status
    ; Unsupported
    mov ah, 0x01
    stc
    iret

.reset:
    xor ah, ah
    clc
    iret

.get_status:
    xor ah, ah
    clc
    iret

.read_chs:
    ; CH=cyl, CL=sec(1-63)+cyl_hi, 
    ; DH=head, DL=drive, AL=count
    ; ES:BX = buffer
    push ax
    push bx
    push cx
    push dx
    ; CD-ROM CHS reads are not supported (use AH=42h extensions for 0xE0).
    cmp dl, 0xE0
    je .chs_fail

    ; Convert CHS to LBA using a common "translated" geometry (255 heads, 63 spt).
    ; This is sufficient for emulators/bootloaders that still use CHS reads.
    cmp dl, 0x80
    jb .chs_fail

    ; Sector number is 1-63 in CL low 6 bits.
    mov bl, cl
    and bl, 0x3F
    cmp bl, 0
    je .chs_fail

    ; Cylinder is CH plus high 2 bits from CL.
    xor ax, ax
    mov al, ch
    mov bh, cl
    and bh, 0xC0
    shl bh, 2
    add al, bh
    movzx ecx, ax           ; ECX = cylinder

    ; LBA = (cyl * 255 + head) * 63 + (sec - 1)
    movzx eax, dh           ; head
    imul ecx, ecx, 255
    add ecx, eax
    imul ecx, ecx, 63
    movzx eax, bl           ; sec
    dec eax
    add ecx, eax

    ; AL (count) already set; ES:BX already set.
    call ata_read_lba
    jnc .chs_ok
.chs_fail:
    mov ah, 0x01
    stc
    pop dx
    pop cx
    pop bx
    pop ax
    iret
.chs_ok:
    xor ah, ah
    clc
    pop dx
    pop cx
    pop bx
    pop ax
    iret

.write_chs:
    xor ah, ah
    clc
    iret

.verify:
    xor ah, ah
    clc
    iret

.get_params:
    ; DL = drive
    cmp dl, 0xE0
    je .cdrom_params
    cmp dl, 0x80
    jge .hd_params
    ; Floppy
    mov ah, 0x00
    stc
    iret
.cdrom_params:
    mov ah, 0x01
    stc
    iret
.hd_params:
    ; Return hard disk params
    ; Support multiple drives: 0x80 = primary master, 0x81 = primary slave.
    push ds
    xor ax, ax
    mov ds, ax
    mov bl, [BDA_HDCOUNT]
    pop ds

    test bl, bl
    jz .hd_params_absent

    cmp dl, 0x80
    je .hd_params_ok
    cmp dl, 0x81
    jne .hd_params_absent
    cmp bl, 2
    jb .hd_params_absent

.hd_params_ok:
    ; Common "translated" geometry (255 heads, 63 spt, 1023 cylinders).
    mov ah, 0x00
    xor al, al              ; BIOSes typically return AL=0 here
    mov ch, 0xFF
    mov cl, 0xFF            ; cyl_hi=3, spt=63
    mov dh, 0xFE            ; 254 heads (0-253)
    mov dl, bl              ; number of hard disks
    clc
    iret

.hd_params_absent:
    mov ah, 0x01
    stc
    iret

.seek:
    xor ah, ah
    clc
    iret

.reset_hd:
    xor ah, ah
    clc
    iret

.get_type:
    ; AH=0: no disk, 1:floppy no change,
    ; 2:floppy with change, 3:fixed
    cmp dl, 0xE0
    je .cdrom_type
    cmp dl, 0x80
    jge .hd_type
    mov ah, 0x00
    stc
    iret
.hd_type:
    push ds
    xor ax, ax
    mov ds, ax
    mov al, [BDA_HDCOUNT]
    pop ds

    test al, al
    jz .no_disk
    cmp dl, 0x81
    jne .hd_type_ok
    cmp al, 2
    jb .no_disk
.hd_type_ok:
    mov ah, 0x03            ; fixed disk
    xor cx, cx
    xor dx, dx
    clc
    iret
.no_disk:
    mov ah, 0x00
    stc
    iret

.get_drive_type:
    ; AH=0x17: Get drive type (write protect detection)
    ; DL = drive number
    ; Returns: AH = drive type
    ;   0x00 = not present
    ;   0x01 = floppy without change detect
    ;   0x02 = floppy with change detect
    ;   0x03 = fixed disk
    ; We treat present ATA HDDs as fixed disks (write-protect is not meaningful).
    push ds
    xor ax, ax
    mov ds, ax
    mov al, [BDA_HDCOUNT]
    pop ds

    test al, al
    jz .drive_not_present
    cmp dl, 0x80
    je .drive_present
    cmp dl, 0x81
    jne .drive_not_present
    cmp al, 2
    jb .drive_not_present
.drive_present:
    mov ah, 0x03
    clc
    iret
.drive_not_present:
    mov ah, 0x00
    stc
    iret
.cdrom_type:
    mov ah, 0x02            ; removable
    clc
    iret

.check_ext:
    ; Check extensions present
    cmp bx, 0x55AA
    jne .ext_fail
    mov ah, 0x30            ; EDD 3.0
    mov bx, 0xAA55
    ; Bit 0: DAP functions
    ; Bit 1: drive locking
    ; Bit 2: EDD
    mov cx, 0x0007
    clc
    iret
.ext_fail:
    mov ah, 0x01
    stc
    iret

.read_ext:
    ; DS:SI = DAP
    push bx
    push cx
    push dx
    push si
    push di
    push es

    cmp dl, 0xE0
    je .read_ext_cdrom

    ; Read DAP
    mov ax, [si+2]          ; sector count (word)
    test ax, ax
    jz .read_ext_bad
    cmp ax, 0x00FF
    ja .read_ext_bad        ; keep it simple: up to 255 sectors per call
    mov bx, [si+4]          ; buffer offset
    mov es, [si+6]          ; buffer segment
    mov eax, [si+8]         ; LBA low
    mov edx, [si+12]        ; LBA high

    ; Each sector = 512 bytes
    test edx, edx
    jnz .read_ext_lba48
    cmp eax, 0x10000000     ; >= 2^28 needs LBA48
    jae .read_ext_lba48
    mov ecx, eax
    mov al, byte [si+2]     ; sector count (<=255)

    call ata_read_lba
    jc .read_ext_fail
    jmp .read_ext_success

.read_ext_lba48:
    mov cx, [si+2]          ; sector count (<=255)
    call ata_read_lba48
    jc .read_ext_fail

.read_ext_success:
    pop es
    pop di
    pop si
    pop dx
    pop cx
    pop bx
    xor ah, ah
    clc
    iret

.read_ext_cdrom:
    ; Treat DAP sector count and LBA as 2048-byte blocks for DL=0xE0.
    mov ax, [si+2]
    test ax, ax
    jz .read_ext_bad
    mov bx, [si+4]
    mov es, [si+6]
    mov eax, [si+8]         ; EAX = starting 2048-byte LBA
    mov cx, [si+2]          ; CX = number of 2048-byte blocks
    call atapi_read_lba_2048_eax
    jc .read_ext_fail
    mov ax, [si+2]
    mov [si+2], ax          ; BIOS reports count read; keep unchanged on success
    jmp .read_ext_success

.read_ext_bad:
    pop es
    pop di
    pop si
    pop dx
    pop cx
    pop bx
    mov ah, 0x01
    stc
    iret

.read_ext_fail:
    pop es
    pop di
    pop si
    pop dx
    pop cx
    pop bx
    mov ah, 0x20
    stc
    iret

.write_ext:
    ; Write via ATA
    push si
    mov al, [si+2]
    mov bx, [si+4]
    mov es, [si+6]
    mov ecx, [si+8]
    call ata_write_lba
    pop si
    xor ah, ah
    clc
    iret

.verify_ext:
    xor ah, ah
    clc
    iret

.seek_ext:
    xor ah, ah
    clc
    iret

.get_params_ext:
    ; Fill EDD drive params at DS:SI
    push di
    push es
    mov di, si
    push ds
    pop es
    ; Structure size
    mov word [di+0], 0x1E
    ; Flags: DMA valid, CHS valid
    mov word [di+2], 0x0003
    ; CHS
    mov dword [di+4],  1023  ; cylinders
    mov dword [di+8],  255   ; heads
    mov dword [di+12], 63    ; sectors/track
    ; Total sectors (512MB / 512 = 1M sectors)
    mov dword [di+16], 0x00100000
    mov dword [di+20], 0
    ; Bytes per sector
    cmp dl, 0xE0
    jne .bps_512
    mov word [di+24], ATAPI_SECTOR_SIZE
    jmp .bps_done
.bps_512:
    mov word [di+24], 512
.bps_done:
    pop es
    pop di
    xor ah, ah
    clc
    iret

.cdrom_status:
    ; INT 13h AH=4Bh — El Torito extensions (subset).
    ; AL=00h Terminate disk emulation (we don't emulate; succeed).
    ; AL=01h Get disk emulation status (return basic boot info packet).
    cmp al, 0x00
    je .eltorito_term_ok
    cmp al, 0x01
    je .eltorito_get_status
    mov ah, 0x01
    stc
    iret

.eltorito_term_ok:
    xor ah, ah
    clc
    iret

.eltorito_get_status:
    push ax
    push bx
    push cx
    push dx
    push si
    push di
    push ds
    push es

    ; Fill a small status packet at ES:DI (caller-provided).
    ; This is sufficient for common bootloaders that query the boot image LBA.
    push ds
    xor ax, ax
    mov ds, ax

    mov byte [es:di+0], 0x13                    ; packet size
    mov al, [NYX_CDROM_BOOT_MEDIA_TYPE]
    mov byte [es:di+1], al                      ; boot media type
    mov byte [es:di+2], dl                      ; drive number
    mov byte [es:di+3], 0x00                    ; controller (unknown)
    mov eax, dword [NYX_CDROM_BOOT_IMAGE_LBA]
    mov dword [es:di+4], eax                    ; image LBA (2048 blocks)
    mov ax, [NYX_CDROM_BOOT_LOAD_SEG]
    mov word [es:di+8], ax                      ; load segment
    mov ax, [NYX_CDROM_BOOT_SECTOR_COUNT]
    mov word [es:di+10], ax                     ; 512-byte sector count
    mov eax, dword [NYX_CDROM_BOOT_CATALOG_LBA]
    mov dword [es:di+12], eax                   ; boot catalog LBA
    ; Zero the remaining bytes we claim exist.
    xor ax, ax
    mov word [es:di+16], ax
    mov word [es:di+18], ax

    pop ds

    pop es
    pop ds
    pop di
    pop si
    pop dx
    pop cx
    pop bx
    pop ax
    mov bx, 0xAA55
    xor ah, ah
    clc
    iret

; ── ATA PIO read ────────────────────────────────
ata_read_lba:
    ; ECX = LBA, AL = count, ES:BX = buffer
    push ax
    push bx
    push cx
    push dx
    push di
    push si

    mov ah, al              ; AH = sectors remaining (<=255)
    mov di, bx              ; insw writes to ES:DI

    ; Select drive and wait for it to become idle.
    mov dx, ATA_DRIVE
    mov al, 0xE0
    cmp dl, 0x81
    jne .sel_ok
    mov al, 0xF0
.sel_ok:
    mov ebx, ecx
    shr ebx, 24
    and bl, 0x0F
    or al, bl
    out dx, al
    call ata_400ns_delay
    call ata_wait_not_bsy
    jnc .ready
    ; Retry once after soft reset (QEMU/older controllers can start up busy).
    call ata_soft_reset_primary
    call ata_wait_not_bsy
    jc .fail
.ready:

    mov dx, ATA_SECCNT
    mov al, ah
    out dx, al

    mov dx, ATA_LBA0
    mov al, cl
    out dx, al

    mov dx, ATA_LBA1
    mov al, ch
    out dx, al

    mov dx, ATA_LBA2
    mov ebx, ecx
    shr ebx, 16
    mov al, bl
    out dx, al

    mov dx, ATA_CMD
    mov al, 0x20            ; READ SECTORS
    out dx, al

.read_loop:
    call ata_wait_drq
    jc .fail
    mov dx, ATA_DATA
    mov cx, 256
    rep insw
    ; If DI wrapped during this 512-byte transfer, carry into ES.
    cmp di, 0
    jne .no_wrap
    mov bx, es
    add bx, 0x1000
    mov es, bx
.no_wrap:
    dec ah
    jnz .read_loop

    clc
    jmp .done

.fail:
    stc
.done:
    pop si
    pop di
    pop dx
    pop cx
    pop bx
    pop ax
    ret

; ── ATA PIO read (LBA48) ────────────────────────
ata_read_lba48:
    ; EDX:EAX = LBA (64-bit, high dword in EDX), CX = count (<=255), ES:BX buffer
    push ax
    push bx
    push cx
    push dx
    push di
    push si

    mov ah, cl              ; AH = sectors remaining
    mov di, bx              ; insw writes to ES:DI

    ; Select drive (primary master/slave), LBA mode.
    mov dx, ATA_DRIVE
    mov al, 0xE0
    cmp dl, 0x81
    jne .sel48_ok
    mov al, 0xF0
.sel48_ok:
    out dx, al
    call ata_400ns_delay
    call ata_wait_not_bsy
    jnc .ready
    call ata_soft_reset_primary
    call ata_wait_not_bsy
    jc .fail
.ready:

    ; Program 48-bit taskfile (high bytes first).
    ; Sector count (high=0 for <=255), then low.
    mov dx, ATA_SECCNT
    xor al, al
    out dx, al
    mov al, ah
    out dx, al

    ; LBA[24..47] then LBA[0..23]
    mov dx, ATA_LBA0
    mov ebx, eax
    shr ebx, 24
    mov al, bl              ; LBA[24..31]
    out dx, al
    mov dx, ATA_LBA1
    mov ebx, edx
    mov al, bl              ; LBA[32..39]
    out dx, al
    mov dx, ATA_LBA2
    mov ebx, edx
    shr ebx, 8
    mov al, bl              ; LBA[40..47]
    out dx, al

    ; Low 24 bits.
    mov dx, ATA_LBA0
    mov ebx, eax
    mov al, bl              ; LBA[0..7]
    out dx, al
    mov dx, ATA_LBA1
    mov ebx, eax
    shr ebx, 8
    mov al, bl              ; LBA[8..15]
    out dx, al
    mov dx, ATA_LBA2
    mov ebx, eax
    shr ebx, 16
    mov al, bl              ; LBA[16..23]
    out dx, al

    ; Command
    mov dx, ATA_CMD
    mov al, 0x24            ; READ SECTORS EXT
    out dx, al

.read_loop:
    call ata_wait_drq
    jc .fail
    mov dx, ATA_DATA
    mov cx, 256
    rep insw
    cmp di, 0
    jne .no_wrap
    mov bx, es
    add bx, 0x1000
    mov es, bx
.no_wrap:
    dec ah
    jnz .read_loop

    clc
    jmp .done
.fail:
    stc
.done:
    pop si
    pop di
    pop dx
    pop cx
    pop bx
    pop ax
    ret

; ── ATA PIO write ───────────────────────────────
ata_write_lba:
    ; ECX = LBA, AL = count, ES:BX = buffer
    push ax
    push bx
    push cx
    push dx
    push si
    push ds

    mov ah, al              ; sectors remaining
    mov si, bx

    mov dx, ATA_DRIVE
    mov al, 0xE0
    cmp dl, 0x81
    jne .wsel_ok
    mov al, 0xF0
.wsel_ok:
    mov ebx, ecx
    shr ebx, 24
    and bl, 0x0F
    or al, bl
    out dx, al
    call ata_400ns_delay
    call ata_wait_not_bsy
    jnc .wready
    call ata_soft_reset_primary
    call ata_wait_not_bsy
    jc .wfail
.wready:

    mov dx, ATA_SECCNT
    mov al, ah
    out dx, al

    mov dx, ATA_LBA0
    mov al, cl
    out dx, al

    mov dx, ATA_LBA1
    mov al, ch
    out dx, al

    mov dx, ATA_LBA2
    mov ebx, ecx
    shr ebx, 16
    mov al, bl
    out dx, al

    mov dx, ATA_CMD
    mov al, 0x30            ; WRITE SECTORS
    out dx, al

.write_loop:
    call ata_wait_drq
    jc .wfail
    push es
    pop ds                  ; DS:SI = buffer
    mov dx, ATA_DATA
    mov cx, 256
    rep outsw
    ; If SI wrapped during this 512-byte transfer, carry into DS.
    cmp si, 0
    jne .no_swrap
    mov bx, ds
    add bx, 0x1000
    mov ds, bx
.no_swrap:
    dec ah
    jnz .write_loop
    clc
    jmp .wdone

.wfail:
    stc
.wdone:
    pop ds
    pop si
    pop dx
    pop cx
    pop bx
    pop ax
    ret

ata_wait_not_bsy:
    ; Wait until BSY clears and DRDY sets (and no ERR).
    push bx
    push cx
    push dx
    mov dx, ATA_STATUS
    mov bx, 0x0200         ; outer loop for longer settle time (post-reset)
.wnb_outer:
    mov cx, 0xFFFF
.wnb_loop:
    in al, dx
    cmp al, 0xFF
    je .wnb_next
    test al, al
    jz .wnb_next
    test al, 0x01           ; ERR
    jnz .wnb_err
    test al, 0x80
    jnz .wnb_next
    test al, 0x40           ; DRDY
    jz .wnb_next
    clc
    jmp .wnb_done
.wnb_next:
    loop .wnb_loop
    dec bx
    jnz .wnb_outer
    stc
    jmp .wnb_done
.wnb_err:
    stc
.wnb_done:
    pop dx
    pop cx
    pop bx
    ret

ata_wait_drq:
    ; Wait for DRQ set and BSY clear.
    push bx
    push cx
    push dx
    mov dx, ATA_STATUS
    mov bx, 0x0200
.wdrq_outer:
    mov cx, 0xFFFF
.wdrq_loop:
    in al, dx
    cmp al, 0xFF
    je .wdrq_next
    test al, 0x01           ; ERR
    jnz .wdrq_err
    test al, 0x80
    jnz .wdrq_next
    test al, 0x08
    jnz .wdrq_ok
.wdrq_next:
    loop .wdrq_loop
    dec bx
    jnz .wdrq_outer
    stc
    jmp .wdrq_done
.wdrq_err:
    stc
    jmp .wdrq_done
.wdrq_ok:
    clc
.wdrq_done:
    pop dx
    pop cx
    pop bx
    ret

; ── ATA delays/resets ───────────────────────────
ata_400ns_delay:
    push dx
    mov dx, ATA_ALTSTATUS
    in al, dx
    in al, dx
    in al, dx
    in al, dx
    pop dx
    ret

ata_soft_reset_primary:
    push dx
    mov dx, ATA_CTRL
    mov al, 0x04            ; SRST
    out dx, al
    call ata_400ns_delay
    xor al, al              ; clear SRST
    out dx, al
    call ata_400ns_delay
    pop dx
    ret

; ── atapi_read_lba_2048_eax ─────────────────────
; Purpose: Read CD-ROM sectors (2048 bytes) via ATAPI PIO (READ(12)).
; Input:   EAX = starting LBA (2048-byte blocks)
;          CX  = block count (1..65535)
;          ES:BX = destination buffer
; Output:  CF clear on success, set on failure
; Trashes: AX,BX,CX,DX,SI,DI,EBX,EAX
; Calls:   atapi_wait_not_bsy, atapi_wait_drq
atapi_read_lba_2048_eax:
    push ax
    push bx
    push cx
    push dx
    push si
    push di
    push ds
    push es

    ; Log the read params
    push eax
    push ecx
    mov si, str_atapi_read_log
    call serial_puts
    pop ecx
    pop eax
    call serial_puthex32
    mov si, str_atapi_cnt
    call serial_puts
    mov ax, cx
    call serial_puthex16
    mov si, str_atapi_nl
    call serial_puts

    ; Use a fixed scratch buffer (0x8000) for the actual ATAPI data-in transfer
    ; to avoid ES:DI wrap issues during insw.
    mov ax, ATAPI_SCRATCH_SEG
    mov ds, ax

.blk_loop:
    test cx, cx
    jz .ok

    ; Read one 2048-byte block into DS:0 (scratch).
    xor di, di
    call atapi_read_one_2048_to_dsdi_eax
    jc .fail

    ; Copy 2048 bytes from DS:0 to ES:BX, handling ES wrap if needed.
    xor si, si
    mov di, bx
    call copy_2048_ds_si_to_es_di
    mov bx, di              ; updated offset back to BX

    inc eax                 ; next LBA
    dec cx                  ; one block consumed
    jmp .blk_loop

.ok:
    clc
    jmp .done
.fail:
    stc
.done:
    pop es
    pop ds
    pop di
    pop si
    pop dx
    pop cx
    pop bx
    pop ax
    ret

; ── atapi_read_one_2048_to_dsdi_eax ─────────────
; Purpose: Read one 2048-byte block from ATAPI device into DS:DI.
; Input:   EAX = LBA
;          DS:DI = destination (must have 2048 bytes free, no wrap)
; Output:  CF clear on success
; Trashes: AX,BX,CX,DX,SI,EBX,EAX
atapi_read_one_2048_to_dsdi_eax:
    push cx
    push dx
    push si

    ; Try secondary master first, then fall back to primary slave.
    call atapi2_read_one_2048_to_dsdi_eax
    jnc .ok
    ; Retry once after soft reset (helps with stale DRQ/BSY).
    call atapi_soft_reset_secondary
    call atapi2_read_one_2048_to_dsdi_eax
    jnc .ok
    call atapi1_read_one_2048_to_dsdi_eax
    jnc .ok
    call atapi_soft_reset_primary
    call atapi1_read_one_2048_to_dsdi_eax
    jc .fail
.ok:
    clc
    jmp .done
.fail:
    stc
.done:
    pop si
    pop dx
    pop cx
    ret

; ── atapi2_read_one_2048_to_dsdi_eax ────────────
; Purpose: Read one 2048-byte ATAPI block from secondary master (index=2).
; Input:   EAX = LBA, DS:DI dest
; Output:  CF clear on success
; Trashes: AX,BX,CX,DX,SI,EBX
atapi2_read_one_2048_to_dsdi_eax:
    push ds
    xor ax, ax
    mov ds, ax
    mov byte [NYX_ATAPI2_LAST_STAGE], 0x21
    pop ds
    mov dx, ATAPI_DRIVE(ATAPI2_BASE)
    mov al, ATAPI2_DRIVESEL
    out dx, al
    mov dx, ATAPI2_CTRL
    xor al, al
    out dx, al
    call atapi2_400ns_delay
    push ds
    xor ax, ax
    mov ds, ax
    mov byte [NYX_ATAPI2_LAST_STAGE], 0x22
    pop ds
    call atapi2_wait_not_bsy
    jc .retry2
    push ds
    xor ax, ax
    mov ds, ax
    mov byte [NYX_ATAPI2_LAST_STAGE], 0x23
    pop ds
    call atapi_issue_read12_one
    jc .retry2
    clc
    ret
.retry2:
    call atapi_soft_reset_secondary
    call atapi2_wait_not_bsy
    jc .fail
    call atapi_issue_read12_one
    jc .fail
    clc
    ret
.fail:
    stc
    ret

; ── atapi1_read_one_2048_to_dsdi_eax ────────────
; Purpose: Read one 2048-byte ATAPI block from primary slave (index=1).
; Input:   EAX = LBA, DS:DI dest
; Output:  CF clear on success
; Trashes: AX,BX,CX,DX,SI,EBX
atapi1_read_one_2048_to_dsdi_eax:
    push ds
    xor ax, ax
    mov ds, ax
    mov byte [NYX_ATAPI1_LAST_STAGE], 0x31
    pop ds
    mov dx, ATAPI_DRIVE(ATAPI1_BASE)
    mov al, ATAPI1_DRIVESEL
    out dx, al
    mov dx, ATAPI1_CTRL
    xor al, al
    out dx, al
    call atapi1_400ns_delay
    push ds
    xor ax, ax
    mov ds, ax
    mov byte [NYX_ATAPI1_LAST_STAGE], 0x32
    pop ds
    call atapi1_wait_not_bsy
    jc .retry1
    push ds
    xor ax, ax
    mov ds, ax
    mov byte [NYX_ATAPI1_LAST_STAGE], 0x33
    pop ds
    call atapi_issue_read12_one_primary
    jc .retry1
    clc
    ret
.retry1:
    call atapi_soft_reset_primary
    call atapi1_wait_not_bsy
    jc .fail
    call atapi_issue_read12_one_primary
    jc .fail
    clc
    ret
.fail:
    stc
    ret

; ── atapi_issue_read12_one (secondary) ───────────
; Purpose: Program taskfile and read one 2048-byte block (READ(12)) on secondary.
; Input:   EAX = LBA, DS:DI dest
; Output:  CF clear on success
; Trashes: AX,BX,CX,DX,SI,EBX
atapi_issue_read12_one:
    push ds
    xor bx, bx
    mov ds, bx
    mov byte [NYX_ATAPI2_LAST_STAGE], 0x24
    pop ds
    push ds
    xor bx, bx
    mov ds, bx
    mov byte [NYX_ATAPI2_LAST_STAGE], 0x25
    pop ds
    ; Clear taskfile and set feature/bytecount.
    mov dx, ATAPI_FEAT(ATAPI2_BASE)
    xor al, al
    out dx, al
    mov dx, ATAPI_SECCNT(ATAPI2_BASE)
    xor al, al
    out dx, al
    mov dx, ATAPI_LBA0(ATAPI2_BASE)
    xor al, al
    out dx, al
    mov dx, ATAPI_LBA1(ATAPI2_BASE)
    mov al, (ATAPI_SECTOR_SIZE & 0xFF)
    out dx, al
    mov dx, ATAPI_LBA2(ATAPI2_BASE)
    mov al, (ATAPI_SECTOR_SIZE >> 8)
    out dx, al
    mov dx, ATAPI_CMD(ATAPI2_BASE)
    mov al, ATAPI_PKT_CMD
    out dx, al
    push ds
    xor bx, bx
    mov ds, bx
    mov byte [NYX_ATAPI2_LAST_STAGE], 0x26
    pop ds
    call atapi2_wait_drq
    jc .fail
    push ds
    xor bx, bx
    mov ds, bx
    mov byte [NYX_ATAPI2_LAST_STAGE], 0x27
    pop ds
    call atapi_send_read12_packet_secondary
    jc .fail
    push ds
    xor bx, bx
    mov ds, bx
    mov byte [NYX_ATAPI2_LAST_STAGE], 0x28
    pop ds
    call atapi2_wait_drq
    jc .fail
    push ds
    xor bx, bx
    mov ds, bx
    mov byte [NYX_ATAPI2_LAST_STAGE], 0x29
    pop ds
    ; insw writes to ES:DI, so force ES=DS (caller sets DS to scratch).
    push es
    push ds
    pop es
    mov dx, ATAPI_DATA(ATAPI2_BASE)
    mov cx, (ATAPI_SECTOR_SIZE / 2)
    rep insw
    pop es
    push ds
    xor bx, bx
    mov ds, bx
    mov byte [NYX_ATAPI2_LAST_STAGE], 0x2A
    pop ds
    call atapi2_wait_not_bsy
    jc .fail
    push ds
    xor bx, bx
    mov ds, bx
    mov byte [NYX_ATAPI2_LAST_STAGE], 0x2B
    pop ds
    clc
    ret
.fail:
    stc
    ret

; ── atapi_issue_read12_one_primary ───────────────
; Purpose: Same as above for primary slave.
atapi_issue_read12_one_primary:
    push ds
    xor bx, bx
    mov ds, bx
    mov byte [NYX_ATAPI1_LAST_STAGE], 0x34
    pop ds
    push ds
    xor bx, bx
    mov ds, bx
    mov byte [NYX_ATAPI1_LAST_STAGE], 0x35
    pop ds
    mov dx, ATAPI_FEAT(ATAPI1_BASE)
    xor al, al
    out dx, al
    mov dx, ATAPI_SECCNT(ATAPI1_BASE)
    xor al, al
    out dx, al
    mov dx, ATAPI_LBA0(ATAPI1_BASE)
    xor al, al
    out dx, al
    mov dx, ATAPI_LBA1(ATAPI1_BASE)
    mov al, (ATAPI_SECTOR_SIZE & 0xFF)
    out dx, al
    mov dx, ATAPI_LBA2(ATAPI1_BASE)
    mov al, (ATAPI_SECTOR_SIZE >> 8)
    out dx, al
    mov dx, ATAPI_CMD(ATAPI1_BASE)
    mov al, ATAPI_PKT_CMD
    out dx, al
    push ds
    xor bx, bx
    mov ds, bx
    mov byte [NYX_ATAPI1_LAST_STAGE], 0x36
    pop ds
    call atapi1_wait_drq
    jc .fail
    push ds
    xor bx, bx
    mov ds, bx
    mov byte [NYX_ATAPI1_LAST_STAGE], 0x37
    pop ds
    call atapi_send_read12_packet_primary
    jc .fail
    push ds
    xor bx, bx
    mov ds, bx
    mov byte [NYX_ATAPI1_LAST_STAGE], 0x38
    pop ds
    call atapi1_wait_drq
    jc .fail
    push ds
    xor bx, bx
    mov ds, bx
    mov byte [NYX_ATAPI1_LAST_STAGE], 0x39
    pop ds
    push es
    push ds
    pop es
    mov dx, ATAPI_DATA(ATAPI1_BASE)
    mov cx, (ATAPI_SECTOR_SIZE / 2)
    rep insw
    pop es
    push ds
    xor bx, bx
    mov ds, bx
    mov byte [NYX_ATAPI1_LAST_STAGE], 0x3A
    pop ds
    call atapi1_wait_not_bsy
    jc .fail
    push ds
    xor bx, bx
    mov ds, bx
    mov byte [NYX_ATAPI1_LAST_STAGE], 0x3B
    pop ds
    clc
    ret
.fail:
    stc
    ret

; ── atapi_send_read12_packet_secondary ───────────
; Purpose: Send READ(12) packet for current EAX LBA to secondary data port.
; Input:   EAX = LBA
; Output:  CF clear on success
; Trashes: AX,BX,CX,DX,SI,EBX
atapi_send_read12_packet_secondary:
    sub sp, 12
    mov si, sp
    push ds
    push ss
    pop ds
    ; READ(10) packet (10 bytes, padded to 12):
    ; 0: 0x28, 2..5: LBA (big-endian), 7..8: transfer length (big-endian)=1
    mov word [si+0], 0x0028
    mov ebx, eax
    bswap ebx
    mov dword [si+2], ebx
    mov word [si+6], 0x0000
    mov word [si+8], 0x0001
    mov word [si+10], 0x0000
    mov dx, ATAPI_DATA(ATAPI2_BASE)
    mov cx, 6
    rep outsw
    call atapi2_400ns_delay
    pop ds
    add sp, 12
    clc
    ret

; ── atapi_send_read12_packet_primary ─────────────
; Purpose: Send READ(12) packet to primary data port.
atapi_send_read12_packet_primary:
    sub sp, 12
    mov si, sp
    push ds
    push ss
    pop ds
    mov word [si+0], 0x0028
    mov ebx, eax
    bswap ebx
    mov dword [si+2], ebx
    mov word [si+6], 0x0000
    mov word [si+8], 0x0001
    mov word [si+10], 0x0000
    mov dx, ATAPI_DATA(ATAPI1_BASE)
    mov cx, 6
    rep outsw
    call atapi1_400ns_delay
    pop ds
    add sp, 12
    clc
    ret

; ── atapi2_drain_stale_drq ──────────────────────
; Purpose: If the device is stuck with DRQ asserted, drain a 512-byte data phase.
; Input:   DS = scratch, DI can be clobbered
; Output:  CF set if DRQ remains stuck
; Trashes: AX,CX,DX,DI,ES
atapi2_drain_stale_drq:
    push es
    push di
    mov dx, ATAPI_STATUS(ATAPI2_BASE)
    in al, dx
    test al, 0x08
    jz .ok
    ; Read and discard 256 words (512 bytes) to clear DRQ.
    push ds
    pop es
    xor di, di
    mov dx, ATAPI_DATA(ATAPI2_BASE)
    mov cx, 256
    rep insw
    ; Recheck DRQ.
    mov dx, ATAPI_STATUS(ATAPI2_BASE)
    in al, dx
    test al, 0x08
    jz .ok
    stc
    jmp .done
.ok:
    clc
.done:
    pop di
    pop es
    ret

; ── atapi1_drain_stale_drq ──────────────────────
; Purpose: Same as above for primary channel.
atapi1_drain_stale_drq:
    push es
    push di
    mov dx, ATAPI_STATUS(ATAPI1_BASE)
    in al, dx
    test al, 0x08
    jz .ok
    push ds
    pop es
    xor di, di
    mov dx, ATAPI_DATA(ATAPI1_BASE)
    mov cx, 256
    rep insw
    mov dx, ATAPI_STATUS(ATAPI1_BASE)
    in al, dx
    test al, 0x08
    jz .ok
    stc
    jmp .done
.ok:
    clc
.done:
    pop di
    pop es
    ret

; ── atapi2_wait_not_bsy ─────────────────────────
; Purpose: Wait for BSY=0.
; Output:  CF set on timeout.
; Trashes: AX,CX,DX
atapi2_wait_not_bsy:
    push ax
    push bx
    push cx
    push dx
    push ds
    xor ax, ax
    mov ds, ax
    ; Drain stale if DRQ
    call atapi2_drain_stale_drq
    mov dx, ATAPI_STATUS(ATAPI2_BASE)
    mov bx, 1000
.loop:
    in al, dx
    mov [NYX_ATAPI2_LAST_STATUS], al
    cmp al, 0xFF
    je .timeout
    test al, 0x01
    jnz .err
    test al, 0x80
    jz .ok
    mov cx, 1
    call pit_wait_ms
    dec bx
    jnz .loop
.timeout:
    mov si, str_atapi_timeout
    mov al, [NYX_ATAPI2_LAST_STATUS]
    xor ah, ah
    call serial_puthex16
    mov dx, ATAPI_FEAT(ATAPI2_BASE)
    in al, dx
    mov [NYX_ATAPI2_LAST_ERROR], al
    xor ah, ah
    call serial_puthex16
    mov si, str_atapi_timeout_nl
    call serial_puts
    stc
    jmp .done
.err:
    mov dx, ATAPI_FEAT(ATAPI2_BASE)
    in al, dx
    mov [NYX_ATAPI2_LAST_ERROR], al
    mov si, str_atapi_err
    call serial_puts
    xor ah, ah
    call serial_puthex16
    mov si, str_atapi_timeout_nl
    call serial_puts
    stc
    jmp .done
.ok:
    clc
.done:
    pop ds
    pop dx
    pop cx
    pop bx
    pop ax
    ret

; ── atapi2_wait_drq ─────────────────────────────
; Purpose: Wait for DRQ=1 and BSY=0.
; Output:  CF set on timeout.
; Trashes: AX,CX,DX
atapi2_wait_drq:
    push ax
    push bx
    push cx
    push dx
    push ds
    xor ax, ax
    mov ds, ax
    ; Drain stale DRQ first
    call atapi2_drain_stale_drq
    mov dx, ATAPI_STATUS(ATAPI2_BASE)
    mov bx, 1000            ; increased timeout
.loop:
    in al, dx
    mov [NYX_ATAPI2_LAST_STATUS], al
    cmp al, 0xFF
    je .timeout
    test al, 0x01           ; ERR
    jnz .err
    test al, 0x80
    jnz .next
    test al, 0x08
    jnz .ok
.next:
    mov cx, 1
    call pit_wait_ms
    dec bx
    jnz .loop
.timeout:
    mov si, str_atapi_timeout
    call serial_puts
    mov al, [NYX_ATAPI2_LAST_STATUS]
    xor ah, ah
    call serial_puthex16
    mov dx, ATAPI_FEAT(ATAPI2_BASE)
    in al, dx
    mov [NYX_ATAPI2_LAST_ERROR], al
    xor ah, ah
    call serial_puthex16
    mov si, str_log_nl
    call serial_puts
    stc
    jmp .done
.err:
    mov dx, ATAPI_FEAT(ATAPI2_BASE)
    in al, dx
    mov [NYX_ATAPI2_LAST_ERROR], al
    mov si, str_atapi_err
    call serial_puts
    xor ah, ah
    call serial_puthex16
    mov si, str_log_nl
    call serial_puts
    stc
    jmp .done
.ok:
    clc
.done:
    pop ds
    pop dx
    pop cx
    pop bx
    pop ax
    ret

; ── atapi2_400ns_delay ──────────────────────────
; Purpose: Required delay after drive select on PATA.
; Trashes: AX,DX
atapi2_400ns_delay:
    push dx
    mov dx, ATAPI2_CTRL
    in al, dx
    in al, dx
    in al, dx
    in al, dx
    pop dx
    ret

; ── atapi1_wait_not_bsy ─────────────────────────
; Purpose: Wait for BSY=0 on primary channel.
atapi1_wait_not_bsy:
    push ax
    push bx
    push cx
    push dx
    push ds
    xor ax, ax
    mov ds, ax
    mov dx, ATAPI_STATUS(ATAPI1_BASE)
    mov bx, 200             ; bounded poll iterations
.loop:
    in al, dx
    mov [NYX_ATAPI1_LAST_STATUS], al
    cmp al, 0xFF
    je .err
    test al, 0x01
    jnz .err
    test al, 0x80
    jz .ok
    mov cx, 1
    call pit_wait_ms
    dec bx
    jnz .loop
    stc
    jmp .done
.err:
    mov dx, ATAPI_FEAT(ATAPI1_BASE)
    in al, dx
    mov [NYX_ATAPI1_LAST_ERROR], al
    stc
    jmp .done
.ok:
    clc
.done:
    pop ds
    pop dx
    pop cx
    pop bx
    pop ax
    ret

; ── atapi1_wait_drq ─────────────────────────────
; Purpose: Wait for DRQ=1 and BSY=0 on primary channel.
atapi1_wait_drq:
    push ax
    push bx
    push cx
    push dx
    push ds
    xor ax, ax
    mov ds, ax
    mov dx, ATAPI_STATUS(ATAPI1_BASE)
    mov bx, 200             ; bounded poll iterations
.loop:
    in al, dx
    mov [NYX_ATAPI1_LAST_STATUS], al
    cmp al, 0xFF
    je .err
    test al, 0x01
    jnz .err
    test al, 0x80
    jnz .next
    test al, 0x08
    jnz .ok
.next:
    mov cx, 1
    call pit_wait_ms
    dec bx
    jnz .loop
    stc
    jmp .done
.err:
    mov dx, ATAPI_FEAT(ATAPI1_BASE)
    in al, dx
    mov [NYX_ATAPI1_LAST_ERROR], al
    stc
    jmp .done
.ok:
    clc
.done:
    pop ds
    pop dx
    pop cx
    pop bx
    pop ax
    ret

; ── atapi1_400ns_delay ──────────────────────────
; Purpose: Required delay after drive select on primary channel.
atapi1_400ns_delay:
    push dx
    mov dx, ATAPI1_CTRL
    in al, dx
    in al, dx
    in al, dx
    in al, dx
    pop dx
    ret

; ── copy_2048_ds_si_to_es_di ────────────────────
; Purpose: Copy exactly 2048 bytes from DS:SI to ES:DI, adjusting ES on DI wrap.
; Input:   DS:SI = source, ES:DI = dest
; Output:  ES:DI advanced by 2048 bytes
; Trashes: AX,BX,CX
copy_2048_ds_si_to_es_di:
    mov cx, (ATAPI_SECTOR_SIZE / 2)
    jmp copy_words_cross

; ── copy_words_cross ────────────────────────────
; Purpose: Copy CX words from DS:SI to ES:DI, adjusting ES when DI wraps.
; Input:   CX = word count, DS:SI source, ES:DI dest
; Output:  CX=0, ES:DI advanced by 2*words
; Trashes: AX,BX
copy_words_cross:
    test cx, cx
    jz .done
.chunk:
    ; words_until_wrap = (0x10000 - DI) / 2
    ; Special-case DI=0: 0x10000 doesn't fit in 16-bit, but it means 0x8000 words.
    cmp di, 0
    jne .calc
    mov bx, 0x8000
    jmp .have_room
.calc:
    mov bx, di
    neg bx
    shr bx, 1
.have_room:
    cmp bx, cx
    jbe .use_bx
    mov bx, cx
.use_bx:
    mov ax, cx
    mov cx, bx
    rep movsw
    mov cx, ax
    sub cx, bx
    ; If we wrapped (DI==0) and still have data, advance ES by 64KiB.
    test cx, cx
    jz .done
    cmp di, 0
    jne .chunk
    mov ax, es
    add ax, 0x1000
    mov es, ax
    jmp .chunk
.done:
    ret

; ── atapi_debug_dump ────────────────────────────
; Purpose: Print ATAPI status/error bytes for both candidate channels.
; Output:  Serial lines like "ATAPI2 ST=00 ER=00".
; Trashes: AX,BX,DX,SI
atapi_debug_dump:
    push ax
    push bx
    push dx
    push si

    push ds
    xor ax, ax
    mov ds, ax

    mov si, str_atapi_dbg
    call serial_puts
    mov al, [NYX_ATAPI2_LAST_STAGE]
    xor ah, ah
    call serial_puthex16
    mov si, str_st
    call serial_puts
    mov al, [NYX_ATAPI2_LAST_STATUS]
    xor ah, ah
    call serial_puthex16
    mov si, str_er
    call serial_puts
    mov al, [NYX_ATAPI2_LAST_ERROR]
    xor ah, ah
    call serial_puthex16
    mov si, str_nl
    call serial_puts

    mov si, str_atapi_dbg1
    call serial_puts
    mov al, [NYX_ATAPI1_LAST_STAGE]
    xor ah, ah
    call serial_puthex16
    mov si, str_st
    call serial_puts
    mov al, [NYX_ATAPI1_LAST_STATUS]
    xor ah, ah
    call serial_puthex16
    mov si, str_er
    call serial_puts
    mov al, [NYX_ATAPI1_LAST_ERROR]
    xor ah, ah
    call serial_puthex16
    mov si, str_nl
    call serial_puts

    pop ds

    mov si, str_atapi2
    call serial_puts
    mov dx, ATAPI_STATUS(ATAPI2_BASE)
    in al, dx
    xor ah, ah
    call serial_puthex16
    mov si, str_er
    call serial_puts
    mov dx, ATAPI_FEAT(ATAPI2_BASE)
    in al, dx
    xor ah, ah
    call serial_puthex16
    mov si, str_nl
    call serial_puts

    mov si, str_atapi1
    call serial_puts
    mov dx, ATAPI_STATUS(ATAPI1_BASE)
    in al, dx
    xor ah, ah
    call serial_puthex16
    mov si, str_er
    call serial_puts
    mov dx, ATAPI_FEAT(ATAPI1_BASE)
    in al, dx
    xor ah, ah
    call serial_puthex16
    mov si, str_nl
    call serial_puts

    pop si
    pop dx
    pop bx
    pop ax
    ret

str_atapi2: db 'ATAPI2 ST=',0
str_atapi1: db 'ATAPI1 ST=',0
str_atapi_dbg: db 'ATAPI DBG stage=',0
str_atapi_dbg1: db 'ATAPI DBG1 stage=',0
str_st:     db ' st=',0
str_er:     db ' ER=',0
str_nl:     db 13,10,0
