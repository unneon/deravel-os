#![feature(abi_riscv_interrupt)]
#![feature(arbitrary_self_types)]
#![feature(decl_macro)]
#![feature(iter_array_chunks)]
#![feature(never_type)]
#![feature(slice_from_ptr_range)]
#![allow(static_mut_refs)]
#![allow(clippy::type_complexity)]
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
    CAPABILITY_PAGES, CURRENT_PROC, PROCESSES, ProcessState, reserve_process,
    schedule_and_switch_to_userspace,
};
use crate::sbi::{ResetReason, ResetType, log_sbi_metadata};
use crate::virtio::initialize_all_virtio_mmio;
use crate::virtio::virtio_blk::VirtioBlk;
use ::log::{Level, error};
use alloc::borrow::ToOwned;
use alloc::vec;
use core::panic::PanicInfo;
use deravel_types::*;
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

    let fs_tar = reserve_process!(TarFs, "CARGO_BIN_FILE_DERAVEL_FILESYSTEM_TAR");
    let ipc_a = reserve_process!(IpcA, "CARGO_BIN_FILE_DERAVEL_APPS_ipc-a");
    let ipc_b = reserve_process!(IpcB, "CARGO_BIN_FILE_DERAVEL_APPS_ipc-b");
    let ipc_c = reserve_process!(IpcC, "CARGO_BIN_FILE_DERAVEL_APPS_ipc-c");
    // let hello = reserve_process!(Hello, "CARGO_BIN_FILE_DERAVEL_APPS_hello");
    // let shell = reserve_process!(Shell, "CARGO_BIN_FILE_DERAVEL_APPS_shell");
    ipc_a.spawn(IpcAArgs {
        fs: fs_tar.export,
        b: ipc_b.export,
    });
    ipc_b.spawn(IpcBArgs { c: ipc_c.export });
    ipc_c.spawn(IpcCArgs {});
    fs_tar.spawn(TarFsArgs {});
    // hello.spawn(HelloArgs {});
    // shell.spawn(ShellArgs {});

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
    match registers.a6 {
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
        // 6 => {
        //     let capability = Capability(registers.a0 as *const CapabilityCertificate);
        //     let method = registers.a1;
        //     let args_ptr = registers.a2 as *const u8;
        //     let args_len = registers.a3;
        //     let args = unsafe { core::slice::from_raw_parts(args_ptr, args_len) };
        //     let args = core::str::from_utf8(args).unwrap().to_owned();
        //     let caller_name = unsafe { PROCESSES[CURRENT_PROC.unwrap()].name.unwrap() };
        //     debug!("process {caller_name} invoked ipc {capability:?}@{method} {args}");
        //     // let data = registers.a0 as *const u8;
        //     // let data_len = registers.a1;
        //     // let dest_pid = registers.a2;
        //     // let message = unsafe { core::slice::from_raw_parts(data, data_len) };
        //     // let dest_proc = unsafe { &mut PROCESSES[dest_pid] };
        //     // let dest_queue = dest_proc.messages.get_or_insert_default();
        //     // dest_queue.push_back((message.into(), unsafe { CURRENT_PROC.unwrap() }));
        // }
        // 7 => {
        //     if let Some((message, sender_pid)) = unsafe {
        //         PROCESSES[CURRENT_PROC.unwrap()]
        //             .messages
        //             .as_mut()
        //             .and_then(|q| q.pop_front())
        //     } {
        //         let buf = registers.a0 as *mut u8;
        //         let buf_max_len = registers.a1;
        //         assert!(message.len() <= buf_max_len);
        //         let buf = unsafe { core::slice::from_raw_parts_mut(buf, message.len()) };
        //         buf.copy_from_slice(&message);
        //         registers.a0 = message.len();
        //         registers.a1 = sender_pid;
        //     } else {
        //         let proc = unsafe { &mut PROCESSES[CURRENT_PROC.unwrap()] };
        //         proc.state = ProcessState::WaitingForMessage;
        //         proc.registers = registers.clone();
        //         proc.pc = user_pc;
        //
        //         schedule_and_switch_to_userspace();
        //     }
        // }
        8 => {
            let text = registers.a0 as *const u8;
            let text_len = registers.a1;
            assert!(text_len <= 1024);
            let level = registers.a2;
            let text = unsafe { core::slice::from_raw_parts(text, text_len) };
            let mut text_stack = [0; 1024];
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
        10 => {
            let sector = registers.a0 as u64;
            let buf = unsafe { *(registers.a1 as *const [u8; 512]) };

            let satp = riscv::register::satp::read();
            unsafe { riscv::register::satp::set(Mode::Bare, 0, 0) }
            unsafe { DISK.as_mut().unwrap().write(sector, &buf).unwrap() }
            unsafe { riscv::register::satp::write(satp) }
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
        13 => {
            if unsafe { PROCESSES[CURRENT_PROC.unwrap()].state } == ProcessState::WaitingForReply {
                let result = unsafe { PROCESSES[CURRENT_PROC.unwrap()].reply.take().unwrap() };
                let buf_ptr = registers.a4 as *mut u8;
                let buf_len = registers.a5;
                assert!(result.len() <= buf_len);
                let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr, result.len()) };
                buf.copy_from_slice(result.as_bytes());
                registers.a0 = result.len();
                unsafe { PROCESSES[CURRENT_PROC.unwrap()].state = ProcessState::Runnable };
            } else {
                let farthest_cap = RawCapability(registers.a0 as *const CapabilityCertificate);
                let method = registers.a1;
                let args_ptr = registers.a2 as *const u8;
                let args_len = registers.a3;
                let args = unsafe { core::slice::from_raw_parts(args_ptr, args_len) };
                let args = core::str::from_utf8(args).unwrap().to_owned();
                let proc = unsafe { &mut PROCESSES[CURRENT_PROC.unwrap()] };
                proc.state = ProcessState::WaitingForReply;
                proc.registers = registers.clone();
                proc.pc = user_pc;

                let mut capability = farthest_cap;
                let mut sender = unsafe { CURRENT_PROC.unwrap() };
                let original = loop {
                    assert!(capability.is_pointer_valid());
                    let certifier = capability.certifier();
                    let certificate =
                        unsafe { CAPABILITY_PAGES[certifier.0].0[capability.local_index()] };
                    match certificate.unpack() {
                        CapabilityCertificateUnpacked::Granted { grantee } => {
                            assert!(grantee.0 == sender);
                            break capability;
                        }
                        CapabilityCertificateUnpacked::Forwarded { forwardee, inner } => {
                            assert!(forwardee.0 == sender);
                            capability = inner;
                            sender = certifier.0;
                        }
                    }
                };
                let dest = original.certifier();
                let dest = unsafe { &mut PROCESSES[dest.0] };
                dest.messages
                    .get_or_insert_default()
                    .push_back((original, method, args, unsafe {
                        ProcessId(CURRENT_PROC.unwrap())
                    }));

                schedule_and_switch_to_userspace();
            }
        }
        14 => {
            assert!(unsafe { PROCESSES[CURRENT_PROC.unwrap()].currently_serving.is_none() });
            if let Some((cap, method, args, sender)) = unsafe {
                PROCESSES[CURRENT_PROC.unwrap()]
                    .messages
                    .as_mut()
                    .and_then(|q| q.pop_front())
            } {
                let buf = registers.a0 as *mut u8;
                let buf_max_len = registers.a1;
                assert!(args.len() <= buf_max_len);
                let buf = unsafe { core::slice::from_raw_parts_mut(buf, args.len()) };
                buf.copy_from_slice(args.as_bytes());
                registers.a0 = cap.0 as usize;
                registers.a1 = method;
                registers.a2 = args.len();
                registers.a3 = sender.0;
                unsafe { PROCESSES[CURRENT_PROC.unwrap()].currently_serving = Some(sender) };
            } else {
                let proc = unsafe { &mut PROCESSES[CURRENT_PROC.unwrap()] };
                proc.state = ProcessState::WaitingForMessage;
                proc.registers = registers.clone();
                proc.pc = user_pc;

                schedule_and_switch_to_userspace();
            }
        }
        15 => {
            let result_ptr = registers.a0;
            let result_len = registers.a1;
            let result =
                unsafe { core::slice::from_raw_parts(result_ptr as *const u8, result_len) };
            let result = str::from_utf8(result).unwrap().to_owned();
            let caller = unsafe {
                PROCESSES[CURRENT_PROC.unwrap()]
                    .currently_serving
                    .take()
                    .unwrap()
            };
            unsafe { PROCESSES[caller.0].reply = Some(result.into()) };
        }
        _ => panic!("invalid syscall number {}", registers.a6),
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
