#![feature(decl_macro, never_type)]
#![no_std]

use core::arch::asm;

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

macro syscalls($(#[no = $no:literal] pub fn $name:ident($($a0name:ident: $a0type:ty)?) $(-> $return_type:ty)?;)*) {
    $(pub fn $name($($a0name: $a0type)?) $(-> $return_type)? {
        let _result: u64;
        unsafe {
            asm!(
                "ecall",
                $(in("a0") $a0name,)?
                in("a3") $no,
                lateout("a0") _result,
            );
            $(core::mem::transmute_copy::<u64, $return_type>(&_result))?
        }
    })*
}

syscalls! {
    #[no = 1]
    pub fn exit() -> !;

    #[no = 2]
    pub fn putchar(ch: u8);
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
