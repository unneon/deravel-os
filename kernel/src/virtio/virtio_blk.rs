use crate::util::volatile::{Volatile, volatile_struct};
use crate::virtio::queue::{QUEUE_SIZE, Queue};
use crate::virtio::registers::{STATUS_ACKNOWLEDGE, STATUS_DRIVER, STATUS_DRIVER_OK};
use crate::virtio::{NotifySlot, VirtioCommonConfig};
use log::info;

volatile_struct! { pub VirtioBlkConfig
    capacity: Readonly u64,
}

#[repr(C, packed)]
struct Header {
    type_: u32,
    reserved: u32,
    sector: u64,
}

pub struct VirtioBlk {
    device: Volatile<VirtioBlkConfig>,
    queue: Queue,
}

#[derive(Debug)]
pub struct VirtioBlkError;

pub const VIRTIO_BLK_T_IN: u32 = 0;
pub const VIRTIO_BLK_T_OUT: u32 = 1;

impl VirtioBlk {
    pub fn new(
        common: Volatile<VirtioCommonConfig>,
        notify: NotifySlot,
        device: Volatile<VirtioBlkConfig>,
    ) -> VirtioBlk {
        common.device_status().write(0);
        common.device_status().write_bitor(STATUS_ACKNOWLEDGE as u8);
        common.device_status().write_bitor(STATUS_DRIVER as u8);
        let queue = Queue::new(0, common, &notify, QUEUE_SIZE);
        common.device_status().write_bitor(STATUS_DRIVER_OK as u8);

        let capacity = device.capacity().read();
        info!("disk has a capacity of {capacity} sectors");

        VirtioBlk { device, queue }
    }

    #[allow(dead_code)]
    pub fn read(&mut self, sector: u64, buf: &mut [u8; 512]) -> Result<(), VirtioBlkError> {
        let header = Header {
            type_: VIRTIO_BLK_T_IN,
            reserved: 0,
            sector,
        };
        let mut status: u8 = 0;
        self.queue.descriptor_readonly(0, &header, Some(1));
        self.queue.descriptor_writeonly(1, buf, Some(2));
        self.queue.descriptor_writeonly(2, &mut status, None);
        self.queue.send_and_recv(0);
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
        self.queue.descriptor_readonly(0, &header, Some(1));
        self.queue.descriptor_readonly(1, buf, Some(2));
        self.queue.descriptor_writeonly(2, &mut status, None);
        self.queue.send_and_recv(0);
        result_from_status(status)
    }

    pub fn capacity(&self) -> usize {
        self.device.capacity().read() as usize
    }
}

fn result_from_status(status: u8) -> Result<(), VirtioBlkError> {
    match status {
        0 => Ok(()),
        1 => Err(VirtioBlkError),
        _ => unreachable!(),
    }
}
