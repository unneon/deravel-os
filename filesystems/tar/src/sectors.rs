use crate::SECTOR_SIZE;
use deravel_kernel_api::*;

pub struct SequentialSectors {
    drive: Capability<Drive>,
    current: usize,
    capacity: usize,
}

impl SequentialSectors {
    pub fn new(drive: Capability<Drive>) -> SequentialSectors {
        SequentialSectors {
            drive,
            current: 0,
            capacity: drive.capacity() as usize,
        }
    }

    pub fn read(&mut self, buf: &mut [u8; SECTOR_SIZE]) {
        assert!(self.current < self.capacity);
        let data = self.drive.read(self.current as u64);
        buf.copy_from_slice(&data);
        self.current += 1;
    }

    pub fn write(&mut self, buf: &[u8; SECTOR_SIZE]) {
        assert!(self.current < self.capacity);
        self.drive.write(self.current as u64, buf);
        self.current += 1;
    }

    pub fn is_finished(&self) -> bool {
        self.current == self.capacity
    }
}
