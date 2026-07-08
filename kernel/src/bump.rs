use core::ops::Range;

#[derive(Debug)]
pub struct BumpAllocator {
    range: Range<usize>,
}

impl BumpAllocator {
    pub const fn new(range: Range<usize>) -> BumpAllocator {
        BumpAllocator { range }
    }

    pub fn allocate(&mut self, size: usize, alignment: usize) -> usize {
        let pointer = self.range.start.next_multiple_of(alignment);
        let new_start = pointer + size;
        assert!(new_start <= self.range.end);
        self.range.start = new_start;
        pointer
    }
}
