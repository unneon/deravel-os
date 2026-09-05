use crate::capability::grant_kernel_capability;
use crate::drvli::DriveServer;
use crate::interrupt::InterruptHandler;
use crate::page::Page;
use crate::sync::Mutex;
use crate::util::fmt::memory::fmt_memory_size;
use crate::util::volatile::{Readonly, Volatile, volatile_struct};
use crate::virtio::queue::{QUEUE_SIZE, Queue};
use crate::virtio::registers::{STATUS_ACKNOWLEDGE, STATUS_DRIVER, STATUS_DRIVER_OK};
use crate::virtio::{Capabilities, Isr};
use crate::virtual_memory::{VirtualMemoryLoader, VirtualMemoryMapping};
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use deravel_types::{Capability, PAGE_SIZE, ProcessId, SharedMemory};
use log::*;

volatile_struct! { pub Config
    capacity: Readonly u64,
}

#[repr(C, packed)]
struct Header {
    type_: u32,
    reserved: u32,
    sector: u64,
}

pub struct VirtioBlk {
    isr: Isr,
    state: Mutex<State>,
}

struct State {
    device: Volatile<'static, Config, Readonly>,
    queue: Queue<0>,
}

struct MappedRegion {
    blk: &'static VirtioBlk,
    sector_offset: u64,
}

#[derive(Debug)]
pub struct VirtioBlkError;

pub const SECTOR_SIZE: usize = 512;

pub const VIRTIO_BLK_T_IN: u32 = 0;
pub const VIRTIO_BLK_T_OUT: u32 = 1;

impl VirtioBlk {
    pub fn new(mut caps: Capabilities<Config, Readonly>) -> VirtioBlk {
        let mut common = caps.common;
        common.device_status().write(0);
        common.device_status().write_bitor(STATUS_ACKNOWLEDGE as u8);
        common.device_status().write_bitor(STATUS_DRIVER as u8);

        let capacity = caps.device.capacity().read() as usize;
        info!("found a {} drive", fmt_memory_size(capacity * SECTOR_SIZE));

        let queue = Queue::new(&mut common, &caps.notify, QUEUE_SIZE);
        common.device_status().write_bitor(STATUS_DRIVER_OK as u8);

        VirtioBlk {
            isr: caps.isr,
            state: Mutex::new(State {
                device: caps.device,
                queue,
            }),
        }
    }

    pub fn read(&self, sector: u64, buf: &mut [u8; SECTOR_SIZE]) -> Result<(), VirtioBlkError> {
        let header = Header {
            type_: VIRTIO_BLK_T_IN,
            reserved: 0,
            sector,
        };
        let mut state = self.state.lock();
        let mut status: u8 = 0;
        state.queue.descriptor_readonly(0, &header, Some(1));
        state.queue.descriptor_writeonly(1, buf, Some(2));
        state.queue.descriptor_writeonly(2, &mut status, None);
        state.queue.send_and_recv(0);
        result_from_status(status)
    }

    pub fn write(&self, sector: u64, buf: &[u8; SECTOR_SIZE]) -> Result<(), VirtioBlkError> {
        let header = Header {
            type_: VIRTIO_BLK_T_OUT,
            reserved: 0,
            sector,
        };
        let mut state = self.state.lock();
        let mut status: u8 = 0;
        state.queue.descriptor_readonly(0, &header, Some(1));
        state.queue.descriptor_readonly(1, buf, Some(2));
        state.queue.descriptor_writeonly(2, &mut status, None);
        state.queue.send_and_recv(0);
        result_from_status(status)
    }

    pub fn capacity(&self) -> u64 {
        self.state.lock().device.capacity().read()
    }
}

impl InterruptHandler for VirtioBlk {
    fn handle(&self) {
        self.isr.clear();
    }
}

impl DriveServer for VirtioBlk {
    fn read(&self, _: ProcessId, sector: u64) -> Vec<u8> {
        let mut buf = Box::new([0u8; SECTOR_SIZE]);
        self.read(sector, &mut buf).unwrap();
        Vec::from(buf as Box<[u8]>)
    }

    fn read_mapped(
        &self,
        sender: ProcessId,
        first_sector: u64,
        sector_count: u64,
    ) -> Capability<SharedMemory> {
        let region = MappedRegion {
            // TODO: Add 'static to {}Server or figure out kernel cap lifetime design.
            blk: unsafe { &*(self as *const _) },
            sector_offset: first_sector,
        };
        let sector_count = usize::try_from(sector_count).unwrap();
        let size = sector_count.checked_mul(SECTOR_SIZE).unwrap();
        assert!(size.is_multiple_of(PAGE_SIZE));
        grant_kernel_capability(sender, Arc::new(VirtualMemoryMapping::new(region, size)))
    }

    fn write(&self, _: ProcessId, sector: u64, data: &[u8]) {
        self.write(sector, data.try_into().unwrap()).unwrap()
    }

    fn capacity(&self, _: ProcessId) -> u64 {
        self.capacity()
    }
}

impl VirtualMemoryLoader for MappedRegion {
    fn load_page(&self, page_index: usize) -> Box<Page> {
        let sector_offset = self.sector_offset + (page_index * PAGE_SIZE / SECTOR_SIZE) as u64;
        let mut page = Box::new(Page([0; _]));
        for (i, block) in page.0.as_chunks_mut().0.iter_mut().enumerate() {
            self.blk.read(sector_offset + i as u64, block).unwrap();
        }
        page
    }
}

fn result_from_status(status: u8) -> Result<(), VirtioBlkError> {
    match status {
        0 => Ok(()),
        1 => Err(VirtioBlkError),
        _ => unreachable!(),
    }
}
