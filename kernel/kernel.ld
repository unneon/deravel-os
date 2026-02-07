ENTRY(boot)

SECTIONS {
    . = 0x80200000;

    .text :{
        text_start = .;
        KEEP(*(.text.boot));
        *(.text .text.*);
        text_end = .;
    }

    .rodata : ALIGN(4096) {
        rodata_start = .;
        *(.rodata .rodata.*);
        rodata_end = .;
    }

    . = ALIGN(4096);
    readwrite_start = .;

    .data :{
        data_start = .;
        *(.data .data.*);
        data_end = .;
    }

    .bss : ALIGN(4) {
        bss_start = .;
        *(.bss .bss.* .sbss .sbss.*);
        bss_end = .;
    }

    . = ALIGN(4);
    . += 128 * 1024;
    stack_top = .;

    . = ALIGN(4096);
    heap_start = .;
    . += 64 * 1024 * 1024;
    heap_end = .;

    readwrite_end = .;
}
