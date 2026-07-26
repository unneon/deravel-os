use crate::pci::capability::PciCapability;
use crate::util::volatile::VolatileCellWithPureReads;
use core::mem::ManuallyDrop;
use core::ops::{Deref, DerefMut};

#[repr(align(4096))]
pub union Config<T> {
    data: ManuallyDrop<T>,
    bytes: [u8; 4096],
}

pub union ConfigUntyped {
    common: ManuallyDrop<Config<Common>>,
    general_device: ManuallyDrop<Config<GeneralDevice>>,
}

#[repr(C)]
pub struct Common {
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

#[repr(C)]
pub struct GeneralDevice {
    pub common: Common,
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

const _: () = assert!(size_of::<Config<Common>>() == 4096);
const _: () = assert!(size_of::<Config<GeneralDevice>>() == 4096);

impl ConfigUntyped {
    pub fn as_general_device(&mut self) -> Option<&mut Config<GeneralDevice>> {
        if self.header_type != 0x0 {
            return None;
        }
        Some(unsafe { &mut self.general_device })
    }
}

impl Config<GeneralDevice> {
    pub fn walk_capabilities(&self) -> impl Iterator<Item = &PciCapability> {
        assert_ne!(self.status.read() & (1 << 4), 0);
        let mut pointer = self.capabilities_pointer & !0x3;
        core::iter::from_fn(move || {
            if pointer == 0 {
                return None;
            }
            let cap =
                unsafe { &*(&raw const self.bytes[pointer as usize] as *const PciCapability) };
            pointer = cap.next;
            Some(cap)
        })
    }
}

impl<T> Deref for Config<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &self.data }
    }
}

impl<T> DerefMut for Config<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut self.data }
    }
}

impl Deref for ConfigUntyped {
    type Target = Common;

    fn deref(&self) -> &Common {
        unsafe { &self.common.data }
    }
}

impl Deref for GeneralDevice {
    type Target = Common;

    fn deref(&self) -> &Common {
        &self.common
    }
}

impl DerefMut for GeneralDevice {
    fn deref_mut(&mut self) -> &mut Common {
        &mut self.common
    }
}
