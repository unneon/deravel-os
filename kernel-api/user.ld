ENTRY(__deravel_entry)

SECTIONS {
    . = 0x1000000;

    .text : ALIGN(4096) {
        *(.text.entry);
        *(.text .text.*);
    }

    .rodata : ALIGN(4096) {
        *(.rodata .rodata.*);
    }

    .data : ALIGN(4096) {
        *(.data .data.*);
    }

    .bss : ALIGN(4096) {
        *(.bss .bss.* .sbss .sbss.*);

        . = ALIGN(16);
        . += 64 * 1024;
        __deravel_stack_top = .;
    }
}
