use crate::buddy::BuddyMemoryAllocator;
use crate::page::physical_to_identity_mapped;
use crate::sync::Mutex;
use crate::util::fmt_memory_size;
use alloc::alloc::Global;
use alloc::vec::Vec;
use core::alloc::{AllocError, Allocator, GlobalAlloc, Layout};
use core::ops::Range;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering};
use deravel_types::PAGE_SIZE;
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

pub struct Heap;

unsafe extern "C" {
    static image_start: u8;
    static image_end: u8;
    static mut heap_start: u8;
    static mut heap_end: u8;
}

#[global_allocator]
static HEAP: Heap = Heap;

static BUDDY: Mutex<Option<BuddyMemoryAllocator<Global>>> = Mutex::new(None);

static ALLOCATED_SO_FAR: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Heap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        assert!(layout.align() <= PAGE_SIZE);
        let page_count = layout.size().div_ceil(PAGE_SIZE);
        let page_offset = ALLOCATED_SO_FAR.fetch_add(page_count, Ordering::Relaxed);
        let max_pages =
            ((&raw const heap_end) as usize - (&raw const heap_start) as usize) / PAGE_SIZE;
        assert!(max_pages - page_offset >= page_count);
        (&raw mut heap_start).wrapping_byte_add(PAGE_SIZE * page_offset)
    }

    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}

pub fn initialize_heap(dt: &Fdt, dt_ptr: *const u8) {
    let mut available = collect_memory(dt);
    reserve_ranges_from_dt(dt, &mut available);
    reserve_dt_memory(dt, dt_ptr, &mut available);
    reserve_kernel_range(&mut available);
    let Some(largest_available) = available
        .iter()
        .max_by_key(|range| unsafe { range.end.byte_offset_from_unsigned(range.start) })
    else {
        return;
    };
    for a in &available {
        let start = a.start as usize;
        let end = a.end as usize;
        let size = fmt_memory_size(end - start);
        if a != largest_available {
            info!("ignoring RAM {start:#x}..{end:#x} ({size})");
        } else {
            info!("using RAM {start:#x}..{end:#x} ({size})");
        }
    }
    *BUDDY.lock() = Some(unsafe {
        BuddyMemoryAllocator::new(
            core::ptr::slice_from_raw_parts_mut(
                physical_to_identity_mapped(largest_available.start as *mut u8),
                largest_available.end as usize - largest_available.start as usize,
            ),
            Global,
        )
    });
}

fn collect_memory(dt: &Fdt) -> Vec<Range<*const u8>> {
    dt.memory()
        .regions()
        .map(|reg| {
            let start = reg.starting_address;
            let end = start.wrapping_byte_add(reg.size.unwrap());
            start..end
        })
        .collect()
}

fn reserve_ranges_from_dt(dt: &Fdt, available: &mut Vec<Range<*const u8>>) {
    for reserved in dt.find_node("/reserved-memory").unwrap().children() {
        for reg in reserved.reg().into_iter().flatten() {
            let start = reg.starting_address;
            let end = start.wrapping_byte_add(reg.size.unwrap());
            reserve_range(start..end, available);
        }
    }
}

fn reserve_dt_memory(dt: &Fdt, dt_ptr: *const u8, available: &mut Vec<Range<*const u8>>) {
    reserve_range(dt_ptr..dt_ptr.wrapping_byte_add(dt.total_size()), available)
}

fn reserve_kernel_range(available: &mut Vec<Range<*const u8>>) {
    let start = &raw const image_start;
    let end = &raw const image_end;
    reserve_range(start..end, available);
}

fn reserve_range(reserved: Range<*const u8>, available: &mut Vec<Range<*const u8>>) {
    *available = available
        .iter()
        .flat_map(|available| {
            [
                available.start..available.end.min(reserved.start),
                available.start.max(reserved.end)..available.end,
            ]
        })
        .filter(|available| available.end > available.start)
        .collect();
}

pub fn log_heap_statistics() {
    info!(
        "allocated {} pages in total",
        ALLOCATED_SO_FAR.load(Ordering::Relaxed)
    );
}
