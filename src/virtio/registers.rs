macro register {
    ($offset:literal, $type:ident) => {},
    ($offset:literal, $type:ident, read $name:ident $($tail:tt)*) => {
        #[allow(dead_code)]
        pub fn $name(&self) -> $type {
            unsafe { ((self.base_address + $offset) as *const $type).read_volatile() }
        }
        register!($offset, $type $($tail)*);
    },
    ($offset:literal, $type:ident, write $name:ident $($tail:tt)*) => {
        #[allow(dead_code)]
        pub fn $name(&self, value: $type) {
            unsafe { ((self.base_address + $offset) as *mut $type).write_volatile(value) }
        }
        register!($offset, $type $($tail)*);
    },
    ($offset:literal, $type:ident, or $name:ident $($tail:tt)*) => {
        pub fn $name(&self, value: $type) {
            unsafe {
                let p = ((self.base_address + $offset) as *mut $type);
                p.write_volatile(p.read_volatile() | value)
            }
        }
        register!($offset, $type $($tail)*);
    },
}

pub struct LegacyMmioDeviceRegisters {
    pub base_address: usize,
}

pub const STATUS_ACKNOWLEDGE: u32 = 1;
pub const STATUS_DRIVER: u32 = 2;
pub const STATUS_DRIVER_OK: u32 = 4;

impl LegacyMmioDeviceRegisters {
    register!(0x000, u32, read magic_value);
    register!(0x004, u32, read version);
    register!(0x008, u32, read device_id);
    register!(0x00c, u32, read vendor_id);
    register!(0x010, u32, read device_features);
    register!(0x014, u32, write set_device_features_sel);
    register!(0x020, u32, write set_driver_features);
    register!(0x024, u32, write set_driver_features_sel);
    register!(0x028, u32, write set_guest_page_size);
    register!(0x030, u32, write set_queue_sel);
    register!(0x034, u32, read queue_size_max);
    register!(0x038, u32, write set_queue_size);
    register!(0x03c, u32, write set_queue_align);
    register!(0x040, u32, read queue_pfn, write set_queue_pfn);
    register!(0x050, u32, write set_queue_notify);
    register!(0x070, u32, read device_status, write set_device_status, or or_device_status);
}

impl core::fmt::Display for LegacyMmioDeviceRegisters {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:#x}", self.base_address)
    }
}
