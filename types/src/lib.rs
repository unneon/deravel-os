#![feature(decl_macro)]
#![feature(never_type)]
#![feature(ptr_metadata)]
#![allow(clippy::missing_safety_doc)]
#![no_std]
extern crate alloc;

pub mod abi;
mod actor;
mod align;
mod capability;
mod drvli;
pub mod input;
pub mod memory;
mod process_id;
mod ring_buffer;

pub use actor::Actor;
pub use align::{CACHE_LINE_SIZE, CacheLineAligned};
pub use capability::*;
pub use drvli::*;
pub use process_id::ProcessId;
pub use ring_buffer::{RingBuffer, UntypedRingBuffer};

#[derive(Debug)]
#[repr(C, align(4096))]
pub struct ProcessInputs<T: ProcessTag> {
    pub id: ProcessId,
    pub riscv_timebase_frequency: Option<usize>,
    pub args: T::Args,
}

pub struct SharedMemory;

pub const MAX_PROCESSES: usize = 8;

pub const PAGE_SIZE: usize = 4096;

impl Interface for SharedMemory {
    const NAME: &'static str = "shared_memory";
}
