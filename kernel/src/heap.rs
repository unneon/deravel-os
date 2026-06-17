use alloc::vec::Vec;
use core::alloc::{GlobalAlloc, Layout};
use core::ops::Range;
use core::sync::atomic::{AtomicUsize, Ordering};
use deravel_types::PAGE_SIZE;
use fdt::Fdt;
use log::*;

pub struct Heap;

unsafe extern "C" {
    static mut kernel_start: u8;
    static mut kernel_end: u8;
    static mut heap_start: u8;
    static mut heap_end: u8;
}

#[global_allocator]
static HEAP: Heap = Heap;

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

pub fn initialize_heap(dt: &Fdt) {
    let mut available = collect_memory(dt);
    reserve_ranges_from_dt(dt, &mut available);
    reserve_kernel_range(&mut available);
    for a in available {
        info!("found RAM {:#x}..{:#x}", a.start as usize, a.end as usize);
    }
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

fn reserve_kernel_range(available: &mut Vec<Range<*const u8>>) {
    let start = &raw const kernel_start;
    let end = &raw const kernel_end;
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
