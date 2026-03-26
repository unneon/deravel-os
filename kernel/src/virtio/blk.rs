use crate::drvli::DriveServer;
use crate::interrupt::InterruptHandler;
use crate::util::volatile::{Readonly, Volatile, volatile_struct};
use crate::virtio::Capabilities;
use crate::virtio::queue::{QUEUE_SIZE, Queue};
use crate::virtio::registers::{STATUS_ACKNOWLEDGE, STATUS_DRIVER, STATUS_DRIVER_OK};
use alloc::boxed::Box;
use alloc::vec::Vec;
use log::{debug, info};
use riscv::register::satp::Mode;

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
    isr: Volatile<u8, Readonly>,
    device: Volatile<VirtioBlkConfig>,
    queue: Queue<0>,
}

#[derive(Debug)]
pub struct VirtioBlkError;

pub const VIRTIO_BLK_T_IN: u32 = 0;
pub const VIRTIO_BLK_T_OUT: u32 = 1;

impl VirtioBlk {
    pub fn new(caps: Capabilities<VirtioBlkConfig>) -> VirtioBlk {
        let common = caps.common;
        common.device_status().write(0);
        common.device_status().write_bitor(STATUS_ACKNOWLEDGE as u8);
        common.device_status().write_bitor(STATUS_DRIVER as u8);

        let capacity = caps.device.capacity().read();
        info!("drive has a capacity of {capacity} sectors");

        let queue = Queue::new(common, &caps.notify, QUEUE_SIZE);
        common.device_status().write_bitor(STATUS_DRIVER_OK as u8);

        VirtioBlk {
            isr: caps.isr,
            device: caps.device,
            queue,
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
        let old_satp = riscv::register::satp::read();
        unsafe { riscv::register::satp::set(Mode::Bare, 0, 0) }
        let capacity = self.device.capacity().read() as usize;
        unsafe { riscv::register::satp::write(old_satp) }
        capacity
    }
}

impl InterruptHandler for VirtioBlk {
    fn handle(&self) {
        debug!("interrupt handler, isr {:#x}", self.isr.read());
    }
}

impl DriveServer for VirtioBlk {
    fn read(&self, sector: u64) -> Vec<u8> {
        // TODO: Use a mutex here.
        #[allow(invalid_reference_casting)]
        let this = unsafe { &mut *(self as *const _ as *mut VirtioBlk) };
        let mut buf = Box::new([0u8; 512]);
        this.read(sector, &mut buf).unwrap();
        Vec::from(buf as Box<[u8]>)
    }

    fn write(&self, sector: u64, data: &[u8]) {
        // TODO: Use a mutex here.
        #[allow(invalid_reference_casting)]
        let this = unsafe { &mut *(self as *const _ as *mut VirtioBlk) };
        this.write(sector, data.try_into().unwrap()).unwrap()
    }

    fn capacity(&self) -> u64 {
        self.capacity() as u64
    }
}

fn result_from_status(status: u8) -> Result<(), VirtioBlkError> {
    match status {
        0 => Ok(()),
        1 => Err(VirtioBlkError),
        _ => unreachable!(),
    }
}
