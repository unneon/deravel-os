use core::marker::PhantomData;
use core::mem::transmute;
use core::ops::{BitOr, Deref};

pub macro features($driver:ident $struct:ident $base:literal $($has_name:ident $enable_name:ident $bit:literal)*) {
    #[derive(Default)]
    pub struct $struct(u32);

    impl $struct {
        $(pub fn $has_name(&self) -> bool {
            self.0 & (1 << $bit) != 0
        }

        pub fn $enable_name(&mut self) {
            self.0 |= 1 << $bit;
        })*
    }

    impl From<$struct> for u32 {
        fn from(features: $struct) -> u32 {
            features.0
        }
    }
}

pub macro mmio($pub:vis $struct:ident $(<$($param:ident),*>)? $(where $param0:ident: $req0:ident)? $($offset:literal $field_name:ident: $access:ident $field_type:ty,)*) {
    $pub struct $struct $(<$($param),*> ($(PhantomData<$param>),*))?;

    impl$(<$($param),*>)? $struct $(<$($param),*>)? $(where $param0: $req0)? {
        $($pub fn $field_name(self: Mmio<Self>) -> Mmio<$field_type, crate::virtio::registers::$access> {
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

mmio! { pub Registers<T>
    0x000 magic_value: Readonly u32,
    0x004 version: Readonly u32,
    0x008 device_id: Readonly u32,
    0x00c vendor_id: Readonly u32,
    0x010 host_features: Readonly u32,
    0x014 host_features_sel: Writeonly u32,
    0x020 driver_features: Writeonly u32,
    0x024 driver_features_sel: Writeonly u32,
    0x028 guest_page_size: Writeonly u32,
    0x030 queue_sel: Writeonly u32,
    0x034 queue_size_max: Readonly u32,
    0x038 queue_size: Writeonly u32,
    0x03c queue_align: Writeonly u32,
    0x040 queue_pfn: ReadWrite u32,
    0x050 queue_notify: Writeonly u32,
    0x070 status: ReadWrite u32,
    0x100 config: ReadWrite T,
}

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

impl Registers<()> {
    pub unsafe fn new(base_address: *mut Self) -> Mmio<Self> {
        Mmio(base_address, PhantomData)
    }

    pub unsafe fn with_configuration<T>(self: Mmio<Registers<()>>) -> Mmio<Registers<T>> {
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
