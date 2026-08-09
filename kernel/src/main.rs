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
mod heap;
mod interrupt;
mod log;
mod page;
mod pci;
mod plic;
mod process;
mod sbi;
mod shared_memory;
mod shutdown;
mod stack;
mod sync;
mod user;
mod util;
mod virtio;
mod virtual_memory;

use crate::arch::{
    RiscvRegisters, enable_interrupts, enable_kernel_trap_handler, is_page_fault, return_to_user,
};
use crate::capability::{grant_kernel_capability, reserve_kernel_capability};
use crate::device_tree::initialize_timebase_frequency;
use crate::drvli::{SyscallHandler, dispatch_syscall};
use crate::elf::elf;
use crate::heap::granularity::page_granular_vec;
use crate::heap::{initialize_early_heap, initialize_heap};
use crate::interrupt::INTERRUPTS;
use crate::log::{initialize_log, log_userspace};
use crate::page::{PageFlags, initialize_late_memory_mapping, virt_to_phys};
use crate::pci::initialize_all_pci;
use crate::plic::{initialize_plic, plic_claim, plic_complete};
use crate::process::spawner::ProcessSpawnerService;
use crate::process::{
    Message, ProcessState, get_process, kill, reserve_process, schedule_and_switch_to_userspace,
};
use crate::sbi::{ResetReason, ResetType, log_sbi_metadata};
use crate::shutdown::KernelShutdown;
use crate::stack::{UserCtx, initialize_kernel_stack};
use crate::user::UserPtr;
use crate::util::untyped_box::UntypedBox;
use ::log::*;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::alloc::Layout;
use core::ops::DerefMut;
use core::panic::PanicInfo;
use deravel_types::memory::USER_STACK_GUARD;
use deravel_types::*;
use fdt::Fdt;
use riscv::interrupt::Trap;
use riscv::interrupt::supervisor::{Exception, Interrupt};

extern "C" fn main(_hart_id: u64, dt_ptr: *const u8) -> ! {
    initialize_log();
    enable_kernel_trap_handler();
    // initialize_late_memory_mapping();
    initialize_early_heap();
    let dt = unsafe { Fdt::from_ptr(dt_ptr) }.unwrap();
    initialize_timebase_frequency(&dt);
    log_sbi_metadata();
    initialize_heap(&dt, dt_ptr);
    initialize_kernel_stack();
    let (virtio_blk, virtio_net, virtio_gpu, virtio_keyboard, virtio_mouse) =
        initialize_all_pci(&dt);
    initialize_plic(&dt);
    enable_interrupts();

    /*
    let fat = reserve_process::<FatFs>(elf!("CARGO_BIN_FILE_DERAVEL_FILESYSTEM_FAT"));
    let windowing = reserve_process::<Windowing>(elf!("CARGO_BIN_FILE_DERAVEL_APPS_windowing"));

    windowing.spawn(WindowingArgs {
        display: reserve_kernel_capability(virtio_gpu),
        keyboard: reserve_kernel_capability(virtio_keyboard),
        mouse: reserve_kernel_capability(virtio_mouse),
        fs: fat.export,
        image_viewer: reserve_kernel_capability(Box::leak(Box::new(ProcessSpawnerService::<
            ImageViewer,
        >::new(elf!(
            "CARGO_BIN_FILE_DERAVEL_APPS_image_viewer"
        ))))),
        net: reserve_kernel_capability(virtio_net),
        shutdown: reserve_kernel_capability(&KernelShutdown),
        terminal: reserve_kernel_capability(Box::leak(Box::new(
            ProcessSpawnerService::<Terminal>::new(elf!("CARGO_BIN_FILE_DERAVEL_APPS_terminal")),
        ))),
        shell: reserve_kernel_capability(Box::leak(Box::new(ProcessSpawnerService::<Shell>::new(
            elf!("CARGO_BIN_FILE_DERAVEL_APPS_shell"),
        )))),
    });
    fat.spawn(FatFsArgs {
        drive: reserve_kernel_capability(virtio_blk),
    });
     */

    // TODO: initialize_hart_stack should take a callback and pass this with the correct lifetime.
    let hart = unsafe { &mut *(riscv::register::sscratch::read() as *mut UserCtx) };
    schedule_and_switch_to_userspace(hart);
}

fn handle_kernel_trap(_: &mut RiscvRegisters) -> ! {
    let scause = riscv::register::scause::read()
        .cause()
        .try_into::<Interrupt, Exception>();
    let stval = riscv::register::stval::read();
    let pc = riscv::register::sepc::read();

    // TODO: Enable handling interrupts in kernel mode again.

    panic!("unexpected kernel trap, scause {scause:?} stval {stval:#x} pc {pc:#x}");
}

fn handle_user_trap(user: &mut UserCtx) -> ! {
    enable_kernel_trap_handler();
    let scause = riscv::register::scause::read()
        .cause()
        .try_into::<Interrupt, Exception>();
    let stval = riscv::register::stval::read();
    let user_pc = riscv::register::sepc::read();
    // TODO: Don't copy this to the stack.
    let mut registers = user.process().registers.clone();
    if scause == Ok(Trap::Exception(Exception::UserEnvCall)) {
        user.process().pc = user_pc + 4;
        unsafe { riscv::register::sepc::write(user_pc + 4) }
        let Err(err) = dispatch_syscall(&mut registers, user);
        kill!(user, "{err}");
    } else if scause == Ok(Trap::Interrupt(Interrupt::SupervisorTimer)) {
        sbi::set_timer(u64::MAX);
        return_to_user(&registers)
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
        return_to_user(&registers)
    } else if is_page_fault(scause) {
        if USER_STACK_GUARD.contains(&stval) {
            kill!(user, "stack overflow")
        }
        let mut proc = user.process();
        if let Some(vmm) = proc
            .virtual_memory_mappings
            .iter()
            .find(|vmm| vmm.0.contains(&stval) && !proc.page_table.is_mapped(stval))
        {
            let page_index = (stval - vmm.0.start) / PAGE_SIZE;
            vmm.1
                .load_page(vmm.0.start, page_index, &mut proc.page_table);
            drop(proc);
            riscv::asm::sfence_vma_all();
            return_to_user(&registers)
        }
        kill!(user, proc, "forbidden access to {stval:#x}")
    } else {
        panic!("unexpected trap, scause {scause:?}, stval {stval:#x}, user pc {user_pc:#x}");
    }
}

impl SyscallHandler for () {
    fn exit(user: &mut UserCtx) -> ! {
        user.process().state = ProcessState::Finished;
        schedule_and_switch_to_userspace(user);
    }

    fn ipc_call(
        user: &mut UserCtx,
        cap: RawCapability,
        method: usize,
        args_buffer: UserPtr<[u8]>,
        mut result_buffer: UserPtr<[u8]>,
    ) -> usize {
        let mut proc = user.process();
        let cap = match cap.validate(proc.id) {
            Ok(cap) => cap,
            Err(err) => kill!(user, proc, "{err}"),
        };

        match cap.certifier() {
            Actor::Userspace(dest) => {
                let Some(mut dest) = get_process(dest).lock_if_some() else {
                    // This can't actually happen because capability validation will catch this
                    // earlier, but let's check in case the design changes later.
                    kill!(user, proc, "ipc send to nonexistent process");
                };

                proc.state = ProcessState::WaitingForReply {
                    from: cap.certifier(),
                    result_buffer,
                };

                dest.messages.push_back(Message {
                    cap,
                    method,
                    args: args_buffer.copy_to_kernel(),
                    sender: user.pid(),
                });

                drop(proc);
                drop(dest);
                schedule_and_switch_to_userspace(user);
            }
            Actor::Kernel => {
                drop(proc);
                let handler = capability::get_handler(cap.local_index());
                let result = handler.call_method(method, &args_buffer.copy_to_kernel(), user.pid());
                if let Err(err) = result_buffer.write_to_user(&result) {
                    kill!(user, "{err}")
                };
                result.len()
            }
        }
    }

    fn ipc_receive(
        user: &mut UserCtx,
        mut args: UserPtr<[u8]>,
    ) -> (Option<RawCapability>, usize, usize, Option<ProcessId>) {
        let mut proc = user.process();
        if proc.currently_serving.is_some() {
            kill!(user, proc, "ipc receive without replying to previous one")
        }
        let Some(message) = proc.messages.pop_front() else {
            return (None, 0, 0, None);
        };
        if let Err(err) = args.write_to_user(&message.args) {
            kill!(user, proc, "{err}")
        };
        proc.currently_serving = Some(message.sender);
        (
            Some(message.cap),
            message.method,
            message.args.len(),
            Some(message.sender),
        )
    }

    fn ipc_reply(user: &mut UserCtx, result: UserPtr<[u8]>) {
        let mut proc = user.process();
        let Some(caller) = proc.currently_serving.take() else {
            kill!(user, proc, "ipc_reply called without matching ipc_serve")
        };
        let mut caller = get_process(caller).lock_if_some().unwrap();
        if let ProcessState::WaitingForReply {
            from,
            result_buffer,
        } = &caller.state
        {
            if *from != Actor::Userspace(proc.id) {
                kill!(user, proc, "replied to process waiting for someone else");
            }
            caller.state = ProcessState::ReadyReply {
                reply: result.copy_to_kernel(),
                result_buffer: result_buffer.clone(),
            };
        } else if let ProcessState::WaitingForStreamMap { from } = caller.state {
            if from != Actor::Userspace(proc.id) {
                kill!(user, proc, "replied to process waiting for someone else");
            }
            let Ok((stream, declared_size)) =
                postcard::from_bytes::<(RawCapability, usize)>(&result.copy_to_kernel())
            else {
                kill!(user, proc, "invalid stream map reply")
            };
            let Ok(ring) = stream.validate(caller.id) else {
                kill!(user, proc, "ring {stream:?} not valid for sender");
            };
            if ring.certifier() != Actor::Kernel {
                kill!(user, proc, "shared memory must be granted by the kernel");
            }
            let handler = capability::get_handler(ring.local_index());
            let length = handler.shared_memory_size();
            if !length.is_multiple_of(PAGE_SIZE) {
                kill!(user, proc, "stream size must be a multiple of page size")
            }
            if length < 2 * CACHE_LINE_SIZE + declared_size {
                kill!(user, proc, "stream length does not match memory size")
            }
            caller.state = ProcessState::ReadyStreamMap {
                ring,
                declared_size,
            };
        } else {
            unimplemented!()
        }
    }

    fn ipc_stream(user: &mut UserCtx, cap: RawCapability, stream: usize) -> (*mut (), usize) {
        let mut proc = user.process();
        let cap = match cap.validate(proc.id) {
            Ok(cap) => cap,
            Err(err) => kill!(user, proc, "{err}"),
        };
        match cap.certifier() {
            Actor::Userspace(original_pid) => {
                proc.state = ProcessState::WaitingForStreamMap {
                    from: original_pid.into(),
                };
                let mut dest = get_process(original_pid).lock_if_some().unwrap();
                dest.messages.push_back(Message {
                    cap,
                    method: 1000 + stream,
                    args: Vec::new(),
                    sender: user.pid(),
                });

                drop(proc);
                drop(dest);
                schedule_and_switch_to_userspace(user);
            }
            Actor::Kernel => {
                let handler = capability::get_handler(cap.local_index());
                let ring_buffer = handler.map_stream(stream);
                let ring_buffer_size = size_of_val(ring_buffer);
                let ring_buffer_layout =
                    Layout::from_size_align(ring_buffer_size, PAGE_SIZE).unwrap();

                let virt = proc.heap.alloc(ring_buffer_layout).unwrap();
                proc.page_table.map(
                    virt,
                    virt_to_phys(ring_buffer as *const _ as *const u8) as usize,
                    PAGE_SIZE,
                    PageFlags::read_write().user(),
                );
                riscv::asm::sfence_vma_all();
                (virt as *mut (), ring_buffer.0.data.0.len())
            }
        }
    }

    fn alloc(user: &mut UserCtx, size: usize) -> *mut u8 {
        let size = size.next_multiple_of(PAGE_SIZE);
        let pages = Arc::new(UntypedBox::new(
            page_granular_vec![0u8; size].into_boxed_slice(),
        ));
        let virt = user.process().alloc(pages, PageFlags::read_write().user());
        riscv::asm::sfence_vma_all();
        virt
    }

    fn alloc_shared(user: &mut UserCtx, size: usize) -> (*mut u8, Capability<SharedMemory>) {
        let size = size.next_multiple_of(PAGE_SIZE);
        let pages = Arc::new(UntypedBox::new(
            page_granular_vec![0u8; size].into_boxed_slice(),
        ));
        let virt = user
            .process()
            .alloc(pages.clone(), PageFlags::read_write().user());
        riscv::asm::sfence_vma_all();
        let cap = grant_kernel_capability(
            user.pid(),
            Box::leak(Box::new(shared_memory::SharedMemory { backing: pages })),
        );
        (virt, cap)
    }

    fn map_shared(user: &mut UserCtx, cap: Capability<SharedMemory>) -> (*mut u8, usize) {
        let mut proc = user.process();
        let cap = match cap.validate(proc.id) {
            Ok(cap) => cap,
            Err(err) => kill!(user, proc, "{err}"),
        };
        if cap.certifier() != Actor::Kernel {
            kill!(user, proc, "non-kernel shared memory capability")
        }

        let handler = capability::get_handler(cap.local_index());
        let length = handler.shared_memory_size();
        let padded_length = length.next_multiple_of(PAGE_SIZE);
        let layout = Layout::from_size_align(padded_length, PAGE_SIZE).unwrap();

        let virt = proc.heap.alloc(layout).unwrap();
        let proc = proc.deref_mut();
        handler.shared_memory_map(
            virt,
            &mut proc.page_table,
            &mut proc.virtual_memory_mappings,
        );
        riscv::asm::sfence_vma_all();

        (virt as *mut u8, length)
    }

    fn free(user: &mut UserCtx, ptr: UserPtr<u8>) {
        let mut proc = user.process();
        if proc.dealloc(ptr.as_ptr()).is_err() {
            kill!(user, proc, "free of unallocated pointer")
        }
    }

    fn yield_(user: &mut UserCtx) {
        schedule_and_switch_to_userspace(user);
    }

    fn log(user: &mut UserCtx, message: UserPtr<[u8]>, level: u64) {
        let Ok(text) = String::from_utf8(message.copy_to_kernel()) else {
            kill!(user, "invalid utf-8")
        };
        let level = match level {
            0 => Level::Error,
            1 => Level::Warn,
            2 => Level::Info,
            3 => Level::Debug,
            4 => Level::Trace,
            _ => kill!(user, "invalid log level"),
        };
        log_userspace(level, &user.process(), &text);
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
