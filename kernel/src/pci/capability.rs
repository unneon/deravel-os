pub unsafe trait VendorPciCapability {}

pub struct BirAndOffset(u32);

pub struct MessageControl(u16);

#[repr(C)]
#[derive(Debug)]
pub struct PciCapability {
    pub vndr: u8,
    pub next: u8,
}

#[repr(C)]
#[derive(Debug)]
pub struct PciMsiXCapability {
    pub cap: PciCapability,
    pub message_control: MessageControl,
    pub table: BirAndOffset,
    pub pending: BirAndOffset,
}

const PCI_CAP_ID_VNDR: u8 = 0x09;
const PCI_CAP_ID_MSI_X: u8 = 0x11;

impl BirAndOffset {
    pub fn bir(&self) -> u8 {
        self.0 as u8 & 0b111
    }

    pub fn offset(&self) -> u32 {
        self.0 & !0b111
    }
}

impl MessageControl {
    pub fn table_size(&self) -> u16 {
        self.0 & ((1 << 11) - 1)
    }

    pub fn function_mask(&self) -> bool {
        self.0 & (1 << 14) != 0
    }

    pub fn enable(&self) -> bool {
        self.0 & (1 << 15) != 0
    }
}

impl PciCapability {
    pub unsafe fn get_vendor<T: VendorPciCapability>(&self) -> Option<&T> {
        if self.vndr == PCI_CAP_ID_VNDR {
            Some(unsafe { &*(self as *const Self as *const T) })
        } else {
            None
        }
    }

    pub fn get_msi_x(&self) -> Option<&PciMsiXCapability> {
        if self.vndr == PCI_CAP_ID_MSI_X {
            Some(unsafe { &*(self as *const Self as *const PciMsiXCapability) })
        } else {
            None
        }
    }
}

impl core::fmt::Debug for BirAndOffset {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("BirAndOffset")
            .field("bir", &self.bir())
            .field("offset", &self.offset())
            .finish()
    }
}

impl core::fmt::Debug for MessageControl {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MessageControl")
            .field("table_size", &self.table_size())
            .field("function_mask", &self.function_mask())
            .field("enable", &self.enable())
            .finish()
    }
}
