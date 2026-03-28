ENTRY(_start)

SECTIONS {
    /* GRUB loads x86 kernels starting at the 1MB mark */
    . = 1M;

    .multiboot :
    {
        /* The multiboot header must be at the very front of the ELF */
        KEEP(*(.multiboot))
    }

    .text :
    {
        *(.text .text.*)
    }

    .rodata :
    {
        *(.rodata .rodata.*)
    }

    .data :
    {
        *(.data .data.*)
    }

    .bss :
    {
        *(COMMON)
        *(.bss .bss.*)
    }
}
