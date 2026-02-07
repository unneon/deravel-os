use crate::{heap_end, heap_start};
use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicUsize, Ordering};

pub struct Heap;

#[global_allocator]
static HEAP: Heap = Heap;

static ALLOCATED_SO_FAR: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Heap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        assert!(layout.align() <= 4096);
        let page_count = layout.size().div_ceil(4096);
        let page_offset = ALLOCATED_SO_FAR.fetch_add(page_count, Ordering::Relaxed);
        let max_pages = ((&raw const heap_end) as usize - (&raw const heap_start) as usize) / 4096;
        assert!(max_pages - page_offset >= page_count);
        unsafe { (&raw mut heap_start).byte_add(4096 * page_offset) }
    }

    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}
