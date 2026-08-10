SECTIONS {
    . = 0x80200000;
    image_start = .;

    .text.start :{
        KEEP(*(.text.start));
    }

    . = image_start + 8192;
    .rela.dyn :{
        *(.rela.dyn .rela.dyn.*);
        . += 24;
    }

    .text :{
        *(.text .text.*);
    }

    .rodata :{
        *(.rodata .rodata.*);
    }

    .data :{
        *(.data .data.*);
    }

    .bss : ALIGN(8) {
        bss_start = .;
        *(.bss .bss.* .sbss .sbss.*);
        . = ALIGN(8);
        bss_end = .;
    }

    . = ALIGN(16);
    . += 512 * 1024;
    early_stack_top = .;

    early_heap_start = .;
    . += 2 * 1024 * 1024;
    early_heap_end = .;

    image_end = .;

    /DISCARD/ :{
        *(.dynsym .gnu.hash .hash .dynstr .eh_frame .dynamic);
    }
}
