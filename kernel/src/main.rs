#![feature(allocator_api)]
#![feature(arbitrary_self_types)]
#![feature(atomic_ptr_null)]
#![feature(const_convert)]
#![feature(const_trait_impl)]
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
mod syscall;
mod user;
mod util;
mod virtio;
mod virtual_memory;

use crate::arch::{
    RiscvRegisters, enable_interrupts, enable_kernel_trap_handler, is_page_fault, return_to_user,
};
use crate::capability::reserve_kernel_capability;
use crate::device_tree::initialize_timebase_frequency;
use crate::drvli::dispatch_syscall;
use crate::elf::elf;
use crate::heap::{initialize_early_heap, initialize_heap};
use crate::interrupt::INTERRUPTS;
use crate::log::initialize_log;
use crate::pci::initialize_all_pci;
use crate::plic::{initialize_plic, plic_claim, plic_complete};
use crate::process::{kill_manual, reserve_process, schedule_and_switch_to_userspace};
use crate::sbi::{ResetReason, ResetType, log_sbi_metadata};
use crate::shutdown::KernelShutdown;
use crate::stack::{UserCtx, initialize_kernel_stack};
use crate::syscall::SyscallAction;
use ::log::*;
use core::panic::PanicInfo;
use deravel_types::memory::USER_STACK_GUARD;
use deravel_types::*;
use fdt::Fdt;
use riscv::interrupt::Trap;
use riscv::interrupt::supervisor::{Exception, Interrupt};

extern "C" fn main(_hart_id: u64, dt_ptr: *const u8) -> ! {
    initialize_log();
    enable_kernel_trap_handler();
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

    let fat = reserve_process(elf!(FatFs, "deravel-filesystem-fat"));

    let windowing = reserve_process(elf!(Windowing, "windowing"));

    windowing.spawn(WindowingArgs {
        display: reserve_kernel_capability(virtio_gpu),
        keyboard: reserve_kernel_capability(virtio_keyboard),
        mouse: reserve_kernel_capability(virtio_mouse),
        fs: fat.export,
        image_viewer: reserve_kernel_capability(elf!(ImageViewer, "image_viewer")),
        net: reserve_kernel_capability(virtio_net),
        shutdown: reserve_kernel_capability(&KernelShutdown),
        terminal: reserve_kernel_capability(elf!(Terminal, "terminal")),
        shell: reserve_kernel_capability(elf!(Shell, "shell")),
    });
    fat.spawn(FatFsArgs {
        drive: reserve_kernel_capability(virtio_blk),
    });

    // TODO: initialize_hart_stack should take a callback and pass this with the correct lifetime.
    let hart = unsafe { &mut *(riscv::register::sscratch::read() as *mut UserCtx) };
    unsafe { schedule_and_switch_to_userspace(hart) }
}

extern "C" fn handle_kernel_trap(_: &mut RiscvRegisters) -> ! {
    let scause = riscv::register::scause::read()
        .cause()
        .try_into::<Interrupt, Exception>();
    let stval = riscv::register::stval::read();
    let pc = riscv::register::sepc::read();

    // TODO: Enable handling interrupts in kernel mode again.

    panic!("unexpected kernel trap, scause {scause:?} stval {stval:#x} pc {pc:#x}");
}

extern "C" fn handle_user_trap(user: &mut UserCtx) -> ! {
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
        match dispatch_syscall(&mut registers, user) {
            Ok(()) => {}
            Err(SyscallAction::UserErr(err)) => {
                kill_manual!(user, "{err}");
                unsafe { schedule_and_switch_to_userspace(user) }
            }
            Err(SyscallAction::Yield) => unsafe { schedule_and_switch_to_userspace(user) },
        }
    } else if scause == Ok(Trap::Interrupt(Interrupt::SupervisorTimer)) {
        sbi::set_timer(u64::MAX);
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
    } else if is_page_fault(scause) {
        if USER_STACK_GUARD.contains(&stval) {
            kill_manual!(user, "stack overflow");
            unsafe { schedule_and_switch_to_userspace(user) }
        }
        let mut proc = user.process();
        let Some(vmm) = proc
            .virtual_memory_mappings
            .iter()
            .find(|vmm| vmm.0.contains(&stval) && !proc.page_table.is_mapped(stval))
        else {
            kill_manual!(user, "forbidden access to {stval:#x}");
            drop(proc);
            unsafe { schedule_and_switch_to_userspace(user) }
        };
        let page_index = (stval - vmm.0.start) / PAGE_SIZE;
        vmm.1
            .load_page(vmm.0.start, page_index, &mut proc.page_table);
        drop(proc);
        riscv::asm::sfence_vma_all();
    } else {
        panic!("unexpected trap, scause {scause:?}, stval {stval:#x}, user pc {user_pc:#x}");
    }
    unsafe { return_to_user(&registers) }
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
