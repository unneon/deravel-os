use crate::arch::{RiscvRegisters, switch_to_userspace_full};
use crate::elf::load_elf;
use crate::heap::log_heap_statistics;
use crate::page::{PAGE_SIZE, PageFlags, PageTable, map_pages};
use crate::sbi;
use crate::sbi::{ResetReason, ResetType};
use alloc::boxed::Box;
use alloc::collections::VecDeque;
use core::marker::PhantomData;
use deravel_types::capability::{CAPABILITIES_START, Capability};
use deravel_types::drvli::ProcessTag;
use deravel_types::{INPUTS_ADDRESS, ProcessId, ProcessInputs};
use riscv::register::satp::{Mode, Satp};

pub macro reserve_process($tag:ident, $env:literal) {{
    const ELF: crate::page::PageAligned<[u8; include_bytes!(env!($env)).len()]> =
        crate::page::PageAligned(*include_bytes!(env!($env)));
    reserve_process::<deravel_types::drvli::$tag>(&ELF.0)
}}

#[repr(C, align(4096))]
#[derive(Clone, Copy)]
pub struct CapabilityPage([Capability; PAGE_SIZE / size_of::<Capability>()]);

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum ProcessState {
    Unused,
    Runnable,
    Finished,
    WaitingForMessage,
    Reserved,
    WaitingForReply,
}

pub struct Process {
    pub name: Option<&'static str>,
    pub state: ProcessState,
    pub registers: RiscvRegisters,
    pub pc: usize,
    pub page_table: *const PageTable,
    pub heap_pages_allocated: usize,
    pub messages: Option<Box<VecDeque<(Box<[u8]>, usize)>>>,
}

pub struct ProcessReservation<T: ProcessTag> {
    pub id: ProcessId,
    pub elf: &'static [u8],
    pub export: Capability,
    pub _phantom: PhantomData<T>,
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
pub static mut CAPABILITY_PAGES: [CapabilityPage; PROCESS_COUNT] =
    [CapabilityPage([Capability(core::ptr::null()); PAGE_SIZE / size_of::<Capability>()]);
        PROCESS_COUNT];

impl Process {
    pub fn satp(&self) -> Satp {
        let mut satp = Satp::from_bits(0);
        satp.set_ppn(self.page_table as usize / PAGE_SIZE);
        satp.set_mode(Mode::Sv39);
        satp
    }
}

impl<T: ProcessTag> ProcessReservation<T> {
    pub fn spawn(self, args: T::Capabilities) {
        create_process::<T>(T::NAME, self.elf, ProcessInputs { id: self.id, args })
    }
}

pub fn reserve_process<T: ProcessTag>(elf: &'static [u8]) -> ProcessReservation<T> {
    let pid = find_free_process_slot().expect("exhausted all process slots");
    let proc = unsafe { &mut PROCESSES[pid] };
    proc.state = ProcessState::Reserved;
    ProcessReservation {
        id: ProcessId(pid),
        elf,
        export: Capability::new(ProcessId(pid), 0),
        _phantom: PhantomData,
    }
}

pub fn create_process<T: ProcessTag>(name: &'static str, elf: &[u8], inputs: ProcessInputs<T>) {
    let pid = inputs.id.0;
    let mut page_table = Box::new(PageTable::new());
    map_kernel_memory(&mut page_table);
    let entry_point = load_elf(elf, &mut page_table);
    map_capability_memory(&mut page_table, pid);
    map_inputs_memory(&mut page_table, inputs);

    let proc = unsafe { &mut PROCESSES[pid] };
    proc.name = Some(name);
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

fn map_capability_memory(pages: &mut PageTable, pid: usize) {
    let pre_v = CAPABILITIES_START;
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

fn map_inputs_memory<T: ProcessTag>(pages: &mut PageTable, inputs: ProcessInputs<T>) {
    let page = Box::leak(Box::new(inputs));
    map_pages(
        pages,
        INPUTS_ADDRESS,
        page as *mut _ as usize,
        PageFlags::readonly().user(),
        1,
    );
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
    let Some(next_pid) = find_runnable_process() else {
        log_heap_statistics();
        sbi::system_reset(ResetType::Shutdown, ResetReason::NoReason).unwrap()
    };
    let next = unsafe { &PROCESSES[next_pid] };
    unsafe { CURRENT_PROC = Some(next_pid) }

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
        if process.state == ProcessState::Runnable
            || (process.state == ProcessState::WaitingForMessage
                && process.messages.as_ref().is_some_and(|q| !q.is_empty()))
        {
            return Some(scan_index);
        }
    }

    None
}
