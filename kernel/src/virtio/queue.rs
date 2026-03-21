use crate::util::volatile::Volatile;
use crate::virtio::VirtioCommonConfig;
use crate::virtio::registers::Registers;
use deravel_types::PAGE_SIZE;

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
    pub fn initialize(&self, queue_index: u16, regs: Volatile<VirtioCommonConfig>) {
        regs.queue_select().write(queue_index);
        assert!(QUEUE_SIZE <= regs.queue_size().read() as usize);
        regs.queue_size().write(QUEUE_SIZE as u16);
        regs.queue_desc().write(&raw const self.descriptors as u64);
        regs.queue_driver().write(&raw const self.available as u64);
        regs.queue_device().write(&raw const self.used as u64);
        regs.queue_enable().write(1);
    }

    pub fn initialize_legacy<T>(&self, queue_index: u32, regs: Volatile<Registers<T>>) {
        regs.queue_sel().write(queue_index);
        assert_eq!(regs.queue_pfn().read(), 0);
        assert!(QUEUE_SIZE <= regs.queue_size_max().read() as usize);
        regs.queue_size().write(QUEUE_SIZE as u32);
        regs.queue_align().write(PAGE_SIZE as u32);
        regs.queue_pfn()
            .write(((self as *const _ as usize) / PAGE_SIZE) as u32);
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
