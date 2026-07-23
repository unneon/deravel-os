use crate::util::fmt::memory::fmt_memory_size;

pub struct HeapStats {
    // Does not include internal fragmentation.
    pub alloc: usize,
    // Does include internal fragmentation.
    pub free: usize,
}

impl core::fmt::Display for HeapStats {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let a = fmt_memory_size(self.alloc);
        let fr = fmt_memory_size(self.free);
        write!(f, "{a} allocated, {fr} free ")
    }
}
