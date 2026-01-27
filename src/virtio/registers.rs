use core::marker::PhantomData;
use core::mem::transmute;
use core::ops::{BitOr, Deref};

pub macro mmio($pub:vis struct $struct:ident $(<$($param:ident),*>)? { $($offset:literal $field_name:ident: $field_type:ident $access:ident,)* }) {
    $pub struct $struct $(<$($param),*> ($(PhantomData<$param>),*))?;

    impl$(<$($param),*>)? $struct $(<$($param),*>)? {
        $($pub fn $field_name(self: Mmio<Self, ReadWrite>) -> Mmio<$field_type, crate::virtio::registers::$access> {
            unsafe { transmute(self.0.byte_add($offset)) }
        })*
    }
}

pub trait Readable {}
pub trait Writable {}

pub struct Mmio<T, Access = ReadWrite>(*mut T, PhantomData<Access>);

pub struct Readonly;
pub struct Writeonly;
pub struct ReadWrite;

mmio! { pub struct Registers<T> {
    0x000 magic_value: u32 Readonly,
    0x004 version: u32 Readonly,
    0x008 device_id: u32 Readonly,
    0x00c vendor_id: u32 Readonly,
    0x010 host_features: u32 Readonly,
    0x014 host_features_sel: u32 Writeonly,
    0x020 driver_features: u32 Writeonly,
    0x024 driver_features_sel: u32 Writeonly,
    0x028 guest_page_size: u32 Writeonly,
    0x030 queue_sel: u32 Writeonly,
    0x034 queue_size_max: u32 Readonly,
    0x038 queue_size: u32 Writeonly,
    0x03c queue_align: u32 Writeonly,
    0x040 queue_pfn: u32 ReadWrite,
    0x050 queue_notify: u32 Writeonly,
    0x070 status: u32 ReadWrite,
    0x100 config: T ReadWrite,
} }

pub const STATUS_ACKNOWLEDGE: u32 = 1;
pub const STATUS_DRIVER: u32 = 2;
pub const STATUS_DRIVER_OK: u32 = 4;

impl<T: Copy, Access: Readable> Mmio<T, Access> {
    pub fn read(&self) -> T {
        unsafe { self.0.read_volatile() }
    }
}

impl<T, Access: Writable> Mmio<T, Access> {
    pub fn write(&self, value: T) {
        unsafe { self.0.write_volatile(value) }
    }
}

impl<T: BitOr<Output = T>, Access: Readable + Writable> Mmio<T, Access> {
    pub fn or(&self, value: T) {
        unsafe { self.0.write_volatile(self.0.read_volatile() | value) }
    }
}

impl<T> Registers<T> {
    pub unsafe fn new(base_address: *mut Self) -> Mmio<Self, ReadWrite> {
        Mmio(base_address, PhantomData)
    }
}

impl Registers<()> {
    pub unsafe fn with_configuration<T>(
        self: Mmio<Registers<()>, ReadWrite>,
    ) -> Mmio<Registers<T>, ReadWrite> {
        Mmio(self.0 as *mut Registers<T>, PhantomData)
    }
}

impl Readable for Readonly {}
impl Writable for Writeonly {}
impl Readable for ReadWrite {}
impl Writable for ReadWrite {}

impl<T, Access> Deref for Mmio<T, Access> {
    type Target = T;

    fn deref(&self) -> &T {
        unreachable!()
    }
}

impl<T, Access> Clone for Mmio<T, Access> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T, Access> Copy for Mmio<T, Access> {}

impl<T, Access> core::fmt::Display for Mmio<T, Access> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:#x}", self.0 as usize)
    }
}
