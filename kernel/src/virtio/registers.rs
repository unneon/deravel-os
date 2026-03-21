use crate::util::volatile::{Volatile, volatile_struct_as_offsets};

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

volatile_struct_as_offsets! { pub Registers<T>
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

impl Registers<()> {
    pub unsafe fn new(base_address: *mut Self) -> Volatile<Self> {
        unsafe { Volatile::new(base_address) }
    }

    pub unsafe fn with_configuration<T>(self: Volatile<Registers<()>>) -> Volatile<Registers<T>> {
        let pointer: *mut Registers<()> = From::from(self);
        unsafe { Volatile::new(pointer as *mut Registers<T>) }
    }
}
