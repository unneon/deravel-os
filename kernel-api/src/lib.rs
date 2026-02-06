#![no_std]

use core::arch::naked_asm;

unsafe extern "C" {
    static mut __deravel_stack_top: u8;
}

pub fn exit() -> ! {
    loop {}
}

pub fn putchar(_ch: u8) {}

#[unsafe(link_section = ".text.entry")]
#[unsafe(naked)]
#[unsafe(no_mangle)]
unsafe extern "C" fn __deravel_entry() -> ! {
    naked_asm!(
        "la sp, {stack_top}",
        "call main",
        "call {exit}",
        stack_top = sym __deravel_stack_top,
        exit = sym exit,
    )
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
