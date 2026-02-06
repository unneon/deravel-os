#![feature(abi_riscv_interrupt)]
#![feature(adt_const_params)]
#![feature(arbitrary_self_types)]
#![feature(decl_macro)]
#![feature(exact_div)]
#![feature(iter_array_chunks)]
#![feature(macro_metavar_expr_concat)]
#![feature(never_type)]
#![feature(slice_from_ptr_range)]
#![allow(static_mut_refs)]
#![no_std]
#![no_main]

mod log;
mod sbi;
mod virtio;

use crate::log::initialize_log;
use crate::sbi::{ResetReason, ResetType, log_sbi_metadata};
use crate::virtio::initialize_all_virtio_mmio;
use ::log::{debug, error, info};
use core::arch::naked_asm;
use core::panic::PanicInfo;
use fdt::Fdt;
use riscv::interrupt::supervisor::{Exception, Interrupt};
use riscv::register::stvec::{Stvec, TrapMode};

unsafe extern "C" {
    static mut bss_start: u8;
    static mut bss_end: u8;
    static mut stack_top: u8;
}

pub const PAGE_SIZE: usize = 4096;

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

fn main(_hart_id: u64, device_tree: *const u8) -> ! {
    clear_bss();

    let device_tree = unsafe { Fdt::from_ptr(device_tree) }.unwrap();
    initialize_log(&device_tree);
    initialize_trap_handler();
    log_sbi_metadata();
    initialize_all_virtio_mmio(&device_tree);

    unsafe { IDLE_PROC = create_process(0) };
    unsafe { (*IDLE_PROC).pid = 0 };
    unsafe { CURRENT_PROC = IDLE_PROC };
    unsafe { PROC_A = create_process(proc_a_entry as *const () as usize) };
    unsafe { PROC_B = create_process(proc_b_entry as *const () as usize) };

    yield_();
    panic!("switched to idle process");
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ProcessState {
    Unused,
    Runnable,
}

#[derive(Clone, Copy)]
struct Process {
    pid: u32,
    state: ProcessState,
    sp: usize,
    stack: [u8; 8192],
}

const PROCESS_COUNT: usize = 8;
static mut PROCESSES: [Process; PROCESS_COUNT] = unsafe { core::mem::zeroed() };
static mut PROC_A: *mut Process = core::ptr::null_mut();
static mut PROC_B: *mut Process = core::ptr::null_mut();
static mut CURRENT_PROC: *mut Process = core::ptr::null_mut();
static mut IDLE_PROC: *mut Process = core::ptr::null_mut();

fn create_process(pc: usize) -> *mut Process {
    let (i, proc) = unsafe {
        PROCESSES
            .iter_mut()
            .enumerate()
            .find(|(_, proc)| proc.state == ProcessState::Unused)
    }
    .unwrap();
    let stack_size = proc.stack.len();
    unsafe { *(proc.stack.as_ptr().byte_add(stack_size).byte_sub(8 * 13) as *mut usize) = pc };
    proc.pid = i as u32 + 1;
    proc.state = ProcessState::Runnable;
    proc.sp = (proc.stack.as_ptr() as usize) + stack_size - 8 * 13;
    proc
}

fn delay() {
    for _ in 0..1_000_000_000 {
        riscv::asm::nop();
    }
}

fn proc_a_entry() {
    info!("starting process A");
    loop {
        debug!("A");
        yield_();
        delay();
    }
}

fn proc_b_entry() {
    info!("starting process B");
    loop {
        debug!("B");
        yield_();
        delay();
    }
}

fn yield_() {
    let current_proc = unsafe { &*CURRENT_PROC };
    let mut next = unsafe { IDLE_PROC };
    for i in 0..PROCESS_COUNT {
        let proc = unsafe { &PROCESSES[(current_proc.pid as usize + i) % PROCESS_COUNT] };
        if proc.state == ProcessState::Runnable && proc.pid > 0 {
            next = proc as *const Process as *mut Process;
            break;
        }
    }

    if next == unsafe { CURRENT_PROC } {
        return;
    }

    unsafe { riscv::register::sscratch::write((*next).stack.as_ptr().add(8192) as usize) };

    let prev = unsafe { CURRENT_PROC };
    unsafe { CURRENT_PROC = next };
    unsafe { switch_context(&mut (*prev).sp, &mut (*next).sp) }
}

#[unsafe(naked)]
unsafe extern "C" fn switch_context(prev_sp: *mut usize, next_sp: *mut usize) {
    naked_asm!(
        "addi sp, sp, -13 * 8",
        "sd ra, 0 * 8(sp)",
        "sd s0, 1 * 8(sp)",
        "sd s1, 2 * 8(sp)",
        "sd s2, 3 * 8(sp)",
        "sd s3, 4 * 8(sp)",
        "sd s4, 5 * 8(sp)",
        "sd s5, 6 * 8(sp)",
        "sd s6, 7 * 8(sp)",
        "sd s7, 8 * 8(sp)",
        "sd s8, 9 * 8(sp)",
        "sd s9, 10 * 8(sp)",
        "sd s10, 11 * 8(sp)",
        "sd s11, 12 * 8(sp)",
        "sd sp, (a0)",
        "ld sp, (a1)",
        "ld ra, 0 * 8(sp)",
        "ld s0, 1 * 8(sp)",
        "ld s1, 2 * 8(sp)",
        "ld s2, 3 * 8(sp)",
        "ld s3, 4 * 8(sp)",
        "ld s4, 5 * 8(sp)",
        "ld s5, 6 * 8(sp)",
        "ld s6, 7 * 8(sp)",
        "ld s7, 8 * 8(sp)",
        "ld s8, 9 * 8(sp)",
        "ld s9, 10 * 8(sp)",
        "ld s10, 11 * 8(sp)",
        "ld s11, 12 * 8(sp)",
        "addi sp, sp, 13 * 8",
        "ret",
    )
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
unsafe extern "C" fn trap_entry() {
    naked_asm!(
        ".align 4",

        "csrrw sp, sscratch, sp",

        "addi sp, sp, -8 * 31",
        "sd ra, 8 * 0(sp)",
        "sd gp, 8 * 1(sp)",
        "sd tp, 8 * 2(sp)",
        "sd t0, 8 * 3(sp)",
        "sd t1, 8 * 4(sp)",
        "sd t2, 8 * 5(sp)",
        "sd t3, 8 * 6(sp)",
        "sd t4, 8 * 7(sp)",
        "sd t5, 8 * 8(sp)",
        "sd t6, 8 * 9(sp)",
        "sd a0, 8 * 10(sp)",
        "sd a1, 8 * 11(sp)",
        "sd a2, 8 * 12(sp)",
        "sd a3, 8 * 13(sp)",
        "sd a4, 8 * 14(sp)",
        "sd a5, 8 * 15(sp)",
        "sd a6, 8 * 16(sp)",
        "sd a7, 8 * 17(sp)",
        "sd s0, 8 * 18(sp)",
        "sd s1, 8 * 19(sp)",
        "sd s2, 8 * 20(sp)",
        "sd s3, 8 * 21(sp)",
        "sd s4, 8 * 22(sp)",
        "sd s5, 8 * 23(sp)",
        "sd s6, 8 * 24(sp)",
        "sd s7, 8 * 25(sp)",
        "sd s8, 8 * 26(sp)",
        "sd s9, 8 * 27(sp)",
        "sd s10, 8 * 28(sp)",
        "sd s11, 8 * 29(sp)",

        "csrr a0, sscratch",
        "sd a0, 8 * 30(sp)",

        "addi a0, sp, 8 * 31",
        "csrw sscratch, a0",

        "mv a0, sp",
        "call {handle_trap}",

        handle_trap = sym handle_trap,
    )
}

fn handle_trap() {
    let scause = riscv::register::scause::read()
        .cause()
        .try_into::<Interrupt, Exception>()
        .unwrap();
    let stval = riscv::register::stval::read();
    let user_pc = riscv::register::sepc::read();
    panic!("unexpected trap scause={scause:?} stval={stval:#x} user_pc={user_pc:#x}");
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    error!("{}", info);
    let _ = sbi::system_reset(ResetType::Shutdown, ResetReason::SystemFailure);
    loop {}
}
