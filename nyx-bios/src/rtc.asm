; src/rtc.asm — RTC/CMOS and INT 0x1A time/date functions

%define RTC_PORT_INDEX  0x70
%define RTC_PORT_DATA   0x71

%define RTC_SECOND      0x00
%define RTC_MINUTE      0x02
%define RTC_HOUR        0x04
%define RTC_DAY         0x07
%define RTC_MONTH       0x08
%define RTC_YEAR        0x09
%define RTC_CENTURY     0x32    ; Some systems

%define RTC_REG_A       0x0A
%define RTC_REG_B       0x0B
%define RTC_REG_C       0x0C
%define RTC_REG_D       0x0D

; CMOS/RTC NMI bit
%define RTC_NMI_DISABLE 0x80

; ── RTC Initialization ────────────────────────────
rtc_init:
    push ax
    push cx
    push dx
    
    ; Enable RTC interrupts (update ended, alarm)
    mov al, RTC_REG_B
    out RTC_PORT_INDEX, al
    in al, RTC_PORT_DATA
    or al, 0x70              ; Set UIE, AIE (update/alarm interrupt enable)
    out RTC_PORT_DATA, al
    
    ; Select register B for next operations
    mov al, RTC_REG_B
    out RTC_PORT_INDEX, al
    
    pop dx
    pop cx
    pop ax
    ret

; ── Read RTC Register ─────────────────────────────
rtc_read_reg:
    ; AL = register number, returns AL = value
    or al, RTC_NMI_DISABLE   ; Disable NMI when reading
    out RTC_PORT_INDEX, al
    in al, RTC_PORT_DATA
    ret

; ── Write RTC Register ───────────────────────────
rtc_write_reg:
    ; AL = register number, AH = value
    push ax
    or al, RTC_REG_D         ; Keep NMI disabled (bit 7)
    out RTC_PORT_INDEX, al
    mov al, ah
    out RTC_PORT_DATA, al
    pop ax
    ret

; ── Get RTC Time (BCD) ───────────────────────────
rtc_get_time:
    push cx
    push dx
    
    ; Wait for update in progress to clear
    mov al, RTC_REG_A
    call rtc_read_reg
    test al, 0x80
    jnz .wait
    
    ; Read hours, minutes, seconds
    mov al, RTC_HOUR
    call rtc_read_reg
    mov ch, al
    
    mov al, RTC_MINUTE
    call rtc_read_reg
    mov cl, al
    
    mov al, RTC_SECOND
    call rtc_read_reg
    mov dh, al
    
    pop ax ; dummy for bx
    pop ax ; dummy for cx
    pop dx
    pop cx
    ret
    
.wait:
    ; Busy wait for RTC update
    mov al, RTC_REG_A
    call rtc_read_reg
    test al, 0x80
    jnz .wait
    jmp rtc_get_time

; ── Get RTC Date (BCD) ───────────────────────────
rtc_get_date:
    push cx
    push dx
    
    ; Wait for update in progress
    mov al, RTC_REG_A
    call rtc_read_reg
    test al, 0x80
    jnz .wait
    
    ; Read year, month, day
    mov al, RTC_YEAR
    call rtc_read_reg
    mov dh, al
    
    mov al, RTC_MONTH
    call rtc_read_reg
    mov cl, al
    
    mov al, RTC_DAY
    call rtc_read_reg
    
    pop dx
    pop cx
    ret
    
.wait:
    mov al, RTC_REG_A
    call rtc_read_reg
    test al, 0x80
    jnz .wait
    jmp rtc_get_date

; ── INT 0x1A Time/Date Handler ──────────────────
int1a_rtc_handler:
    cmp ah, 0x00
    je .get_time
    cmp ah, 0x01
    je .set_time
    cmp ah, 0x02
    je .get_date
    cmp ah, 0x03
    je .set_date
    cmp ah, 0x04
    je .get_date_century
    cmp ah, 0x05
    je .set_date_century
    cmp ah, 0x06
    je .get_alarm
    cmp ah, 0x07
    je .set_alarm
    ; Unknown
    stc
    iret

.get_time:
    ; AH=00: Get time
    ; Returns CH=hour, CL=minute, DH=second, DL=0 if no daylight savings
    call rtc_get_time
    ; AL = 0 (success)
    mov al, 0
    iret

.set_time:
    ; AH=01: Set time
    ; CH=hour, CL=minute, DH=second
    push ax
    mov al, RTC_HOUR
    mov ah, ch
    call rtc_write_reg
    mov al, RTC_MINUTE
    mov ah, cl
    call rtc_write_reg
    mov al, RTC_SECOND
    mov ah, dh
    call rtc_write_reg
    pop ax
    xor al, al
    iret

.get_date:
    ; AH=04: Get date
    ; Returns DH=month, DL=day, CX=year, BH=century
    mov al, RTC_YEAR
    call rtc_read_reg
    mov cl, al              ; Year (BCD)
    
    mov al, RTC_MONTH
    call rtc_read_reg
    mov dh, al              ; Month (BCD)
    
    mov al, RTC_DAY
    call rtc_read_reg
    mov dl, al              ; Day (BCD)
    
    mov al, RTC_CENTURY
    call rtc_read_reg
    mov ch, al              ; Century (BCD)
    
    ; CX now has Century (CH) and Year (CL) in BCD
    mov ah, 0x00
    clc
    iret

.set_date:
    ; AH=03: Set date
    ; DH=month, DL=day, CX=year
    ; Not fully implemented - would need BCD conversion
    xor al, al
    iret

.get_date_century:
    ; AH=04: Get century
    mov al, RTC_CENTURY
    call rtc_read_reg
    mov ch, al
    xor al, al
    iret

.set_date_century:
    ; AH=05: Set century
    mov al, RTC_CENTURY
    mov ah, ch
    call rtc_write_reg
    xor al, al
    iret

.get_alarm:
    ; AH=06: Get alarm time
    mov al, RTC_HOUR
    call rtc_read_reg
    mov ch, al
    mov al, RTC_MINUTE
    call rtc_read_reg
    mov cl, al
    mov al, RTC_SECOND
    call rtc_read_reg
    mov dh, al
    xor al, al
    iret

.set_alarm:
    ; AH=07: Set alarm time
    ; CH=hour, CL=minute, DH=second
    push ax
    mov al, RTC_HOUR
    mov ah, ch
    call rtc_write_reg
    mov al, RTC_MINUTE
    mov ah, cl
    call rtc_write_reg
    mov al, RTC_SECOND
    mov ah, dh
    call rtc_write_reg
    ; Enable alarm interrupt
    mov al, RTC_REG_B
    out RTC_PORT_INDEX, al
    in al, RTC_PORT_DATA
    or al, 0x20            ; Alarm interrupt enable
    out RTC_PORT_DATA, al
    pop ax
    xor al, al
    iret

; ── CMOS Read (direct access) ───────────────────
cmos_read:
    ; AL = index, returns AL = data
    out RTC_PORT_INDEX, al
    in al, RTC_PORT_DATA
    ret

; ── CMOS Write ───────────────────────────────────
cmos_write:
    ; AL = index, AH = data
    out RTC_PORT_INDEX, al
    mov al, ah
    out RTC_PORT_DATA, al
    ret

; ── Get CMOS battery status ─────────────────────
cmos_get_battery:
    mov al, RTC_REG_D
    call rtc_read_reg
    test al, 0x80
    jz .battery_ok
    ; Battery dead
    mov al, 0x01
    ret
.battery_ok:
    mov al, 0x00
    ret
