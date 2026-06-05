use crate::drvli::SharedMemoryServer;
use deravel_types::ProcessId;

pub struct SharedMemory {
    pub physical_address: u64,
    pub length: u64,
}

impl SharedMemoryServer for SharedMemory {
    fn physical_address(&self, _: ProcessId) -> u64 {
        self.physical_address
    }

    fn length(&self, _: ProcessId) -> u64 {
        self.length
    }
}
