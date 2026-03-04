#![feature(pointer_is_aligned_to)]
#![no_std]

use crate::drvli::ProcessTag;

pub mod capability;
pub mod drvli;

#[repr(transparent)]
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ProcessId(pub usize);

#[repr(C)]
pub struct ProcessInputs<T: ProcessTag> {
    pub id: ProcessId,
    pub args: T::Capabilities,
}

pub const INPUTS_ADDRESS: usize = 0x3000000;

impl core::fmt::Debug for ProcessId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}
