use crate::page::{PAGE_SIZE, PageAligned};
use crate::virtio::queue::Queue;
use crate::virtio::registers::{
    Mmio, Registers, STATUS_ACKNOWLEDGE, STATUS_DRIVER, STATUS_DRIVER_OK, mmio,
};
use log::{debug, info};

mmio! { pub struct Configuration {
    0x000 capacity: u64 Readonly,
} }

#[repr(C, packed)]
struct Header {
    type_: u32,
    reserved: u32,
    sector: u64,
}

pub struct VirtioBlk {
    regs: Mmio<Registers<Configuration>>,
}

#[derive(Debug)]
pub struct VirtioBlkError;

pub const VIRTIO_BLK_T_IN: u32 = 0;
pub const VIRTIO_BLK_T_OUT: u32 = 1;

static mut VIRTQ: PageAligned<Queue> = unsafe { core::mem::zeroed() };

impl VirtioBlk {
    pub fn new(regs: Mmio<Registers<Configuration>>) -> VirtioBlk {
        initialize_device(regs);
        VirtioBlk { regs }
    }

    pub fn read(&mut self, sector: u64, buf: &mut [u8; 512]) -> Result<(), VirtioBlkError> {
        let header = Header {
            type_: VIRTIO_BLK_T_IN,
            reserved: 0,
            sector,
        };
        let mut status: u8 = 0;
        let queue = unsafe { &mut *VIRTQ };
        queue.descriptor_readonly(0, &header, Some(1));
        queue.descriptor_writeonly(1, buf, Some(2));
        queue.descriptor_writeonly(2, &mut status, None);
        queue.send_and_recv(0, 0, self.regs);
        result_from_status(status)
    }

    #[allow(dead_code)]
    pub fn write(&mut self, sector: u64, buf: &[u8; 512]) -> Result<(), VirtioBlkError> {
        let header = Header {
            type_: VIRTIO_BLK_T_OUT,
            reserved: 0,
            sector,
        };
        let mut status: u8 = 0;
        let queue = unsafe { &mut *VIRTQ };
        queue.descriptor_readonly(0, &header, Some(1));
        queue.descriptor_readonly(1, buf, Some(2));
        queue.descriptor_writeonly(2, &mut status, None);
        queue.send_and_recv(0, 0, self.regs);
        result_from_status(status)
    }

    pub fn demo(&mut self) {
        let mut buf = [0; 512];
        self.read(0, &mut buf).unwrap();
        debug!("read from disk: {:?}", str::from_utf8(&buf).unwrap());
    }
}

fn initialize_device(regs: Mmio<Registers<Configuration>>) {
    regs.status().write(0);
    regs.status().or(STATUS_ACKNOWLEDGE);
    regs.status().or(STATUS_DRIVER);

    regs.guest_page_size().write(PAGE_SIZE as u32);

    unsafe { &VIRTQ }.initialize(0, regs);

    regs.status().or(STATUS_DRIVER_OK);

    info!(
        "disk has a capacity of {} sectors",
        regs.config().capacity().read()
    );
}

fn result_from_status(status: u8) -> Result<(), VirtioBlkError> {
    match status {
        0 => Ok(()),
        1 => Err(VirtioBlkError),
        _ => unreachable!(),
    }
}
