use crate::SECTOR_SIZE;
use deravel_kernel_api::{disk_capacity, disk_read, disk_write};

pub struct SequentialSectors {
    current: usize,
    capacity: usize,
}

impl SequentialSectors {
    pub fn new() -> SequentialSectors {
        SequentialSectors {
            current: 0,
            capacity: disk_capacity(),
        }
    }

    pub fn read(&mut self, buf: &mut [u8; SECTOR_SIZE]) {
        assert!(self.current < self.capacity);
        disk_read(self.current, buf);
        self.current += 1;
    }

    pub fn write(&mut self, buf: &[u8; SECTOR_SIZE]) {
        assert!(self.current < self.capacity);
        disk_write(self.current, buf);
        self.current += 1;
    }

    pub fn is_finished(&self) -> bool {
        self.current == self.capacity
    }
}
