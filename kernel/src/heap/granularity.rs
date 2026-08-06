use alloc::alloc::Global;
use core::alloc::{AllocError, Allocator, Layout};
use core::ptr::NonNull;
use deravel_types::PAGE_SIZE;

pub macro page_granular_vec($elem:expr; $n:expr) {{
    let elem = $elem;
    let n = $n;
    let mut v = ::alloc::vec::Vec::with_capacity_in(n, PageGranular::new());
    for _ in 0..n {
        v.push(elem.clone());
    }
    v
}}

pub struct PageGranular<A: Allocator = Global>(A);

impl PageGranular {
    pub fn new() -> PageGranular {
        PageGranular(Global)
    }
}

unsafe impl<A: Allocator> Allocator for PageGranular<A> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.0.allocate(page_granular(layout))
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        unsafe { self.0.deallocate(ptr, page_granular(layout)) }
    }
}

fn page_granular(layout: Layout) -> Layout {
    Layout::from_size_align(layout.size().max(PAGE_SIZE), layout.align().max(PAGE_SIZE)).unwrap()
}
