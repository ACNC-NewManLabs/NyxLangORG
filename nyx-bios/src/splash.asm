; src/splash.asm — Full-screen boot UI

%define SPLASH_SEG    0xA000
%define SPLASH_W      320
%define SPLASH_H      200
%define SPLASH_BAR_X  60
%define SPLASH_BAR_Y  160
%define SPLASH_BAR_W  200
%define SPLASH_BAR_H  8
%define SPLASH_TEXT_X 60
%define SPLASH_TEXT_Y 138
%define SPLASH_TEXT_W 200
%define SPLASH_TEXT_H 10

%define THEME_BG_BASE   0
%define THEME_BG_STEP   1
%define THEME_BAR_FRAME 2
%define THEME_BAR_FILL  3
%define THEME_BAR_EMPTY 4
%define THEME_TEXT      5
%define THEME_SIZE      6

splash_init:
    push ax
    push bx
    push cx
    push dx
    push si
    push di
    push bp
    push es

    mov ax, 0x0013
    int 0x10

    mov ax, SPLASH_SEG
    mov es, ax

    ; Background gradient (theme)
    xor di, di
    call splash_get_theme_ptr
    mov al, [cs:si+THEME_BG_BASE]
    mov dl, [cs:si+THEME_BG_STEP]
    mov bx, 0
.bg_row:
    mov cx, SPLASH_W
.bg_col:
    stosb
    loop .bg_col
    add al, dl
    inc bl
    cmp bl, SPLASH_H
    jne .bg_row

    ; Draw logo from data table
    mov si, splash_logo_rects
.logo_loop:
    mov al, [cs:si]
    cmp al, 0xFF
    je .logo_done
    mov di, si
    mov bl, al
    xor bh, bh
    mov dl, [cs:di+1]
    xor dh, dh
    mov cl, [cs:di+2]
    xor ch, ch
    mov al, [cs:di+3]
    xor ah, ah
    mov si, ax
    mov al, [cs:di+4]
    call splash_fill_rect
    mov si, di
    add si, 5
    jmp .logo_loop
.logo_done:

    ; Progress bar frame
    call splash_get_theme_ptr
    mov bp, si
    mov bx, SPLASH_BAR_X
    mov dx, SPLASH_BAR_Y
    mov cx, SPLASH_BAR_W
    mov si, 1
    mov al, [cs:bp+THEME_BAR_FRAME]
    call splash_fill_rect
    mov bx, SPLASH_BAR_X
    mov dx, SPLASH_BAR_Y + SPLASH_BAR_H
    mov cx, SPLASH_BAR_W
    mov si, 1
    mov al, [cs:bp+THEME_BAR_FRAME]
    call splash_fill_rect
    mov bx, SPLASH_BAR_X
    mov dx, SPLASH_BAR_Y
    mov cx, 1
    mov si, SPLASH_BAR_H
    mov al, [cs:bp+THEME_BAR_FRAME]
    call splash_fill_rect
    mov bx, SPLASH_BAR_X + SPLASH_BAR_W
    mov dx, SPLASH_BAR_Y
    mov cx, 1
    mov si, SPLASH_BAR_H
    mov al, [cs:bp+THEME_BAR_FRAME]
    call splash_fill_rect

    ; Empty bar
    mov bx, SPLASH_BAR_X + 1
    mov dx, SPLASH_BAR_Y + 1
    mov cx, SPLASH_BAR_W - 1
    mov si, SPLASH_BAR_H - 1
    mov al, [cs:bp+THEME_BAR_EMPTY]
    call splash_fill_rect

    pop es
    pop bp
    pop di
    pop si
    pop dx
    pop cx
    pop bx
    pop ax
    ret

splash_progress:
    ; AL = percent (0..100)
    push ax
    push bx
    push cx
    push dx
    push si
    push di
    push bp
    push es

    mov ax, SPLASH_SEG
    mov es, ax

    call splash_get_theme_ptr
    mov bp, si

    mov bl, al
    xor bh, bh
    mov ax, SPLASH_BAR_W - 2
    mul bx
    mov bx, 100
    div bx
    mov di, ax

    ; Clear bar area
    mov bx, SPLASH_BAR_X + 1
    mov dx, SPLASH_BAR_Y + 1
    mov cx, SPLASH_BAR_W - 1
    mov si, SPLASH_BAR_H - 1
    mov al, [cs:bp+THEME_BAR_EMPTY]
    call splash_fill_rect

    ; Fill progress
    mov cx, di
    cmp cx, 0
    je .done
    mov bx, SPLASH_BAR_X + 1
    mov dx, SPLASH_BAR_Y + 1
    mov si, SPLASH_BAR_H - 1
    mov al, [cs:bp+THEME_BAR_FILL]
    call splash_fill_rect

.done:
    pop es
    pop bp
    pop di
    pop si
    pop dx
    pop cx
    pop bx
    pop ax
    ret

splash_end:
    push ax
    mov ax, 0x0003
    int 0x10
    call video_init
    pop ax
    ret

splash_get_theme_ptr:
    ; Returns SI = pointer to current theme entry
    push ax
    push bx
    mov al, [NYX_THEME_IDX]
    xor ah, ah
    mov bl, THEME_SIZE
    mul bl
    mov si, splash_themes
    add si, ax
    pop bx
    pop ax
    ret

splash_stage:
    ; AL = stage id
    push ax
    push bx
    push cx
    push dx
    push si
    push bp
    push es

    mov ax, SPLASH_SEG
    mov es, ax

    mov bh, al
    call splash_get_theme_ptr
    mov bp, si
    mov cl, [cs:bp+THEME_BG_STEP]
    ; Approximate background color at text Y
    mov ah, 0
    mov bl, SPLASH_TEXT_Y
    shr bl, 1
    mov al, cl
    xor ah, ah
    mul bl
    add al, [cs:bp+THEME_BG_BASE]
    mov bx, SPLASH_TEXT_X
    mov dx, SPLASH_TEXT_Y
    mov cx, SPLASH_TEXT_W
    mov si, SPLASH_TEXT_H
    call splash_fill_rect

    ; Draw stage name
    mov ah, [cs:bp+THEME_TEXT]
    mov al, bh
    call splash_stage_name
    cmp si, 0
    je .done
    mov bx, SPLASH_TEXT_X
    mov dx, SPLASH_TEXT_Y
    mov al, ah
    call splash_draw_text

.done:
    pop es
    pop bp
    pop si
    pop dx
    pop cx
    pop bx
    pop ax
    ret

splash_stage_name:
    ; AL = stage id, returns SI = string (CS), or SI=0
    push ax
    mov si, 0
    cmp al, 0x07
    je .ivt
    cmp al, 0x08
    je .pci
    cmp al, 0x09
    je .disk
    cmp al, 0x0A
    je .kbd
    cmp al, 0x0B
    je .acpi
    cmp al, 0x0C
    je .smbios
    cmp al, 0x0D
    je .dma
    cmp al, 0x0E
    je .rtc
    jmp .done
.ivt:    mov si, str_stage_ivt
    jmp .done
.pci:    mov si, str_stage_pci
    jmp .done
.disk:   mov si, str_stage_disk
    jmp .done
.kbd:    mov si, str_stage_kbd
    jmp .done
.acpi:   mov si, str_stage_acpi
    jmp .done
.smbios: mov si, str_stage_smbios
    jmp .done
.dma:    mov si, str_stage_dma
    jmp .done
.rtc:    mov si, str_stage_rtc
.done:
    pop ax
    ret

splash_draw_text:
    ; AL = color, BX = x, DX = y, CS:SI = string
    push ax
    push bx
    push cx
    push dx
    push si
    push es

    mov ax, SPLASH_SEG
    mov es, ax
    mov cl, al

.next:
    cs lodsb
    test al, al
    jz .out
    mov ah, cl
    call splash_draw_glyph
    add bx, 6
    jmp .next

.out:
    pop es
    pop si
    pop dx
    pop cx
    pop bx
    pop ax
    ret

splash_draw_glyph:
    ; AL = char, AH = color, BX = x, DX = y
    push ax
    push bx
    push cx
    push dx
    push si
    push di
    push bp

    call splash_glyph_ptr
    cmp si, 0
    je .done

    ; Compute base offset
    mov di, dx
    mov ax, SPLASH_W
    mul di
    add ax, bx
    mov di, ax
    mov cx, 5
.col:
    mov al, [cs:si]
    inc si
    mov dl, al
    mov bx, di
    mov bp, 7
.row:
    test dl, 0x01
    jz .skip
    mov [es:bx], ah
.skip:
    shr dl, 1
    add bx, SPLASH_W
    dec bp
    jnz .row
    inc di
    loop .col

.done:
    pop bp
    pop di
    pop si
    pop dx
    pop cx
    pop bx
    pop ax
    ret

splash_glyph_ptr:
    ; AL = char, returns SI = glyph (5 bytes) or 0
    push ax
    mov si, 0
    cmp al, 'A'
    jb .check_digit
    cmp al, 'Z'
    ja .check_digit
    sub al, 'A'
    xor ah, ah
    mov si, font_alpha
    mov bl, 5
    mul bl
    add si, ax
    jmp .done
.check_digit:
    cmp al, '0'
    jb .check_underscore
    cmp al, '9'
    ja .check_underscore
    sub al, '0'
    xor ah, ah
    mov si, font_digit
    mov bl, 5
    mul bl
    add si, ax
    jmp .done
.check_underscore:
    cmp al, '_'
    jne .check_dash
    mov si, font_underscore
    jmp .done
.check_dash:
    cmp al, '-'
    jne .check_space
    mov si, font_dash
    jmp .done
.check_space:
    cmp al, ' '
    jne .done
    mov si, font_space
.done:
    pop ax
    ret

splash_fill_rect:
    ; BX = x, DX = y, CX = w, SI = h, AL = color, ES = A000
    push ax
    push bx
    push cx
    push dx
    push si
    push di

    mov di, dx
    mov ax, SPLASH_W
    mul di
    add ax, bx
    mov di, ax
.set_rows:
    mov dx, si

.row:
    push cx
    rep stosb
    pop cx
    add di, SPLASH_W
    sub di, cx
    dec dx
    jnz .row

    pop di
    pop si
    pop dx
    pop cx
    pop bx
    pop ax
    ret

; Theme data (bg base, bg step, bar frame, bar fill, bar empty, text)
splash_themes:
    ; NEBULA
    db 1, 1, 13, 11, 0, 15
    ; EMBER
    db 4, 1, 14, 12, 0, 15
    ; GLACIER
    db 9, 1, 11, 10, 0, 15

; Logo rectangles: x, y, w, h, color (0xFF terminator)
splash_logo_rects:
    ; N
    db 40, 40, 12, 60, 56
    db 64, 40, 12, 60, 56
    db 52, 40, 12, 60, 56
    ; Y
    db 90, 40, 12, 30, 62
    db 114, 40, 12, 30, 62
    db 102, 70, 12, 30, 62
    ; X
    db 140, 40, 12, 60, 52
    db 164, 40, 12, 60, 52
    db 152, 40, 12, 60, 52
    db 0xFF

; Stage labels (uppercase for 5x7 font)
str_stage_ivt:    db 'IVT', 0
str_stage_pci:    db 'PCI', 0
str_stage_disk:   db 'DISK', 0
str_stage_kbd:    db 'KBD', 0
str_stage_acpi:   db 'ACPI', 0
str_stage_smbios: db 'SMBIOS', 0
str_stage_dma:    db 'DMA', 0
str_stage_rtc:    db 'RTC', 0

; 5x7 font data (columns, LSB = top)
font_alpha:
    ; A-Z
    db 0x7E,0x09,0x09,0x09,0x7E  ; A
    db 0x7F,0x49,0x49,0x49,0x36  ; B
    db 0x3E,0x41,0x41,0x41,0x41  ; C
    db 0x7F,0x41,0x41,0x41,0x3E  ; D
    db 0x7F,0x49,0x49,0x49,0x41  ; E
    db 0x7F,0x09,0x09,0x09,0x01  ; F
    db 0x3E,0x41,0x41,0x49,0x79  ; G
    db 0x7F,0x08,0x08,0x08,0x7F  ; H
    db 0x41,0x41,0x7F,0x41,0x41  ; I
    db 0x30,0x40,0x41,0x3F,0x01  ; J
    db 0x7F,0x08,0x14,0x22,0x41  ; K
    db 0x7F,0x40,0x40,0x40,0x40  ; L
    db 0x7F,0x02,0x0C,0x02,0x7F  ; M
    db 0x7F,0x04,0x08,0x10,0x7F  ; N
    db 0x3E,0x41,0x41,0x41,0x3E  ; O
    db 0x7F,0x09,0x09,0x09,0x06  ; P
    db 0x3E,0x41,0x51,0x21,0x5E  ; Q
    db 0x7F,0x09,0x19,0x29,0x46  ; R
    db 0x46,0x49,0x49,0x49,0x31  ; S
    db 0x01,0x01,0x7F,0x01,0x01  ; T
    db 0x3F,0x40,0x40,0x40,0x3F  ; U
    db 0x1F,0x20,0x40,0x20,0x1F  ; V
    db 0x3F,0x40,0x38,0x40,0x3F  ; W
    db 0x63,0x14,0x08,0x14,0x63  ; X
    db 0x03,0x04,0x78,0x04,0x03  ; Y
    db 0x61,0x51,0x49,0x45,0x43  ; Z

font_digit:
    ; 0-9
    db 0x3E,0x51,0x49,0x45,0x3E  ; 0
    db 0x00,0x42,0x7F,0x40,0x00  ; 1
    db 0x42,0x61,0x51,0x49,0x46  ; 2
    db 0x41,0x49,0x49,0x49,0x36  ; 3
    db 0x18,0x14,0x12,0x7F,0x10  ; 4
    db 0x4F,0x49,0x49,0x49,0x31  ; 5
    db 0x3E,0x49,0x49,0x49,0x30  ; 6
    db 0x01,0x71,0x09,0x05,0x03  ; 7
    db 0x36,0x49,0x49,0x49,0x36  ; 8
    db 0x06,0x49,0x49,0x49,0x3E  ; 9

font_underscore:
    db 0x40,0x40,0x40,0x40,0x40
font_dash:
    db 0x08,0x08,0x08,0x08,0x08
font_space:
    db 0x00,0x00,0x00,0x00,0x00
