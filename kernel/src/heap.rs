mod available;
pub mod buddy;
pub mod bump;
pub mod granularity;
mod singleton;
pub mod stats;
mod sync;

use crate::heap::available::{collect_available, collect_reserved};
use crate::heap::buddy::BuddyAllocator;
use crate::heap::bump::BumpAllocator;
use crate::heap::singleton::singleton_allocator;
use crate::heap::sync::SyncAllocator;
use crate::page::virt_to_phys;
use crate::util::fmt::memory::fmt_memory;
use core::alloc::{AllocError, Layout};
use fdt::Fdt;
use itertools::Itertools;
use log::*;

pub trait MutAllocator {
    fn alloc(&mut self, layout: Layout) -> Result<usize, AllocError>;
    fn dealloc(&mut self, ptr: usize, layout: Layout);
}

singleton_allocator! {
    struct BuddyNodeAllocator for BUDDY_NODE_ALLOCATOR;
}

#[global_allocator]
static BUDDY_ALLOCATOR: SyncAllocator<BuddyAllocator<BuddyNodeAllocator>> = SyncAllocator::new();
static BUDDY_NODE_ALLOCATOR: SyncAllocator<BumpAllocator> = SyncAllocator::new();

pub fn initialize_heap(dt: &Fdt, dt_ptr: *const u8) {
    initialize_buddy_node_allocator();
    initialize_buddy_allocator(dt, dt_ptr);
}

fn initialize_buddy_node_allocator() {
    unsafe extern "C" {
        static mut early_heap_start: u8;
        static mut early_heap_end: u8;
    }
    unsafe {
        BUDDY_NODE_ALLOCATOR.set(BumpAllocator::new(
            &raw mut early_heap_start as usize..&raw mut early_heap_end as usize,
        ))
    }
}

fn initialize_buddy_allocator(dt: &Fdt, dt_ptr: *const u8) {
    let available = collect_available(dt).exactly_one().ok().unwrap();
    info!("found RAM {}", fmt_memory(&virt_to_phys(available.clone())));

    let mut buddy = BuddyAllocator::new_in(
        available.start as usize..available.end as usize,
        BuddyNodeAllocator,
    );
    for reserved in collect_reserved(dt, dt_ptr) {
        buddy.reserve_range(reserved.start as usize..reserved.end as usize);
    }
    unsafe { BUDDY_ALLOCATOR.set(buddy) }
}

pub fn log_heap_usage() {
    let buddy_usage = unsafe { &BUDDY_ALLOCATOR.lock_inner().as_ref().unwrap().stats() };
    info!("buddy had {buddy_usage}");
    let early_heap_usage = unsafe { &BUDDY_NODE_ALLOCATOR.lock_inner().as_ref().unwrap().stats() };
    info!("early bump had {early_heap_usage}");
}
