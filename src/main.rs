#![feature(decl_macro)]
#![feature(rustc_attrs)]
#![feature(slice_from_ptr_range)]
#![allow(internal_features)]
#![no_std]
#![no_main]

mod sbi;

use core::arch::{asm, naked_asm};
use core::panic::PanicInfo;

unsafe extern "C" {
    static mut bss_start: u8;
    static mut bss_end: u8;
    static mut stack_top: u8;
}

unsafe fn bss() -> &'static mut [u8] {
    unsafe { core::slice::from_mut_ptr_range(&raw mut bss_start..&raw mut bss_end) }
}

#[unsafe(link_section = ".text.boot")]
#[unsafe(naked)]
#[unsafe(no_mangle)]
unsafe extern "C" fn boot() -> ! {
    naked_asm!("
        la sp, {stack_top}
        j {main}
    ",
        stack_top = sym stack_top,
        main = sym main,
    )
}

fn main() -> ! {
    // TODO: Is zeroing bss here necessary? How does Rust handle bss and rodata initialization for this target?
    unsafe { bss() }.fill(0);

    sbi::console_writeln!("\n\nHello World!");

    let version = sbi::get_spec_version();
    let version_major = version.major();
    let version_minor = version.minor();
    let impl_id = sbi::get_impl_id();
    let impl_id_number = impl_id.number();
    let impl_name = impl_id.name().unwrap_or("<unknown>");
    let impl_version = sbi::get_impl_version();
    sbi::console_writeln!(
        "SBI version: {version_major}.{version_minor}
SBI implementation: {impl_name} [{impl_id_number}] v{impl_version}",
    );

    loop {
        unsafe { asm!("wfi") }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    sbi::console_writeln!("{}", info);
    loop {}
}
