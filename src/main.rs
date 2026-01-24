#![feature(abi_riscv_interrupt)]
#![feature(adt_const_params)]
#![feature(arbitrary_self_types)]
#![feature(decl_macro)]
#![feature(never_type)]
#![feature(slice_from_ptr_range)]
#![allow(static_mut_refs)]
#![no_std]
#![no_main]

mod page;
mod sbi;
mod virtio;

use crate::sbi::{ResetReason, ResetType};
use core::arch::{asm, naked_asm};
use core::panic::PanicInfo;
use riscv::interrupt::supervisor::{Exception, Interrupt};
use riscv::register::stvec::{Stvec, TrapMode};
use virtio::virtio_blk::VirtioBlk;
use virtio::virtio_net::VirtioNet;

unsafe extern "C" {
    static mut bss_start: u8;
    static mut bss_end: u8;
    static mut stack_top: u8;
}

#[unsafe(link_section = ".text.boot")]
#[unsafe(naked)]
#[unsafe(no_mangle)]
unsafe extern "C" fn boot() -> ! {
    naked_asm!(
        "la sp, {stack_top}",
        "j {main}",
        stack_top = sym stack_top,
        main = sym main,
    )
}

fn main() -> ! {
    clear_bss();
    register_trap_handler();
    log_sbi_metadata();

    let mut virtio_blk = VirtioBlk::new(0x1000_1000);
    let mut buf = [0; 512];
    virtio_blk.read(0, &mut buf).unwrap();
    sbi::console_writeln!("read from disk: {:?}", str::from_utf8(&buf).unwrap());

    let mut virtio_net = VirtioNet::new(0x1000_2000);
    virtio_net.arp_handshake();

    sbi::system_reset(ResetType::Shutdown, ResetReason::NoReason).unwrap()
}

fn clear_bss() {
    let bss = unsafe { core::slice::from_mut_ptr_range(&raw mut bss_start..&raw mut bss_end) };
    sbi::console_writeln!("bss section is {:#x} bytes long", bss.len());
    bss.fill(0);
}

fn register_trap_handler() {
    let address = trap_handler as *const () as usize;
    unsafe { riscv::register::stvec::write(Stvec::new(address, TrapMode::Direct)) }
}

fn log_sbi_metadata() {
    let spec_version = sbi::get_spec_version();
    sbi::console_writeln!("SBI specification version: {spec_version}");
    let impl_id = sbi::get_impl_id();
    let impl_version = sbi::get_impl_version();
    sbi::console_writeln!("SBI implementation: {impl_id}, version {impl_version}");
}

#[unsafe(no_mangle)]
unsafe extern "riscv-interrupt-s" fn trap_handler() {
    unsafe { asm!(".align 4") }
    let scause = riscv::register::scause::read()
        .cause()
        .try_into::<Interrupt, Exception>()
        .unwrap();
    let stval = riscv::register::stval::read();
    let user_pc = riscv::register::sepc::read();
    panic!("unexpected trap scause={scause:?} stval={stval:#x} user_pc={user_pc:#x}");
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    sbi::console_writeln!("{}", info);
    let _ = sbi::system_reset(ResetType::Shutdown, ResetReason::SystemFailure);
    loop {}
}
