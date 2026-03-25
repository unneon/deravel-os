use crate::pci::capability::PciCapability;
use crate::util::volatile::{Volatile, volatile_struct};

volatile_struct! { pub CommonConfig
    pub vendor_id: Readonly u16,
    pub device_id: Readonly u16,
    pub command: ReadWrite u16,
    pub status: ReadWrite u16,
    pub revision_id: Readonly u8,
    pub prog_if: Readonly u8,
    pub subclass: Readonly u8,
    pub class_code: Readonly u8,
    pub cache_line_size: Readonly u8,
    pub latency_timer: Readonly u8,
    pub header_type: Readonly u8,
    pub bist: Readonly u8,
}

volatile_struct! { pub GeneralDeviceConfig
    pub common: ReadWrite CommonConfig,
    pub bars: ReadWrite [u32; 6],
    pub cardbus_cis_pointer: Readonly u32,
    pub subsystem_vendor_id: Readonly u16,
    pub subsystem_id: Readonly u16,
    pub expansion_rom_base_address: Readonly u32,
    pub capabilities_pointer: Readonly u8,
    _reserved0: Readonly [u8; 3],
    _reserved1: Readonly u32,
    pub interrupt_line: Readonly u8,
    pub interrupt_pin: Readonly u8,
    pub min_grant: Readonly u8,
    pub max_latency: Readonly u8,
}

impl CommonConfig {
    pub fn as_general_device(
        self: Volatile<CommonConfig>,
    ) -> Option<Volatile<GeneralDeviceConfig>> {
        if self.header_type().read() != 0x0 {
            return None;
        }
        let pointer: *mut CommonConfig = From::from(self);
        Some(unsafe { Volatile::new(pointer as *mut GeneralDeviceConfig) })
    }
}

impl GeneralDeviceConfig {
    pub fn walk_capabilities(
        self: Volatile<GeneralDeviceConfig>,
    ) -> impl Iterator<Item = &'static PciCapability> {
        assert_ne!(self.common().status().read() & (1 << 4), 0);
        let config_space: *mut GeneralDeviceConfig = From::from(self);
        let mut pointer = self.capabilities_pointer().read() & !0x3;
        core::iter::from_fn(move || {
            if pointer == 0 {
                return None;
            }
            let cap =
                unsafe { &*(config_space.byte_add(pointer as usize) as *const PciCapability) };
            pointer = cap.next;
            Some(cap)
        })
    }
}
