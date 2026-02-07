use crate::arch::RiscvRegisters;
use crate::elf::load_elf;
use crate::page::{PAGE_SIZE, PageFlags, PageTable, map_pages};
use alloc::boxed::Box;
use riscv::register::satp::{Mode, Satp};

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum ProcessState {
    Unused,
    Runnable,
    Finished,
}

pub struct Process {
    pub state: ProcessState,
    pub registers: RiscvRegisters,
    pub pc: usize,
    pub page_table: *const PageTable,
}

const PROCESS_COUNT: usize = 8;

unsafe extern "C" {
    static text_start: u8;
    static text_end: u8;
    static rodata_start: u8;
    static rodata_end: u8;
    static readwrite_start: u8;
    static readwrite_end: u8;
}

pub static mut PROCESSES: [Process; PROCESS_COUNT] = unsafe { core::mem::zeroed() };
pub static mut CURRENT_PROC: Option<usize> = None;

impl Process {
    pub fn satp(&self) -> Satp {
        let mut satp = Satp::from_bits(0);
        satp.set_ppn(self.page_table as usize / PAGE_SIZE);
        satp.set_mode(Mode::Sv39);
        satp
    }
}

pub fn create_process(elf: &[u8]) {
    let pid = find_free_process_slot().unwrap();

    let mut page_table = Box::new(PageTable::default());
    map_kernel_memory(&mut page_table);
    let entry_point = load_elf(elf, &mut page_table);

    let proc = unsafe { &mut PROCESSES[pid] };
    proc.state = ProcessState::Runnable;
    proc.pc = entry_point;
    proc.page_table = Box::leak(page_table);
}

fn map_kernel_memory(page_table: &mut PageTable) {
    map_kernel_memory_section(
        page_table,
        &raw const text_start,
        &raw const text_end,
        PageFlags::executable(),
    );
    map_kernel_memory_section(
        page_table,
        &raw const rodata_start,
        &raw const rodata_end,
        PageFlags::readonly(),
    );
    map_kernel_memory_section(
        page_table,
        &raw const readwrite_start,
        &raw const readwrite_end,
        PageFlags::readwrite(),
    )
}

fn map_kernel_memory_section(
    page_table: &mut PageTable,
    start: *const u8,
    end: *const u8,
    flags: PageFlags,
) {
    let start = start as usize;
    assert!(start.is_multiple_of(PAGE_SIZE));
    let page_count = (end as usize - start).div_ceil(PAGE_SIZE);
    map_pages(page_table, start, start, flags, page_count);
}

fn find_free_process_slot() -> Option<usize> {
    for (i, process) in unsafe { PROCESSES.iter_mut().enumerate() } {
        if process.state == ProcessState::Unused {
            return Some(i);
        }
    }
    None
}

pub fn find_runnable_process() -> Option<usize> {
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
