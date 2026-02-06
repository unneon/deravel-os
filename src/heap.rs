use crate::{heap_end, heap_start};

static mut ALLOCATED_SO_FAR: usize = 0;

pub fn alloc_page() -> &'static mut [u8; 4096] {
    &mut alloc_pages(1)[0]
}

pub fn alloc_pages(n: usize) -> &'static mut [[u8; 4096]] {
    let heap_start_ptr = (&raw mut heap_start) as *mut [u8; 4096];
    let heap_end_ptr = (&raw mut heap_end) as *mut [u8; 4096];
    let max_pages = unsafe { heap_end_ptr.offset_from_unsigned(heap_start_ptr) };
    let allocated_so_far = unsafe { ALLOCATED_SO_FAR };
    assert!(allocated_so_far + n <= max_pages, "out of heap memory");

    unsafe { ALLOCATED_SO_FAR += n };
    let pointer = unsafe { heap_start_ptr.add(allocated_so_far) };
    unsafe { core::slice::from_raw_parts_mut(pointer, n) }
}
