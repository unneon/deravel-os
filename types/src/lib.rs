#![feature(pointer_is_aligned_to)]
#![no_std]

mod capability;
mod drvli;

pub use capability::*;
pub use drvli::*;

#[repr(transparent)]
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ProcessId(usize);

#[repr(C)]
pub struct ProcessInputs<T: ProcessTag> {
    pub id: ProcessId,
    pub args: T::Args,
}

pub const INPUTS_ADDRESS: usize = 0x3000000;

pub const PAGE_SIZE: usize = 4096;

impl ProcessId {
    pub fn new(id: usize) -> ProcessId {
        assert!(id < MAX_PROCESSES);
        ProcessId(id)
    }

    pub fn as_usize(&self) -> usize {
        self.0
    }
}

impl core::fmt::Debug for ProcessId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}
