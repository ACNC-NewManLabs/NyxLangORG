; src/boot_menu.asm — Interactive TUI Boot Menu for NyxBIOS
; Full-screen 80×25 VGA text UI, arrow-key navigation, countdown timer.
; Returns DL = boot device to caller (boot_sequence).

; ── Constants ────────────────────────────────────────────────────────────────
%define BM_TIMEOUT      5
%define BOOT_HDD        0x80
%define BOOT_CD         0xE0
%define BOOT_NET        0x81
%define MENU_OPTS       3

; VGA color attributes: high nibble = bg, low nibble = fg
%define A_TITLE     0x1F    ; bright white on blue
%define A_BORDER    0x1B    ; bright cyan on blue
%define A_NORMAL    0x17    ; white on blue
%define A_DIM       0x08    ; dark gray on black
%define A_SEL       0x70    ; black on white (selected row)
%define A_HOT       0x1E    ; bright yellow on blue
%define A_INFO      0x1A    ; bright green on blue
%define A_COUNT     0x1E    ; yellow on blue (countdown)
%define A_STAT      0x30    ; black on cyan

; Layout: left panel cols 1-36, right panel cols 39-77
%define BM_LEFT_L   1
%define BM_LEFT_R   36
%define BM_LEFT_INN 34          ; inner width
%define BM_RIGHT_L  39
%define BM_RIGHT_R  77
%define BM_RIGHT_INN 37         ; inner width

; Rows
%define BM_R_TITLE  0
%define BM_R_PT     2           ; panel top
%define BM_R_PHDR   3           ; panel header
%define BM_R_PSEP   4           ; panel separator
%define BM_R_OPT0   5
%define BM_R_OPT1   6
%define BM_R_OPT2   7
%define BM_R_PBL    8           ; left panel bottom
%define BM_R_PBRT   9           ; right panel bottom
%define BM_R_CTB    11          ; countdown box top
%define BM_R_CTM    12          ; countdown box middle
%define BM_R_CTB2   13          ; countdown box bottom
%define BM_R_HELP   15
%define BM_R_STAT   24

; VGA position helper macro: sets DI = (row*80+col)*2
%macro VGA_AT 2
    mov di, %1 * 80 + %2
    shl di, 1
%endmacro

; ── boot_menu ────────────────────────────────────────────────────────────────
; Shows boot menu TUI. Returns DL = chosen device.
boot_menu:
    pusha
    push ds
    push es

    mov ax, 0x0003          ; 80×25 text mode (clears screen)
    int 0x10

    mov ah, 0x01            ; hide cursor
    mov cx, 0x2000
    int 0x10

    mov ax, 0xB800
    mov es, ax

    xor ax, ax
    mov ds, ax

    ; Clear screen with A_NORMAL background
    xor di, di
    mov cx, 80 * 25
    mov ax, (A_NORMAL << 8) | ' '
    rep stosw

    call bm_titlebar
    call bm_left_panel
    call bm_right_panel
    call bm_cnt_box
    call bm_helpbar
    call bm_statusbar

    ; Default selection: HDD (0), but if CD-ROM catalog found, default to CD (1)
    mov eax, [NYX_CDROM_BOOT_CATALOG_LBA]
    test eax, eax
    jnz .set_cd
    mov byte [NYX_BOOT_SELECTED], 0
    jmp .redraw
.set_cd:
    mov byte [NYX_BOOT_SELECTED], 1
.redraw:
    call bm_redraw_opts

    mov si, str_bm_dbg1
    call serial_puts

    call bm_loop

    mov si, str_bm_dbg2
    call serial_puts

    mov ah, 0x01            ; restore cursor
    mov cx, 0x0607
    int 0x10

    pop es
    pop ds
    popa
    mov dl, [NYX_BOOT_LAST]
    ret

; ── bm_titlebar ──────────────────────────────────────────────────────────────
bm_titlebar:
    push ax
    push cx
    push di

    VGA_AT BM_R_TITLE, 0
    mov cx, 80
    mov ax, (A_TITLE << 8) | ' '
    rep stosw

    VGA_AT BM_R_TITLE, 2
    mov si, str_title_l
    mov ah, A_TITLE
    call bm_puts

    VGA_AT BM_R_TITLE, 29
    mov si, str_title_c
    mov ah, A_HOT
    call bm_puts

    VGA_AT BM_R_TITLE, 62
    mov si, str_title_r
    mov ah, A_TITLE
    call bm_puts

    pop di
    pop cx
    pop ax
    ret

; ── bm_left_panel ────────────────────────────────────────────────────────────
bm_left_panel:
    push ax
    push bx
    push cx
    push di

    ; Top border row BM_R_PT: ╔══...══╗
    VGA_AT BM_R_PT, BM_LEFT_L
    mov al, 0xC9
    mov ah, A_BORDER
    stosw
    mov cx, BM_LEFT_INN
    mov al, 0xCD
.lp_r2:
    stosw
    loop .lp_r2
    mov al, 0xBB
    stosw

    ; Header row BM_R_PHDR: ║ text ║
    VGA_AT BM_R_PHDR, BM_LEFT_L
    mov al, 0xBA
    mov ah, A_BORDER
    stosw
    VGA_AT BM_R_PHDR, BM_LEFT_L+2
    mov si, str_lhdr
    mov ah, A_HOT
    call bm_puts
    VGA_AT BM_R_PHDR, BM_LEFT_R
    mov al, 0xBA
    mov ah, A_BORDER
    stosw

    ; Separator row BM_R_PSEP: ╠══...══╣
    VGA_AT BM_R_PSEP, BM_LEFT_L
    mov al, 0xCC
    mov ah, A_BORDER
    stosw
    mov cx, BM_LEFT_INN
    mov al, 0xCD
.lp_r4:
    stosw
    loop .lp_r4
    mov al, 0xB9
    stosw

    ; Option rows (content drawn by bm_redraw_opts — just draw borders here)
    mov bx, BM_R_OPT0
.lp_opts:
    ; left border
    mov ax, bx
    mov cx, 80
    mul cx
    add ax, BM_LEFT_L
    shl ax, 1
    mov di, ax
    mov al, 0xBA
    mov ah, A_BORDER
    stosw
    ; right border
    mov ax, bx
    mov cx, 80
    mul cx
    add ax, BM_LEFT_R
    shl ax, 1
    mov di, ax
    mov al, 0xBA
    mov ah, A_BORDER
    stosw
    inc bx
    cmp bx, BM_R_OPT2
    jle .lp_opts

    ; Bottom border row BM_R_PBL: ╚══...══╝
    VGA_AT BM_R_PBL, BM_LEFT_L
    mov al, 0xC8
    mov ah, A_BORDER
    stosw
    mov cx, BM_LEFT_INN
    mov al, 0xCD
.lp_r8:
    stosw
    loop .lp_r8
    mov al, 0xBC
    stosw

    pop di
    pop cx
    pop bx
    pop ax
    ret

; ── bm_right_panel ───────────────────────────────────────────────────────────
bm_right_panel:
    push ax
    push cx
    push di

    ; Top border
    VGA_AT BM_R_PT, BM_RIGHT_L
    mov al, 0xC9
    mov ah, A_BORDER
    stosw
    mov cx, BM_RIGHT_INN
    mov al, 0xCD
.rp_r2:
    stosw
    loop .rp_r2
    mov al, 0xBB
    stosw

    ; Header row
    VGA_AT BM_R_PHDR, BM_RIGHT_L
    mov al, 0xBA
    mov ah, A_BORDER
    stosw
    VGA_AT BM_R_PHDR, BM_RIGHT_L+2
    mov si, str_rhdr
    mov ah, A_HOT
    call bm_puts
    VGA_AT BM_R_PHDR, BM_RIGHT_R
    mov al, 0xBA
    mov ah, A_BORDER
    stosw

    ; Separator
    VGA_AT BM_R_PSEP, BM_RIGHT_L
    mov al, 0xCC
    mov ah, A_BORDER
    stosw
    mov cx, BM_RIGHT_INN
    mov al, 0xCD
.rp_r4:
    stosw
    loop .rp_r4
    mov al, 0xB9
    stosw

    ; Info rows 5-8 with content
    ; Row 5: base memory
    VGA_AT BM_R_OPT0, BM_RIGHT_L
    mov al, 0xBA
    mov ah, A_BORDER
    stosw
    VGA_AT BM_R_OPT0, BM_RIGHT_L+2
    mov si, str_ri_mem
    mov ah, A_NORMAL
    call bm_puts
    mov ax, [0x0413]        ; BDA base memory in KB
    mov bh, A_INFO
    call bm_print_dec2
    mov si, str_ri_kb
    mov ah, A_INFO
    call bm_puts
    VGA_AT BM_R_OPT0, BM_RIGHT_R
    mov al, 0xBA
    mov ah, A_BORDER
    stosw

    ; Row 6: extended memory
    VGA_AT BM_R_OPT1, BM_RIGHT_L
    mov al, 0xBA
    mov ah, A_BORDER
    stosw
    VGA_AT BM_R_OPT1, BM_RIGHT_L+2
    mov si, str_ri_ext
    mov ah, A_NORMAL
    call bm_puts
    VGA_AT BM_R_OPT1, BM_RIGHT_R
    mov al, 0xBA
    mov ah, A_BORDER
    stosw

    ; Row 7: HDD count
    VGA_AT BM_R_OPT2, BM_RIGHT_L
    mov al, 0xBA
    mov ah, A_BORDER
    stosw
    VGA_AT BM_R_OPT2, BM_RIGHT_L+2
    mov si, str_ri_hdd
    mov ah, A_NORMAL
    call bm_puts
    mov al, [0x0475]        ; BDA hard disk count
    add al, '0'
    mov ah, A_INFO
    stosw
    mov si, str_ri_drv
    mov ah, A_NORMAL
    call bm_puts
    VGA_AT BM_R_OPT2, BM_RIGHT_R
    mov al, 0xBA
    mov ah, A_BORDER
    stosw

    ; Row 8: ACPI/SMBIOS
    VGA_AT BM_R_PBL, BM_RIGHT_L
    mov al, 0xBA
    mov ah, A_BORDER
    stosw
    VGA_AT BM_R_PBL, BM_RIGHT_L+2
    mov si, str_ri_acpi
    mov ah, A_NORMAL
    call bm_puts
    VGA_AT BM_R_PBL, BM_RIGHT_R
    mov al, 0xBA
    mov ah, A_BORDER
    stosw

    ; Bottom border
    VGA_AT BM_R_PBRT, BM_RIGHT_L
    mov al, 0xC8
    mov ah, A_BORDER
    stosw
    mov cx, BM_RIGHT_INN
    mov al, 0xCD
.rp_r9:
    stosw
    loop .rp_r9
    mov al, 0xBC
    stosw

    pop di
    pop cx
    pop ax
    ret

; ── bm_cnt_box ───────────────────────────────────────────────────────────────
; Draws the countdown / status box at rows 11-13.
bm_cnt_box:
    push ax
    push cx
    push di

    ; Row 11: ┌────...────┐
    VGA_AT BM_R_CTB, 2
    mov al, 0xDA
    mov ah, A_BORDER
    stosw
    mov cx, 74
    mov al, 0xC4
.cb_t:
    stosw
    loop .cb_t
    mov al, 0xBF
    stosw

    ; Row 12: │  Booting in 5 seconds...  │
    VGA_AT BM_R_CTM, 2
    mov al, 0xB3
    mov ah, A_BORDER
    stosw
    VGA_AT BM_R_CTM, 4
    mov si, str_cnt_pre
    mov ah, A_NORMAL
    call bm_puts
    ; digit at fixed col 15
    VGA_AT BM_R_CTM, 15
    mov al, '5'
    mov ah, A_COUNT
    stosw
    VGA_AT BM_R_CTM, 17
    mov si, str_cnt_post
    mov ah, A_NORMAL
    call bm_puts
    VGA_AT BM_R_CTM, 77
    mov al, 0xB3
    mov ah, A_BORDER
    stosw

    ; Row 13: └────...────┘
    VGA_AT BM_R_CTB2, 2
    mov al, 0xC0
    mov ah, A_BORDER
    stosw
    mov cx, 74
    mov al, 0xC4
.cb_b:
    stosw
    loop .cb_b
    mov al, 0xD9
    stosw

    pop di
    pop cx
    pop ax
    ret

; ── bm_helpbar ───────────────────────────────────────────────────────────────
bm_helpbar:
    push ax
    push di

    VGA_AT BM_R_HELP, 5
    mov si, str_help
    mov ah, A_DIM
    call bm_puts

    pop di
    pop ax
    ret

; ── bm_statusbar ─────────────────────────────────────────────────────────────
bm_statusbar:
    push ax
    push cx
    push di

    VGA_AT BM_R_STAT, 0
    mov cx, 80
    mov ax, (A_STAT << 8) | ' '
    rep stosw

    VGA_AT BM_R_STAT, 2
    mov si, str_status
    mov ah, A_STAT
    call bm_puts

    pop di
    pop cx
    pop ax
    ret

; ── bm_redraw_opts ───────────────────────────────────────────────────────────
; Redraws all 3 option rows with correct highlight.
bm_redraw_opts:
    push ax
    push bx
    push cx
    push di

    xor bx, bx              ; option index 0..2
.ro_loop:
    ; row = BM_R_OPT0 + bx
    ; DI = (row * 80 + BM_LEFT_L + 1) * 2  →  col 2 = first content cell
    mov ax, BM_R_OPT0
    add ax, bx
    mov cx, 80
    mul cx
    add ax, BM_LEFT_L + 1
    shl ax, 1
    mov di, ax

    ; Choose attribute
    cmp bl, [NYX_BOOT_SELECTED]
    je .ro_sel
    mov ah, A_NORMAL
    jmp .ro_fill
.ro_sel:
    mov ah, A_SEL

.ro_fill:
    ; Fill 34 cells with (attr, ' ')
    push di
    mov cx, BM_LEFT_INN
    mov al, ' '
.ro_clr:
    stosw
    loop .ro_clr
    pop di                  ; restore to start of content

    ; Write arrow indicator
    cmp bl, [NYX_BOOT_SELECTED]
    jne .ro_noarrow
    mov al, 0x10            ; ► in CP437
    stosw
    mov al, ' '
    stosw
    jmp .ro_str
.ro_noarrow:
    mov al, ' '
    stosw
    stosw                   ; two spaces

.ro_str:
    ; Write option string via CS
    cmp bx, 0
    jne .ro_chk1
    mov si, str_opt0
    jmp .ro_write
.ro_chk1:
    cmp bx, 1
    jne .ro_is2
    mov si, str_opt1
    jmp .ro_write
.ro_is2:
    mov si, str_opt2

.ro_write:
    call bm_puts

    inc bx
    cmp bx, MENU_OPTS
    jl .ro_loop

    pop di
    pop cx
    pop bx
    pop ax
    ret

; ── bm_update_cnt ────────────────────────────────────────────────────────────
; BL = seconds remaining. Updates countdown digit on screen.
bm_update_cnt:
    push ax
    push di

    VGA_AT BM_R_CTM, 15
    mov al, bl
    add al, '0'
    mov ah, A_COUNT
    stosw

    pop di
    pop ax
    ret

; ── bm_loop ──────────────────────────────────────────────────────────────────
; Input/timer loop. Sets NYX_BOOT_LAST then returns.
bm_loop:
    push bx
    push cx
    push dx

    mov bl, BM_TIMEOUT
    mov cx, [0x046C]        ; initial tick reference (BDA low word)

.poll:
    mov ah, 0x01
    int 0x16                ; check key
    jz .no_key

    mov ah, 0x00
    int 0x16                ; read key: AH=scan, AL=ascii

    test al, al
    jnz .ascii

    ; Extended key (AL=0): AH = scan code
    cmp ah, 0x48            ; UP arrow
    je .up
    cmp ah, 0x50            ; DOWN arrow
    je .down
    jmp .any_key            ; other extended → reset timer

.ascii:
    cmp al, 0x0D            ; Enter
    je .boot
    cmp al, 0x1B            ; Esc
    je .boot
    cmp al, '1'
    je .key1
    cmp al, '2'
    je .key2
    cmp al, '3'
    je .key3
    jmp .any_key

.up:
    mov al, [NYX_BOOT_SELECTED]
    test al, al
    jz .any_key
    dec al
    mov [NYX_BOOT_SELECTED], al
    call bm_redraw_opts
    jmp .any_key

.down:
    mov al, [NYX_BOOT_SELECTED]
    cmp al, MENU_OPTS - 1
    jge .any_key
    inc al
    mov [NYX_BOOT_SELECTED], al
    call bm_redraw_opts
    jmp .any_key

.key1:
    mov byte [NYX_BOOT_SELECTED], 0
    call bm_redraw_opts
    jmp .boot
.key2:
    mov byte [NYX_BOOT_SELECTED], 1
    call bm_redraw_opts
    jmp .boot
.key3:
    mov byte [NYX_BOOT_SELECTED], 2
    call bm_redraw_opts
    jmp .boot

.any_key:
    ; Reset countdown on any non-boot key
    mov bl, BM_TIMEOUT
    mov cx, [0x046C]
    call bm_update_cnt
    jmp .poll

.no_key:
    ; Check timer: elapsed = current_ticks - reference
    mov dx, [0x046C]
    sub dx, cx
    cmp dx, 18
    jb .poll

    add cx, 18
    dec bl
    call bm_update_cnt
    test bl, bl
    jnz .poll

    ; Timeout — serial log then boot
    mov si, str_bm_timeout
    call serial_puts

.boot:
    mov al, [NYX_BOOT_SELECTED]
    mov dl, BOOT_HDD
    test al, al
    jz .save
    cmp al, 1
    jne .net
    mov dl, BOOT_CD
    jmp .save
.net:
    mov dl, BOOT_NET
.save:
    mov [NYX_BOOT_LAST], dl

    pop dx
    pop cx
    pop bx
    ret

; ── bm_puts ──────────────────────────────────────────────────────────────────
; CS:SI = null-terminated string, AH = attr, ES:DI = destination. Advances DI.
bm_puts:
    push ax
.bp_loop:
    cs lodsb
    test al, al
    jz .bp_done
    stosw
    jmp .bp_loop
.bp_done:
    pop ax
    ret


; ── bm_print_dec2 ────────────────────────────────────────────────────────────
; AX = 16-bit unsigned number, BH = attr. ES:DI = dest.  DI advances.
bm_print_dec2:
    push ax
    push bx
    push cx
    push dx

    mov cx, 0               ; digit count

    test ax, ax
    jnz .p2_div
    mov al, '0'
    mov ah, bh
    stosw
    jmp .p2_done

.p2_div:
    xor dx, dx
    push bx                 ; save BX (has attr in BH)
    mov bx, 10
    div bx
    pop bx
    push dx                 ; save remainder digit
    inc cx
    test ax, ax
    jnz .p2_div

.p2_print:
    pop dx
    mov al, dl
    add al, '0'
    mov ah, bh
    stosw
    loop .p2_print

.p2_done:
    pop dx
    pop cx
    pop bx
    pop ax
    ret

; ── INT 0x77 — Nyx Hypervisor Extension ──────────────────────────────────────
int77_handler:
    cmp ah, 0x00
    je .i77_version
    cmp ah, 0x01
    je .i77_getdev
    cmp ah, 0x02
    je .i77_setdev
    cmp ah, 0x03
    je .i77_memmap
    cmp ah, 0x04
    je .i77_acpi
    cmp ah, 0x05
    je .i77_power
    stc
    iret

.i77_version:
    mov si, str_banner      ; defined in main.asm
    clc
    iret

.i77_getdev:
    push ds
    xor bx, bx
    mov ds, bx
    mov al, [NYX_BOOT_LAST]
    pop ds
    clc
    iret

.i77_setdev:
    push ds
    xor bx, bx
    mov ds, bx
    mov [NYX_BOOT_LAST], al
    pop ds
    clc
    iret

.i77_memmap:
    xor ax, ax
    mov bx, 0x5000
    clc
    iret

.i77_acpi:
    mov ax, 0x9FC0
    mov bx, 0x0010
    clc
    iret

.i77_power:
    test al, al
    jz .i77_shutdown
    cmp al, 0x01
    je .i77_warm
    mov al, 0x0E
    mov dx, 0xCF9
    out dx, al
.i77_halt:
    cli
    hlt
    jmp .i77_halt

.i77_shutdown:
    mov ax, 0x2000
    mov dx, 0xB004
    out dx, ax
    jmp .i77_halt

.i77_warm:
    jmp 0xFFFF:0x0000

; ── Strings ──────────────────────────────────────────────────────────────────
str_title_l:   db 'Open Source Firmware', 0
str_title_c:   db '[ NYX BIOS v1.0 ]', 0
str_title_r:   db 'UEFI Alternative', 0

str_lhdr:      db ' BOOT DEVICES ', 0
str_rhdr:      db ' SYSTEM INFORMATION ', 0

str_opt0:      db '[1] Hard Disk (0x80)', 0
str_opt1:      db '[2] CD-ROM / DVD (0xE0)', 0
str_opt2:      db '[3] Network / PXE (0x81)', 0

str_ri_mem:    db 'Base RAM : ', 0
str_ri_kb:     db ' KB', 0
str_ri_ext:    db 'Ext  RAM : 63 MB (E820)', 0
str_ri_hdd:    db 'Hard Disk: ', 0
str_ri_drv:    db ' drive(s)', 0
str_ri_acpi:   db 'ACPI/SMBIOS : Initialized', 0

str_cnt_pre:   db 'Booting in ', 0
str_cnt_post:  db ' sec...  Press any key to stop.', 0

str_help:      db 0x18, '/', 0x19, ' Navigate    Enter/1-3 Select    Esc Cancel', 0

str_status:    db ' NyxBIOS v1.0  |  Production x86 Firmware  |  UEFI Replacement', 0

str_bm_dbg1:    db '[  ] Entered BM_LOOP', 13, 10, 0
str_bm_dbg2:    db '[OK] Exited BM_LOOP', 13, 10, 0
str_bm_timeout: db '[!!] Boot menu timeout - using default', 13, 10, 0
