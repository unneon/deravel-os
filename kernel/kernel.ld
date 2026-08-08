SECTIONS {
    . = 0x80200000;
    image_start = .;

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

    .got :{
        *(.got .got.*);
    }

    .data :{
        *(.data .data.*);
    }

    .bss :{
        bss_start = .;
        *(.bss .bss.* .sbss .sbss.*);
        bss_end = .;
    }

    . = ALIGN(16);
    . += 512 * 1024;
    early_stack_top = .;

    . = ALIGN(4096);
    early_heap_start = .;
    . += 2 * 1024 * 1024;
    early_heap_end = .;

    readwrite_end = .;
    image_end = .;
}
