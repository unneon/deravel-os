use crate::MAX_PROCESSES;
use core::num::NonZeroU16;

#[repr(transparent)]
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ProcessId(NonZeroU16);

impl ProcessId {
    #[track_caller]
    pub fn new(id: u16) -> ProcessId {
        assert!(
            (id as usize) < MAX_PROCESSES,
            "process id exceeds MAX_PROCESSES"
        );
        ProcessId(NonZeroU16::new(id).expect("process id must be non-zero"))
    }

    pub fn as_u16(&self) -> u16 {
        self.0.get()
    }
}

impl core::fmt::Debug for ProcessId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}
