use crate::arch::{RiscvRegisters, switch_to_user};
use crate::buddy::BuddyAllocator;
use crate::capability::{capability_certificate, capability_pages_physical_address};
use crate::device_tree::timebase_frequency;
use crate::elf::load_elf;
use crate::heap::BuddyHeap;
use crate::heap::granularity::PageGranular;
use crate::page::{
    Page, PageFlags, PageTable, TopPageTable, map_direct_mapping, map_kernel_image, map_pages,
    virt_to_phys,
};
use crate::shutdown;
use crate::stack::UserCtx;
use crate::sync::{Mutex, MutexGuard};
use crate::user::UserPtr;
use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::vec;
use alloc::vec::Vec;
use core::num::NonZeroUsize;
use core::sync::atomic::{AtomicU16, Ordering};
use deravel_types::memory::{USER_INPUTS, USER_STACK};
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
    pub page_table: Box<TopPageTable>,
    pub virtual_memory: BuddyAllocator,
    pub messages: VecDeque<Message, BuddyHeap>,
    pub currently_serving: Option<ProcessId>,
}
unsafe impl Send for Process {}

pub struct ProcessReservation<T: ProcessTag> {
    pub id: ProcessId,
    pub elf: &'static [u8],
    #[allow(dead_code)]
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

    pub fn spawn_with_ready_caps(self, args: T::Args) {
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

pub fn create_process<T: ProcessTag>(name: &'static str, elf: &[u8], inputs: ProcessInputs<T>) {
    let pid = inputs.id;
    let mut page_table = Box::new(PageTable::new());
    map_direct_mapping(&mut page_table);
    map_kernel_image(&mut page_table);
    let entry_point = load_elf(elf, &mut page_table);
    map_capability_memory(&mut page_table, pid);
    map_inputs_memory(&mut page_table, inputs);
    map_user_stack(&mut page_table);

    let mut proc = get_process(pid).lock();
    *proc = Some(Process {
        id: pid,
        name,
        state: ProcessState::Runnable,
        registers: RiscvRegisters {
            sp: USER_STACK.end,
            ..RiscvRegisters::default()
        },
        pc: entry_point,
        page_table,
        virtual_memory: BuddyAllocator::new(0x4000000..0x80000000),
        messages: VecDeque::new_in(BuddyHeap),
        currently_serving: None,
    });
}

fn map_capability_memory(table: &mut TopPageTable, pid: ProcessId) {
    let pre_v = CAPABILITIES_START;
    let pre_p = capability_pages_physical_address();
    let own_v = pre_v + pid.as_u16() as usize * PAGE_SIZE;
    let own_p = pre_p + pid.as_u16() as usize * PAGE_SIZE;
    let suf_v = own_v + PAGE_SIZE;
    let suf_p = own_p + PAGE_SIZE;
    let suf_l = PROCESS_COUNT - pid.as_u16() as usize - 1;
    map_pages(
        table,
        pre_v,
        pre_p,
        PageFlags::readonly().user(),
        (pid.as_u16() as usize) * PAGE_SIZE,
    );
    map_pages(
        table,
        own_v,
        own_p,
        PageFlags::readwrite().user(),
        PAGE_SIZE,
    );
    map_pages(
        table,
        suf_v,
        suf_p,
        PageFlags::readonly().user(),
        suf_l * PAGE_SIZE,
    );
}

fn map_inputs_memory<T: ProcessTag>(table: &mut TopPageTable, inputs: ProcessInputs<T>) {
    let size = USER_INPUTS.end - USER_INPUTS.start;
    assert!(size_of::<ProcessInputs<T>>() <= size);
    let page = Box::leak(Box::new_in(inputs, PageGranular::new()));
    let virt = USER_INPUTS.start;
    let phys = virt_to_phys(page as *mut _) as usize;
    map_pages(table, virt, phys, PageFlags::readonly().user(), size);
}

fn map_user_stack(table: &mut TopPageTable) {
    let stack_size = USER_STACK.end - USER_STACK.start;
    let pages = Vec::leak(vec![Page::zeroed(); stack_size / PAGE_SIZE]);
    let phys = virt_to_phys(pages.as_ptr()) as usize;
    let virt = USER_STACK.start;
    map_pages(table, virt, phys, PageFlags::readwrite().user(), stack_size);
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

pub fn find_runnable_process(user: Option<&UserCtx>) -> Option<MutexGuard<'static, Process>> {
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
        }
    }

    None
}

pub fn inspect_can_progress(proc: &mut Process) {
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
