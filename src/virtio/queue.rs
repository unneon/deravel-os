use crate::page::{PAGE_SIZE, PageAligned};
use crate::virtio::registers::LegacyMmioDeviceRegisters;

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

#[repr(C, packed)]
pub struct Used {
    pub flags: u16,
    pub index: u16,
    pub ring: [UsedElement; QUEUE_SIZE],
}

#[repr(C, packed)]
pub struct Queue {
    pub descriptors: [Descriptor; QUEUE_SIZE],
    pub available: Available,
    _used_pad: [u8; PAGE_SIZE - size_of::<[Descriptor; QUEUE_SIZE]>() - size_of::<Available>()],
    pub used: Used,
}

const VIRTQ_DESC_F_NEXT: u16 = 1;
const VIRTQ_DESC_F_WRITE: u16 = 2;

impl Queue {
    pub fn initialize(
        self: &PageAligned<Self>,
        queue_index: u32,
        regs: &LegacyMmioDeviceRegisters,
    ) {
        regs.set_queue_sel(queue_index);
        assert_eq!(regs.queue_pfn(), 0);
        assert!(QUEUE_SIZE <= regs.queue_size_max() as usize);
        regs.set_queue_size(QUEUE_SIZE as u32);
        regs.set_queue_align(PAGE_SIZE as u32);
        regs.set_queue_pfn(((self as *const _ as usize) / PAGE_SIZE) as u32);
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

    pub fn send_and_recv(
        &mut self,
        descriptor: u16,
        queue_index: u32,
        regs: &LegacyMmioDeviceRegisters,
    ) {
        self.available.ring[self.available.index as usize % QUEUE_SIZE] = descriptor;
        self.available.index += 1;
        riscv::asm::fence();
        regs.set_queue_notify(queue_index);
        while unsafe { (&raw const self.used.index).read_volatile() } < self.available.index {}
    }
}
