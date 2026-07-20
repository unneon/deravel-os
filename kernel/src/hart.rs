use crate::process::{Process, get_process};
use crate::sync::MutexGuard;
use alloc::boxed::Box;
use deravel_types::ProcessId;

#[repr(align(16))]
pub struct UserCtx {
    pub pid: ProcessId,
}

#[repr(C, align(4096))]
pub struct HartStack {
    data: [u8; STACK_SIZE - size_of::<UserCtx>().next_multiple_of(16)],
    ctx: UserCtx,
}

const STACK_SIZE: usize = 128 * 1024;

impl UserCtx {
    pub fn process(&self) -> MutexGuard<'_, Process> {
        get_process(self.pid).lock_if_some().unwrap()
    }
}

impl HartStack {
    pub fn new() -> Box<HartStack> {
        unsafe { Box::new_zeroed().assume_init() }
    }

    pub fn as_raw_ctx(&mut self) -> *mut UserCtx {
        &raw mut self.ctx
    }
}
