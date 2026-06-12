use crate::capability::Handler;
use alloc::vec::Vec;
use deravel_types::{ProcessId, UntypedRingBuffer};

pub struct SharedMemory {
    pub physical_address: usize,
    pub size: usize,
}

impl Handler<deravel_types::SharedMemory> for SharedMemory {
    fn call_method(&self, _: usize, _: &[u8], _: ProcessId) -> Vec<u8> {
        unreachable!()
    }

    fn map_stream(&self, _: usize) -> &'static UntypedRingBuffer {
        unreachable!()
    }

    fn shared_memory(&self) -> (usize, usize) {
        (self.physical_address, self.size)
    }
}
