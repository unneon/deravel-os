use crate::STACK_SIZE;
use crate::process::{Process, get_process};
use crate::sync::MutexGuard;
use alloc::boxed::Box;
use deravel_types::ProcessId;

#[repr(align(16))]
pub struct HartContext {
    current_pid: Option<ProcessId>,
}

#[repr(C, align(4096))]
pub struct HartStack {
    data: [u8; STACK_SIZE - size_of::<HartContext>().next_multiple_of(16)],
    ctx: HartContext,
}

impl HartContext {
    pub fn try_current_pid(&self) -> Option<ProcessId> {
        self.current_pid
    }

    pub fn current_pid(&self) -> ProcessId {
        self.current_pid.unwrap()
    }

    pub fn current_process(&self) -> MutexGuard<'_, Process> {
        get_process(self.current_pid()).lock()
    }

    pub fn set_current_pid(&mut self, pid: ProcessId) {
        self.current_pid = Some(pid);
    }
}

impl HartStack {
    pub fn new() -> Box<HartStack> {
        unsafe { Box::new_zeroed().assume_init() }
    }

    pub fn as_raw_ctx(&mut self) -> *mut HartContext {
        &raw mut self.ctx
    }
}
