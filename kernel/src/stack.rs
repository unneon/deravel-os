use crate::process::{Process, get_process};
use crate::sync::MutexGuard;
use alloc::boxed::Box;
use deravel_types::ProcessId;

#[repr(C, align(4096))]
struct KernelStack {
    data: [u8; STACK_SIZE - size_of::<UserCtx>().next_multiple_of(16)],
    ctx: UserCtx,
}

#[repr(align(16))]
pub struct UserCtx {
    pub pid: ProcessId,
}

const STACK_SIZE: usize = 32 * 1024;

impl UserCtx {
    pub fn process(&self) -> MutexGuard<'_, Process> {
        get_process(self.pid).lock_if_some().unwrap()
    }
}

pub fn initialize_kernel_stack() {
    let stack: &mut KernelStack = Box::leak(unsafe { Box::new_zeroed().assume_init() });
    unsafe { riscv::register::sscratch::write(&raw mut stack.ctx as usize) }
}
