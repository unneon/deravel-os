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

mod elf;
mod heap;
mod log;
mod sbi;
mod virtio;

use crate::elf::load_elf;
use crate::heap::alloc_page;
use crate::log::initialize_log;
use crate::sbi::{ResetReason, ResetType, log_sbi_metadata};
use crate::virtio::initialize_all_virtio_mmio;
use ::log::{error, info};
use core::arch::{asm, naked_asm};
use core::mem::transmute;
use core::panic::PanicInfo;
use fdt::Fdt;
use riscv::interrupt::Trap;
use riscv::interrupt::supervisor::{Exception, Interrupt};
use riscv::register::satp::{Mode, Satp};
use riscv::register::stvec::{Stvec, TrapMode};

unsafe extern "C" {
    static mut kernel_base: u8;
    static mut bss_start: u8;
    static mut bss_end: u8;
    static mut stack_top: u8;
    static mut heap_start: u8;
    static mut heap_end: u8;
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

const HELLO_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_DERAVEL_HELLO"));

fn main(_hart_id: u64, device_tree: *const u8) -> ! {
    clear_bss();

    let device_tree = unsafe { Fdt::from_ptr(device_tree) }.unwrap();
    initialize_log(&device_tree);
    initialize_trap_handler();
    log_sbi_metadata();
    initialize_all_virtio_mmio(&device_tree);

    create_process(HELLO_ELF);

    yield_();
    unreachable!()
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ProcessState {
    Unused,
    Runnable,
    Finished,
}

#[derive(Clone, Copy)]
struct Process {
    pid: u32,
    state: ProcessState,
    sp: usize,
    page_table: *const PageTable,
    stack: [u8; 8192],
}

struct PageTable([usize; 4096 / 8]);

const PROCESS_COUNT: usize = 8;
const PAGE_V: usize = 1 << 0;
const PAGE_R: usize = 1 << 1;
const PAGE_W: usize = 1 << 2;
const PAGE_X: usize = 1 << 3;
const PAGE_U: usize = 1 << 4;

static mut PROCESSES: [Process; PROCESS_COUNT] = unsafe { core::mem::zeroed() };
static mut CURRENT_PROC: Option<usize> = None;

fn user_entry() -> ! {
    let mut sstatus = riscv::register::sstatus::read();
    sstatus.set_spie(true);

    unsafe { riscv::register::sepc::write(0x1000000) };
    unsafe { riscv::register::sstatus::write(sstatus) };
    unsafe { asm!("sret", options(noreturn)) }
}

fn map_page(table2: &mut PageTable, virtual_addr: usize, physical_addr: usize, flags: usize) {
    assert!(virtual_addr.is_multiple_of(4096));
    assert!(physical_addr.is_multiple_of(4096));

    let vpn2 = (virtual_addr >> 30) & ((1 << 9) - 1);
    if table2.0[vpn2] & PAGE_V == 0 {
        let table1 = alloc_page();
        table2.0[vpn2] = (((table1 as *mut _ as usize) / 4096) << 10) | PAGE_V;
    }

    let table1 = unsafe { &mut *(((table2.0[vpn2] >> 10) * 4096) as *mut PageTable) };
    let vpn1 = (virtual_addr >> 21) & ((1 << 9) - 1);
    if table1.0[vpn1] & PAGE_V == 0 {
        let table0 = alloc_page();
        table1.0[vpn1] = (((table0 as *mut _ as usize) / 4096) << 10) | PAGE_V;
    }

    let table0 = unsafe { &mut *(((table1.0[vpn1] >> 10) * 4096) as *mut PageTable) };
    let vpn0 = (virtual_addr >> 12) & ((1 << 9) - 1);
    assert!(table0.0[vpn0] & PAGE_V == 0);

    table0.0[vpn0] = ((physical_addr / PAGE_SIZE) << 10) | PAGE_V | flags;
}

fn create_process(elf: &[u8]) -> *mut Process {
    let (i, proc) = unsafe {
        PROCESSES
            .iter_mut()
            .enumerate()
            .find(|(_, proc)| proc.state == ProcessState::Unused)
    }
    .unwrap();
    let stack_size = proc.stack.len();
    unsafe {
        *(proc.stack.as_ptr().byte_add(stack_size).byte_sub(8 * 13) as *mut usize) =
            user_entry as *const () as usize
    };

    let page_table =
        unsafe { transmute::<&'static mut [u8; 4096], &'static mut PageTable>(alloc_page()) };
    let mut physical_addr = (&raw const kernel_base) as usize;
    while physical_addr < (&raw const heap_end) as usize {
        map_page(
            page_table,
            physical_addr,
            physical_addr,
            PAGE_R | PAGE_W | PAGE_X,
        );
        physical_addr += 4096;
    }

    load_elf(elf, page_table);

    proc.pid = i as u32 + 1;
    proc.state = ProcessState::Runnable;
    proc.sp = (proc.stack.as_ptr() as usize) + stack_size - 8 * 13;
    proc.page_table = page_table;
    proc
}

fn yield_() {
    let Some(next_index) = find_runnable_process() else {
        info!("shutting down due to all processes finishing");
        sbi::system_reset(ResetType::Shutdown, ResetReason::NoReason).unwrap()
    };

    if Some(next_index) == unsafe { CURRENT_PROC } {
        return;
    }

    let next = unsafe { &mut PROCESSES[next_index] };
    let prev_sp = match unsafe { CURRENT_PROC } {
        Some(current) => unsafe { &raw mut PROCESSES[current].sp },
        None => {
            static mut DUMMY_SP: usize = 0;
            &raw mut DUMMY_SP
        }
    };

    let mut satp = Satp::from_bits(0);
    satp.set_ppn(next.page_table as usize / 4096);
    satp.set_mode(Mode::Sv39);

    riscv::asm::sfence_vma_all();
    unsafe { riscv::register::satp::write(satp) };
    unsafe { riscv::register::sscratch::write(next.stack.as_ptr().add(8192) as usize) };

    unsafe { CURRENT_PROC = Some(next_index) };
    unsafe { switch_context(prev_sp, &next.sp) }
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

#[unsafe(naked)]
unsafe extern "C" fn switch_context(prev_sp: *mut usize, next_sp: *const usize) {
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

#[repr(C)]
struct TrapFrame {
    ra: usize,
    gp: usize,
    tp: usize,
    t0: usize,
    t1: usize,
    t2: usize,
    t3: usize,
    t4: usize,
    t5: usize,
    t6: usize,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
    a6: usize,
    a7: usize,
    s0: usize,
    s1: usize,
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
    sp: usize,
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

        "ld ra, 8 * 0(sp)",
        "ld gp, 8 * 1(sp)",
        "ld tp, 8 * 2(sp)",
        "ld t0, 8 * 3(sp)",
        "ld t1, 8 * 4(sp)",
        "ld t2, 8 * 5(sp)",
        "ld t3, 8 * 6(sp)",
        "ld t4, 8 * 7(sp)",
        "ld t5, 8 * 8(sp)",
        "ld t6, 8 * 9(sp)",
        "ld a0, 8 * 10(sp)",
        "ld a1, 8 * 11(sp)",
        "ld a2, 8 * 12(sp)",
        "ld a3, 8 * 13(sp)",
        "ld a4, 8 * 14(sp)",
        "ld a5, 8 * 15(sp)",
        "ld a6, 8 * 16(sp)",
        "ld a7, 8 * 17(sp)",
        "ld s0, 8 * 18(sp)",
        "ld s1, 8 * 19(sp)",
        "ld s2, 8 * 20(sp)",
        "ld s3, 8 * 21(sp)",
        "ld s4, 8 * 22(sp)",
        "ld s5, 8 * 23(sp)",
        "ld s6, 8 * 24(sp)",
        "ld s7, 8 * 25(sp)",
        "ld s8, 8 * 26(sp)",
        "ld s9, 8 * 27(sp)",
        "ld s10, 8 * 28(sp)",
        "ld s11, 8 * 29(sp)",
        "ld sp, 8 * 30(sp)",
        "sret",

        handle_trap = sym handle_trap,
    )
}

fn handle_trap(trap_frame: &TrapFrame) {
    let scause = riscv::register::scause::read()
        .cause()
        .try_into::<Interrupt, Exception>()
        .unwrap();
    let stval = riscv::register::stval::read();
    let mut user_pc = riscv::register::sepc::read();
    if scause == Trap::Exception(Exception::UserEnvCall) {
        handle_syscall(trap_frame);
        user_pc += 4;
    } else {
        panic!("unexpected trap scause={scause:?} stval={stval:#x} user_pc={user_pc:#x}");
    }
    unsafe { riscv::register::sepc::write(user_pc) };
}

fn handle_syscall(trap_frame: &TrapFrame) {
    match trap_frame.a3 {
        1 => {
            unsafe { PROCESSES[CURRENT_PROC.unwrap()].state = ProcessState::Finished };
            yield_();
        }
        2 => {
            let ch = trap_frame.a0 as u8;
            sbi::debug_console_write_byte(ch).unwrap();
        }
        _ => panic!("invalid syscall number {}", trap_frame.a3),
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let location = info.location().unwrap();
    let message = info.message();
    error!("panicked at {location}: {message}");
    let _ = sbi::system_reset(ResetType::Shutdown, ResetReason::SystemFailure);
    loop {}
}
