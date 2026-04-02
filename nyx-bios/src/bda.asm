; SPDX-License-Identifier: MIT
; Nyx BIOS — https://github.com/surya/nyx-bios
; Copyright (c) 2026 Surya

; ── src/bda.asm — BIOS Data Area and Extended BIOS Data Area Setup ───────

; BDA is at segment 0x0040 (absolute address 0x400)
%define BDA_SEGMENT 0x0040

; BDA Offsets
%define BDA_COM1_IO           0x00
%define BDA_COM2_IO           0x02
%define BDA_COM3_IO           0x04
%define BDA_COM4_IO           0x06
%define BDA_LPT1_IO           0x08
%define BDA_LPT2_IO           0x0A
%define BDA_LPT3_IO           0x0C
%define BDA_EBDA_SEG          0x0E
%define BDA_EQUIPMENT_WORD    0x10
%define BDA_MEM_SIZE_KB       0x13
%define BDA_KBD_FLAGS1        0x17
%define BDA_KBD_FLAGS2        0x18
%define BDA_KBD_ALT_KEYPAD    0x19
%define BDA_KBD_BUF_HEAD      0x1A
%define BDA_KBD_BUF_TAIL      0x1C
%define BDA_KBD_BUFFER        0x1E
%define BDA_VIDEO_MODE        0x49
%define BDA_VIDEO_COLS        0x4A
%define BDA_VIDEO_PAGE_SIZE   0x4C
%define BDA_VIDEO_PAGE_START  0x4E
%define BDA_CURSOR_POS        0x50 ; 8 pages, 2 bytes per page
%define BDA_CURSOR_SHAPE      0x60
%define BDA_VIDEO_PAGE        0x62
%define BDA_VIDEO_IO_PORT     0x63
%define BDA_TIMER_LOW         0x6C
%define BDA_TIMER_HIGH        0x70
%define BDA_TIMER_OVERFLOW    0x71
%define BDA_HD_COUNT          0x75
%define BDA_VIDEO_ROWS        0x84

; EBDA Segment (places EBDA at 0x9FC00, just below 640KB)
%define EBDA_SEGMENT 0x9FC0

; ── bda_ebda_init ────────────────────────────────────────────────────────
; Purpose : Initialize the BIOS Data Area (BDA) and Extended BDA (EBDA)
; Input   : None
; Output  : None
; Trashes : AX, CX, DI, ES
bda_ebda_init:
    push ax
    push cx
    push di
    push es

    ; Point ES to the BDA
    mov ax, BDA_SEGMENT
    mov es, ax

    ; Zero out the entire BDA (0x400 to 0x500)
    xor di, di
    xor al, al
    mov cx, 256
    rep stosb

    ; --- Populate BDA Fields ---

    ; COM1 at 0x3F8, COM2 at 0x2F8
    mov word [es:BDA_COM1_IO], 0x03F8
    mov word [es:BDA_COM2_IO], 0x02F8

    ; LPT1 at 0x378
    mov word [es:BDA_LPT1_IO], 0x0378

    ; Set EBDA segment pointer. Our E820 map reserves 1KB at 0x9FC00.
    mov word [es:BDA_EBDA_SEG], EBDA_SEGMENT

    ; Set base memory size to 632KB (leaves 8KB for ACPI/EBDA)
    mov word [es:BDA_MEM_SIZE_KB], 632

    ; Equipment Word:
    ; Bit 0: Has floppy (legacy standard, set to 1)
    ; Bit 1: Has FPU
    ; Bits 5-4: Initial video mode (11 = 80x25 text)
    ; Bits 7-6: Number of floppy drives (00 = 1)
    ; Bit 9: Has PS/2 Mouse
    ; Bits 15-14: Number of printers (01 = 1)
    mov ax, 0b0100_0000_0011_0011
    mov word [es:BDA_EQUIPMENT_WORD], ax

    ; Keyboard buffer pointers (initially empty)
    mov word [es:BDA_KBD_BUF_HEAD], BDA_KBD_BUFFER
    mov word [es:BDA_KBD_BUF_TAIL], BDA_KBD_BUFFER

    ; Video settings for standard 80x25 color text mode
    mov byte [es:BDA_VIDEO_MODE], 3      ; Mode 3: 80x25 color text
    mov word [es:BDA_VIDEO_COLS], 80     ; 80 columns
    mov word [es:BDA_VIDEO_PAGE_SIZE], 4096 ; 80*25*2
    mov byte [es:BDA_VIDEO_ROWS], 24     ; 25 rows (0-indexed)
    mov word [es:BDA_VIDEO_IO_PORT], 0x3D4 ; VGA CRTC port

    ; Set cursor shape to a standard underline
    mov word [es:BDA_CURSOR_SHAPE], 0x0706

    ; Timer tick count (initially zero)
    mov dword [es:BDA_TIMER_LOW], 0

    ; Hard disk count (detected by INT 13h init)
    mov byte [es:BDA_HD_COUNT], 1 ; Assume 1 HDD for now

    ; --- Initialize EBDA ---
    ; Zero out the 1KB EBDA space
    mov ax, EBDA_SEGMENT
    mov es, ax
    xor di, di
    xor al, al
    mov cx, 1024
    rep stosb

    pop es
    pop di
    pop cx
    pop ax
    ret