use crate::arch::{RiscvRegisters, switch_to_userspace_full};
use crate::elf::load_elf;
use crate::heap::log_heap_statistics;
use crate::page::{PAGE_SIZE, PageAligned, PageFlags, PageTable, map_pages};
use crate::sbi;
use crate::sbi::{ResetReason, ResetType};
use alloc::boxed::Box;
use log::{error, trace};
use riscv::register::satp::{Mode, Satp};

pub macro create_process($name:literal) {{
    const ELF: PageAligned<
        [u8; include_bytes!(env!(concat!("CARGO_BIN_FILE_DERAVEL_APPS_", $name))).len()],
    > = PageAligned(*include_bytes!(env!(concat!(
        "CARGO_BIN_FILE_DERAVEL_APPS_",
        $name
    ))));
    create_process($name, &ELF.0)
}}

#[derive(Clone, Copy)]
pub struct Capability(#[allow(dead_code)] usize);

#[repr(C, align(4096))]
#[derive(Clone, Copy)]
pub struct CapabilityPage([Capability; PAGE_SIZE / size_of::<Capability>()]);

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum ProcessState {
    Unused,
    Runnable,
    Finished,
}

pub struct Process {
    pub name: Option<&'static str>,
    pub state: ProcessState,
    pub registers: RiscvRegisters,
    pub pc: usize,
    pub page_table: *const PageTable,
    pub message: Option<(Box<[u8]>, usize)>,
}

const CAPABILITY_START: usize = 0x2000000;
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
pub static mut CAPABILITY_PAGES: [CapabilityPage; PROCESS_COUNT] =
    [CapabilityPage([Capability(0); PAGE_SIZE / size_of::<Capability>()]); PROCESS_COUNT];

impl Process {
    pub fn satp(&self) -> Satp {
        let mut satp = Satp::from_bits(0);
        satp.set_ppn(self.page_table as usize / PAGE_SIZE);
        satp.set_mode(Mode::Sv39);
        satp
    }
}

pub fn create_process(name: &'static str, elf: &[u8]) {
    let Some(pid) = find_free_process_slot() else {
        error!("exhausted all process slots");
        return;
    };

    let mut page_table = Box::new(PageTable::new());
    map_kernel_memory(&mut page_table);
    let entry_point = load_elf(elf, &mut page_table);
    map_capability_memory(&mut page_table, pid);

    let proc = unsafe { &mut PROCESSES[pid] };
    proc.name = Some(name);
    proc.state = ProcessState::Runnable;
    proc.registers.a0 = pid;
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

fn map_capability_memory(pages: &mut PageTable, pid: usize) {
    let pre_v = CAPABILITY_START;
    let pre_p = &raw const CAPABILITY_PAGES as usize;
    let own_v = pre_v + pid * PAGE_SIZE;
    let own_p = pre_p + pid * PAGE_SIZE;
    let suf_v = own_v + PAGE_SIZE;
    let suf_p = own_p + PAGE_SIZE;
    let suf_l = PROCESS_COUNT - pid - 1;
    map_pages(pages, pre_v, pre_p, PageFlags::readonly().user(), pid);
    map_pages(pages, own_v, own_p, PageFlags::readwrite().user(), 1);
    map_pages(pages, suf_v, suf_p, PageFlags::readonly().user(), suf_l);
}

fn find_free_process_slot() -> Option<usize> {
    for (i, process) in unsafe { PROCESSES.iter_mut().enumerate() } {
        if process.state == ProcessState::Unused {
            return Some(i);
        }
    }
    None
}

pub fn schedule_and_switch_to_userspace() -> ! {
    let prev_pid = unsafe { CURRENT_PROC };
    let Some(next_pid) = find_runnable_process() else {
        log_heap_statistics();
        sbi::system_reset(ResetType::Shutdown, ResetReason::NoReason).unwrap()
    };
    let next = unsafe { &PROCESSES[next_pid] };
    unsafe { CURRENT_PROC = Some(next_pid) }

    if let Some(prev_pid) = prev_pid {
        trace!("switching from {prev_pid} to {next_pid}");
    } else {
        trace!("switching to {next_pid}");
    }

    switch_to_userspace_full(next);
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
