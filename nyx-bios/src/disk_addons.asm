%define ATAPI2_CTRL   0x376

; ── atapi_soft_reset_secondary ─────────────────
; Purpose: Soft reset ATAPI secondary master (SRST pulse).
; Trashes: AX,DX
atapi_soft_reset_secondary:
    push dx
    mov dx, ATAPI2_CTRL
    mov al, 0x04            ; set SRST (ATAPI Device Reset bit)
    out dx, al
    call atapi2_400ns_delay
    xor al, al              ; clear SRST
    out dx, al
    call atapi2_400ns_delay
    call atapi2_drain_stale_drq
    pop dx
    ret

%define ATAPI1_CTRL   0x3F6

; ── atapi_soft_reset_primary ───────────────────
; Purpose: Soft reset ATAPI primary slave (SRST pulse).
; Trashes: AX,DX
atapi_soft_reset_primary:
    push dx
    mov dx, ATAPI1_CTRL
    mov al, 0x04
    out dx, al
    call atapi1_400ns_delay
    xor al, al
    out dx, al
    call atapi1_400ns_delay
    call atapi1_drain_stale_drq
    pop dx
    ret

; ── Logging strings ─────────────────────────────
str_atapi_read_log db 'ATAPI rd LBA=', 0
str_atapi_cnt     db ' cnt=', 0
str_atapi_nl      db 13, 10, 0
