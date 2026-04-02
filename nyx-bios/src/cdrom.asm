; src/cdrom.asm — El Torito CD-ROM boot

%define CDROM_DRIVE  0xE0
%define CDROM_SECTOR 2048

cdrom_boot:
    POST 0x30
    mov si, str_cdrom_boot
    call serial_puts

    mov si, str_try_cdrom_probe
    call serial_puts

    ; Probe ATAPI presence first
    call atapi_probe_any
    jc .no_cdrom
    mov si, str_cdrom_ok_probe
    call serial_puts

    ; Enable A20 first
    call a20_enable

    ; Read El Torito boot catalog
    ; First: read Primary Volume Descriptor
    ; sector 16 of ISO
    POST 0x31
    call cdrom_read_pvd
    jc .fail
    mov si, str_cdrom_pvd_ok
    call serial_puts

    ; Find El Torito Boot Record
    POST 0x32
    call cdrom_find_eltorito
    jc .fail
    mov si, str_cdrom_br_ok
    call serial_puts

    ; Read boot catalog
    POST 0x33
    call cdrom_read_catalog
    jc .fail
    mov si, str_cdrom_cat_ok
    call serial_puts

    ; Load boot image
    POST 0x34
    call cdrom_load_boot_image
    jc .fail
    mov si, str_cdrom_img_ok
    call serial_puts

    mov si, str_cdrom_ok
    call serial_puts
    mov si, str_log_handoff
    call serial_puts

    ; BIOS-style handoff: DL=boot drive, CS:IP=0000:7C00
    POST 0x35
    mov dl, CDROM_DRIVE
    xor ax, ax
    mov ds, ax
    mov es, ax
    jmp 0x0000:0x7C00

.no_cdrom:
    mov si, str_no_cdrom_probe
    call serial_puts
    stc
    ret

.fail:
    mov si, str_cdrom_fail_stage
    call serial_puts
    mov si, str_cdrom_fail
    call serial_puts
    call atapi_debug_dump
    stc
    ret

cdrom_read_pvd:
    ; Read ISO9660 PVD sector 16 into 0x0800:0000, check "CD001" signature [1:5]
    push es
    push bx
    mov si, str_log_pvd_read
    call serial_puts
    mov ax, 0x0800
    mov es, ax
    xor bx, bx
    mov eax, 16
    mov cx, 1
    call atapi_read_lba_2048_eax
    jc .fail
    ; Validate PVD: type=1 [0], ID="CD001" [1:6)
    mov al, [es:0]
    cmp al, 0x01
    jne .fail
    mov al, [es:1]
    cmp al, 'C'
    jne .fail_pvd
    mov al, [es:2]
    cmp al, 'D'
    jne .fail_pvd
    mov al, [es:3]
    cmp al, '0'
    jne .fail_pvd
    mov al, [es:4]
    cmp al, '0'
    jne .fail_pvd
    mov al, [es:5]
    cmp al, '1'
    jne .fail_pvd
    pop bx
    pop es
    clc
    ret
.fail_pvd:
    mov si, str_err_pvd_sig
    call serial_puts
.fail:
    pop bx
    pop es
    stc
    ret

cdrom_find_eltorito:
    ; Scan Volume Descriptors for an El Torito Boot Record descriptor (type=0).
    ; The ISO9660 volume descriptor set begins at LBA 16.
    push es
    push bx
    mov ax, 0x0800
    mov es, ax

    ; Scan from sector 16 up to a small limit.
    mov dx, 16              ; starting sector
.scan:
    ; Read sector DX into ES:0
    xor bx, bx
    xor eax, eax
    mov ax, dx
    mov cx, 1
    call atapi_read_lba_2048_eax
    jc .not_found

    ; Check descriptor type
    mov al, [es:0]
    cmp al, 0xFF            ; terminator
    je .not_found
    cmp al, 0x00            ; boot record descriptor
    jne .next

    ; Validate standard volume descriptor id "CD001" and version=1.
    mov al, [es:1]
    cmp al, 'C'
    jne .next
    mov al, [es:2]
    cmp al, 'D'
    jne .next
    mov al, [es:3]
    cmp al, '0'
    jne .next
    mov al, [es:4]
    cmp al, '0'
    jne .next
    mov al, [es:5]
    cmp al, '1'
    jne .next
    mov al, [es:6]
    cmp al, 0x01
    jne .next

    ; Validate Boot System Identifier begins with "EL TORITO".
    mov al, [es:7]
    cmp al, 'E'
    jne .next
    mov al, [es:8]
    cmp al, 'L'
    jne .next
    mov al, [es:9]
    cmp al, ' '
    jne .next
    mov al, [es:10]
    cmp al, 'T'
    jne .next
    mov al, [es:11]
    cmp al, 'O'
    jne .next
    mov al, [es:12]
    cmp al, 'R'
    jne .next
    mov al, [es:13]
    cmp al, 'I'
    jne .next
    mov al, [es:14]
    cmp al, 'T'
    jne .next
    mov al, [es:15]
    cmp al, 'O'
    jne .next

    jmp .found
.next:
    inc dx
    cmp dx, 32              ; max scan (exclusive)
    jl .scan
    jmp .not_found

.found:
    ; Boot Catalog pointer is a 32-bit LBA at offset 0x47.
    mov si, str_log_br_ok
    call serial_puts
    ; Log catalog LBA and store it for later.
    push eax
    mov eax, dword [es:0x47]
    mov si, str_log_cat_lba
    call serial_puts
    call serial_puthex32
    mov si, str_log_nl
    call serial_puts
    pop eax
    push ds
    xor bx, bx
    mov ds, bx
    mov eax, dword [es:0x47]
    mov dword [NYX_CDROM_BOOT_CATALOG_LBA], eax
    pop ds
    clc
    pop bx
    pop es
    ret

.not_found:
    stc
    pop bx
    pop es
    ret

cdrom_read_catalog:
    ; Read boot catalog, validate header ID=01h [0] and key bytes 55AAh at [0x1E].
    push es
    push bx
    mov si, str_log_cat_read
    call serial_puts
    mov ax, 0x0900
    mov es, ax
    xor bx, bx
    push ds
    xor dx, dx
    mov ds, dx
    mov eax, dword [NYX_CDROM_BOOT_CATALOG_LBA]
    pop ds
    mov cx, 1
    call atapi_read_lba_2048_eax
    jc .fail
    mov al, [es:0]
    cmp al, 0x01
    jne .fail_cat
    cmp word [es:0x1E], 0xAA55
    jne .fail_cat
    pop bx
    pop es
    clc
    ret
.fail_cat:
    mov si, str_err_cat_sig
    call serial_puts
.fail:
    pop bx
    pop es
    stc
    ret

cdrom_load_boot_image:
    ; Parse/log default entry (offset 32)
    push es
    mov ax, 0x0900
    mov es, ax
    mov si, str_log_img_parse
    call serial_puts

    ; Default entry at offset 0x20.
    ; Boot indicator must be 0x88 for bootable.
    mov al, [es:0x20]
    cmp al, 0x88
    jne .fail

    mov al, [es:33]         ; media type
    push ds
    xor dx, dx
    mov ds, dx
    mov [NYX_CDROM_BOOT_MEDIA_TYPE], al
    pop ds

    ; We always load to 0000:7C00 and handoff with CS:IP=0000:7C00.
    xor ax, ax
    push ds
    xor dx, dx
    mov ds, dx
    mov [NYX_CDROM_BOOT_LOAD_SEG], ax
    pop ds

    ; System type
    mov bl, [es:36]
    ; Sector count - log it
    mov bx, word [es:38]
    push ds
    xor dx, dx
    mov ds, dx
    mov [NYX_CDROM_BOOT_SECTOR_COUNT], bx
    pop ds
    mov si, str_log_img_cnt
    call serial_puts
    mov ax, bx
    call serial_puthex16
    mov si, str_log_nl
    call serial_puts

    ; Load RBA (LBA) - log it
    push ds
    xor bx, bx
    mov ds, bx
    mov eax, dword [es:40]
    mov dword [NYX_CDROM_BOOT_IMAGE_LBA], eax
    pop ds
    mov si, str_log_img_lba
    call serial_puts
    push eax
    mov eax, dword [es:40]
    call serial_puthex32
    mov si, str_log_nl
    call serial_puts
    pop eax

    ; Load the boot image:
    ; - boot_image_lba is in 2048-byte blocks (ISO logical sectors)
    ; - boot_sector_count is in 512-byte "virtual sectors" (El Torito)
    push ds
    xor dx, dx
    mov ds, dx
    mov ax, [NYX_CDROM_BOOT_LOAD_SEG]
    mov bx, [NYX_CDROM_BOOT_SECTOR_COUNT]
    mov eax, dword [NYX_CDROM_BOOT_IMAGE_LBA]
    pop ds

    test bx, bx
    jnz .have_count
    ; Default to 2048 bytes (4 * 512) if sector count is zero.
    mov bx, 4
.have_count:

    ; ES:DI = destination (load_seg:7C00)
    mov es, ax
    mov di, 0x7C00
    mov si, str_log_img_load
    call serial_puts

    ; bytes_remaining = boot_sector_count * 512
    xor edx, edx
    mov dx, bx
    shl edx, 9

.load_loop:
    test edx, edx
    jz .verify

    ; Read one 2048-byte ISO sector to 0x0800:0000.
    push dx
    push eax
    push es
    push di
    push ax
    mov ax, 0x0800
    mov es, ax
    xor bx, bx
    mov cx, 1
    call atapi_read_lba_2048_eax
    pop ax
    pop di
    pop es
    pop eax
    pop dx
    jc .fail

    ; Copy min(2048, bytes_remaining) from 0x0800:0000 to ES:DI.
    push ds
    mov ax, 0x0800
    mov ds, ax
    xor si, si
    cmp edx, CDROM_SECTOR
    jb .small_copy
    ; Copy 2048 bytes (1024 words), may wrap ES:DI.
    mov cx, 1024
    call copy_words_cross
    sub edx, CDROM_SECTOR
    jmp .after_copy
.small_copy:
    ; Copy EDX bytes (must be even); convert to words.
    mov cx, dx
    shr cx, 1
    call copy_words_cross
    xor edx, edx
.after_copy:
    pop ds

    inc eax                 ; next ISO sector
    jmp .load_loop

.verify:
    ; Verify boot signature at loaded image (0xAA55 at offset 510).
    push ds
    push es
    xor ax, ax
    mov ds, ax
    mov ax, [NYX_CDROM_BOOT_LOAD_SEG]
    mov es, ax
    mov ax, [es:0x7C00+510]
    pop es
    pop ds
    cmp ax, 0xAA55
    jne .fail_sig

    clc
    pop es
    ret

.fail_sig:
    mov si, str_cdrom_sig
    call serial_puts
    stc
    pop es
    ret

.fail:
    stc
    pop es
    ret

str_cdrom_boot: db 'Booting from CD-ROM...', 13, 10, 0
str_cdrom_ok:   db 'Boot image loaded OK', 13, 10, 0
str_cdrom_fail: db 'CD-ROM boot failed', 13, 10, 0
str_cdrom_sig:  db 'Boot image missing AA55', 13, 10, 0
str_cdrom_pvd_ok: db 'CD-ROM PVD OK', 13, 10, 0
str_cdrom_br_ok:  db 'El Torito boot record OK', 13, 10, 0
str_cdrom_cat_ok: db 'Boot catalog OK', 13, 10, 0
str_cdrom_img_ok: db 'Boot image OK', 13, 10, 0
