mod available;
pub mod buddy;
pub mod bump;
pub mod granularity;
pub mod stats;

use crate::heap::available::{collect_available, collect_reserved};
use crate::sync::Mutex;
use crate::util::fmt::memory::fmt_memory;
use buddy::BuddyMemoryAllocator;
use bump::BumpMemoryAllocator;
use core::alloc::{AllocError, Allocator, GlobalAlloc, Layout};
use core::ptr::NonNull;
use fdt::Fdt;
use itertools::Itertools;
use log::*;

macro singleton_allocator($name:ident, $instance:path) {
    #[derive(Clone, Copy)]
    struct $name;

    unsafe impl Allocator for $name {
        fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
            $instance.allocate(layout)
        }

        unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
            unsafe { $instance.deallocate(ptr, layout) }
        }
    }
}

singleton_allocator!(BuddyHeap, BUDDY);
singleton_allocator!(EarlyBumpHeap, EARLY_BUMP);

pub struct GlobalAllocator;

#[global_allocator]
static GLOBAL_ALLOCATOR: GlobalAllocator = GlobalAllocator;

static BUDDY: Mutex<Option<BuddyMemoryAllocator<EarlyBumpHeap>>> = Mutex::new(None);
static EARLY_BUMP: Mutex<Option<BumpMemoryAllocator>> = Mutex::new(None);

unsafe impl GlobalAlloc for GlobalAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        BUDDY
            .try_lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .allocate_mut(layout)
            .map(|p| p.as_mut_ptr())
            .unwrap_or_default()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { BuddyHeap.deallocate(NonNull::new(ptr).unwrap(), layout) }
    }
}

fn initialize_early_heap() {
    unsafe extern "C" {
        static mut early_heap_start: u8;
        static mut early_heap_end: u8;
    }
    *EARLY_BUMP.lock() = Some(unsafe {
        BumpMemoryAllocator::new(&raw mut early_heap_start..&raw mut early_heap_end)
    });
}

pub fn initialize_heap(dt: &Fdt, dt_ptr: *const u8) {
    initialize_early_heap();

    let available = collect_available(dt).exactly_one().ok().unwrap();
    info!("found RAM {}", fmt_memory(&available));

    let mut buddy = unsafe { BuddyMemoryAllocator::new(available, EarlyBumpHeap) };
    for reserved in collect_reserved(dt, dt_ptr) {
        buddy.reserve_range(reserved);
    }
    *BUDDY.lock() = Some(buddy);
}

pub fn log_heap_usage() {
    let buddy_usage = &BUDDY.lock().as_ref().unwrap().stats();
    info!("buddy had {buddy_usage}");
    let early_heap_usage = &EARLY_BUMP.lock().as_ref().unwrap().stats();
    info!("early bump had {early_heap_usage}");
}
