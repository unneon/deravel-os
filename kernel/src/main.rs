#![feature(allocator_api)]
#![feature(arbitrary_self_types)]
#![feature(atomic_ptr_null)]
#![feature(decl_macro)]
#![feature(generic_const_exprs)]
#![feature(iter_intersperse)]
#![feature(never_type)]
#![feature(ptr_metadata)]
#![feature(slice_from_ptr_range)]
#![feature(slice_ptr_get)]
#![allow(incomplete_features)]
#![allow(clippy::deref_addrof)]
#![allow(clippy::missing_safety_doc)]
#![allow(clippy::type_complexity)]
#![no_std]
#![no_main]

extern crate alloc;

mod arch;
mod buddy;
mod bump;
mod capability;
mod device_tree;
mod drvli;
mod elf;
mod hart;
mod heap;
mod interrupt;
mod log;
mod page;
mod pci;
mod plic;
mod process;
mod process_spawner;
mod sbi;
mod shared_memory;
mod sync;
mod user;
mod util;
mod virtio;

use crate::arch::{RiscvRegisters, initialize_trap_handler, switch_to_userspace_registers_only};
use crate::capability::{grant_kernel_capability, reserve_kernel_capability};
use crate::device_tree::initialize_timebase_frequency;
use crate::drvli::{SyscallHandler, dispatch_syscall};
use crate::elf::elf;
use crate::hart::{HartContext, HartStack};
use crate::heap::{BuddyHeap, initialize_early_heap, initialize_heap};
use crate::interrupt::INTERRUPTS;
use crate::log::{initialize_log, log_userspace};
use crate::page::{PageFlags, initialize_memory_mapping, map_pages, phys_to_virt, virt_to_phys};
use crate::pci::initialize_all_pci;
use crate::plic::{initialize_plic, plic_claim, plic_complete};
use crate::process::{
    Message, ProcessState, get_process, reserve_process, schedule_and_switch_to_userspace,
};
use crate::process_spawner::ProcessSpawnerService;
use crate::sbi::{ResetReason, ResetType, SbiShutdown, log_sbi_metadata};
use crate::user::UserPtr;
use ::log::*;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::alloc::{Allocator, Layout};
use core::mem::replace;
use core::panic::PanicInfo;
use deravel_types::*;
use fdt::Fdt;
use riscv::interrupt::Trap;
use riscv::interrupt::supervisor::{Exception, Interrupt};

macro kill {
    ($hart:ident, $proc:expr, $($tt:tt)*) => {
        {
            crate::process::kill_process($proc, format_args!($($tt)*));
            schedule_and_switch_to_userspace($hart)
        }
    },
    ($hart:ident, $($tt:tt)*) => {
        kill!($hart, $hart.current_process(), $($tt)*)
    }
}

fn main(_hart_id: u64, dt_ptr: *const u8) -> ! {
    clear_bss();

    initialize_log();
    initialize_early_heap();
    initialize_hart_stack();
    initialize_trap_handler();
    initialize_memory_mapping();
    let dt = unsafe { Fdt::from_ptr(phys_to_virt(dt_ptr)) }.unwrap();
    initialize_timebase_frequency(&dt);
    log_sbi_metadata();
    initialize_heap(&dt, dt_ptr);
    let (virtio_blk, virtio_net, virtio_gpu, virtio_keyboard, virtio_mouse) =
        initialize_all_pci(&dt);
    initialize_plic(&dt);
    enable_interrupts();

    let fs_tar = reserve_process::<TarFs>(elf!("CARGO_BIN_FILE_DERAVEL_FILESYSTEM_TAR"));
    let windowing = reserve_process::<Windowing>(elf!("CARGO_BIN_FILE_DERAVEL_APPS_windowing"));

    windowing.spawn(WindowingArgs {
        display: reserve_kernel_capability(virtio_gpu),
        keyboard: reserve_kernel_capability(virtio_keyboard),
        mouse: reserve_kernel_capability(virtio_mouse),
        fs: fs_tar.export,
        net: reserve_kernel_capability(virtio_net),
        shutdown: reserve_kernel_capability(&SbiShutdown),
        terminal: reserve_kernel_capability(Box::leak(Box::new(
            ProcessSpawnerService::<Terminal>::new(elf!("CARGO_BIN_FILE_DERAVEL_APPS_terminal")),
        ))),
        shell: reserve_kernel_capability(Box::leak(Box::new(ProcessSpawnerService::<Shell>::new(
            elf!("CARGO_BIN_FILE_DERAVEL_APPS_shell"),
        )))),
    });
    fs_tar.spawn(TarFsArgs {
        drive: reserve_kernel_capability(virtio_blk),
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
    let stack = Box::leak(HartStack::new());
    unsafe { riscv::register::sscratch::write(stack.as_raw_ctx() as usize) }
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
        .try_into::<Interrupt, Exception>();
    let stval = riscv::register::stval::read();
    let user_pc = riscv::register::sepc::read();
    if scause == Ok(Trap::Exception(Exception::UserEnvCall)) {
        let Err(err) = dispatch_syscall(user_pc, registers, hart);
        kill!(hart, "{err}");
    } else if scause == Ok(Trap::Interrupt(Interrupt::SupervisorTimer)) {
        sbi::set_timer(u64::MAX);
        switch_to_userspace_registers_only(registers)
    } else if scause == Ok(Trap::Interrupt(Interrupt::SupervisorExternal)) {
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
        switch_to_userspace_registers_only(registers)
    } else {
        panic!("unexpected trap scause={scause:?} stval={stval:#x} user_pc={user_pc:#x}");
    }
}

impl SyscallHandler for () {
    fn exit(_: usize, _: &mut RiscvRegisters, hart: &mut HartContext) -> ! {
        hart.current_process().state = ProcessState::Finished;
        schedule_and_switch_to_userspace(hart);
    }

    fn ipc_call(
        user_pc: usize,
        registers: &mut RiscvRegisters,
        hart: &mut HartContext,
        cap: RawCapability,
        method: usize,
        args_buffer: UserPtr<[u8]>,
        mut result_buffer: UserPtr<[u8]>,
    ) -> usize {
        let mut proc = hart.current_process();
        if let ProcessState::ReadyReply { reply } =
            replace(&mut proc.state, ProcessState::Transitional)
        {
            if let Err(err) = result_buffer.write_to_user(&reply) {
                proc.state = ProcessState::Finished;
                kill!(hart, proc, "{err}")
            };
            proc.state = ProcessState::Runnable;
            reply.len()
        } else {
            let cap = match cap.validate(proc.id) {
                Ok(cap) => cap,
                Err(err) => kill!(hart, proc, "{err}"),
            };

            proc.state = ProcessState::WaitingForReply {
                from: cap.certifier(),
            };
            proc.registers = registers.clone();
            proc.pc = user_pc;

            match cap.certifier() {
                Actor::Userspace(dest) => {
                    let Some(mut dest) = get_process(dest).lock_if_some() else {
                        // This can't actually happen because capability validation will catch this
                        // earlier, but let's check in case the design changes later.
                        kill!(hart, proc, "ipc send to nonexistent process");
                    };

                    dest.messages.push_back(Message {
                        cap,
                        method,
                        args: args_buffer.copy_to_kernel(),
                        sender: hart.current_pid(),
                    });

                    drop(proc);
                    drop(dest);
                    schedule_and_switch_to_userspace(hart);
                }
                Actor::Kernel => {
                    drop(proc);
                    let handler = capability::get_handler(cap.local_index());
                    let result = handler.call_method(
                        method,
                        &args_buffer.copy_to_kernel(),
                        hart.current_pid(),
                    );
                    if let Err(err) = result_buffer.write_to_user(&result) {
                        kill!(hart, "{err}")
                    };
                    hart.current_process().state = ProcessState::Runnable;
                    result.len()
                }
            }
        }
    }

    fn ipc_receive(
        _: usize,
        _: &mut RiscvRegisters,
        hart: &mut HartContext,
        mut args: UserPtr<[u8]>,
    ) -> (Option<RawCapability>, usize, usize, Option<ProcessId>) {
        let mut proc = hart.current_process();
        if proc.currently_serving.is_some() {
            kill!(hart, proc, "ipc receive without replying to previous one")
        }
        let Some(message) = proc.messages.pop_front() else {
            return (None, 0, 0, None);
        };
        if let Err(err) = args.write_to_user(&message.args) {
            kill!(hart, proc, "{err}")
        };
        proc.currently_serving = Some(message.sender);
        (
            Some(message.cap),
            message.method,
            message.args.len(),
            Some(message.sender),
        )
    }

    fn ipc_reply(_: usize, _: &mut RiscvRegisters, hart: &mut HartContext, result: UserPtr<[u8]>) {
        let mut proc = hart.current_process();
        let Some(caller) = proc.currently_serving.take() else {
            kill!(hart, proc, "ipc_reply called without matching ipc_serve")
        };
        let mut caller = get_process(caller).lock_if_some().unwrap();
        if let ProcessState::WaitingForReply { from } = caller.state {
            if from != Actor::Userspace(proc.id) {
                kill!(hart, proc, "replied to process waiting for someone else");
            }
            caller.state = ProcessState::ReadyReply {
                reply: result.copy_to_kernel(),
            };
        } else if let ProcessState::WaitingForStreamMap { from } = caller.state {
            if from != Actor::Userspace(proc.id) {
                kill!(hart, proc, "replied to process waiting for someone else");
            }
            let Ok(stream) = serde_json::from_slice(&result.copy_to_kernel()) else {
                kill!(hart, proc, "invalid stream map reply")
            };
            caller.state = ProcessState::ReadyStreamMap { stream };
        } else {
            unimplemented!()
        }
    }

    fn ipc_stream(
        user_pc: usize,
        registers: &mut RiscvRegisters,
        hart: &mut HartContext,
        cap: RawCapability,
        stream: usize,
    ) -> (*mut (), usize) {
        let mut proc = hart.current_process();
        let cap = match cap.validate(proc.id) {
            Ok(cap) => cap,
            Err(err) => kill!(hart, proc, "{err}"),
        };
        match cap.certifier() {
            Actor::Userspace(original_pid) => {
                if let ProcessState::ReadyStreamMap {
                    stream: (ring, declared_size),
                } = proc.state
                {
                    let ring = match ring.validate(original_pid) {
                        Ok(ring) => ring,
                        Err(err) => kill!(hart, proc, "{err}"),
                    };
                    if ring.certifier() != Actor::Kernel {
                        kill!(hart, proc, "non-kernel shared memory capability")
                    }

                    let handler = capability::get_handler(ring.local_index());
                    let (physical_address, length) = handler.shared_memory();
                    if !length.is_multiple_of(PAGE_SIZE) {
                        kill!(hart, proc, "stream size must be a multiple of page size")
                    }
                    if length < 2 * CACHE_LINE_SIZE + declared_size {
                        kill!(hart, proc, "stream length does not match memory size")
                    }

                    let layout = Layout::from_size_align(length, PAGE_SIZE).unwrap();
                    let virtual_addr = proc.virtual_memory.alloc(layout).unwrap();
                    map_pages(
                        unsafe { &mut *proc.page_table },
                        virtual_addr,
                        physical_address,
                        PageFlags::readwrite().user(),
                        length,
                    );

                    proc.state = ProcessState::Runnable;
                    (virtual_addr as *mut (), declared_size)
                } else {
                    proc.state = ProcessState::WaitingForStreamMap {
                        from: original_pid.into(),
                    };
                    proc.registers = registers.clone();
                    proc.pc = user_pc;
                    let mut dest = get_process(original_pid).lock_if_some().unwrap();
                    dest.messages.push_back(Message {
                        cap,
                        method: 1000 + stream,
                        args: Vec::new(),
                        sender: hart.current_pid(),
                    });

                    drop(proc);
                    drop(dest);
                    schedule_and_switch_to_userspace(hart);
                }
            }
            Actor::Kernel => {
                let handler = capability::get_handler(cap.local_index());
                let ring_buffer = handler.map_stream(stream);
                let ring_buffer_size = size_of_val(ring_buffer);
                let ring_buffer_layout =
                    Layout::from_size_align(ring_buffer_size, PAGE_SIZE).unwrap();

                let virtual_addr = proc.virtual_memory.alloc(ring_buffer_layout).unwrap();
                map_pages(
                    unsafe { &mut *proc.page_table },
                    virtual_addr,
                    ring_buffer as *const _ as *const u8 as usize,
                    PageFlags::readwrite().user(),
                    PAGE_SIZE,
                );
                (virtual_addr as *mut (), ring_buffer.0.data.0.len())
            }
        }
    }

    fn alloc(_: usize, _: &mut RiscvRegisters, hart: &mut HartContext, size: usize) -> *mut u8 {
        let padded_size = size.next_multiple_of(PAGE_SIZE);
        let layout = Layout::from_size_align(padded_size, PAGE_SIZE).unwrap();
        let mut proc = hart.current_process();
        let virtual_addr = proc.virtual_memory.alloc(layout).unwrap();
        let physical_addr =
            virt_to_phys(BuddyHeap.allocate(layout).unwrap().as_ptr().as_mut_ptr()) as usize;
        map_pages(
            unsafe { &mut *proc.page_table },
            virtual_addr,
            physical_addr,
            PageFlags::readwrite().user(),
            padded_size,
        );
        virtual_addr as *mut u8
    }

    fn alloc_shared(
        _: usize,
        _: &mut RiscvRegisters,
        hart: &mut HartContext,
        size: usize,
    ) -> (*mut u8, Capability<SharedMemory>) {
        let padded_size = size.next_multiple_of(PAGE_SIZE);
        let layout = Layout::from_size_align(padded_size, PAGE_SIZE).unwrap();
        let mut proc = hart.current_process();
        let virtual_addr = proc.virtual_memory.alloc(layout).unwrap();
        let physical_addr =
            virt_to_phys(BuddyHeap.allocate(layout).unwrap().as_ptr().as_mut_ptr()) as usize;
        map_pages(
            unsafe { &mut *proc.page_table },
            virtual_addr,
            physical_addr,
            PageFlags::readwrite().user(),
            padded_size,
        );
        let cap = grant_kernel_capability(
            hart.current_pid(),
            Box::leak(Box::new(shared_memory::SharedMemory {
                physical_address: physical_addr,
                size,
            })),
        );
        (virtual_addr as *mut u8, cap)
    }

    fn map_shared(
        _: usize,
        _: &mut RiscvRegisters,
        hart: &mut HartContext,
        cap: Capability<SharedMemory>,
    ) -> (*mut u8, usize) {
        let mut proc = hart.current_process();
        let cap = match cap.validate(proc.id) {
            Ok(cap) => cap,
            Err(err) => kill!(hart, proc, "{err}"),
        };
        if cap.certifier() != Actor::Kernel {
            kill!(hart, proc, "non-kernel shared memory capability")
        }

        let handler = capability::get_handler(cap.local_index());
        let (physical_address, length) = handler.shared_memory();
        let padded_length = length.next_multiple_of(PAGE_SIZE);
        let layout = Layout::from_size_align(padded_length, PAGE_SIZE).unwrap();

        let virtual_addr = proc.virtual_memory.alloc(layout).unwrap();
        map_pages(
            unsafe { &mut *proc.page_table },
            virtual_addr,
            physical_address,
            PageFlags::readwrite().user(),
            padded_length,
        );

        (virtual_addr as *mut u8, length)
    }

    fn yield_(user_pc: usize, registers: &mut RiscvRegisters, hart: &mut HartContext) {
        let mut current_proc = hart.current_process();
        current_proc.registers = registers.clone();
        current_proc.pc = user_pc + 4;
        drop(current_proc);
        schedule_and_switch_to_userspace(hart);
    }

    fn log(
        _: usize,
        _: &mut RiscvRegisters,
        hart: &mut HartContext,
        message: UserPtr<[u8]>,
        level: u64,
    ) {
        let Ok(text) = String::from_utf8(message.copy_to_kernel()) else {
            kill!(hart, "invalid utf-8")
        };
        let level = match level {
            0 => Level::Error,
            1 => Level::Warn,
            2 => Level::Info,
            3 => Level::Debug,
            4 => Level::Trace,
            _ => kill!(hart, "invalid log level"),
        };
        log_userspace(level, &hart.current_process(), &text);
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let location_slot;
    let location = if let Some(location) = info.location() {
        location_slot = location;
        format_args!(" at {location_slot}")
    } else {
        format_args!("")
    };
    let message = info.message();
    error!("panicked{location}: {message}");
    let _ = sbi::system_reset(ResetType::Shutdown, ResetReason::SystemFailure);
    loop {}
}
