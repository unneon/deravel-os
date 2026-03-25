pub unsafe trait VendorPciCapability {}

#[repr(C)]
#[derive(Debug)]
pub struct PciCapability {
    pub vndr: u8,
    pub next: u8,
}

const PCI_CAP_ID_VNDR: u8 = 0x09;

impl PciCapability {
    pub unsafe fn get_vendor<T: VendorPciCapability>(&self) -> Option<&T> {
        if self.vndr == PCI_CAP_ID_VNDR {
            Some(unsafe { &*(self as *const Self as *const T) })
        } else {
            None
        }
    }
}
