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

pub const VIRTIO_BLK_T_IN: u32 = 0;
pub const VIRTIO_BLK_T_OUT: u32 = 1;
pub const VIRTQ_DESC_F_NEXT: u16 = 1;
pub const VIRTQ_DESC_F_WRITE: u16 = 2;

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

    pub fn send_and_recv(
        &mut self,
        descriptor: u16,
        queue_index: u32,
        regs: &LegacyMmioDeviceRegisters,
    ) {
        let last_index = self.available.index;
        let used_index_pointer = &raw const self.available.index;
        self.available.ring[last_index as usize % QUEUE_SIZE] = descriptor;
        self.available.index += 1;
        riscv::asm::fence();
        regs.set_queue_sel(queue_index);
        while unsafe { used_index_pointer.read_volatile() } < last_index + 1 {}
    }
}
