pub mod spawner;

use crate::arch::{RiscvRegisters, switch_to_user};
use crate::buddy::BuddyAllocator;
use crate::capability::{capability_certificate, capability_pages_physical_address};
use crate::device_tree::timebase_frequency;
use crate::elf::load_elf;
use crate::heap::BuddyHeap;
use crate::heap::granularity::{PageGranular, page_granular_vec};
use crate::page::{PageFlags, PageTable, map_hh_direct_mapping, map_kernel_image, virt_to_phys};
use crate::shutdown::shutdown;
use crate::stack::UserCtx;
use crate::sync::{Mutex, MutexGuard};
use crate::user::UserPtr;
use crate::util::untyped_box::UntypedBox;
use crate::virtual_memory::VirtualMemoryRawMapping;
use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::num::NonZeroUsize;
use core::ops::Range;
use core::sync::atomic::{AtomicU16, Ordering};
use deravel_types::memory::{USER_CAPABILITIES, USER_HEAP, USER_INPUTS, USER_STACK};
use deravel_types::*;
use log::*;

pub macro kill {
    ($user:ident, $proc:expr, $($tt:tt)*) => {
        {
            let mut proc = $proc;
            let pid = proc.id;
            let name = proc.name;
            error!("killed {name}{pid:?}, {}", format_args!($($tt)*));
            proc.state = ProcessState::Finished;
            drop(proc);
            crate::schedule_and_switch_to_userspace($user)
        }
    },
    ($user:ident, $($tt:tt)*) => {
        kill!($user, $user.process(), $($tt)*)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ProcessState {
    Runnable,
    Finished,
    WaitingForReply {
        from: Actor,
        result_buffer: UserPtr<[u8]>,
    },
    WaitingForStreamMap {
        from: Actor,
    },
    ReadyReply {
        reply: Vec<u8>,
        result_buffer: UserPtr<[u8]>,
    },
    ReadyStreamMap {
        ring: RawCapability,
        declared_size: usize,
    },
}

pub struct Process {
    pub id: ProcessId,
    pub name: &'static str,
    pub state: ProcessState,
    // TODO: This gets overwritten on user trap, should have a wrapper type.
    pub registers: RiscvRegisters,
    pub pc: usize,
    pub page_table: Box<PageTable>,
    pub heap: BuddyAllocator,
    pub messages: VecDeque<Message, BuddyHeap>,
    pub currently_serving: Option<ProcessId>,
    allocated: Vec<(usize, Arc<UntypedBox<PageGranular>>)>,
    pub virtual_memory_mappings: Vec<(Range<usize>, &'static (dyn VirtualMemoryRawMapping + Sync))>,
}

pub struct ProcessReservation<T: ProcessTag> {
    id: ProcessId,
    elf: &'static [u8],
    pub export: Capability<T::Export>,
}

pub struct Message {
    pub cap: RawCapability,
    pub method: usize,
    pub args: Vec<u8>,
    pub sender: ProcessId,
}

pub const PROCESS_COUNT: usize = 8;

static PROCESSES: [Mutex<Option<Process>>; PROCESS_COUNT] = [const { Mutex::new(None) }; _];
static PROCESSES_RESERVED: AtomicU16 = AtomicU16::new(0);

impl Process {
    pub fn alloc(&mut self, backing: Arc<UntypedBox<PageGranular>>, flags: PageFlags) -> *mut u8 {
        let virt = self.heap.alloc(backing.layout()).unwrap();
        self.alloc_at(virt, backing, flags);
        virt as *mut u8
    }

    pub fn alloc_at(
        &mut self,
        virt: usize,
        backing: Arc<UntypedBox<PageGranular>>,
        flags: PageFlags,
    ) {
        let size = backing.layout().size();
        let phys = virt_to_phys(backing.as_untyped_ptr().addr());
        self.page_table.map(virt, phys, size, flags);
        self.allocated.push((virt, backing));
    }

    pub fn dealloc(&mut self, ptr: *mut u8) -> Result<(), ()> {
        let slot = self
            .allocated
            .iter()
            .enumerate()
            .find(|a| a.1.0 == ptr as usize)
            .ok_or(())?;
        let (virt, backing) = self.allocated.swap_remove(slot.0);
        self.page_table.unmap(
            virt,
            virt_to_phys(backing.as_untyped_ptr()) as usize,
            backing.byte_size(),
        );
        self.heap.dealloc(virt, backing.layout());
        Ok(())
    }
}

impl<T: ProcessTag> ProcessReservation<T> {
    pub fn spawn(self, args: T::Args) {
        args.for_all(|cap: RawCapability| {
            capability_certificate(cap).store(
                CapabilityCertificateValue::granted(self.id),
                Ordering::Relaxed,
            )
        });

        create_process::<T>(
            T::NAME,
            self.elf,
            ProcessInputs {
                id: self.id,
                riscv_timebase_frequency: timebase_frequency().map(NonZeroUsize::get),
                args,
            },
        )
    }

    fn spawn_with_ready_caps(self, args: T::Args) {
        create_process::<T>(
            T::NAME,
            self.elf,
            ProcessInputs {
                id: self.id,
                riscv_timebase_frequency: timebase_frequency().map(NonZeroUsize::get),
                args,
            },
        )
    }
}

pub fn get_process(pid: ProcessId) -> &'static Mutex<Option<Process>> {
    &PROCESSES[pid.as_u16() as usize]
}

pub fn reserve_process<T: ProcessTag>(elf: &'static [u8]) -> ProcessReservation<T> {
    let pid = ProcessId::new(PROCESSES_RESERVED.fetch_add(1, Ordering::Relaxed) + 1);
    ProcessReservation {
        id: pid,
        elf,
        export: unsafe { Capability::new(RawCapability::new(pid, 0)) },
    }
}

fn create_process<T: ProcessTag>(name: &'static str, elf: &[u8], inputs: ProcessInputs<T>) {
    let mut proc = Process {
        id: inputs.id,
        name,
        state: ProcessState::Runnable,
        registers: RiscvRegisters {
            sp: USER_STACK.end,
            ..RiscvRegisters::default()
        },
        pc: 0,
        page_table: Box::new(PageTable::new()),
        heap: BuddyAllocator::new(USER_HEAP),
        messages: VecDeque::new_in(BuddyHeap),
        currently_serving: None,
        allocated: Vec::new(),
        virtual_memory_mappings: Vec::new(),
    };
    map_hh_direct_mapping(&mut proc.page_table);
    map_kernel_image(&mut proc.page_table);
    load_elf(elf, &mut proc);
    map_capability_memory(&mut proc.page_table, proc.id);
    map_inputs_memory(&mut proc, inputs);
    map_user_stack(&mut proc);

    let pid = proc.id;
    *get_process(pid).lock() = Some(proc);
}

fn map_capability_memory(table: &mut PageTable, pid: ProcessId) {
    let pre_v = USER_CAPABILITIES.start;
    let pre_p = capability_pages_physical_address();
    let own_v = pre_v + pid.as_u16() as usize * PAGE_SIZE;
    let own_p = pre_p + pid.as_u16() as usize * PAGE_SIZE;
    let suf_v = own_v + PAGE_SIZE;
    let suf_p = own_p + PAGE_SIZE;
    let suf_l = PROCESS_COUNT - pid.as_u16() as usize - 1;
    table.map(
        pre_v,
        pre_p,
        (pid.as_u16() as usize) * PAGE_SIZE,
        PageFlags::readonly().user(),
    );
    table.map(own_v, own_p, PAGE_SIZE, PageFlags::read_write().user());
    table.map(
        suf_v,
        suf_p,
        suf_l * PAGE_SIZE,
        PageFlags::readonly().user(),
    );
}

fn map_inputs_memory<T: ProcessTag>(proc: &mut Process, inputs: ProcessInputs<T>) {
    let size = USER_INPUTS.end - USER_INPUTS.start;
    assert!(size_of::<ProcessInputs<T>>() <= size);
    let inputs = Arc::new(UntypedBox::new(Box::new_in(inputs, PageGranular::new())));
    proc.alloc_at(USER_INPUTS.start, inputs, PageFlags::readonly().user());
}

fn map_user_stack(proc: &mut Process) {
    let stack_size = USER_STACK.end - USER_STACK.start;
    let pages = Arc::new(UntypedBox::new(
        page_granular_vec![0u8; stack_size].into_boxed_slice(),
    ));
    proc.alloc_at(USER_STACK.start, pages, PageFlags::read_write().user());
}

pub fn schedule_and_switch_to_userspace(user: &mut UserCtx) -> ! {
    let Some(mut next) = find_runnable_process(Some(user)) else {
        shutdown()
    };
    user.set_process(&mut next);
    let Err(err) = switch_to_user(next);
    // TODO: Refactor recursion away.
    kill!(user, "{err}");
}

fn find_runnable_process(user: Option<&UserCtx>) -> Option<MutexGuard<'static, Process>> {
    let scan_start = match user {
        Some(hart) => hart.pid().as_u16() + 1,
        None => 0,
    };

    for scan_offset in 0..PROCESS_COUNT as u16 {
        let scan_index = (scan_start + scan_offset) % PROCESS_COUNT as u16;
        if let Some(mut proc) = PROCESSES[scan_index as usize].lock_if_some() {
            inspect_can_progress(&mut proc);
            if matches!(
                proc.state,
                ProcessState::Runnable
                    | ProcessState::ReadyReply { .. }
                    | ProcessState::ReadyStreamMap { .. }
            ) {
                return Some(proc);
            }
            if proc.state == ProcessState::Finished {
                drop(proc);
                // TODO: Race condition here.
                *PROCESSES[scan_index as usize].lock() = None;
            }
        }
    }

    None
}

fn inspect_can_progress(proc: &mut Process) {
    if let ProcessState::WaitingForReply { from, .. } | ProcessState::WaitingForStreamMap { from } =
        &proc.state
        && let Actor::Userspace(from) = from
    {
        let from = get_process(*from).lock_if_some().unwrap();
        if matches!(from.state, ProcessState::Finished) {
            proc.state = ProcessState::Finished;
            warn!(
                "stopping {}{:?} waiting on finished {}{:?}",
                proc.name, proc.id, from.name, from.id
            );
        }
    }
}
