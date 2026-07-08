use crate::page::{Page, PageTable};
use deravel_types::PAGE_SIZE;

// Does not include the V flag, as this one impacts the meaning of other bits.
#[derive(Clone, Copy)]
pub struct PageFlags(usize);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(transparent)]
pub struct PageTableEntry<const LEVEL: usize>(pub usize);

pub enum PageTableEntryUnpacked<const LEVEL: usize>
where
    [(); LEVEL - 1]:,
{
    // This variant could have a u63 for kernel use.
    Invalid,
    Indirect {
        phys_ptr: *mut PageTable<{ LEVEL - 1 }>,
    },
    #[allow(dead_code)]
    Leaf {
        flags: PageFlags,
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

impl<const LEVEL: usize> PageTableEntry<LEVEL> {
    pub fn indirect(table: *mut PageTable<{ LEVEL - 1 }>) -> PageTableEntry<LEVEL> {
        PageTableEntry(((table as usize / PAGE_SIZE) << 10) | PAGE_V)
    }

    pub fn leaf(physical_addr: usize, flags: PageFlags) -> PageTableEntry<LEVEL> {
        PageTableEntry(((physical_addr / PAGE_SIZE) << 10) | PAGE_V | flags.0)
    }

    pub fn unpack(&self) -> PageTableEntryUnpacked<LEVEL>
    where
        [(); LEVEL - 1]:,
    {
        if !self.is_valid() {
            PageTableEntryUnpacked::Invalid
        } else if self.is_indirect() {
            let phys_ptr = self.physical_page_pointer() as *mut _;
            PageTableEntryUnpacked::Indirect { phys_ptr }
        } else {
            let flags = PageFlags(self.0 & 0b1111_1110);
            let phys_ptr = self.physical_page_pointer() as *mut _;
            PageTableEntryUnpacked::Leaf { flags, phys_ptr }
        }
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
