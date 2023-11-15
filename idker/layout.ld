ENTRY(_start)
OUTPUT_FORMAT(elf64-x86-64)

KERNEL_OFFSET = 0xFFFF800000100000;

SECTIONS {
    . = KERNEL_OFFSET;

    . += SIZEOF_HEADERS;

    .text ALIGN(0x1000) : AT(ADDR(.text) - KERNEL_OFFSET) {
        __text_start = .;
        *(.text*)
        . = ALIGN(0x1000);
        __text_end = .;
    }

    .rodata ALIGN(0x1000) : AT(ADDR(.rodata) - KERNEL_OFFSET) {
        __rodata_start = .;
        *(.rodata*)
        . = ALIGN(0x1000);
        __rodata_end = .;
    }

    .data ALIGN(0x1000) : AT(ADDR(.data) - KERNEL_OFFSET) {
        __data_start = .;
        *(.data*)
        *(.got*)
        . = ALIGN(0x1000);
        __data_end = .;
        __bss_start = .;
        *(.bss*)
        . = ALIGN(0x1000);
        __bss_end = .;
    }

    .tdata ALIGN(0x1000) : AT(ADDR(.tdata) - KERNEL_OFFSET) {
        __tdata_start = .;
        *(.tdata*)
        . = ALIGN(0x1000);
        __tdata_end = .;
        __tbss_start = .;
        *(.tbss*)
        . += 8;
        . = ALIGN(0x1000);
        __tbss_end = .;
    }

    __end = .;

    /DISCARD/ : {
        *(.comment*)
        *(.eh_frame*)
        *(.gcc_except_table*)
        *(.note*)
        *(.rel.eh_frame*)
    }
}
