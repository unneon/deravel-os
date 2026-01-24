use crate::page::{PAGE_SIZE, PageAligned};
use crate::virtio::queue::Queue;
use crate::virtio::registers::{
    LegacyMmioDeviceRegisters, STATUS_ACKNOWLEDGE, STATUS_DRIVER, STATUS_DRIVER_OK,
};

#[repr(C, packed)]
struct Header {
    type_: u32,
    reserved: u32,
    sector: u64,
}

pub struct VirtioBlk {
    regs: LegacyMmioDeviceRegisters,
}

#[derive(Debug)]
pub struct VirtioBlkError;

pub const VIRTIO_BLK_T_IN: u32 = 0;
pub const VIRTIO_BLK_T_OUT: u32 = 1;

static mut VIRTQ: PageAligned<Queue> = unsafe { core::mem::zeroed() };

impl VirtioBlk {
    pub fn new(base_address: usize) -> VirtioBlk {
        let regs = LegacyMmioDeviceRegisters::new(base_address);
        initialize_device(&regs);
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
        queue.send_and_recv(0, 0, &self.regs);
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
        queue.send_and_recv(0, 0, &self.regs);
        result_from_status(status)
    }
}

fn initialize_device(regs: &LegacyMmioDeviceRegisters) {
    assert_eq!(regs.magic_value(), 0x74726976);
    assert_eq!(regs.version(), 1);
    assert_eq!(regs.device_id(), 2);

    regs.set_device_status(0);
    regs.or_device_status(STATUS_ACKNOWLEDGE);
    regs.or_device_status(STATUS_DRIVER);

    regs.set_guest_page_size(PAGE_SIZE as u32);

    unsafe { &VIRTQ }.initialize(0, regs);

    regs.or_device_status(STATUS_DRIVER_OK);
}

fn result_from_status(status: u8) -> Result<(), VirtioBlkError> {
    match status {
        0 => Ok(()),
        1 => Err(VirtioBlkError),
        _ => unreachable!(),
    }
}
