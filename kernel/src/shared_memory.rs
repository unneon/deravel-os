use crate::capability::Handler;
use crate::heap::granularity::PageGranular;
use crate::page::virt_to_phys;
use crate::util::untyped_box::UntypedBox;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ops::Deref;
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

    fn shared_memory(&self) -> (usize, usize) {
        (
            virt_to_phys(Arc::deref(&self.backing).as_untyped_ptr().addr()),
            self.backing.byte_size(),
        )
    }
}
