use crate::capability::Handler;
use crate::heap::granularity::PageGranular;
use crate::page::{PageFlags, TopPageTable, virt_to_phys};
use crate::util::untyped_box::UntypedBox;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ops::{Deref, Range};
use deravel_types::{ProcessId, UntypedRingBuffer};

#[derive(Clone)]
pub struct SharedMemory {
    pub backing: Arc<UntypedBox<PageGranular>>,
}

impl Handler<deravel_types::SharedMemory> for SharedMemory {
    fn call_method(&self, _: usize, _: &[u8], _: ProcessId) -> Vec<u8> {
        unreachable!()
    }

    fn map_stream(&self, _: usize) -> &'static UntypedRingBuffer {
        unreachable!()
    }

    fn shared_memory_map(
        &self,
        virt: usize,
        page_table: &mut TopPageTable,
        _: &mut Vec<(
            Range<usize>,
            &'static (dyn Handler<deravel_types::SharedMemory> + Sync),
        )>,
    ) {
        let phys = virt_to_phys(Arc::deref(&self.backing).as_untyped_ptr().addr());
        page_table.map_pages(
            virt,
            phys,
            self.backing.byte_size(),
            PageFlags::readwrite().user(),
        );
    }

    fn shared_memory_size(&self) -> usize {
        self.backing.byte_size()
    }

    fn virtual_memory_load(&self, _: usize, _: usize, _: &mut TopPageTable) {
        unreachable!()
    }
}
