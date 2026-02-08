#![feature(abi_riscv_interrupt)]
#![feature(arbitrary_self_types)]
#![feature(decl_macro)]
#![feature(iter_array_chunks)]
#![feature(never_type)]
#![feature(slice_from_ptr_range)]
#![allow(static_mut_refs)]
#![no_std]
#![no_main]

extern crate alloc;

mod arch;
mod elf;
mod heap;
mod log;
mod page;
mod process;
mod sbi;
mod virtio;

use crate::arch::{
    RiscvRegisters, initialize_trap_handler, switch_to_userspace_full,
    switch_to_userspace_registers_only,
};
use crate::elf::const_elf;
use crate::heap::log_heap_statistics;
use crate::log::initialize_log;
use crate::page::{PageTable, map_pages};
use crate::process::{
    CURRENT_PROC, PROCESSES, ProcessState, create_process, find_runnable_process,
};
use crate::sbi::{ResetReason, ResetType, log_sbi_metadata};
use crate::virtio::initialize_all_virtio_mmio;
use ::log::{error, info};
use core::panic::PanicInfo;
use fdt::Fdt;
use riscv::interrupt::Trap;
use riscv::interrupt::supervisor::{Exception, Interrupt};

const_elf!(HELLO_ELF "CARGO_BIN_FILE_DERAVEL_HELLO");
const_elf!(IPC_A_ELF "CARGO_BIN_FILE_DERAVEL_IPC_A");
const_elf!(IPC_B_ELF "CARGO_BIN_FILE_DERAVEL_IPC_B");
const_elf!(IPC_C_ELF "CARGO_BIN_FILE_DERAVEL_IPC_C");
const_elf!(SHELL_ELF "CARGO_BIN_FILE_DERAVEL_SHELL");

fn main(_hart_id: u64, device_tree: *const u8) -> ! {
    clear_bss();

    let device_tree = unsafe { Fdt::from_ptr(device_tree) }.unwrap();
    initialize_log(&device_tree);
    initialize_trap_handler();
    log_sbi_metadata();
    initialize_all_virtio_mmio(&device_tree);

    // create_process(&HELLO_ELF.0);
    create_process(&IPC_A_ELF.0);
    create_process(&IPC_B_ELF.0);
    create_process(&IPC_C_ELF.0);
    // create_process(&SHELL_ELF.0);

    schedule_and_switch_to_userspace();
}

fn clear_bss() {
    unsafe extern "C" {
        static mut bss_start: u8;
        static mut bss_end: u8;
    }
    let bss = unsafe { core::slice::from_mut_ptr_range(&raw mut bss_start..&raw mut bss_end) };
    bss.fill(0);
}

fn schedule_and_switch_to_userspace() -> ! {
    let Some(next_pid) = find_runnable_process() else {
        info!("shutting down due to all processes finishing");
        log_heap_statistics();
        sbi::system_reset(ResetType::Shutdown, ResetReason::NoReason).unwrap()
    };
    let next = unsafe { &PROCESSES[next_pid] };
    unsafe { CURRENT_PROC = Some(next_pid) }

    switch_to_userspace_full(next);
}

fn handle_trap(registers: &mut RiscvRegisters) -> ! {
    let scause = riscv::register::scause::read()
        .cause()
        .try_into::<Interrupt, Exception>()
        .unwrap();
    let stval = riscv::register::stval::read();
    let user_pc = riscv::register::sepc::read();
    if scause == Trap::Exception(Exception::UserEnvCall) {
        handle_syscall(user_pc, registers);
    } else {
        panic!("unexpected trap scause={scause:?} stval={stval:#x} user_pc={user_pc:#x}");
    }
}

fn handle_syscall(user_pc: usize, registers: &mut RiscvRegisters) -> ! {
    match registers.a3 {
        1 => {
            unsafe { PROCESSES[CURRENT_PROC.unwrap()].state = ProcessState::Finished }
            schedule_and_switch_to_userspace();
        }
        2 => {
            let ch = registers.a0 as u8;
            sbi::debug_console_write_byte(ch).unwrap();
        }
        3 => {
            let mut c = [0];
            while sbi::debug_console_read(&mut c).unwrap() == 0 {}
            registers.a0 = c[0] as usize;
        }
        4 => {
            let process = unsafe { &mut PROCESSES[CURRENT_PROC.unwrap()] };
            process.registers = registers.clone();
            process.pc = user_pc + 4;
            schedule_and_switch_to_userspace();
        }
        _ => panic!("invalid syscall number {}", registers.a3),
    }

    unsafe { riscv::register::sepc::write(user_pc + 4) };
    switch_to_userspace_registers_only(registers);
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let location = info.location().unwrap();
    let message = info.message();
    error!("panicked at {location}: {message}");
    let _ = sbi::system_reset(ResetType::Shutdown, ResetReason::SystemFailure);
    loop {}
}
