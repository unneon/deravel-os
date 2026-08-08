use crate::page::{Page, PageTable, phys_to_virt, virt_to_phys};
use alloc::boxed::Box;
use core::fmt::Write;
use deravel_types::PAGE_SIZE;

// Does not include the V flag, as this one impacts the meaning of other bits.
#[derive(Clone, Copy)]
pub struct PageFlags(usize);

#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[repr(transparent)]
pub struct PageTableEntry(pub usize);

#[derive(Debug)]
pub enum PageTableEntryUnpacked<'a> {
    Invalid,
    Indirect(&'a PageTable),
    Leaf {
        #[allow(dead_code)]
        flags: PageFlags,
        phys_ptr: *mut Page,
    },
}

#[derive(Debug)]
pub enum PageTableEntryUnpackedMut<'a> {
    Invalid,
    Indirect(&'a mut PageTable),
    Leaf {
        #[allow(dead_code)]
        flags: PageFlags,
        #[allow(dead_code)]
        phys_ptr: *mut Page,
    },
}

const PAGE_V: usize = 1 << 0;
const PAGE_R: usize = 1 << 1;
const PAGE_W: usize = 1 << 2;
const PAGE_X: usize = 1 << 3;
const PAGE_U: usize = 1 << 4;

impl PageFlags {
    pub fn readonly() -> PageFlags {
        PageFlags(PAGE_R)
    }

    pub fn readwrite() -> PageFlags {
        PageFlags(PAGE_R | PAGE_W)
    }

    pub fn executable() -> PageFlags {
        PageFlags(PAGE_R | PAGE_X)
    }

    pub fn user(self) -> PageFlags {
        PageFlags(self.0 | PAGE_U)
    }

    pub fn is_writable(&self) -> bool {
        self.0 & PAGE_W != 0
    }
}

impl PageTableEntry {
    pub fn invalid() -> PageTableEntry {
        PageTableEntry(0)
    }

    pub fn indirect(table: Box<PageTable>) -> PageTableEntry {
        PageTableEntry(((virt_to_phys(Box::into_raw(table)) as usize / PAGE_SIZE) << 10) | PAGE_V)
    }

    pub fn leaf(phys: usize, flags: PageFlags) -> PageTableEntry {
        PageTableEntry(((phys / PAGE_SIZE) << 10) | PAGE_V | flags.0)
    }

    pub fn unpack(&self) -> PageTableEntryUnpacked<'_> {
        if !self.is_valid() {
            PageTableEntryUnpacked::Invalid
        } else if self.is_indirect() {
            let indirect = phys_to_virt(self.physical_page_pointer() as *const PageTable);
            PageTableEntryUnpacked::Indirect(unsafe { &*indirect })
        } else {
            let flags = PageFlags(self.0 & 0b1111_1110);
            let phys_ptr = self.physical_page_pointer() as *mut _;
            PageTableEntryUnpacked::Leaf { flags, phys_ptr }
        }
    }

    pub fn unpack_mut(&mut self) -> PageTableEntryUnpackedMut<'_> {
        if !self.is_valid() {
            PageTableEntryUnpackedMut::Invalid
        } else if self.is_indirect() {
            let indirect = phys_to_virt(self.physical_page_pointer() as *mut PageTable);
            PageTableEntryUnpackedMut::Indirect(unsafe { &mut *indirect })
        } else {
            let flags = PageFlags(self.0 & 0b1111_1110);
            let phys_ptr = self.physical_page_pointer() as *mut _;
            PageTableEntryUnpackedMut::Leaf { flags, phys_ptr }
        }
    }

    pub fn take(&mut self) -> PageTableEntry {
        core::mem::replace(self, PageTableEntry::invalid())
    }

    fn physical_page_pointer(&self) -> usize {
        self.physical_page_number() * PAGE_SIZE
    }

    fn physical_page_number(&self) -> usize {
        debug_assert!(self.is_valid());
        self.0 >> 10
    }

    fn is_indirect(&self) -> bool {
        self.is_valid() && self.0 & (PAGE_R | PAGE_W | PAGE_X) == 0
    }

    pub fn is_valid(&self) -> bool {
        self.0 & PAGE_V != 0
    }
}

impl Drop for PageTableEntry {
    fn drop(&mut self) {
        if self.is_indirect() {
            let indirect = phys_to_virt(self.physical_page_pointer() as *mut PageTable);
            let _ = unsafe { Box::from_raw(indirect) };
        }
    }
}

impl core::fmt::Debug for PageFlags {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.0 == 0 {
            return f.write_str("0");
        }
        if self.0 & PAGE_V != 0 {
            f.write_char('V')?;
        }
        if self.0 & PAGE_R != 0 {
            f.write_char('R')?;
        }
        if self.0 & PAGE_W != 0 {
            f.write_char('W')?;
        }
        if self.0 & PAGE_X != 0 {
            f.write_char('X')?;
        }
        if self.0 & PAGE_U != 0 {
            f.write_char('U')?;
        }
        Ok(())
    }
}
