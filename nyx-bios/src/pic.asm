; src/pic.asm — 8259A PIC driver

%define PIC1_CMD  0x20
%define PIC1_DATA 0x21
%define PIC2_CMD  0xA0
%define PIC2_DATA 0xA1
%define PIC_EOI   0x20

pic_init:
    push ax

    ; ICW1: cascade mode, edge triggered
    mov al, 0x11
    out PIC1_CMD, al
    out PIC2_CMD, al

    ; ICW2: use the classic BIOS-compatible vectors
    ;       master IRQ0-7  -> INT 0x08-0x0F
    ;       slave  IRQ8-15 -> INT 0x70-0x77
    mov al, 0x08
    out PIC1_DATA, al
    mov al, 0x70
    out PIC2_DATA, al

    ; ICW3: master has slave on IRQ2
    mov al, 0x04
    out PIC1_DATA, al
    ; ICW3: slave ID = 2
    mov al, 0x02
    out PIC2_DATA, al

    ; ICW4: 8086 mode
    mov al, 0x01
    out PIC1_DATA, al
    out PIC2_DATA, al

    ; Mask all except timer(0) and keyboard(1)
    mov al, 0xFC            ; unmask IRQ0, IRQ1
    out PIC1_DATA, al
    mov al, 0xFF            ; mask all slave
    out PIC2_DATA, al

    pop ax
    ret

pic_eoi_master:
    push ax
    mov al, PIC_EOI
    out PIC1_CMD, al
    pop ax
    ret

pic_eoi_slave:
    push ax
    mov al, PIC_EOI
    out PIC2_CMD, al
    out PIC1_CMD, al
    pop ax
    ret

pic_unmask_irq:
    ; AL = IRQ number to unmask
    push ax
    push cx
    push dx
    cmp al, 8
    jge .slave
    mov dx, PIC1_DATA
    jmp .do_unmask
.slave:
    sub al, 8
    mov dx, PIC2_DATA
.do_unmask:
    mov cl, al
    in al, dx               ; read mask
    mov ah, al              ; save mask in AH
    mov al, 1
    shl al, cl              ; 1 << irq
    not al                  ; ~(1 << irq)
    and ah, al              ; clear irq bit
    mov al, ah
    out dx, al
    pop dx
    pop cx
    pop ax
    ret
