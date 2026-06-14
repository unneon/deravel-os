use crate::allocators::TrivialAllocator;
use crate::arch::{RiscvRegisters, switch_to_userspace_full};
use crate::capability::{
    capability_page, capability_pages_physical_address, kernel_capability_page,
};
use crate::device_tree::timebase_frequency;
use crate::elf::load_elf;
use crate::hart::HartContext;
use crate::heap::log_heap_statistics;
use crate::page::{PageFlags, PageTable, map_pages};
use crate::sbi;
use crate::sbi::{ResetReason, ResetType};
use crate::sync::Mutex;
use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::sync::atomic::Ordering;
use deravel_types::*;
use riscv::register::satp::{Mode, Satp};

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum ProcessState {
    Unused,
    Runnable,
    Finished,
    WaitingForMessage,
    Reserved,
    WaitingForReply,
    WaitingForStreamMap,
}

pub struct Process {
    pub name: Option<&'static str>,
    pub state: ProcessState,
    pub registers: Option<RiscvRegisters>,
    pub pc: usize,
    pub page_table: *mut PageTable,
    pub virtual_memory: TrivialAllocator,
    pub messages: Option<Box<VecDeque<(RawCapability, usize, Vec<u8>, ProcessId)>>>,
    #[allow(clippy::box_collection)]
    pub reply: Option<Box<Vec<u8>>>,
    pub stream_map: Option<(RawCapability, usize)>,
    pub currently_serving: Option<ProcessId>,
}
unsafe impl Send for Process {}

pub struct ProcessReservation<T: ProcessTag> {
    pub id: ProcessId,
    pub elf: &'static [u8],
    #[allow(dead_code)]
    pub export: Capability<T::Export>,
}

pub const PROCESS_COUNT: usize = 8;

unsafe extern "C" {
    static text_start: u8;
    static text_end: u8;
    static rodata_start: u8;
    static rodata_end: u8;
    static readwrite_start: u8;
    static readwrite_end: u8;
}

static PROCESSES: [Mutex<Process>; PROCESS_COUNT] = unsafe { core::mem::zeroed() };

impl Process {
    pub fn satp(&self) -> Satp {
        let mut satp = Satp::from_bits(0);
        satp.set_ppn(self.page_table as usize / PAGE_SIZE);
        satp.set_mode(Mode::Sv39);
        satp
    }
}

impl<T: ProcessTag> ProcessReservation<T> {
    pub fn spawn(self, args: T::Args) {
        args.for_all(|cap: RawCapability| {
            match cap.certifier() {
                Actor::Userspace(pid) => capability_page(pid),
                Actor::Kernel => kernel_capability_page(),
            }
            .0[cap.local_index()]
            .store(
                CapabilityCertificateValue::granted(self.id),
                Ordering::Relaxed,
            )
        });

        create_process::<T>(
            T::NAME,
            self.elf,
            ProcessInputs {
                common: CommonProcessInputs {
                    id: self.id,
                    riscv_timebase_frequency: timebase_frequency(),
                },
                args,
            },
        )
    }

    pub fn spawn_with_ready_caps(self, args: T::Args) {
        create_process::<T>(
            T::NAME,
            self.elf,
            ProcessInputs {
                common: CommonProcessInputs {
                    id: self.id,
                    riscv_timebase_frequency: timebase_frequency(),
                },
                args,
            },
        )
    }
}

pub fn get_process(pid: ProcessId) -> &'static Mutex<Process> {
    &PROCESSES[pid.as_u16() as usize]
}

pub fn reserve_process<T: ProcessTag>(elf: &'static [u8]) -> ProcessReservation<T> {
    let pid = find_free_process_slot().expect("exhausted all process slots");
    let mut proc = get_process(pid).lock();
    proc.state = ProcessState::Reserved;
    ProcessReservation {
        id: pid,
        elf,
        export: Capability(RawCapability::new(pid, 0), PhantomData),
    }
}

pub fn create_process<T: ProcessTag>(name: &'static str, elf: &[u8], inputs: ProcessInputs<T>) {
    let pid = inputs.common.id;
    let mut page_table = Box::new(PageTable::new());
    map_kernel_memory(&mut page_table);
    let entry_point = load_elf(elf, &mut page_table);
    map_capability_memory(&mut page_table, pid);
    map_inputs_memory(&mut page_table, inputs);

    let mut proc = get_process(pid).lock();
    proc.name = Some(name);
    proc.state = ProcessState::Runnable;
    proc.registers = Some(RiscvRegisters::default());
    proc.pc = entry_point;
    proc.page_table = Box::leak(page_table);
    proc.virtual_memory = TrivialAllocator::new_range(0x4000000, 0x5000000);
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

fn map_capability_memory(pages: &mut PageTable, pid: ProcessId) {
    let pre_v = CAPABILITIES_START;
    let pre_p = capability_pages_physical_address();
    let own_v = pre_v + pid.as_u16() as usize * PAGE_SIZE;
    let own_p = pre_p + pid.as_u16() as usize * PAGE_SIZE;
    let suf_v = own_v + PAGE_SIZE;
    let suf_p = own_p + PAGE_SIZE;
    let suf_l = PROCESS_COUNT - pid.as_u16() as usize - 1;
    map_pages(
        pages,
        pre_v,
        pre_p,
        PageFlags::readonly().user(),
        pid.as_u16() as usize,
    );
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

fn find_free_process_slot() -> Option<ProcessId> {
    for (i, process) in PROCESSES.iter().enumerate().skip(1) {
        if process.lock().state == ProcessState::Unused {
            return Some(ProcessId::new(i as u16));
        }
    }
    None
}

pub fn schedule_and_switch_to_userspace(hart: &mut HartContext) -> ! {
    let Some(next_pid) = find_runnable_process(hart) else {
        log_heap_statistics();
        sbi::system_reset(ResetType::Shutdown, ResetReason::NoReason).unwrap()
    };
    let next = get_process(next_pid).lock();
    hart.set_current_pid(next_pid);

    switch_to_userspace_full(next);
}

pub fn find_runnable_process(hart: &HartContext) -> Option<ProcessId> {
    let scan_start = match hart.try_current_pid() {
        Some(current) => current.as_u16() + 1,
        None => 0,
    };

    for scan_offset in 0..PROCESS_COUNT as u16 {
        let scan_index = (scan_start + scan_offset) % PROCESS_COUNT as u16;
        let process = PROCESSES[scan_index as usize].lock();
        if process.state == ProcessState::Runnable
            || (process.state == ProcessState::WaitingForMessage
                && process.messages.as_ref().is_some_and(|q| !q.is_empty()))
            || (process.state == ProcessState::WaitingForReply && process.reply.is_some())
            || (process.state == ProcessState::WaitingForStreamMap && process.stream_map.is_some())
        {
            return Some(ProcessId::new(scan_index));
        }
    }

    None
}
