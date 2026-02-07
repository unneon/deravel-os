#![feature(abi_riscv_interrupt)]
#![feature(arbitrary_self_types)]
#![feature(decl_macro)]
#![feature(iter_array_chunks)]
#![feature(never_type)]
#![feature(slice_from_ptr_range)]
#![allow(static_mut_refs)]
#![no_std]
#![no_main]

extern crate alloc;

mod elf;
mod heap;
mod log;
mod page;
mod sbi;
mod virtio;

use crate::elf::load_elf;
use crate::log::initialize_log;
use crate::page::{PAGE_R, PAGE_SIZE, PAGE_W, PAGE_X, PageTable, map_pages};
use crate::sbi::{ResetReason, ResetType, log_sbi_metadata};
use crate::virtio::initialize_all_virtio_mmio;
use ::log::{error, info};
use alloc::boxed::Box;
use core::arch::{asm, naked_asm};
use core::panic::PanicInfo;
use fdt::Fdt;
use riscv::interrupt::Trap;
use riscv::interrupt::supervisor::{Exception, Interrupt};
use riscv::register::satp::{Mode, Satp};
use riscv::register::stvec::{Stvec, TrapMode};

unsafe extern "C" {
    static mut kernel_start: u8;
    static mut bss_start: u8;
    static mut bss_end: u8;
    static mut stack_top: u8;
    static mut heap_start: u8;
    static mut heap_end: u8;
    static mut kernel_end: u8;
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

const HELLO_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_DERAVEL_HELLO"));

fn main(_hart_id: u64, device_tree: *const u8) -> ! {
    clear_bss();

    let device_tree = unsafe { Fdt::from_ptr(device_tree) }.unwrap();
    initialize_log(&device_tree);
    initialize_trap_handler();
    log_sbi_metadata();
    initialize_all_virtio_mmio(&device_tree);

    create_process(HELLO_ELF);

    switch_to_scheduled_userspace();
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ProcessState {
    Unused,
    Runnable,
    Finished,
}

struct Process {
    state: ProcessState,
    registers: RiscvRegisters,
    pc: usize,
    page_table: *const PageTable,
}

const PROCESS_COUNT: usize = 8;

static mut PROCESSES: [Process; PROCESS_COUNT] = unsafe { core::mem::zeroed() };
static mut CURRENT_PROC: Option<usize> = None;

fn create_process(elf: &[u8]) {
    let pid = find_free_process_slot().unwrap();

    let mut page_table = Box::new(PageTable::default());
    map_kernel_memory(&mut page_table);
    load_elf(elf, &mut page_table);

    let proc = unsafe { &mut PROCESSES[pid] };
    proc.state = ProcessState::Runnable;
    proc.pc = 0x1000000;
    proc.page_table = Box::leak(page_table);
}

fn find_free_process_slot() -> Option<usize> {
    for (i, process) in unsafe { PROCESSES.iter_mut().enumerate() } {
        if process.state == ProcessState::Unused {
            return Some(i);
        }
    }
    None
}

fn map_kernel_memory(page_table: &mut PageTable) {
    let kernel_physical_address = (&raw const kernel_start) as usize;
    let kernel_page_count =
        ((&raw const kernel_end as usize) - (&raw const kernel_start as usize)).div_ceil(PAGE_SIZE);
    map_pages(
        page_table,
        kernel_physical_address,
        kernel_physical_address,
        PAGE_R | PAGE_W | PAGE_X,
        kernel_page_count,
    );
}

fn switch_to_scheduled_userspace() -> ! {
    let Some(next_pid) = find_runnable_process() else {
        info!("shutting down due to all processes finishing");
        sbi::system_reset(ResetType::Shutdown, ResetReason::NoReason).unwrap()
    };

    unsafe { CURRENT_PROC = Some(next_pid) };

    let next = unsafe { &mut PROCESSES[next_pid] };

    let mut satp = Satp::from_bits(0);
    satp.set_ppn(next.page_table as usize / PAGE_SIZE);
    satp.set_mode(Mode::Sv39);

    let mut sstatus = riscv::register::sstatus::read();
    sstatus.set_spie(true);

    riscv::asm::sfence_vma_all();
    unsafe { riscv::register::satp::write(satp) };
    unsafe { riscv::register::sepc::write(next.pc) };
    unsafe { riscv::register::sstatus::write(sstatus) };

    raw_switch_to_userspace(&next.registers)
}

fn raw_switch_to_userspace(registers: &RiscvRegisters) -> ! {
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

fn find_runnable_process() -> Option<usize> {
    let current = unsafe { CURRENT_PROC };
    let scan_start = match current {
        Some(current) => current + 1,
        None => 0,
    };

    for scan_offset in 0..PROCESS_COUNT {
        let scan_index = (scan_start + scan_offset) % PROCESS_COUNT;
        let process = unsafe { &PROCESSES[scan_index] };
        if process.state == ProcessState::Runnable {
            return Some(scan_index);
        }
    }

    None
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RiscvRegisters {
    ra: usize,
    sp: usize,
    gp: usize,
    tp: usize,
    t0: usize,
    t1: usize,
    t2: usize,
    s0: usize,
    s1: usize,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
    a6: usize,
    a7: usize,
    s2: usize,
    s3: usize,
    s4: usize,
    s5: usize,
    s6: usize,
    s7: usize,
    s8: usize,
    s9: usize,
    s10: usize,
    s11: usize,
    t3: usize,
    t4: usize,
    t5: usize,
    t6: usize,
}

fn clear_bss() {
    let bss = unsafe { core::slice::from_mut_ptr_range(&raw mut bss_start..&raw mut bss_end) };
    bss.fill(0);
}

fn initialize_trap_handler() {
    let address = trap_entry as *const () as usize;
    unsafe { riscv::register::stvec::write(Stvec::new(address, TrapMode::Direct)) }
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
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

fn handle_trap(registers: &RiscvRegisters) -> ! {
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

fn handle_syscall(user_pc: usize, registers: &RiscvRegisters) -> ! {
    match registers.a3 {
        1 => {
            unsafe { PROCESSES[CURRENT_PROC.unwrap()].state = ProcessState::Finished };
            switch_to_scheduled_userspace();
        }
        2 => {
            let ch = registers.a0 as u8;
            sbi::debug_console_write_byte(ch).unwrap();
        }
        _ => panic!("invalid syscall number {}", registers.a3),
    }

    unsafe { riscv::register::sepc::write(user_pc + 4) };
    raw_switch_to_userspace(registers);
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let location = info.location().unwrap();
    let message = info.message();
    error!("panicked at {location}: {message}");
    let _ = sbi::system_reset(ResetType::Shutdown, ResetReason::SystemFailure);
    loop {}
}
