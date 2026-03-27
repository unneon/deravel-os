use crate::pci::capability::PciCapability;
use crate::util::volatile::VolatileCellWithPureReads;
use core::mem::ManuallyDrop;
use core::ops::{Deref, DerefMut};

#[repr(align(4096))]
pub union Config {
    common: ManuallyDrop<CommonConfig>,
    general_device: ManuallyDrop<GeneralDeviceConfig>,
}

#[repr(C)]
pub struct CommonConfig {
    pub vendor_id: u16,
    pub device_id: u16,
    pub command: VolatileCellWithPureReads<u16>,
    pub status: VolatileCellWithPureReads<u16>,
    pub revision_id: u8,
    pub prog_if: u8,
    pub subclass: u8,
    pub class_code: u8,
    pub cache_line_size: u8,
    pub latency_timer: u8,
    pub header_type: u8,
    pub bist: u8,
}

#[allow(dead_code)]
#[repr(C)]
pub struct GeneralDeviceConfig {
    pub common: CommonConfig,
    pub bars: [VolatileCellWithPureReads<u32>; 6],
    pub cardbus_cis_pointer: u32,
    pub subsystem_vendor_id: u16,
    pub subsystem_id: u16,
    pub expansion_rom_base_address: u32,
    pub capabilities_pointer: u8,
    _reserved0: [u8; 3],
    _reserved1: u32,
    pub interrupt_line: u8,
    pub interrupt_pin: u8,
    pub min_grant: u8,
    pub max_latency: u8,
}

impl Config {
    pub fn as_general_device(&mut self) -> Option<&mut GeneralDeviceConfig> {
        if self.header_type != 0x0 {
            return None;
        }
        Some(unsafe { &mut self.general_device })
    }
}

impl GeneralDeviceConfig {
    pub fn walk_capabilities(&self) -> impl Iterator<Item = &'static PciCapability> {
        assert_ne!(self.status.read() & (1 << 4), 0);
        let config_space = self as *const GeneralDeviceConfig;
        let mut pointer = self.capabilities_pointer & !0x3;
        core::iter::from_fn(move || {
            if pointer == 0 {
                return None;
            }
            // TODO: Should I do all this with unions for some safety?
            let cap =
                unsafe { &*(config_space.byte_add(pointer as usize) as *const PciCapability) };
            pointer = cap.next;
            Some(cap)
        })
    }
}

impl Deref for Config {
    type Target = CommonConfig;

    fn deref(&self) -> &CommonConfig {
        unsafe { &self.common }
    }
}

impl Deref for GeneralDeviceConfig {
    type Target = CommonConfig;

    fn deref(&self) -> &CommonConfig {
        &self.common
    }
}

impl DerefMut for GeneralDeviceConfig {
    fn deref_mut(&mut self) -> &mut CommonConfig {
        &mut self.common
    }
}
