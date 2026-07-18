use crate::buddy::BuddyMemoryAllocator;
use crate::bump::BumpMemoryAllocator;
use crate::page::phys_to_virt;
use crate::sync::Mutex;
use crate::util::fmt_memory;
use alloc::alloc::Global;
use alloc::vec::Vec;
use core::alloc::{AllocError, Allocator, GlobalAlloc, Layout};
use core::iter::once;
use core::ops::Range;
use core::ptr::NonNull;
use fdt::Fdt;
use log::*;

macro singleton_allocator($name:ident, $instance:path) {
    pub struct $name;

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

static BUDDY: Mutex<Option<BuddyMemoryAllocator<Global>>> = Mutex::new(None);
static EARLY_BUMP: Mutex<Option<BumpMemoryAllocator>> = Mutex::new(None);

unsafe impl GlobalAlloc for GlobalAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        EarlyBumpHeap
            .allocate(layout)
            .map(|p| p.as_mut_ptr())
            .unwrap_or_default()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { EarlyBumpHeap.deallocate(NonNull::new(ptr).unwrap(), layout) }
    }
}

pub fn initialize_early_heap() {
    unsafe extern "C" {
        static mut heap_start: u8;
        static mut heap_end: u8;
    }
    *EARLY_BUMP.lock() =
        Some(unsafe { BumpMemoryAllocator::new(&raw mut heap_start..&raw mut heap_end) });
}

pub fn initialize_heap(dt: &Fdt, dt_ptr: *const u8) {
    let available = collect_available(dt);
    assert_eq!(available.len(), 1);
    let available = available[0].clone();
    info!("found RAM {}", fmt_memory(&available));

    let mut buddy = unsafe { BuddyMemoryAllocator::new(phys_to_virt(available), Global) };
    for reserved in collect_reserved(dt, dt_ptr) {
        buddy.reserve_range(phys_to_virt(reserved));
    }
    *BUDDY.lock() = Some(buddy);
}

fn collect_available(dt: &Fdt) -> Vec<Range<*mut u8>> {
    dt.memory()
        .regions()
        .map(|reg| {
            let start = reg.starting_address as *mut u8;
            let end = start.wrapping_byte_add(reg.size.unwrap());
            start..end
        })
        .collect()
}

fn collect_reserved(dt: &Fdt, dt_ptr: *const u8) -> impl Iterator<Item = Range<*const u8>> {
    reserved_ranges_from_dt(dt)
        .chain(once(reserved_kernel_range()))
        .chain(once(reserved_dt_memory(dt, dt_ptr)))
}

fn reserved_ranges_from_dt(dt: &Fdt) -> impl Iterator<Item = Range<*const u8>> {
    dt.find_node("/reserved-memory")
        .unwrap()
        .children()
        .flat_map(|reserved| {
            reserved.reg().into_iter().flatten().map(|reg| {
                let start = reg.starting_address;
                let end = start.wrapping_byte_add(reg.size.unwrap());
                start..end
            })
        })
}

fn reserved_kernel_range() -> Range<*const u8> {
    unsafe extern "C" {
        static image_start: u8;
        static image_end: u8;
    }
    &raw const image_start..&raw const image_end
}

fn reserved_dt_memory(dt: &Fdt, dt_ptr: *const u8) -> Range<*const u8> {
    dt_ptr..dt_ptr.wrapping_byte_add(dt.total_size())
}
