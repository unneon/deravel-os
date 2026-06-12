#![feature(arbitrary_self_types)]
#![feature(atomic_ptr_null)]
#![feature(decl_macro)]
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
mod device_tree;
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
mod shared_memory;
mod sync;
mod uart;
mod util;
mod virtio;

use crate::arch::{RiscvRegisters, initialize_trap_handler, switch_to_userspace_registers_only};
use crate::capability::{CAPABILITY_PAGES, grant_kernel_capability, reserve_kernel_capability};
use crate::device_tree::find_timebase_frequency;
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
use ::log::{Level, error, trace};
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

static mut TIMEBASE_FREQUENCY: f64 = f64::NAN;

fn main(_hart_id: u64, device_tree: *const u8) -> ! {
    clear_bss();

    let device_tree = unsafe { Fdt::from_ptr(device_tree) }.unwrap();
    unsafe { TIMEBASE_FREQUENCY = find_timebase_frequency(&device_tree).unwrap() as f64 }
    initialize_log();
    initialize_trap_handler();
    log_sbi_metadata();
    let (virtio_blk, virtio_net, virtio_gpu, virtio_keyboard, virtio_mouse) =
        initialize_all_pci(&device_tree);
    initialize_plic(&device_tree);
    initialize_hart_stack();
    enable_interrupts();

    let fs_tar = reserve_process::<TarFs>(elf!("CARGO_BIN_FILE_DERAVEL_FILESYSTEM_TAR"));
    // let ipc_a = reserve_process::<IpcA>(elf!("CARGO_BIN_FILE_DERAVEL_APPS_ipc-a"));
    // let ipc_b = reserve_process::<IpcB>(elf!("CARGO_BIN_FILE_DERAVEL_APPS_ipc-b"));
    // let ipc_c = reserve_process::<IpcC>(elf!("CARGO_BIN_FILE_DERAVEL_APPS_ipc-c"));
    // let hello = reserve_process::<Hello>(elf!("CARGO_BIN_FILE_DERAVEL_APPS_hello"));
    let shell = reserve_process::<Shell>(elf!("CARGO_BIN_FILE_DERAVEL_APPS_shell"));
    let terminal = reserve_process::<Terminal>(elf!("CARGO_BIN_FILE_DERAVEL_APPS_terminal"));
    let windowing = reserve_process::<Windowing>(elf!("CARGO_BIN_FILE_DERAVEL_APPS_windowing"));

    // ipc_a.spawn(IpcAArgs {
    //     fs: fs_tar.export,
    //     b: ipc_b.export,
    // });
    // ipc_b.spawn(IpcBArgs { c: ipc_c.export });
    // ipc_c.spawn(IpcCArgs {});
    // hello.spawn(HelloArgs {
    //     console: reserve_kernel_capability(&SbiConsole),
    // });
    shell.spawn(ShellArgs {
        console: terminal.export,
        fs: fs_tar.export,
        net: reserve_kernel_capability(virtio_net),
    });
    fs_tar.spawn(TarFsArgs {
        drive: reserve_kernel_capability(virtio_blk),
    });
    terminal.spawn(TerminalArgs {
        windowing: windowing.export,
    });
    windowing.spawn(WindowingArgs {
        display: reserve_kernel_capability(virtio_gpu),
        keyboard: reserve_kernel_capability(virtio_keyboard),
        mouse: reserve_kernel_capability(virtio_mouse),
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
    trace!("validating capability {farthest_cap:?} presented by {current_pid}");
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
                trace!("... granted by {certifier:?} to {grantee:?}");
                assert!(grantee == sender);
                break capability;
            }
            CapabilityCertificateUnpacked::Forwarded { forwardee, inner } => {
                trace!("... forwarded {inner:?} by {certifier:?} to {forwardee:?}");
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
                        let result =
                            handler.call_method(method, &args, ProcessId::new(current_pid));
                        copy_to_user(&result, registers.a4 as *mut u8, registers.a5);
                        registers.a0 = result.len();
                        current_proc.state = ProcessState::Runnable;
                    }
                }
            }
        }
        2 => {
            let result = copy_from_user(registers.a0 as *const u8, registers.a1);
            let caller = current_proc.currently_serving.take().unwrap();
            let mut caller = PROCESSES[caller.as_usize()].lock();
            if caller.state == ProcessState::WaitingForReply {
                caller.reply = Some(result.into());
            } else if caller.state == ProcessState::WaitingForStreamMap {
                caller.stream_map = Some(serde_json::from_slice(&result).unwrap());
            } else {
                unimplemented!()
            }
        }
        3 => {
            let page_count = registers.a0;
            let pages = vec![[0; PAGE_SIZE]; page_count];
            let page_table = unsafe { &mut *(current_proc.page_table as *mut PageTable) };
            let virtual_addr = current_proc
                .virtual_memory
                .allocate(page_count * PAGE_SIZE, PAGE_SIZE);
            map_pages(
                page_table,
                virtual_addr,
                pages.as_ptr() as usize,
                PageFlags::readwrite().user(),
                page_count,
            );
            registers.a0 = virtual_addr;
        }
        4 => {
            let farthest_cap =
                RawCapability::from_pointer(registers.a0 as *mut CapabilityCertificate);
            let original = validate_untrusted_capability(farthest_cap, current_pid);
            assert_eq!(
                original.certifier(),
                Actor::Kernel,
                "shared memory capabilities can only be created by the kernel"
            );
            let handler = capability::get_handler(original.local_index());
            let (physical_address, length) = handler.shared_memory();
            assert!(length.is_multiple_of(PAGE_SIZE));

            let virtual_addr = current_proc.virtual_memory.allocate(length, PAGE_SIZE);
            let page_table = unsafe { &mut *(current_proc.page_table as *mut PageTable) };
            map_pages(
                page_table,
                virtual_addr,
                physical_address,
                PageFlags::readwrite().user(),
                length / PAGE_SIZE,
            );

            registers.a0 = virtual_addr;
            registers.a1 = length;
        }
        5 => {
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
        6 => {
            let farthest_cap =
                RawCapability::from_pointer(registers.a0 as *mut CapabilityCertificate);
            let stream = registers.a1;

            let original = validate_untrusted_capability(farthest_cap, current_pid);
            match original.certifier() {
                Actor::Userspace(original_pid) => {
                    if current_proc.state == ProcessState::WaitingForStreamMap {
                        let (ring, declared_size) = current_proc.stream_map.take().unwrap();
                        let ring = validate_untrusted_capability(ring, original_pid.as_usize());
                        assert_eq!(ring.certifier(), Actor::Kernel);

                        let handler = capability::get_handler(ring.local_index());
                        let (physical_address, length) = handler.shared_memory();
                        assert!(length.is_multiple_of(PAGE_SIZE));
                        assert!(length >= 2 * CACHE_LINE_SIZE + declared_size);

                        let virtual_addr = current_proc.virtual_memory.allocate(length, PAGE_SIZE);
                        let page_table =
                            unsafe { &mut *(current_proc.page_table as *mut PageTable) };
                        map_pages(
                            page_table,
                            virtual_addr,
                            physical_address,
                            PageFlags::readwrite().user(),
                            length / PAGE_SIZE,
                        );

                        registers.a0 = virtual_addr;
                        registers.a1 = declared_size;
                        current_proc.state = ProcessState::Runnable;
                    } else {
                        current_proc.state = ProcessState::WaitingForStreamMap;
                        current_proc.registers = registers.clone();
                        current_proc.pc = user_pc;
                        let mut dest = PROCESSES[original_pid.as_usize()].lock();
                        dest.messages.get_or_insert_default().push_back((
                            original,
                            1000 + stream,
                            Vec::new(),
                            ProcessId::new(current_pid),
                        ));

                        drop(current_proc);
                        drop(dest);
                        schedule_and_switch_to_userspace(hart);
                    }
                }
                Actor::Kernel => {
                    let handler = capability::get_handler(original.local_index());
                    let ring_buffer = handler.map_stream(stream);
                    let ring_buffer_size = size_of_val(ring_buffer);

                    let page_table = unsafe { &mut *(current_proc.page_table as *mut PageTable) };
                    let virtual_addr = current_proc
                        .virtual_memory
                        .allocate(ring_buffer_size, PAGE_SIZE);
                    map_pages(
                        page_table,
                        virtual_addr,
                        ring_buffer as *const _ as *const u8 as usize,
                        PageFlags::readwrite().user(),
                        1,
                    );
                    registers.a0 = virtual_addr;
                    registers.a1 = ring_buffer.0.data.0.len();
                }
            }
        }
        7 => {
            current_proc.registers = registers.clone();
            current_proc.pc = user_pc + 4;

            drop(current_proc);
            schedule_and_switch_to_userspace(hart);
        }
        8 => {
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
                registers.a0 = 0;
            }
        }
        9 => {
            let size = registers.a0.next_multiple_of(PAGE_SIZE);
            let memory = vec![0u8; size];
            let memory = Vec::leak(memory);
            let memory = shared_memory::SharedMemory {
                physical_address: memory.as_ptr() as usize,
                size,
            };
            let cap =
                grant_kernel_capability(ProcessId::new(current_pid), Box::leak(Box::new(memory)));
            registers.a0 = cap.as_usize();
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
