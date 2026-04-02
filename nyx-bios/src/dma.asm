; src/dma.asm — DMA 8237 controller and NMI support

; DMA I/O ports
%define DMA1_BASE      0x00    ; Channel 0-3
%define DMA2_BASE      0xC0    ; Channel 4-7
%define DMA1_CMD       0x08    ; Command register
%define DMA1_MASK      0x0A    ; Mask register
%define DMA1_MODE      0x0B    ; Mode register
%define DMA1_CLEAR     0x0C    ; Clear flip-flop
%define DMA1_MASTER    0x0D    ; Master clear
%define DMA1_RESET     0x0E    ; Reset mask

%define DMA2_CMD       0xD0
%define DMA2_MASK      0xD4
%define DMA2_MODE      0xD6
%define DMA2_CLEAR     0xD8
%define DMA2_MASTER    0xDA
%define DMA2_RESET     0xDC

; NMI ports
%define NMI_PORT       0x70    ; CMOS/RTC port A

; ── DMA Initialization ───────────────────────────
dma_init:
    push ax
    push cx
    
    POST 0x40
    
    ; Master clear (channel 0-3)
    mov al, 0xFF
    out DMA1_MASTER, al
    
    ; Wait a bit
    xor ax, ax
.wait1:
    dec ax
    jnz .wait1
    
    ; Master clear (channel 4-7)
    mov al, 0xFF
    out DMA2_MASTER, al
    
    ; Wait
    xor ax, ax
.wait2:
    dec ax
    jnz .wait2
    
    ; Disable all channels initially (set mask bits)
    mov al, 0x0F        ; Mask channels 0-3
    out DMA1_MASK, al
    
    mov al, 0xFF        ; Mask channels 4-7
    out DMA2_MASK, al
    
    ; Set default mode (verify/transfer)
    mov al, 0x40        ; Mode: verify mode
    out DMA1_MODE, al
    
    POST 0x41
    
    pop cx
    pop ax
    ret

; ── NMI Enable ──────────────────────────────────
nmi_enable:
    push ax
    ; Enable NMI by clearing bit 7 of port 0x70
    in al, NMI_PORT
    and al, 0x7F
    out NMI_PORT, al
    pop ax
    ret

; ── NMI Disable ─────────────────────────────────
nmi_disable:
    push ax
    ; Disable NMI by setting bit 7 of port 0x70
    in al, NMI_PORT
    or al, 0x80
    out NMI_PORT, al
    pop ax
    ret

; ── NMI Status (check if NMI occurred) ──────────
nmi_status:
    push ax
    ; Read status from port 0x61 (keyboard controller port B)
    in al, 0x61
    test al, 0x80
    jnz .nmi_occurred
    xor al, al
    jmp .done
.nmi_occurred:
    mov al, 0x80
.done:
    pop ax
    ret

; ── NMI Handler ──────────────────────────────────
nmi_handler:
    push ax
    push dx
    
    ; Read NMI source
    in al, 0x61
    mov ah, al
    
    ; Clear NMI
    in al, 0x61
    or al, 0x80
    out 0x61, al
    and al, 0x7F
    out 0x61, al
    
    ; Check if it's an I/O check (parity error)
    test ah, 0x40
    jnz .io_check
    test ah, 0x80
    jnz .nmi_from_kbd
    
    ; Unknown NMI source - just return
    jmp .nmi_done
    
.io_check:
    ; I/O Check / Parity Error
    jmp .nmi_done
    
.nmi_from_kbd:
    ; Keyboard NMI (keyboard error)
    jmp .nmi_done
    
.nmi_done:
    pop dx
    pop ax
    iret

; ── DMA channel setup (stub) ───────────────────
dma_setup_channel:
    ; DMA channel setup - stub for now
    clc
    ret
