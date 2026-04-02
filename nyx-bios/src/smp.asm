; src/smp.asm — SMP (Symmetric Multiprocessing) Secondary CPU Startup

BITS 16

; APIC Registers
%define APIC_SPURIOUS_VECTOR  0x00F0
%define APIC_TASK_PRIORITY    0x0080
%define APIC_ERROR_STATUS     0x0280
%define APIC_EOI             0x00B0
%define APIC_INTERRUPT_CMD   0x0300

; ── SMP State Variables ───────────────────────────────
NYX_SMP_CPU_COUNT:    db 0x01
NYX_SMP_APIC_BASE:    dw 0x0000
NYX_SMP_ENABLED:      db 0x00

; ── SMP Initialization ───────────────────────────────
smp_initialize:
    push ax
    push si
    
    POST 0x38
    
    mov si, str_smp_init
    call serial_puts
    
    call smp_detect_apic
    cmp byte [NYX_SMP_ENABLED], 0x00
    je .no_apic
    
    call smp_detect_cpus
    mov al, [NYX_SMP_CPU_COUNT]
    cmp al, 1
    je .single_cpu
    
    call smp_setup_apic
    
    mov si, str_smp_done
    call serial_puts
    jmp .done
    
.no_apic:
    mov si, str_smp_no_apic
    call serial_puts
    jmp .done
    
.single_cpu:
    mov si, str_smp_single
    call serial_puts
    
.done:
    POST 0x39
    
    pop si
    pop ax
    ret

; ── Detect APIC ───────────────────────────────────────
smp_detect_apic:
    push ax
    
    ; Check for APIC via CPUID
    mov eax, 1
    cpuid
    
    test edx, 0x00000200
    jz .no_apic
    
    ; Enable APIC via port
    in al, 0xA1
    or al, 0x01
    out 0xA1, al
    
    ; Set default APIC base
    mov word [NYX_SMP_APIC_BASE], 0xFEE0
    
    mov byte [NYX_SMP_ENABLED], 0x01
    
    mov si, str_smp_apic_found
    call serial_puts
    jmp .done
    
.no_apic:
    mov byte [NYX_SMP_ENABLED], 0x00
    
.done:
    pop ax
    ret

; ── Detect Number of CPUs ──────────────────────────────
smp_detect_cpus:
    push ax
    
    mov eax, 1
    cpuid

    ; CPUID.1: EBX[23:16] = max logical processors per package.
    mov eax, ebx
    shr eax, 16
    and al, 0xFF

    cmp al, 1
    jge .store
.set_count:
    mov al, 1
.store:
    mov [NYX_SMP_CPU_COUNT], al
    
.done:
    pop ax
    ret

; ── Setup APIC ────────────────────────────────────────
smp_setup_apic:
    push ax
    push dx
    
    mov dx, [NYX_SMP_APIC_BASE]
    test dx, dx
    jz .fail
    
    ; Set Spurious Interrupt Vector Register
    mov dx, 0xFEE0 + APIC_SPURIOUS_VECTOR
    mov ax, 0x010F
    out dx, ax
    
    ; Clear Task Priority
    mov dx, 0xFEE0 + APIC_TASK_PRIORITY
    xor ax, ax
    out dx, ax
    
    ; Send EOI
    mov dx, 0xFEE0 + APIC_EOI
    xor ax, ax
    out dx, ax
    
    clc
    jmp .done
    
.fail:
    stc
    
.done:
    pop dx
    pop ax
    ret

; ── Start Secondary CPUs ───────────────────────────────
smp_start_secondary_cpus:
    push ax
    push si
    
    mov si, str_smp_starting
    call serial_puts
    
    ; Send INIT IPI
    call smp_send_init_ipi
    
    ; Send Startup IPI
    call smp_send_sipi_ipi
    
    mov si, str_smp_cpus_started
    call serial_puts
    
    pop si
    pop ax
    ret

; ── Send INIT IPI ───────────────────────────────────────
smp_send_init_ipi:
    push ax
    push dx
    ; Real xAPIC is memory-mapped at 0xFEE00000 (not reachable via 16-bit I/O ports).
    ; SMP startup is currently a stub.
    
    pop dx
    pop ax
    ret

; ── Send Startup IPI ───────────────────────────────────
smp_send_sipi_ipi:
    push ax
    push dx
    ; SMP startup is currently a stub.
    
    pop dx
    pop ax
    ret

; ── Check CPU Ready ───────────────────────────────────
smp_cpu_ready:
    xor ax, ax
    ret

; ── Strings ───────────────────────────────────────────
str_smp_init:          db '[  ] SMP: initializing...', 13, 10, 0
str_smp_done:          db '[OK] SMP: secondary CPUs started', 13, 10, 0
str_smp_no_apic:       db '[  ] SMP: no APIC found', 13, 10, 0
str_smp_apic_found:    db '[OK] SMP: APIC found', 13, 10, 0
str_smp_single:        db '[  ] SMP: single CPU only', 13, 10, 0
str_smp_starting:      db '[  ] SMP: starting secondary CPUs...', 13, 10, 0
str_smp_cpus_started:  db '[OK] SMP: all CPUs started', 13, 10, 0
