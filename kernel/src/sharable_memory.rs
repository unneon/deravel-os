use crate::capability::Handler;
use crate::heap::granularity::PageGranular;
use crate::page::{PageFlags, PageTable, virt_to_phys};
use crate::util::untyped_box::UntypedBox;
use crate::virtual_memory::VirtualMemoryRawMapping;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ops::{Deref, Range};
use deravel_types::{ProcessId, UntypedRingBuffer};

#[derive(Clone)]
pub struct ShareableMemory {
    pub backing: Arc<UntypedBox<PageGranular>>,
}

impl Handler<deravel_types::SharedMemory> for ShareableMemory {
    fn call_method(&self, _: usize, _: &[u8], _: ProcessId) -> Vec<u8> {
        unreachable!()
    }

    fn map_stream(&self, _: usize) -> &'static UntypedRingBuffer {
        unreachable!()
    }

    fn shared_memory_map(
        &self,
        virt: usize,
        page_table: &mut PageTable,
        _: &mut Vec<(Range<usize>, &'static (dyn VirtualMemoryRawMapping + Sync))>,
    ) {
        let phys = virt_to_phys(Arc::deref(&self.backing).as_untyped_ptr().addr());
        page_table.map(
            virt,
            phys,
            self.backing.byte_size(),
            PageFlags::read_write().user(),
        );
    }

    fn shared_memory_size(&self) -> usize {
        self.backing.byte_size()
    }
}
