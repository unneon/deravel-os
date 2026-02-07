use crate::process::Process;
use crate::{handle_trap, main};
use core::arch::{asm, naked_asm};
use riscv::register::mtvec::TrapMode;
use riscv::register::stvec::Stvec;

unsafe extern "C" {
    static mut stack_top: u8;
}

#[repr(C)]
#[derive(Clone)]
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

#[unsafe(link_section = ".text.boot")]
#[unsafe(naked)]
#[unsafe(no_mangle)]
unsafe extern "C" fn boot() -> ! {
    naked_asm!(
        "la sp, {stack_top}",
        "j {main}",
        stack_top = sym stack_top,
        main = sym main,
    )
}

pub fn initialize_trap_handler() {
    let address = trap_entry as *const () as usize;
    unsafe { riscv::register::stvec::write(Stvec::new(address, TrapMode::Direct)) }
}

pub fn switch_to_userspace_full(next: &Process) -> ! {
    riscv::asm::sfence_vma_all();
    unsafe { riscv::register::satp::write(next.satp()) };
    riscv::asm::sfence_vma_all();
    unsafe { riscv::register::sepc::write(next.pc) };
    switch_to_userspace_registers_only(&next.registers)
}

pub fn switch_to_userspace_registers_only(registers: &RiscvRegisters) -> ! {
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
unsafe extern "C" fn trap_entry() -> ! {
    naked_asm!(
        ".align 4",
        "csrw sscratch, sp",
        "la sp, {stack_top}",
        "addi sp, sp, -8 * 31",

        "sd ra, 8 * 0(sp)",

        "csrr ra, sscratch",
        "sd ra, 8 * 1(sp)", // ra here is the original sp

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
        "call {handle_trap}",

        stack_top = sym stack_top,
        handle_trap = sym handle_trap,
    )
}
