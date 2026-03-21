use core::ops::Range;

#[derive(Debug)]
pub struct TrivialAllocator {
    range: Range<usize>,
}

impl TrivialAllocator {
    pub fn new(size: usize) -> TrivialAllocator {
        TrivialAllocator { range: 0..size }
    }

    pub fn allocate(&mut self, size: usize, alignment: usize) -> usize {
        let pointer = self.range.start.next_multiple_of(alignment);
        let new_start = pointer + size;
        assert!(new_start <= self.range.end);
        self.range.start = new_start;
        pointer
    }
}
