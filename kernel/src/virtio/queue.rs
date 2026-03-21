use crate::util::volatile::Volatile;
use crate::virtio::{NotifySlot, VirtioCommonConfig};
use alloc::alloc::{alloc_zeroed, handle_alloc_error};
use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::alloc::Layout;

pub const QUEUE_SIZE: usize = 16;

#[repr(C, align(16))]
#[derive(Clone, Default)]
pub struct Descriptor {
    pub address: u64,
    pub length: u32,
    pub flags: u16,
    pub next: u16,
}

#[repr(C, align(2))]
pub struct AvailableRing {
    pub flags: u16,
    pub index: u16,
    pub ring: [u16],
}

#[repr(C)]
pub struct UsedElement {
    pub id: u32,
    pub len: u32,
}

#[repr(C, align(4))]
pub struct UsedRing {
    pub flags: u16,
    pub index: u16,
    pub ring: [UsedElement],
}

pub struct Queue {
    pub descriptors: Vec<Descriptor>,
    pub available: Box<AvailableRing>,
    pub used: Box<UsedRing>,
    pub notify: Volatile<u16>,
    pub index: u16,
}

const VIRTQ_DESC_F_NEXT: u16 = 1;
const VIRTQ_DESC_F_WRITE: u16 = 2;

impl AvailableRing {
    fn new(size: usize) -> Box<AvailableRing> {
        let layout = Layout::from_size_align(4 + 2 * size, 2).unwrap();
        let thin = unsafe { alloc_zeroed(layout) };
        if thin.is_null() {
            handle_alloc_error(layout);
        }
        let fat = core::ptr::from_raw_parts_mut(thin, size);
        unsafe { Box::from_raw(fat) }
    }
}

impl UsedRing {
    fn new(size: usize) -> Box<UsedRing> {
        let layout = Layout::from_size_align(4 + 8 * size, 4).unwrap();
        let thin = unsafe { alloc_zeroed(layout) };
        if thin.is_null() {
            handle_alloc_error(layout);
        }
        let fat = core::ptr::from_raw_parts_mut(thin, size);
        unsafe { Box::from_raw(fat) }
    }
}

impl Queue {
    pub fn new(
        index: u16,
        common: Volatile<VirtioCommonConfig>,
        notify: &NotifySlot,
        size: usize,
    ) -> Queue {
        common.queue_select().write(index);

        assert!(size <= common.queue_size().read() as usize);
        common.queue_size().write(size as u16);

        let descriptors = vec![Descriptor::default(); size];
        let available = AvailableRing::new(size);
        let used = UsedRing::new(size);
        common.queue_desc().write(descriptors.as_ptr() as u64);
        common
            .queue_driver()
            .write(&*available as *const _ as *const u8 as u64);
        common
            .queue_device()
            .write(&*used as *const _ as *const u8 as u64);

        common.queue_enable().write(1);

        let notify = unsafe { notify.select(common) };
        Queue {
            descriptors,
            available,
            used,
            notify,
            index,
        }
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

    pub fn send_and_recv(&mut self, descriptor: u16) {
        self.available.ring[self.available.index as usize % QUEUE_SIZE] = descriptor;
        riscv::asm::fence();
        self.available.index += 1;
        riscv::asm::fence();
        self.notify.write(self.index);
        while unsafe { (&raw const self.used.index).read_volatile() } < self.available.index {}
    }
}
