use crate::heap::MutAllocator;
use crate::heap::stats::HeapStats;
use core::alloc::{AllocError, Layout};
use core::ops::Range;

pub struct BumpAllocator {
    range: Range<usize>,
    // Does not include internal fragmentation.
    allocated: usize,
}

impl BumpAllocator {
    pub const fn new(range: Range<usize>) -> BumpAllocator {
        BumpAllocator {
            range,
            allocated: 0,
        }
    }

    pub fn stats(&self) -> HeapStats {
        HeapStats {
            alloc: self.allocated,
            free: self.range.end - self.range.start,
        }
    }
}

impl MutAllocator for BumpAllocator {
    fn alloc(&mut self, layout: Layout) -> Result<usize, AllocError> {
        let pointer = self.range.start.next_multiple_of(layout.align());
        let new_start = pointer + layout.size();
        if new_start > self.range.end {
            return Err(AllocError);
        }
        self.range.start = new_start;
        self.allocated += layout.size();
        Ok(pointer)
    }

    fn dealloc(&mut self, _: usize, _: Layout) {}
}
