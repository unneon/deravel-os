ENTRY(boot)

SECTIONS {
    . = 0x80200000;
    kernel_base = .;

    .text :{
        KEEP(*(.text.boot));
        *(.text .text.*);
    }

    .rodata : ALIGN(4) {
        *(.rodata .rodata.*);
    }

    .data : ALIGN(4) {
        *(.data .data.*);
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
}
