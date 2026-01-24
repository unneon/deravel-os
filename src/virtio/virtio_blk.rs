use crate::page::{PAGE_SIZE, PageAligned};
use crate::virtio::queue::{
    Queue, VIRTIO_BLK_T_IN, VIRTIO_BLK_T_OUT, VIRTQ_DESC_F_NEXT, VIRTQ_DESC_F_WRITE,
};
use crate::virtio::registers::{
    LegacyMmioDeviceRegisters, STATUS_ACKNOWLEDGE, STATUS_DRIVER, STATUS_DRIVER_OK,
};

#[repr(C, packed)]
struct RequestHeader {
    type_: u32,
    reserved: u32,
    sector: u64,
}

enum RequestType {
    Read,
    Write,
}

pub struct VirtioBlk {
    regs: LegacyMmioDeviceRegisters,
}

#[derive(Debug)]
pub struct VirtioBlkError;

static mut VIRTQ: PageAligned<Queue> = unsafe { core::mem::zeroed() };

impl VirtioBlk {
    pub fn new(base_address: usize) -> VirtioBlk {
        let regs = LegacyMmioDeviceRegisters::new(base_address);
        initialize_device(&regs);
        VirtioBlk { regs }
    }

    pub fn read(&mut self, sector: u64, buf: &mut [u8; 512]) -> Result<(), VirtioBlkError> {
        request(sector, buf.as_ptr(), RequestType::Read, &self.regs)
    }

    #[allow(dead_code)]
    pub fn write(&mut self, sector: u64, buf: &[u8; 512]) -> Result<(), VirtioBlkError> {
        request(sector, buf.as_ptr(), RequestType::Write, &self.regs)
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

fn request(
    sector: u64,
    buf: *const u8,
    request_type: RequestType,
    regs: &LegacyMmioDeviceRegisters,
) -> Result<(), VirtioBlkError> {
    let request = RequestHeader {
        type_: match request_type {
            RequestType::Write => VIRTIO_BLK_T_OUT,
            RequestType::Read => VIRTIO_BLK_T_IN,
        },
        reserved: 0,
        sector,
    };
    let status: u8 = 0;

    let queue = unsafe { &mut *VIRTQ };

    queue.descriptors[0].address = &request as *const _ as u64;
    queue.descriptors[0].length = 16;
    queue.descriptors[0].flags = VIRTQ_DESC_F_NEXT;
    queue.descriptors[0].next = 1;

    queue.descriptors[1].address = buf as u64;
    queue.descriptors[1].length = 512;
    queue.descriptors[1].flags = VIRTQ_DESC_F_NEXT
        | match request_type {
            RequestType::Read => VIRTQ_DESC_F_WRITE,
            RequestType::Write => 0,
        };
    queue.descriptors[1].next = 2;

    queue.descriptors[2].address = &status as *const _ as u64;
    queue.descriptors[2].length = 1;
    queue.descriptors[2].flags = VIRTQ_DESC_F_WRITE;

    queue.send_and_recv(0, 0, regs);
    match status {
        0 => Ok(()),
        1 => Err(VirtioBlkError),
        _ => unreachable!(),
    }
}
