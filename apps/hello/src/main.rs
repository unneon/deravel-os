#![no_std]
#![no_main]

use core::arch::asm;

fn main() {
    unsafe { asm!("ecall", in("a0") 12, in("a1") 34, in("a2") 56, in("a3") 78) };
}

deravel_kernel_api::app! { main }
