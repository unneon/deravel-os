use crate::page::PageFlags;
use crate::page::entry::{PageTableEntry, PageTableEntryUnpacked};
use alloc::boxed::Box;
use deravel_types::PAGE_SIZE;

#[repr(align(4096))]
pub struct PageTable<const LEVEL: usize>(
    pub [PageTableEntry<LEVEL>; PAGE_SIZE / size_of::<usize>()],
);

pub type TopPageTable = PageTable<2>;

impl<const LEVEL: usize> PageTable<LEVEL> {
    pub const fn new() -> PageTable<LEVEL> {
        PageTable([PageTableEntry(0); _])
    }

    unsafe fn get_or_create_indirect(
        &mut self,
        vpn_segment: usize,
    ) -> &'static mut PageTable<{ LEVEL - 1 }> {
        match self.0[vpn_segment].unpack() {
            PageTableEntryUnpacked::Invalid => {
                let indirect = Box::leak(Box::new(PageTable::new()));
                self.0[vpn_segment] = PageTableEntry::indirect(indirect as *mut _);
                indirect
            }
            PageTableEntryUnpacked::Indirect { phys_ptr } => unsafe { &mut *phys_ptr },
            PageTableEntryUnpacked::Leaf { .. } => unreachable!(),
        }
    }
}

impl TopPageTable {
    pub fn map_page(&mut self, virtual_addr: usize, physical_addr: usize, flags: PageFlags) {
        assert!(virtual_addr.is_multiple_of(PAGE_SIZE));
        assert!(physical_addr.is_multiple_of(PAGE_SIZE));

        let vpn2 = (virtual_addr >> 30) & ((1 << 9) - 1);
        let table1 = unsafe { self.get_or_create_indirect(vpn2) };
        let vpn1 = (virtual_addr >> 21) & ((1 << 9) - 1);
        let table0 = unsafe { table1.get_or_create_indirect(vpn1) };

        let vpn0 = (virtual_addr >> 12) & ((1 << 9) - 1);
        assert!(!table0.0[vpn0].is_valid());
        table0.0[vpn0] = PageTableEntry::leaf(physical_addr, flags);
    }
}

impl<const LEVEL: usize> Default for PageTable<LEVEL> {
    fn default() -> PageTable<LEVEL> {
        PageTable([PageTableEntry::default(); _])
    }
}
