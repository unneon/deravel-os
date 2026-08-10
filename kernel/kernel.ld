SECTIONS {
    . = 0x80200000;
    image_start = .;

    .text.start :{
        KEEP(*(.text.start));
        ASSERT(. <= image_start + 4096, "_start too large");
    }

    . = image_start + 4096;
    .initial_page_table :{
        FILL(0);
        . += 4096;
    }

    .rela.dyn :{
        *(.rela.dyn .rela.dyn.*);
        FILL(0);
        . += 24;
    }

    .text : ALIGN(4096) {
        text_start = .;
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
