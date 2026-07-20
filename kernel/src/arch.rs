use crate::page::{PageFlags, map_pages, satp};
use crate::process::{Process, ProcessState};
use crate::stack::UserCtx;
use crate::sync::MutexGuard;
use crate::user::UserSyscallError;
use crate::{capability, handle_kernel_trap, handle_user_trap, main};
use core::alloc::Layout;
use core::arch::{asm, naked_asm};
use deravel_types::PAGE_SIZE;
use riscv::register::mtvec::TrapMode;
use riscv::register::stvec::Stvec;

#[repr(C)]
#[derive(Clone, Debug, Default)]
pub struct RiscvRegisters {
    pub ra: usize,
    pub sp: usize,
    pub gp: usize,
    pub tp: usize,
    pub t0: usize,
    pub t1: usize,
    pub t2: usize,
    pub s0: usize,
    pub s1: usize,
    pub a0: usize,
    pub a1: usize,
    pub a2: usize,
    pub a3: usize,
    pub a4: usize,
    pub a5: usize,
    pub a6: usize,
    pub a7: usize,
    pub s2: usize,
    pub s3: usize,
    pub s4: usize,
    pub s5: usize,
    pub s6: usize,
    pub s7: usize,
    pub s8: usize,
    pub s9: usize,
    pub s10: usize,
    pub s11: usize,
    pub t3: usize,
    pub t4: usize,
    pub t5: usize,
    pub t6: usize,
}

// These check the type of symbol used in naked assembly blocks for trap entry handlers. I don't
// think there's a better way to type-check this.
const _: fn(&mut RiscvRegisters) -> ! = handle_kernel_trap;
const _: fn(&mut UserCtx) -> ! = handle_user_trap;

#[unsafe(link_section = ".text.boot")]
#[unsafe(naked)]
#[unsafe(no_mangle)]
unsafe extern "C" fn _start() -> ! {
    unsafe extern "C" {
        static mut early_stack_top: u8;
    }
    naked_asm!(
        "la sp, {early_stack_top}",
        "j {main}",
        early_stack_top = sym early_stack_top,
        main = sym main,
    )
}

pub fn enable_kernel_trap_handler() {
    let address = supervisor_trap_entry as *const () as usize;
    unsafe { riscv::register::stvec::write(Stvec::new(address, TrapMode::Direct)) }
}

pub fn enable_user_trap_handler() {
    let address = user_trap_entry as *const () as usize;
    unsafe { riscv::register::stvec::write(Stvec::new(address, TrapMode::Direct)) }
}

pub fn switch_to_user(mut next: MutexGuard<Process>) -> Result<!, UserSyscallError> {
    unsafe { riscv::register::satp::write(satp(&next.page_table)) };

    // SFENCE.VMA is required after SATP write. (RISC-V Privileged 12.2.1).
    riscv::asm::sfence_vma_all();

    match &mut next.state {
        ProcessState::Runnable => {}
        ProcessState::ReadyReply {
            reply,
            result_buffer,
        } => {
            result_buffer.write_to_user(reply)?;
            next.registers.a0 = reply.len();
            next.state = ProcessState::Runnable;
        }
        ProcessState::ReadyStreamMap {
            ring,
            declared_size,
        } => {
            let ring = *ring;
            let declared_size = *declared_size;
            let handler = capability::get_handler(ring.local_index());
            let (phys, length) = handler.shared_memory();
            let layout = Layout::from_size_align(length, PAGE_SIZE).unwrap();
            let virt = next.virtual_memory.alloc(layout).unwrap();
            let table = &mut next.page_table;
            map_pages(table, virt, phys, PageFlags::readwrite().user(), length);
            next.registers.a0 = virt;
            next.registers.a1 = declared_size;
            next.state = ProcessState::Runnable;
        }
        _ => panic!("can't switch to process with state {:?}", next.state),
    }

    unsafe { riscv::register::sepc::write(next.pc) };
    let mut status = riscv::register::sstatus::read();
    status.set_spie(true);
    status.set_sum(true);
    unsafe { riscv::register::sstatus::write(status) };
    let registers = &next.registers as *const _;
    drop(next);

    return_to_user(registers)
}

pub fn return_to_user(registers: *const RiscvRegisters) -> ! {
    enable_user_trap_handler();

    unsafe {
        asm!(
            "ld ra, 8 * 0(t6)",
            "ld sp, 8 * 1(t6)",
            "ld gp, 8 * 2(t6)",
            "ld tp, 8 * 3(t6)",
            "ld t0, 8 * 4(t6)",
            "ld t1, 8 * 5(t6)",
            "ld t2, 8 * 6(t6)",
            "ld s0, 8 * 7(t6)",
            "ld s1, 8 * 8(t6)",
            "ld a0, 8 * 9(t6)",
            "ld a1, 8 * 10(t6)",
            "ld a2, 8 * 11(t6)",
            "ld a3, 8 * 12(t6)",
            "ld a4, 8 * 13(t6)",
            "ld a5, 8 * 14(t6)",
            "ld a6, 8 * 15(t6)",
            "ld a7, 8 * 16(t6)",
            "ld s2, 8 * 17(t6)",
            "ld s3, 8 * 18(t6)",
            "ld s4, 8 * 19(t6)",
            "ld s5, 8 * 20(t6)",
            "ld s6, 8 * 21(t6)",
            "ld s7, 8 * 22(t6)",
            "ld s8, 8 * 23(t6)",
            "ld s9, 8 * 24(t6)",
            "ld s10, 8 * 25(t6)",
            "ld s11, 8 * 26(t6)",
            "ld t3, 8 * 27(t6)",
            "ld t4, 8 * 28(t6)",
            "ld t5, 8 * 29(t6)",
            "ld t6, 8 * 30(t6)",
            "sret",
            in("t6") registers,
            options(noreturn),
        )
    }
}

#[unsafe(naked)]
unsafe extern "C" fn supervisor_trap_entry() -> ! {
    naked_asm!(
        ".align 4",

        // TODO: Handle kernel stack overflow guard pages once I add them,

        // As this trap was triggered while in supervisor mode, sp is as trustworthy as sscratch so
        // we might as well use it to avoid complexity. We can also assume it is 16-byte aligend as
        // per the RISC-V ABI. So, let's store the registers immediately below the stack (this also
        // assumes no red zone, which is true on this target).

        "addi sp, sp, -8 * 32",
        "sd ra, 8 * 0(sp)",

        "addi ra, sp, 8 * 32",
        "sd ra, 8 * 1(sp)",

        "sd gp, 8 * 2(sp)",
        "sd tp, 8 * 3(sp)",
        "sd t0, 8 * 4(sp)",
        "sd t1, 8 * 5(sp)",
        "sd t2, 8 * 6(sp)",
        "sd s0, 8 * 7(sp)",
        "sd s1, 8 * 8(sp)",
        "sd a0, 8 * 9(sp)",
        "sd a1, 8 * 10(sp)",
        "sd a2, 8 * 11(sp)",
        "sd a3, 8 * 12(sp)",
        "sd a4, 8 * 13(sp)",
        "sd a5, 8 * 14(sp)",
        "sd a6, 8 * 15(sp)",
        "sd a7, 8 * 16(sp)",
        "sd s2, 8 * 17(sp)",
        "sd s3, 8 * 18(sp)",
        "sd s4, 8 * 19(sp)",
        "sd s5, 8 * 20(sp)",
        "sd s6, 8 * 21(sp)",
        "sd s7, 8 * 22(sp)",
        "sd s8, 8 * 23(sp)",
        "sd s9, 8 * 24(sp)",
        "sd s10, 8 * 25(sp)",
        "sd s11, 8 * 26(sp)",
        "sd t3, 8 * 27(sp)",
        "sd t4, 8 * 28(sp)",
        "sd t5, 8 * 29(sp)",
        "sd t6, 8 * 30(sp)",

        "mv a0, sp",
        "j {handle_kernel_trap}",

        handle_kernel_trap = sym handle_kernel_trap,
    )
}

#[unsafe(naked)]
unsafe extern "C" fn user_trap_entry() -> ! {
    naked_asm!(
        ".align 4",

        "csrrw sp, sscratch, sp",

        // Unlike the normal implementation that saves registers to the stack, we want to save them
        // straight to the process structure. But that means instead of just the stack pointer, we
        // have a stack pointer and the RiscvRegisters pointer. So we temporarily store just ra and
        // gp on the stack.
        "addi sp, sp, -16",
        "sd ra, 8 * 0(sp)",
        "sd gp, 8 * 1(sp)",

        // See UserCtx definition in stack.rs. This can possibly be rewritten with offset_of for
        // some type safety. In inline assembly... But for now, I think this is clearer. If it's
        // wrong, nothing will work anyway.
        "ld ra, 8 * 3(sp)",

        // Move ra and gp from stack to process structure.
        "ld gp, 8 * 0(sp)",
        "sd gp, 8 * 0(ra)",
        "ld gp, 8 * 2(sp)",
        "sd gp, 8 * 2(ra)",
        "addi sp, sp, 16",

        // Store the user sp.
        "csrr gp, sscratch",
        "sd gp, 8 * 1(ra)",

        // Store all the registers not involved in previous shenanigans.
        "sd tp, 8 * 3(ra)",
        "sd t0, 8 * 4(ra)",
        "sd t1, 8 * 5(ra)",
        "sd t2, 8 * 6(ra)",
        "sd s0, 8 * 7(ra)",
        "sd s1, 8 * 8(ra)",
        "sd a0, 8 * 9(ra)",
        "sd a1, 8 * 10(ra)",
        "sd a2, 8 * 11(ra)",
        "sd a3, 8 * 12(ra)",
        "sd a4, 8 * 13(ra)",
        "sd a5, 8 * 14(ra)",
        "sd a6, 8 * 15(ra)",
        "sd a7, 8 * 16(ra)",
        "sd s2, 8 * 17(ra)",
        "sd s3, 8 * 18(ra)",
        "sd s4, 8 * 19(ra)",
        "sd s5, 8 * 20(ra)",
        "sd s6, 8 * 21(ra)",
        "sd s7, 8 * 22(ra)",
        "sd s8, 8 * 23(ra)",
        "sd s9, 8 * 24(ra)",
        "sd s10, 8 * 25(ra)",
        "sd s11, 8 * 26(ra)",
        "sd t3, 8 * 27(ra)",
        "sd t4, 8 * 28(ra)",
        "sd t5, 8 * 29(ra)",
        "sd t6, 8 * 30(ra)",

        // Restore the scratch register, and call into the handler.
        "csrw sscratch, sp",
        "mv a0, sp",
        "j {handle_user_trap}",

        handle_user_trap = sym handle_user_trap,
    )
}
