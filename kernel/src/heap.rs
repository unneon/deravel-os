use crate::page::PAGE_SIZE;
use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicUsize, Ordering};
use log::info;

pub struct Heap;

unsafe extern "C" {
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
        unsafe { (&raw mut heap_start).byte_add(PAGE_SIZE * page_offset) }
    }

    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}

pub fn log_heap_statistics() {
    info!(
        "allocated {} pages in total",
        ALLOCATED_SO_FAR.load(Ordering::Relaxed)
    );
}
