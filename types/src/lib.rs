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
mod process_id;
mod ring_buffer;

pub use actor::Actor;
pub use align::{CACHE_LINE_SIZE, CacheLineAligned};
pub use capability::*;
pub use drvli::*;
pub use process_id::ProcessId;
pub use ring_buffer::{RingBuffer, UntypedRingBuffer};

#[repr(C)]
pub struct ProcessInputs<T: ProcessTag> {
    pub common: CommonProcessInputs,
    pub args: T::Args,
}

#[repr(C)]
pub struct CommonProcessInputs {
    pub id: ProcessId,
    pub riscv_timebase_frequency: f64,
}

pub struct SharedMemory;

pub const INPUTS_ADDRESS: usize = 0x3000000;

pub const MAX_PROCESSES: usize = 8;

pub const LEVEL_0_PAGE_SIZE: usize = PAGE_SIZE;
pub const LEVEL_1_PAGE_SIZE: usize = PAGE_SIZE / size_of::<usize>() * LEVEL_0_PAGE_SIZE;
pub const LEVEL_2_PAGE_SIZE: usize = PAGE_SIZE / size_of::<usize>() * LEVEL_1_PAGE_SIZE;

pub const PAGE_SIZE: usize = 4096;

impl Interface for SharedMemory {
    const NAME: &'static str = "shared_memory";
}
