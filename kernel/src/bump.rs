use crate::heap::stats::HeapStats;
use crate::sync::Mutex;
use crate::util::address::Address;
use core::alloc::{AllocError, Allocator, Layout};
use core::ops::Range;
use core::ptr::NonNull;

pub struct BumpAllocator {
    range: Range<usize>,
    initial_range: Range<usize>,
    // Does not include internal fragmentation.
    allocated: usize,
}

pub struct BumpMemoryAllocator(BumpAllocator);

impl BumpAllocator {
    pub const fn new(range: Range<usize>) -> BumpAllocator {
        BumpAllocator {
            initial_range: range.start..range.end,
            range,
            allocated: 0,
        }
    }

    pub fn alloc(&mut self, layout: Layout) -> Result<usize, AllocError> {
        let pointer = self.range.start.next_multiple_of(layout.align());
        let new_start = pointer + layout.size();
        if new_start > self.range.end {
            return Err(AllocError);
        }
        self.range.start = new_start;
        self.allocated += layout.size();
        Ok(pointer)
    }
}

impl BumpMemoryAllocator {
    pub unsafe fn new(range: Range<*mut u8>) -> BumpMemoryAllocator {
        BumpMemoryAllocator(BumpAllocator::new(range.raw_addr()))
    }

    pub fn initial_range(&self) -> Range<*const u8> {
        self.0.initial_range.start as *const u8..self.0.initial_range.end as *const u8
    }

    pub fn stats(&self) -> HeapStats {
        HeapStats {
            alloc: self.0.allocated,
            free: self.0.range.end - self.0.range.start,
        }
    }
}

unsafe impl Allocator for Mutex<Option<BumpMemoryAllocator>> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let mut self_ = self.lock();
        let ptr = self_.as_mut().unwrap().0.alloc(layout)?;
        let ptr = core::ptr::slice_from_raw_parts_mut(ptr as *mut u8, layout.size());
        Ok(NonNull::new(ptr).unwrap())
    }

    unsafe fn deallocate(&self, _: NonNull<u8>, _: Layout) {}
}
