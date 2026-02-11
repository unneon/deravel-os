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

use crate::arch::{RiscvRegisters, initialize_trap_handler, switch_to_userspace_registers_only};
use crate::log::{initialize_log, log_userspace};
use crate::page::{PAGE_SIZE, PageFlags, PageTable, map_pages};
use crate::process::{
    CURRENT_PROC, PROCESSES, ProcessState, create_process, schedule_and_switch_to_userspace,
};
use crate::sbi::{ResetReason, ResetType, log_sbi_metadata};
use crate::virtio::initialize_all_virtio_mmio;
use crate::virtio::virtio_blk::VirtioBlk;
use ::log::{Level, error};
use alloc::vec;
use core::panic::PanicInfo;
use fdt::Fdt;
use riscv::interrupt::Trap;
use riscv::interrupt::supervisor::{Exception, Interrupt};
use riscv::register::satp::Mode;

static mut DISK: Option<VirtioBlk> = None;

fn main(_hart_id: u64, device_tree: *const u8) -> ! {
    clear_bss();

    let device_tree = unsafe { Fdt::from_ptr(device_tree) }.unwrap();
    initialize_log(&device_tree);
    initialize_trap_handler();
    log_sbi_metadata();
    initialize_all_virtio_mmio(&device_tree);

    create_process!("tar-fs");
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
        8 => {
            let text = registers.a0 as *const u8;
            let text_len = registers.a1;
            assert!(text_len <= 128);
            let level = registers.a2;
            let text = unsafe { core::slice::from_raw_parts(text, text_len) };
            let mut text_stack = [0; 128];
            text_stack[..text_len].copy_from_slice(text);
            let text = unsafe { core::str::from_utf8_unchecked(&text_stack[..text_len]) };
            let level = match level {
                0 => Level::Error,
                1 => Level::Warn,
                2 => Level::Info,
                3 => Level::Debug,
                4 => Level::Trace,
                _ => panic!("invalid log level {level}"),
            };
            log_userspace(
                level,
                unsafe { PROCESSES[CURRENT_PROC.unwrap()].name.unwrap() },
                text,
            );
        }
        9 => {
            let sector = registers.a0 as u64;
            let mut buf = [0; 512];

            let satp = riscv::register::satp::read();
            unsafe { riscv::register::satp::set(Mode::Bare, 0, 0) }
            unsafe { DISK.as_mut().unwrap().read(sector, &mut buf).unwrap() }
            unsafe { riscv::register::satp::write(satp) }

            let user_buf_ptr = unsafe { &mut *(registers.a1 as *mut [u8; 512]) };
            user_buf_ptr.copy_from_slice(&buf);
        }
        11 => {
            let satp = riscv::register::satp::read();
            unsafe { riscv::register::satp::set(Mode::Bare, 0, 0) }
            registers.a0 = unsafe { DISK.as_mut().unwrap().capacity() };
            unsafe { riscv::register::satp::write(satp) }
        }
        12 => {
            let page_count = registers.a0;
            let pages = vec![[0; PAGE_SIZE]; page_count];
            let pages_allocated = unsafe { PROCESSES[CURRENT_PROC.unwrap()].heap_pages_allocated };
            let page_table =
                unsafe { &mut *(PROCESSES[CURRENT_PROC.unwrap()].page_table as *mut PageTable) };
            let virtual_addr = 0x1800000 + pages_allocated * PAGE_SIZE;
            map_pages(
                page_table,
                virtual_addr,
                pages.as_ptr() as usize,
                PageFlags::readwrite().user(),
                page_count,
            );
            unsafe { PROCESSES[CURRENT_PROC.unwrap()].heap_pages_allocated += page_count }
            registers.a0 = virtual_addr;
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
