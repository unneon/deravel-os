use crate::util::volatile::Volatile;
use crate::virtio::{NotifySlot, VirtioCommonConfig};

pub const QUEUE_SIZE: usize = 16;

#[repr(C, packed)]
pub struct Descriptor {
    pub address: u64,
    pub length: u32,
    pub flags: u16,
    pub next: u16,
}

#[repr(C, packed)]
pub struct Available {
    pub flags: u16,
    pub index: u16,
    pub ring: [u16; QUEUE_SIZE],
}

#[repr(C, packed)]
pub struct UsedElement {
    pub id: u32,
    pub len: u32,
}

#[repr(C, align(4096))]
pub struct Used {
    pub flags: u16,
    pub index: u16,
    pub ring: [UsedElement; QUEUE_SIZE],
}

#[repr(C, align(4096))]
pub struct Queue {
    pub descriptors: [Descriptor; QUEUE_SIZE],
    pub available: Available,
    pub used: Used,
}

const VIRTQ_DESC_F_NEXT: u16 = 1;
const VIRTQ_DESC_F_WRITE: u16 = 2;

impl Queue {
    pub fn initialize(
        &self,
        queue_index: u16,
        common: Volatile<VirtioCommonConfig>,
        notify: &NotifySlot,
    ) -> Volatile<u16> {
        common.queue_select().write(queue_index);
        assert!(QUEUE_SIZE <= common.queue_size().read() as usize);
        common.queue_size().write(QUEUE_SIZE as u16);
        common
            .queue_desc()
            .write(&raw const self.descriptors as u64);
        common
            .queue_driver()
            .write(&raw const self.available as u64);
        common.queue_device().write(&raw const self.used as u64);
        common.queue_enable().write(1);
        unsafe { notify.select(common) }
    }

    pub fn descriptor_readonly<T>(&mut self, index: u16, data: &T, next: Option<u16>) {
        let descriptor = &mut self.descriptors[index as usize];
        descriptor.address = data as *const T as u64;
        descriptor.length = size_of::<T>() as u32;
        descriptor.flags = if next.is_some() { VIRTQ_DESC_F_NEXT } else { 0 };
        descriptor.next = next.unwrap_or(0);
    }

    pub fn descriptor_writeonly<T>(&mut self, index: u16, data: &mut T, next: Option<u16>) {
        let descriptor = &mut self.descriptors[index as usize];
        descriptor.address = data as *mut T as u64;
        descriptor.length = size_of::<T>() as u32;
        descriptor.flags = VIRTQ_DESC_F_WRITE | if next.is_some() { VIRTQ_DESC_F_NEXT } else { 0 };
        descriptor.next = next.unwrap_or(0);
    }

    pub fn send(&mut self, descriptor: u16) {
        self.available.ring[self.available.index as usize % QUEUE_SIZE] = descriptor;
        riscv::asm::fence();
        self.available.index += 1;
        riscv::asm::fence();
    }

    pub fn recv(&mut self) {
        while unsafe { (&raw const self.used.index).read_volatile() } < self.available.index {}
    }
}
