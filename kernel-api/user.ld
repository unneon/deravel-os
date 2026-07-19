SECTIONS {
    . = 0x1000000;

    .text : ALIGN(4096) {
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

        ASSERT(. <= 0x2000000, "user application too large");
    }
}
