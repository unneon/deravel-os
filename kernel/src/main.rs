#![feature(abi_riscv_interrupt)]
#![feature(arbitrary_self_types)]
#![feature(atomic_ptr_null)]
#![feature(decl_macro)]
#![feature(iter_array_chunks)]
#![feature(iter_intersperse)]
#![feature(never_type)]
#![feature(ptr_metadata)]
#![feature(slice_from_ptr_range)]
#![feature(unsafe_cell_access)]
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
mod sync;
mod uart;
mod util;
mod virtio;

use crate::arch::{RiscvRegisters, initialize_trap_handler, switch_to_userspace_registers_only};
use crate::capability::{CAPABILITY_PAGES, reserve_kernel_capability};
use crate::elf::elf;
use crate::interrupt::INTERRUPTS;
use crate::log::{initialize_log, log_userspace};
use crate::page::{PageFlags, PageTable, map_pages};
use crate::pci::initialize_all_pci;
use crate::plic::{initialize_plic, plic_claim, plic_complete};
use crate::process::{
    PROCESS_COUNT, PROCESSES, ProcessState, reserve_process, schedule_and_switch_to_userspace,
};
use crate::sbi::{ResetReason, ResetType, log_sbi_metadata};
use ::log::{Level, error};
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::panic::PanicInfo;
use core::sync::atomic::Ordering;
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
    let (_virtio_blk, virtio_gpu) = initialize_all_pci(&device_tree);
    initialize_plic(&device_tree);
    initialize_hart_stack();
    enable_interrupts();

    // let fs_tar = reserve_process::<TarFs>(elf!("CARGO_BIN_FILE_DERAVEL_FILESYSTEM_TAR"));
    // let ipc_a = reserve_process::<IpcA>(elf!("CARGO_BIN_FILE_DERAVEL_APPS_ipc-a"));
    // let ipc_b = reserve_process::<IpcB>(elf!("CARGO_BIN_FILE_DERAVEL_APPS_ipc-b"));
    // let ipc_c = reserve_process::<IpcC>(elf!("CARGO_BIN_FILE_DERAVEL_APPS_ipc-c"));
    // let hello = reserve_process::<Hello>(elf!("CARGO_BIN_FILE_DERAVEL_APPS_hello"));
    // let shell = reserve_process::<Shell>(elf!("CARGO_BIN_FILE_DERAVEL_APPS_shell"));
    let windowing = reserve_process::<Windowing>(elf!("CARGO_BIN_FILE_DERAVEL_APPS_windowing"));

    // ipc_a.spawn(IpcAArgs {
    //     fs: fs_tar.export,
    //     b: ipc_b.export,
    // });
    // ipc_b.spawn(IpcBArgs { c: ipc_c.export });
    // ipc_c.spawn(IpcCArgs {});
    // fs_tar.spawn(TarFsArgs {
    //     drive: reserve_kernel_capability(virtio_blk),
    // });
    // hello.spawn(HelloArgs {
    //     console: reserve_kernel_capability(&SbiConsole),
    // });
    // shell.spawn(ShellArgs {
    //     console: reserve_kernel_capability(&SbiConsole),
    // });
    windowing.spawn(WindowingArgs {
        display: reserve_kernel_capability(virtio_gpu),
    });

    // TODO: initialize_hart_stack should take a callback and pass this with the correct lifetime.
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
        for ie in &INTERRUPTS {
            let ie = ie.lock();
            if let Some(ie) = *ie
                && ie.plic_number == irq
            {
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

fn validate_untrusted_capability(farthest_cap: RawCapability, current_pid: usize) -> RawCapability {
    let mut capability = farthest_cap;
    let mut sender = Actor::Userspace(ProcessId::new(current_pid));
    loop {
        let certifier = capability.certifier();
        let certificate = &CAPABILITY_PAGES[match certifier {
            Actor::Userspace(pid) => pid.as_usize(),
            Actor::Kernel => PROCESS_COUNT,
        }]
        .0[capability.local_index()];
        match certificate.load(Ordering::Relaxed).unpack() {
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
    }
}

fn handle_syscall(user_pc: usize, registers: &mut RiscvRegisters, hart: &mut HartContext) -> ! {
    let current_pid = hart.current_pid.unwrap().as_usize();
    let mut current_proc = PROCESSES[current_pid].lock();
    ::log::trace!("received syscall {}", registers.a6);
    match registers.a6 {
        0 => {
            current_proc.state = ProcessState::Finished;
            drop(current_proc);
            schedule_and_switch_to_userspace(hart);
        }
        1 => {
            if current_proc.state == ProcessState::WaitingForReply {
                let reply = current_proc.reply.take().unwrap();
                copy_to_user(&reply, registers.a4 as *mut u8, registers.a5);
                registers.a0 = reply.len();
                current_proc.state = ProcessState::Runnable;
            } else {
                let farthest_cap =
                    RawCapability::from_pointer(registers.a0 as *mut CapabilityCertificate);
                let method = registers.a1;
                let args = copy_from_user(registers.a2 as *const u8, registers.a3);
                current_proc.state = ProcessState::WaitingForReply;
                current_proc.registers = registers.clone();
                current_proc.pc = user_pc;

                let original = validate_untrusted_capability(farthest_cap, current_pid);
                match original.certifier() {
                    Actor::Userspace(dest) => {
                        let mut dest = PROCESSES[dest.as_usize()].lock();
                        dest.messages.get_or_insert_default().push_back((
                            original,
                            method,
                            args,
                            ProcessId::new(current_pid),
                        ));

                        drop(current_proc);
                        drop(dest);
                        schedule_and_switch_to_userspace(hart);
                    }
                    Actor::Kernel => {
                        let handler = capability::get_handler(original.local_index());
                        let result = handler.handle(method, &args);
                        copy_to_user(&result, registers.a4 as *mut u8, registers.a5);
                        registers.a0 = result.len();
                        current_proc.state = ProcessState::Runnable;
                    }
                }
            }
        }
        2 => {
            assert!(current_proc.currently_serving.is_none());
            if let Some((cap, method, args, sender)) =
                current_proc.messages.as_mut().and_then(|q| q.pop_front())
            {
                copy_to_user(&args, registers.a0 as *mut u8, registers.a1);
                registers.a0 = cap.as_usize();
                registers.a1 = method;
                registers.a2 = args.len();
                registers.a3 = sender.as_usize();
                current_proc.currently_serving = Some(sender);
            } else {
                current_proc.state = ProcessState::WaitingForMessage;
                current_proc.registers = registers.clone();
                current_proc.pc = user_pc;

                drop(current_proc);
                schedule_and_switch_to_userspace(hart);
            }
        }
        3 => {
            let result = copy_from_user(registers.a0 as *const u8, registers.a1);
            let caller = current_proc.currently_serving.take().unwrap();
            PROCESSES[caller.as_usize()].lock().reply = Some(result.into());
        }
        4 => {
            let page_count = registers.a0;
            let pages = vec![[0; PAGE_SIZE]; page_count];
            let pages_allocated = current_proc.heap_pages_allocated;
            let page_table = unsafe { &mut *(current_proc.page_table as *mut PageTable) };
            let virtual_addr = 0x4000000 + pages_allocated * PAGE_SIZE;
            ::log::trace!(
                "alloc from {virtual_addr:#x} to {:#x}",
                virtual_addr + PAGE_SIZE * page_count
            );
            map_pages(
                page_table,
                virtual_addr,
                pages.as_ptr() as usize,
                PageFlags::readwrite().user(),
                page_count,
            );
            current_proc.heap_pages_allocated += page_count;
            registers.a0 = virtual_addr;
            ::log::trace!("finished syscall 4");
        }
        5 => {
            let farthest_cap =
                RawCapability::from_pointer(registers.a0 as *mut CapabilityCertificate);
            let original = validate_untrusted_capability(farthest_cap, current_pid);
            assert_eq!(
                original.certifier(),
                Actor::Kernel,
                "shared memory capabilities can only be created by the kernel"
            );
            todo!()
            // let handler = capability::get_handler(original.local_index());
            // let result = handler.handle(method, &args);
            // copy_to_user(&result, registers.a4 as *mut u8, registers.a5);
            // registers.a0 = result.len();
            // current_proc.state = ProcessState::Runnable;
        }
        6 => {
            let text = copy_from_user(registers.a0 as *const u8, registers.a1);
            let text = String::from_utf8(text).unwrap();
            let level = registers.a2;
            let level = match level {
                0 => Level::Error,
                1 => Level::Warn,
                2 => Level::Info,
                3 => Level::Debug,
                4 => Level::Trace,
                _ => panic!("invalid log level {level}"),
            };
            log_userspace(level, current_proc.name.unwrap(), &text);
        }
        _ => panic!("invalid syscall number {}", registers.a6),
    }

    drop(current_proc);
    unsafe { riscv::register::sepc::write(user_pc + 4) };
    switch_to_userspace_registers_only(registers);
}

fn copy_to_user(bytes: &[u8], user_ptr: *mut u8, user_max_len: usize) {
    assert!(bytes.len() <= user_max_len);
    unsafe { core::slice::from_raw_parts_mut(user_ptr, bytes.len()).copy_from_slice(bytes) }
}

fn copy_from_user(user_ptr: *const u8, user_len: usize) -> Vec<u8> {
    let mut bytes = vec![0; user_len];
    bytes.copy_from_slice(unsafe { core::slice::from_raw_parts(user_ptr, user_len) });
    bytes
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let location = info.location().unwrap();
    let message = info.message();
    error!("panicked at {location}: {message}");
    let _ = sbi::system_reset(ResetType::Shutdown, ResetReason::SystemFailure);
    loop {}
}
