use crate::page::{PageFlags, PageTableEntry, satp, sign_extend};
use crate::process::{Process, schedule_userspace};
use crate::stack::{KernelStack, UserCtx, UserStoredCtx};
use crate::{main, on_kernel_trap, on_user_trap};
use alloc::boxed::Box;
use core::arch::{asm, naked_asm};
use core::mem::ManuallyDrop;
use deravel_types::LEVEL_2_PAGE_SIZE;
use deravel_types::memory::DIRECT_MAPPING;
use riscv::interrupt::Trap;
use riscv::interrupt::supervisor::{Exception, Interrupt};
use riscv::register::mtvec::TrapMode;
use riscv::register::stvec::Stvec;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
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

// I don't think there's a better way to type-check this.
const _: extern "C" fn(u64, *const u8) -> ! = main;
const _: extern "C" fn(&mut UserCtx) -> ! = on_user_trap;
const _: extern "riscv-interrupt-s" fn() = on_kernel_trap;

#[unsafe(link_section = ".text.start")]
#[unsafe(naked)]
#[unsafe(no_mangle)]
unsafe extern "C" fn _start() -> ! {
    unsafe extern "C" {
        static early_stack_top: u8;
        static bss_start: u8;
        static bss_end: u8;
    }
    naked_asm!(
        // Get the pointer to the initial page table. The layout needs to be guaranteed by the
        // linker script, and using Rust sym arguments always seems to result in a dynamic
        // relocation, even though it definitely could be static.

        "auipc t0, 1",

        // Initialize lower half as identity mapping. This is a workaround necessary so that the
        // instruction pointer remains valid after writing the SATP register.

        "li t1, {pte_first}",
        "li t2, {pte_diff}",

        ".map_lower_half:",

        "sd t1, 0(t0)",

        "add t0, t0, 8",
        "add t1, t1, t2",

        "and t3, t0, 2047",
        "bnez t3, .map_lower_half",

        // Initialize higher half as direct mapping. This is the mapping that will be used for
        // relocations and whole lifetime of the kernel is general.

        "li t1, {pte_first}",

        ".map_higher_half:",

        "sd t1, 0(t0)",

        "add t0, t0, 8",
        "add t1, t1, t2",

        "and t3, t0, 2047",
        "bnez t3, .map_higher_half",

        // Enable SATP, switch instruction pointer to the higher half, and switch other long-held
        // pointers to the higher half. The lower half mapping will no longer be used after this
        // part.

        "li t1, 8 << 60", // Sv39 mode
        "srl t2, t0, 12", // Convert from a pointer to physical page number.
        "add t2, t2, -1", // t0 iat the end of the page table, so -1 to PPN.
        "or t1, t1, t2",
        "csrw satp, t1",

        "li t4, {direct_mapping_addend}",
        "add a1, a1, t4",
        "add t0, t0, t4",
        "auipc t1, 0",
        "add t1, t1, t4",
        "jr t1, 10",

        // Go through the .rela.dyn section (it immediately follow the initial page table) and apply
        // all the relocations. Apparently they are all R_RISCV_RELATIVE, which makes it easy. It's
        // impossible to get the size of this table without a (relocation-requiring) symbol, so we
        // use a value with r_info set to 0 as a sentinel.

        ".apply_relocations:",

        "ld t1, 0(t0)", // r_offset: u64,
        "ld t2, 8(t0)", // r_info: u64,
        "ld t3, 16(t0)", // r_addend: i64,

        "beqz t2, .apply_relocations_end",

        "add t1, t1, t4",
        "add t3, t3, t4",
        "sd t3, 0(t1)",

        "add t0, t0, 24",
        "j .apply_relocations",

        ".apply_relocations_end:",

        // Clear the BSS section from assembly, so that all statics are initialized before we enter
        // any Rust code at all.

        "la t0, {bss_start}",
        "la t1, {bss_end}",

        ".clear_bss:",

        "sd x0, 0(t0)",

        "add t0, t0, 8",
        "bne t0, t1, .clear_bss",

        // The environment is done, load the early stack pointer and jump to main.

        "la sp, {early_stack_top}",
        "j {main}",

        pte_first = const ManuallyDrop::new(PageTableEntry::leaf(0, PageFlags::read_write_execute())).0,
        pte_diff = const ManuallyDrop::new(PageTableEntry::leaf(LEVEL_2_PAGE_SIZE, PageFlags::read_write_execute())).0
            - ManuallyDrop::new(PageTableEntry::leaf(0, PageFlags::read_write_execute())).0,
        direct_mapping_addend = const sign_extend(DIRECT_MAPPING.start),
        early_stack_top = sym early_stack_top,
        bss_start = sym bss_start,
        bss_end = sym bss_end,
        main = sym main,
    )
}

pub fn initialize_early_trap() {
    enable_kernel_trap();
}

pub fn initialize_interrupts() {
    let mut sie = riscv::register::sie::read();
    sie.set_sext(true);
    sie.set_stimer(true);
    unsafe { riscv::register::sie::write(sie) }
}

pub fn initial_switch_to_userspace() -> ! {
    let stack = KernelStack::new();
    unsafe { riscv::register::sscratch::write(stack.as_sscratch()) }

    let hart = &mut Box::leak(stack).ctx;
    schedule_userspace(hart)
}

pub fn set_userspace_process(proc: &mut Process, user: &mut UserStoredCtx) {
    unsafe { riscv::register::satp::write(satp(&proc.page_table)) };
    riscv::asm::sfence_vma_all();

    unsafe { riscv::register::sepc::write(proc.pc) }

    user.set_process(proc);
}

pub fn return_to_userspace(registers: &RiscvRegisters) -> ! {
    enable_user_trap();
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
unsafe extern "C" fn user_trap() -> ! {
    naked_asm!(
        ".align 4",

        "csrrw sp, sscratch, sp",

        "add sp, sp, -8 * 32",
        "sd ra, 8 * 0(sp)",

        "csrr ra, sscratch",
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

        "add ra, sp, 8 * 32",
        "csrw sscratch, ra",

        "call {enable_kernel_trap}",

        "mv a0, sp",
        "j {handle_user_trap}",

        enable_kernel_trap = sym enable_kernel_trap,
        handle_user_trap = sym on_user_trap,
    )
}

extern "C" fn enable_kernel_trap() {
    let address = on_kernel_trap as *const () as usize;
    unsafe { riscv::register::stvec::write(Stvec::new(address, TrapMode::Direct)) }
}

fn enable_user_trap() {
    let address = user_trap as *const () as usize;
    unsafe { riscv::register::stvec::write(Stvec::new(address, TrapMode::Direct)) }
}

pub fn is_page_fault(r: riscv::result::Result<Trap<Interrupt, Exception>>) -> bool {
    matches!(
        r,
        Ok(Trap::Exception(
            Exception::LoadPageFault | Exception::StorePageFault | Exception::InstructionPageFault
        ))
    )
}
