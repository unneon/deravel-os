use crate::heap::MutAllocator;
use core::alloc::{AllocError, Layout};
use core::ops::Range;

pub struct BitmapAllocator<T> {
    range: Range<usize>,
    bitmap: T,
}

impl<T> BitmapAllocator<T> {
    pub const fn new(range: Range<usize>, bitmap: T) -> Self {
        BitmapAllocator { range, bitmap }
    }
}

impl<T: AsMut<[usize]>> MutAllocator for BitmapAllocator<T> {
    fn alloc(&mut self, layout: Layout) -> Result<usize, AllocError> {
        assert!(layout.size() == 1 && layout.align() == 1);
        for (i, block) in self.bitmap.as_mut().iter_mut().enumerate() {
            if *block == usize::MAX {
                continue;
            }
            let j = block.trailing_ones() as usize;
            let ptr = self.range.start + i * usize::BITS as usize + j;
            if ptr >= self.range.end {
                return Err(AllocError);
            }
            *block |= 1 << j;
            return Ok(ptr);
        }
        Err(AllocError)
    }

    fn dealloc(&mut self, ptr: usize, layout: Layout) {
        debug_assert!(self.range.contains(&ptr));
        debug_assert!(layout.size() == 1 && layout.align() == 1);
        let ij = ptr - self.range.start;
        let i = ij / usize::BITS as usize;
        let j = ij % usize::BITS as usize;
        let block = &mut self.bitmap.as_mut()[i];
        debug_assert_ne!(*block & (1 << j), 0);
        *block &= !(1 << j);
    }
}
