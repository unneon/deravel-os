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

fn main(_hart_id: u64, device_tree: *const u8) -> ! {
    clear_bss();

    let device_tree = unsafe { Fdt::from_ptr(device_tree) }.unwrap();
    initialize_log(&device_tree);
    initialize_trap_handler();
    log_sbi_metadata();
    initialize_all_virtio_mmio(&device_tree);

    // create_process!("hello");
    create_process!("ipc-a");
    create_process!("ipc-b");
    create_process!("ipc-c");
    // create_process!("shell");

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
        5 => {
            // TODO: Handle user pointers safely.
            let name =
                unsafe { core::slice::from_raw_parts(registers.a0 as *const u8, registers.a1) };
            let name = core::str::from_utf8(name).unwrap();
            for (pid, haystack) in unsafe { PROCESSES.iter().enumerate() } {
                if let Some(haystack_name) = haystack.name
                    && name == haystack_name
                {
                    registers.a0 = pid;
                    break;
                }
            }
        }
        6 => {
            let data = registers.a0 as *const u8;
            let data_len = registers.a1;
            let dest_pid = registers.a2;
            let message = unsafe { core::slice::from_raw_parts(data, data_len) };
            unsafe { PROCESSES[dest_pid].message = Some((message.into(), CURRENT_PROC.unwrap())) }
        }
        7 => {
            let buf = registers.a0 as *mut u8;
            let buf_len = registers.a1;
            let (message, sender_pid) =
                unsafe { PROCESSES[CURRENT_PROC.unwrap()].message.take().unwrap() };
            assert_eq!(message.len(), buf_len);
            let buf = unsafe { core::slice::from_raw_parts_mut(buf, buf_len) };
            buf.copy_from_slice(&message);
            registers.a0 = sender_pid;
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
