#![feature(abi_riscv_interrupt)]
#![feature(arbitrary_self_types)]
#![feature(atomic_ptr_null)]
#![feature(decl_macro)]
#![feature(iter_array_chunks)]
#![feature(iter_intersperse)]
#![feature(never_type)]
#![feature(ptr_metadata)]
#![feature(slice_from_ptr_range)]
#![allow(static_mut_refs)]
#![allow(clippy::missing_safety_doc)]
#![allow(clippy::type_complexity)]
#![no_std]
#![no_main]

extern crate alloc;

mod allocators;
mod arch;
mod capability;
mod drvli;
mod elf;
mod heap;
mod interrupt;
mod log;
mod page;
mod pci;
mod plic;
mod process;
mod sbi;
mod uart;
mod util;
mod virtio;

use crate::arch::{RiscvRegisters, initialize_trap_handler, switch_to_userspace_registers_only};
use crate::capability::{CAPABILITY_PAGES, HANDLERS, reserve_kernel_capability};
use crate::elf::elf;
use crate::interrupt::INTERRUPTS;
use crate::log::{initialize_log, log_userspace};
use crate::page::{PageFlags, PageTable, map_pages};
use crate::pci::initialize_all_pci;
use crate::plic::{initialize_plic, plic_claim, plic_complete};
use crate::process::{
    PROCESS_COUNT, PROCESSES, ProcessState, reserve_process, schedule_and_switch_to_userspace,
};
use crate::sbi::{ResetReason, ResetType, SbiConsole, log_sbi_metadata};
use ::log::{Level, error};
use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::vec;
use core::panic::PanicInfo;
use deravel_types::*;
use fdt::Fdt;
use riscv::interrupt::Trap;
use riscv::interrupt::supervisor::{Exception, Interrupt};
use riscv::register::satp::Mode;

#[repr(align(16))]
struct HartContext {
    current_pid: Option<ProcessId>,
}

#[repr(C, align(4096))]
struct HartStack {
    data: [u8; STACK_SIZE - size_of::<HartContext>().next_multiple_of(16)],
    ctx: HartContext,
}

const STACK_SIZE: usize = 128 * 4096;

fn main(_hart_id: u64, device_tree: *const u8) -> ! {
    clear_bss();

    let device_tree = unsafe { Fdt::from_ptr(device_tree) }.unwrap();
    initialize_log(&device_tree);
    initialize_trap_handler();
    log_sbi_metadata();
    let virtio_blk = initialize_all_pci(&device_tree);
    initialize_plic(&device_tree);
    initialize_hart_stack();
    enable_interrupts();

    let fs_tar = reserve_process::<TarFs>(elf!("CARGO_BIN_FILE_DERAVEL_FILESYSTEM_TAR"));
    let ipc_a = reserve_process::<IpcA>(elf!("CARGO_BIN_FILE_DERAVEL_APPS_ipc-a"));
    let ipc_b = reserve_process::<IpcB>(elf!("CARGO_BIN_FILE_DERAVEL_APPS_ipc-b"));
    let ipc_c = reserve_process::<IpcC>(elf!("CARGO_BIN_FILE_DERAVEL_APPS_ipc-c"));
    let hello = reserve_process::<Hello>(elf!("CARGO_BIN_FILE_DERAVEL_APPS_hello"));
    let shell = reserve_process::<Shell>(elf!("CARGO_BIN_FILE_DERAVEL_APPS_shell"));

    ipc_a.spawn(IpcAArgs {
        fs: fs_tar.export,
        b: ipc_b.export,
    });
    ipc_b.spawn(IpcBArgs { c: ipc_c.export });
    ipc_c.spawn(IpcCArgs {});
    fs_tar.spawn(TarFsArgs {
        drive: reserve_kernel_capability(virtio_blk),
    });
    hello.spawn(HelloArgs {
        console: reserve_kernel_capability(&SbiConsole),
    });
    shell.spawn(ShellArgs {
        console: reserve_kernel_capability(&SbiConsole),
    });

    let hart = unsafe { &mut *(riscv::register::sscratch::read() as *mut HartContext) };
    schedule_and_switch_to_userspace(hart);
}

fn clear_bss() {
    unsafe extern "C" {
        static mut bss_start: u8;
        static mut bss_end: u8;
    }
    let bss = unsafe { core::slice::from_mut_ptr_range(&raw mut bss_start..&raw mut bss_end) };
    bss.fill(0);
}

fn initialize_hart_stack() {
    let stack = Box::leak(Box::new(HartStack {
        data: [0; _],
        ctx: HartContext { current_pid: None },
    }));
    unsafe { riscv::register::sscratch::write((&raw mut stack.ctx) as usize) }
}

fn enable_interrupts() {
    let mut sie = riscv::register::sie::read();
    sie.set_sext(true);
    sie.set_stimer(true);
    unsafe { riscv::register::sie::write(sie) }

    unsafe { riscv::register::sstatus::set_sie() }
}

fn handle_trap(registers: &mut RiscvRegisters, hart: &mut HartContext) -> ! {
    let scause = riscv::register::scause::read()
        .cause()
        .try_into::<Interrupt, Exception>()
        .unwrap();
    let stval = riscv::register::stval::read();
    let user_pc = riscv::register::sepc::read();
    if scause == Trap::Exception(Exception::UserEnvCall) {
        handle_syscall(user_pc, registers, hart);
    } else if scause == Trap::Interrupt(Interrupt::SupervisorTimer) {
        sbi::set_timer(u64::MAX);
        switch_to_userspace_registers_only(registers)
    } else if scause == Trap::Interrupt(Interrupt::SupervisorExternal) {
        let satp = riscv::register::satp::read();
        unsafe { riscv::register::satp::set(Mode::Bare, 0, 0) }
        let irq = plic_claim();
        for ie in unsafe { INTERRUPTS.iter().flatten() } {
            if ie.plic_number == irq {
                ie.handler.handle();
            }
        }
        plic_complete(irq);
        unsafe { riscv::register::satp::write(satp) }
        switch_to_userspace_registers_only(registers)
    } else {
        panic!("unexpected trap scause={scause:?} stval={stval:#x} user_pc={user_pc:#x}");
    }
}

fn handle_syscall(user_pc: usize, registers: &mut RiscvRegisters, hart: &mut HartContext) -> ! {
    let current_pid = hart.current_pid.unwrap().as_usize();
    match registers.a6 {
        0 => {
            unsafe { PROCESSES[current_pid].state = ProcessState::Finished }
            schedule_and_switch_to_userspace(hart);
        }
        1 => {
            if unsafe { PROCESSES[current_pid].state } == ProcessState::WaitingForReply {
                let result = unsafe { PROCESSES[current_pid].reply.take().unwrap() };
                let buf_ptr = registers.a4 as *mut u8;
                let buf_len = registers.a5;
                assert!(result.len() <= buf_len);
                let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr, result.len()) };
                buf.copy_from_slice(result.as_bytes());
                registers.a0 = result.len();
                unsafe { PROCESSES[current_pid].state = ProcessState::Runnable };
            } else {
                let farthest_cap =
                    RawCapability::from_pointer(registers.a0 as *mut CapabilityCertificate);
                let method = registers.a1;
                let args_ptr = registers.a2 as *const u8;
                let args_len = registers.a3;
                let args = unsafe { core::slice::from_raw_parts(args_ptr, args_len) };
                let args = core::str::from_utf8(args).unwrap().to_owned();
                let proc = unsafe { &mut PROCESSES[current_pid] };
                proc.state = ProcessState::WaitingForReply;
                proc.registers = registers.clone();
                proc.pc = user_pc;

                let mut capability = farthest_cap;
                let mut sender = Actor::Userspace(ProcessId::new(current_pid));
                let original = loop {
                    let certifier = capability.certifier();
                    let certificate = unsafe {
                        CAPABILITY_PAGES[match certifier {
                            Actor::Userspace(pid) => pid.as_usize(),
                            Actor::Kernel => PROCESS_COUNT,
                        }]
                        .0[capability.local_index()]
                    };
                    match certificate.unpack() {
                        CapabilityCertificateUnpacked::Granted { grantee } => {
                            assert!(grantee == sender);
                            break capability;
                        }
                        CapabilityCertificateUnpacked::Forwarded { forwardee, inner } => {
                            assert!(forwardee == sender);
                            capability = inner;
                            sender = certifier;
                        }
                    }
                };

                match original.certifier() {
                    Actor::Userspace(dest) => {
                        let dest = unsafe { &mut PROCESSES[dest.as_usize()] };
                        dest.messages.get_or_insert_default().push_back((
                            original,
                            method,
                            args,
                            ProcessId::new(current_pid),
                        ));

                        schedule_and_switch_to_userspace(hart);
                    }
                    Actor::Kernel => {
                        let local_index = original.local_index();
                        let handler = unsafe { HANDLERS[local_index].as_ref().unwrap() };
                        let result = handler.handle(method, args.as_bytes());
                        let buf_ptr = registers.a4 as *mut u8;
                        let buf_len = registers.a5;
                        assert!(result.len() <= buf_len);
                        let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr, result.len()) };
                        buf.copy_from_slice(&result);
                        registers.a0 = result.len();
                        unsafe { PROCESSES[current_pid].state = ProcessState::Runnable };
                    }
                }
            }
        }
        2 => {
            assert!(unsafe { PROCESSES[current_pid].currently_serving.is_none() });
            if let Some((cap, method, args, sender)) = unsafe {
                PROCESSES[current_pid]
                    .messages
                    .as_mut()
                    .and_then(|q| q.pop_front())
            } {
                let buf = registers.a0 as *mut u8;
                let buf_max_len = registers.a1;
                assert!(args.len() <= buf_max_len);
                let buf = unsafe { core::slice::from_raw_parts_mut(buf, args.len()) };
                buf.copy_from_slice(args.as_bytes());
                registers.a0 = cap.as_usize();
                registers.a1 = method;
                registers.a2 = args.len();
                registers.a3 = sender.as_usize();
                unsafe { PROCESSES[current_pid].currently_serving = Some(sender) };
            } else {
                let proc = unsafe { &mut PROCESSES[current_pid] };
                proc.state = ProcessState::WaitingForMessage;
                proc.registers = registers.clone();
                proc.pc = user_pc;

                schedule_and_switch_to_userspace(hart);
            }
        }
        3 => {
            let result_ptr = registers.a0;
            let result_len = registers.a1;
            let result =
                unsafe { core::slice::from_raw_parts(result_ptr as *const u8, result_len) };
            let result = str::from_utf8(result).unwrap().to_owned();
            let caller = unsafe { PROCESSES[current_pid].currently_serving.take().unwrap() };
            unsafe { PROCESSES[caller.as_usize()].reply = Some(result.into()) };
        }
        4 => {
            let page_count = registers.a0;
            let pages = vec![[0; PAGE_SIZE]; page_count];
            let pages_allocated = unsafe { PROCESSES[current_pid].heap_pages_allocated };
            let page_table = unsafe { &mut *(PROCESSES[current_pid].page_table as *mut PageTable) };
            let virtual_addr = 0x1800000 + pages_allocated * PAGE_SIZE;
            map_pages(
                page_table,
                virtual_addr,
                pages.as_ptr() as usize,
                PageFlags::readwrite().user(),
                page_count,
            );
            unsafe { PROCESSES[current_pid].heap_pages_allocated += page_count }
            registers.a0 = virtual_addr;
        }
        5 => {
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
            log_userspace(level, unsafe { PROCESSES[current_pid].name.unwrap() }, text);
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
