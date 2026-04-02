; src/video.asm — VGA + INT 0x10

%define VGA_BASE    0xB800
%define VGA_CRTC    0x3D4
%define VGA_ATTR    0x08    ; light grey on black

video_init:
    push ax
    push dx
    ; Set mode 3: 80x25 color text
    ; (already default but be explicit)
    mov ax, 0x0003
    ; Don't call INT 10h yet (IVT not setup)
    ; Just clear the screen manually
    push es
    mov ax, VGA_BASE
    mov es, ax
    mov di, 0
    mov cx, 80*25
    mov ax, 0x0720          ; space, light grey
    rep stosw
    pop es

    ; Setup cursor
    call video_set_cursor_type
    call video_set_cursor_pos

    pop dx
    pop ax
    ret

video_set_cursor_type:
    push ax
    push dx
    mov dx, VGA_CRTC
    ; Start scanline
    mov al, 0x0A
    out dx, al
    inc dx
    mov al, 0x0D            ; cursor start
    out dx, al
    dec dx
    ; End scanline
    mov al, 0x0B
    out dx, al
    inc dx
    mov al, 0x0E            ; cursor end
    out dx, al
    pop dx
    pop ax
    ret

video_set_cursor_pos:
    ; DH = row, DL = col
    push ax
    push bx
    push dx
    ; Calculate linear position
    xor bh, bh
    mov bl, dh
    mov al, 80
    mul bl
    xor bh, bh
    mov bl, dl
    add ax, bx
    ; Write to CRTC
    mov dx, VGA_CRTC
    mov bx, ax
    ; High byte
    mov al, 0x0E
    out dx, al
    inc dx
    mov al, bh
    out dx, al
    dec dx
    ; Low byte
    mov al, 0x0F
    out dx, al
    inc dx
    mov al, bl
    out dx, al
    pop dx
    pop bx
    pop ax
    ret

video_print_char:
    ; AL = char, BL = attribute
    push es
    push di
    push ax
    mov ax, VGA_BASE
    mov es, ax
    ; Get cursor position
    push ax
    mov ax, [BDA_CURPOS]
    mov dh, ah              ; row
    mov dl, al              ; col
    pop ax
    ; Calculate offset
    push ax
    xor ah, ah
    mov al, dh
    mov cl, 80
    mul cl
    xor dh, dh
    add ax, dx
    shl ax, 1               ; *2 for char+attr
    mov di, ax
    pop ax
    ; Write char and attribute
    stosb
    mov al, bl
    stosb
    ; Advance cursor
    mov ax, [BDA_CURPOS]
    inc al                  ; col++
    cmp al, 80
    jl .no_wrap
    xor al, al              ; col = 0
    inc ah                  ; row++
    cmp ah, 25
    jl .no_scroll
    call video_scroll
    mov ah, 24              ; last row
.no_scroll:
.no_wrap:
    mov [BDA_CURPOS], ax
    mov dh, ah
    mov dl, al
    call video_set_cursor_pos
    pop ax
    pop di
    pop es
    ret

video_scroll:
    ; Scroll screen up one line
    push es
    push ds
    push si
    push di
    push cx
    mov ax, VGA_BASE
    mov es, ax
    mov ds, ax
    mov si, 80*2            ; source: line 1
    mov di, 0               ; dest: line 0
    mov cx, 80*24           ; 24 lines of words
    rep movsw
    ; Clear last line
    mov di, 80*24*2
    mov cx, 80
    mov ax, 0x0720
    rep stosw
    pop cx
    pop di
    pop si
    pop ds
    pop es
    ret

video_print_str:
    ; DS:SI = string
    push ax
    push bx
    mov bl, VGA_ATTR
.loop:
    cs lodsb
    test al, al
    jz .done
    cmp al, 13              ; CR
    je .cr
    cmp al, 10              ; LF
    je .lf
    call video_print_char
    jmp .loop
.cr:
    mov ax, [BDA_CURPOS]
    xor al, al
    mov [BDA_CURPOS], ax
    jmp .loop
.lf:
    mov ax, [BDA_CURPOS]
    inc ah
    cmp ah, 25
    jl .lf_ok
    call video_scroll
    mov ah, 24
.lf_ok:
    mov [BDA_CURPOS], ax
    jmp .loop
.done:
    pop bx
    pop ax
    ret

; ── INT 0x10 handler ────────────────────────────
int10_handler:
    cmp ah, 0x00
    je .set_mode
    cmp ah, 0x01
    je .set_cursor_type
    cmp ah, 0x02
    je .set_cursor_pos
    cmp ah, 0x03
    je .get_cursor
    cmp ah, 0x05
    je .set_page
    cmp ah, 0x06
    je .scroll_up
    cmp ah, 0x07
    je .scroll_down
    cmp ah, 0x08
    je .read_char
    cmp ah, 0x09
    je .write_char_attr
    cmp ah, 0x0A
    je .write_char
    cmp ah, 0x0E
    je .teletype
    cmp ah, 0x0F
    je .get_mode
    cmp ah, 0x11
    je .font_info
    cmp ah, 0x12
    je .video_config
    cmp ah, 0x13
    je .write_string
    cmp ah, 0x4F
    je .vesa
    iret

.set_mode:
    ; AL = mode
    and al, 0x7F
    mov [BDA_VIDEOMODE], al
    call video_init
    iret

.set_cursor_type:
    mov [BDA_CURTYPE], cx
    iret

.set_cursor_pos:
    ; BH = page, DH = row, DL = col
    push ax
    mov ax, dx
    xchg ah, al
    mov [BDA_CURPOS], ax
    call video_set_cursor_pos
    pop ax
    iret

.get_cursor:
    mov cx, [BDA_CURTYPE]
    mov dx, [BDA_CURPOS]
    xchg dh, dl
    iret

.set_page:
    iret

.scroll_up:
    ; AL = lines, BH = attr
    ; CH/CL = top-left, DH/DL = bottom-right
    cmp al, 0
    je .clear_window
    call video_scroll
    iret
.clear_window:
    push es
    push di
    push cx
    mov cx, VGA_BASE
    mov es, cx
    xor di, di
    mov cx, 80*25
    mov ah, bh
    mov al, ' '
    rep stosw
    pop cx
    pop di
    pop es
    iret

.scroll_down:
    iret

.read_char:
    ; Return space
    mov ax, 0x0720
    iret

.write_char_attr:
    ; AL = char, BL = attr, CX = count
    push cx
    push bx
.wca_loop:
    push bx
    mov bl, bh
    call video_print_char
    pop bx
    loop .wca_loop
    pop bx
    pop cx
    iret

.write_char:
    push bx
    mov bl, VGA_ATTR
    call video_print_char
    pop bx
    iret

.teletype:
    push bx
    mov bl, VGA_ATTR
    call video_print_char
    call serial_putchar
    pop bx
    iret

.get_mode:
    mov al, [BDA_VIDEOMODE]
    mov ah, 80
    mov bh, 0
    iret

.font_info:
    ; Return font info for mode
    cmp bl, 0x30
    je .font_data
    iret
.font_data:
    ; Return 8x16 font pointer
    mov cx, 16              ; bytes per char
    mov dx, 256             ; num chars
    iret

.video_config:
    ; AL = 0x10: return info
    cmp al, 0x10
    je .vc_info
    iret
.vc_info:
    mov bh, 0               ; mono/color
    mov bl, 0x03            ; 256KB VRAM
    mov ch, 0               ; feature bits
    mov cl, [BDA_VIDEOMODE]
    iret

.write_string:
    ; ES:BP = string, CX = length
    ; AL = mode, BL = attr
    ; DH/DL = row/col
    push si
    push ax
    push cx
    push dx
    ; Set cursor
    mov [BDA_CURPOS], dx
    mov si, bp
.ws_loop:
    mov al, [es:si]
    inc si
    push bx
    mov bl, bh
    test al, 0x02           ; mode bit 1: use attrs
    jz .ws_noattr
    mov bl, [es:si]
    inc si
.ws_noattr:
    push bx
    mov bl, bl
    call video_print_char
    call serial_putchar
    pop bx
    pop bx
    loop .ws_loop
    pop dx
    pop cx
    pop ax
    pop si
    iret

.vesa:
    ; VESA/ VBE handler
    cmp al, 0x00
    je .vesa_info
    cmp al, 0x01
    je .vesa_mode_info
    cmp al, 0x02
    je .vesa_set_mode
    cmp al, 0x03
    je .vesa_get_mode
    cmp al, 0x04
    je .vesa_save_state
    cmp al, 0x05
    je .vesa_display_start
    cmp al, 0x06
    je .vesa_palette
    cmp al, 0x07
    je .vesa_get_set_logical_scanline
    cmp al, 0x08
    je .vesa_get_set_scanline_length
    
    mov ax, 0x014F          ; not supported
    iret

.vesa_info:
    ; Return VBE info block
    push es
    push di
    mov di, bp
    mov dword [es:di], 0x32454256  ; 'VBE2'
    mov word [es:di+4],  0x0300    ; version 3.0
    ; OEM string
    mov dword [es:di+8], oem_string
    ; Capabilities
    mov dword [es:di+12], 0x00000003     ; DAC8, non-VGA
    ; Video mode pointer (list of supported modes)
    mov dword [es:di+16], vesa_mode_list
    mov word [es:di+20], 0
    ; Memory (in 64KB units)
    mov word [es:di+22], 0x0400    ; 256MB
    
    mov ax, 0x004F
    pop di
    pop es
    iret

.vesa_mode_info:
    ; Return mode info for mode in CX
    ; ES:DI = buffer
    push es
    push di
    
    ; Find mode info
    mov si, vesa_mode_list
    xor ax, ax
.find_mode:
    lodsw
    test ax, ax
    jz .mode_not_found
    cmp ax, cx
    jne .find_mode
    
    ; Found mode - fill in info
    ; For simplicity, return generic info based on resolution
    ; Mode attributes
    mov word [es:di+0x00], 0x009B   ; supported,Color,Graphics
    ; Window attributes
    mov byte [es:di+0x02], 0x01    ; window A
    mov byte [es:di+0x03], 0x01    ; window B
    ; Window granularity
    mov word [es:di+0x04], 0x0040  ; 64KB
    ; Window size
    mov word [es:di+0x06], 0x0040  ; 64KB
    ; Window start segment
    mov word [es:di+0x08], 0xA000
    ; Window function pointer
    mov dword [es:di+0x0A], 0x00000000
    ; Bytes per scanline
    mov word [es:di+0x0E], 0x0800  ; 2048
    
    ; Resolution and bpp based on mode
    ; Assume mode 0x118 (1024x768x32)
    mov word [es:di+0x12], 0x0400  ; width
    mov word [es:di+0x14], 0x0300  ; height
    mov byte [es:di+0x18], 0x20    ; bits per pixel
    
    ; Memory model (0=Text, 1=CGA, 2=Hercules, 3=Planar, 4=Packed, 5=Direct)
    mov byte [es:di+0x19], 0x06    ; Direct (XGA)
    
    ; Bank size (0 for direct modes)
    mov byte [es:di+0x1B], 0x00
    
    ; Linear framebuffer address
    mov dword [es:di+0x28], 0xE0000000
    
    mov ax, 0x004F
    jmp .mode_info_done
    
.mode_not_found:
    mov ax, 0x014F
.mode_info_done:
    pop di
    pop es
    iret

.vesa_set_mode:
    ; Set video mode CX = mode number
    ; BX = mode attributes (linear framebuffer)
    
    ; Check if mode is supported
    mov si, vesa_mode_list
.find_mode_set:
    lodsw
    test ax, ax
    jz .mode_fail
    cmp ax, cx
    jne .find_mode_set
    
    ; Set the mode (simplified)
    ; In real implementation, would program CRTC, etc.
    mov word [BDA_VIDEOMODE], 0x13
    
    mov ax, 0x004F
    iret
    
.mode_fail:
    mov ax, 0x014F
    iret

.vesa_get_mode:
    ; Get current video mode
    mov cx, [BDA_VIDEOMODE]
    mov ax, 0x004F
    iret

.vesa_save_state:
    ; Save video state
    mov ax, 0x004F
    iret

.vesa_display_start:
    ; Set display start
    ; DX:CX = address, BH = pan
    mov ax, 0x004F
    iret

.vesa_palette:
    ; Palette functions
    mov ax, 0x004F
    iret

.vesa_get_set_logical_scanline:
    mov ax, 0x004F
    iret

.vesa_get_set_scanline_length:
    mov ax, 0x004F
    iret

; ── VBE EDID Passthrough ───────────────────────
vbe_edid:
    ; AL = 0x06: Get EDID
    ; ES:DI = buffer for EDID (128 bytes)
    ; Returns: AX = 0x004F if success
    
    ; Try to read EDID via DDC (Display Data Channel)
    ; DDC uses I2C on pins 12/13 of VGA connector
    
    push es
    push di
    push cx
    push dx
    
    ; Clear buffer
    mov cx, 128
    xor ax, ax
    rep stosb
    
    ; Try reading from port 0xA0 (I2C address for EDID)
    ; This is a simplified implementation
    
    ; For now, return not supported
    pop dx
    pop cx
    pop di
    pop es
    
    mov ax, 0x014F    ; Function not supported
    stc
    iret

; VESA data
oem_string:
    db 'Nyx BIOS v1.0', 0

vesa_mode_list:
    ; List of supported VESA modes
    dw 0x0100    ; 640x400x8
    dw 0x0101    ; 640x480x8
    dw 0x0103    ; 800x600x8
    dw 0x0105    ; 1024x768x8
    dw 0x0107    ; 1280x1024x8
    dw 0x0111    ; 640x480x16
    dw 0x0114    ; 800x600x16
    dw 0x0117    ; 1024x768x16
    dw 0x011A    ; 1280x1024x16
    dw 0x0112    ; 640x480x24
    dw 0x0115    ; 800x600x24
    dw 0x0118    ; 1024x768x24
    dw 0x011B    ; 1280x1024x24
    dw 0x0113    ; 640x480x32
    dw 0x0116    ; 800x600x32
    dw 0x0119    ; 1024x768x32
    dw 0x011C    ; 1280x1024x32
    dw 0x0000    ; end of list
