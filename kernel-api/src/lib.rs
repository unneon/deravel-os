#![feature(decl_macro)]
#![no_std]

pub macro app($main:ident) {
    unsafe extern "C" {
        static mut __deravel_stack_top: u8;
    }

    #[unsafe(link_section = ".text.entry")]
    #[unsafe(naked)]
    #[unsafe(no_mangle)]
    unsafe extern "C" fn __deravel_entry() -> ! {
        core::arch::naked_asm!(
            "la sp, {stack_top}",
            "call {main}",
            "call {exit}",
            stack_top = sym __deravel_stack_top,
            main = sym $main,
            exit = sym exit,
        )
    }
}

pub fn exit() -> ! {
    loop {}
}

pub fn putchar(_ch: u8) {}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
