use crate::arch::RiscvRegisters;
use crate::process::{Process, get_process};
use crate::sync::MutexGuard;
use alloc::boxed::Box;
use deravel_types::ProcessId;

#[repr(C, align(4096))]
pub struct KernelStack {
    data: [u8; STACK_SIZE - size_of::<UserStoredCtx>().next_multiple_of(16)],
    pub ctx: UserStoredCtx,
}

#[repr(C)]
pub struct UserCtx {
    pub registers: RiscvRegisters,
    pub stored: UserStoredCtx,
}

#[repr(C, align(16))]
pub struct UserStoredCtx {
    pid: Option<ProcessId>,
}

const _: () = assert!(size_of::<KernelStack>() == STACK_SIZE);

const STACK_SIZE: usize = 32 * 1024;

impl KernelStack {
    pub fn new() -> Box<KernelStack> {
        unsafe { Box::new_zeroed().assume_init() }
    }
}

impl UserCtx {
    pub fn pid(&self) -> ProcessId {
        self.stored.pid()
    }

    pub fn process(&self) -> MutexGuard<'_, Process> {
        self.stored.process()
    }
}

impl UserStoredCtx {
    pub fn pid(&self) -> ProcessId {
        self.pid.unwrap()
    }

    pub fn try_pid(&self) -> Option<ProcessId> {
        self.pid
    }

    pub fn process(&self) -> MutexGuard<'_, Process> {
        get_process(self.pid()).lock_if_some().unwrap()
    }

    pub fn set_process(&mut self, process: &mut Process) {
        self.pid = Some(process.id);
    }
}
