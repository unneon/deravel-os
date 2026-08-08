use crate::drvli::ShutdownServer;
use crate::heap::log_heap_usage;
use crate::sbi;
use crate::sbi::{ResetReason, ResetType};
use deravel_types::ProcessId;

pub struct KernelShutdown;

impl ShutdownServer for KernelShutdown {
    fn shutdown(&self, _: ProcessId) -> ! {
        shutdown()
    }
}

pub fn shutdown() -> ! {
    log_heap_usage();
    sbi::system_reset(ResetType::Shutdown, ResetReason::NoReason).unwrap()
}
