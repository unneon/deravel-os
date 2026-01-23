use crate::PAGE_SIZE;
use crate::virtio_net::QUEUE_SIZE;

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
