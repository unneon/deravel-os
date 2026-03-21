use crate::util::volatile::{ReadWrite, Volatile, volatile_struct};
use crate::virtio::VirtioCommonConfig;
use crate::virtio::queue::Queue;
use crate::virtio::registers::{STATUS_ACKNOWLEDGE, STATUS_DRIVER, STATUS_DRIVER_OK};
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
    common: Volatile<VirtioCommonConfig>,
    device: Volatile<VirtioBlkConfig>,
    notify: usize,
    notify_off_multiplier: u32,
}

#[derive(Debug)]
pub struct VirtioBlkError;

pub const VIRTIO_BLK_T_IN: u32 = 0;
pub const VIRTIO_BLK_T_OUT: u32 = 1;

static mut VIRTQ: Queue = unsafe { core::mem::zeroed() };

impl VirtioBlk {
    pub fn new(
        common: Volatile<VirtioCommonConfig>,
        device: Volatile<VirtioBlkConfig>,
        notify: usize,
        notify_off_multiplier: u32,
    ) -> VirtioBlk {
        initialize_device(common);

        let capacity = device.capacity().read();
        info!("disk has a capacity of {capacity} sectors");

        VirtioBlk {
            common,
            device,
            notify,
            notify_off_multiplier,
        }
    }

    #[allow(dead_code)]
    pub fn read(&mut self, sector: u64, buf: &mut [u8; 512]) -> Result<(), VirtioBlkError> {
        let header = Header {
            type_: VIRTIO_BLK_T_IN,
            reserved: 0,
            sector,
        };
        let mut status: u8 = 0;
        let queue = unsafe { &mut VIRTQ };
        queue.descriptor_readonly(0, &header, Some(1));
        queue.descriptor_writeonly(1, buf, Some(2));
        queue.descriptor_writeonly(2, &mut status, None);
        queue.send(0);
        self.notify_available();
        queue.recv();
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
        let queue = unsafe { &mut VIRTQ };
        queue.descriptor_readonly(0, &header, Some(1));
        queue.descriptor_readonly(1, buf, Some(2));
        queue.descriptor_writeonly(2, &mut status, None);
        queue.send(0);
        self.notify_available();
        queue.recv();
        result_from_status(status)
    }

    pub fn capacity(&self) -> usize {
        self.device.capacity().read() as usize
    }

    fn notify_available(&mut self) {
        let address = self.notify
            + self.common.queue_notify_off().read() as usize * self.notify_off_multiplier as usize;
        let pointer = unsafe { Volatile::<_, ReadWrite>::new(address as *mut u16) };
        pointer.write(0);
    }
}

fn initialize_device(common: Volatile<VirtioCommonConfig>) {
    common.device_status().write(0);
    common.device_status().write_bitor(STATUS_ACKNOWLEDGE as u8);
    common.device_status().write_bitor(STATUS_DRIVER as u8);

    unsafe { &VIRTQ }.initialize(0, common);

    common.device_status().write_bitor(STATUS_DRIVER_OK as u8);
}

fn result_from_status(status: u8) -> Result<(), VirtioBlkError> {
    match status {
        0 => Ok(()),
        1 => Err(VirtioBlkError),
        _ => unreachable!(),
    }
}
