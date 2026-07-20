use crate::page::entry::{PageTableEntry, PageTableEntryUnpacked};
use crate::page::{
    LEVEL_2_PAGE_SIZE, MAX_PHYSICAL_ADDR, PAGE_TABLE_ENTRY_COUNT, PageFlags, phys_to_virt,
    virt_to_phys,
};
use alloc::boxed::Box;
use deravel_types::PAGE_SIZE;

#[repr(align(4096))]
pub struct PageTable<const LEVEL: usize>([PageTableEntry<LEVEL>; PAGE_TABLE_ENTRY_COUNT]);

pub type TopPageTable = PageTable<2>;

impl<const LEVEL: usize> PageTable<LEVEL> {
    pub const fn new() -> PageTable<LEVEL> {
        PageTable([PageTableEntry(0); _])
    }

    fn map_entry(&mut self, vpn_segment: usize, entry: PageTableEntry<LEVEL>) {
        assert!(!self.0[vpn_segment].is_valid());
        self.0[vpn_segment] = entry;
    }

    unsafe fn get_or_create_indirect(
        &mut self,
        vpn_segment: usize,
    ) -> &'static mut PageTable<{ LEVEL - 1 }> {
        match self.0[vpn_segment].unpack() {
            PageTableEntryUnpacked::Invalid => {
                let indirect = Box::leak(Box::new(PageTable::new()));
                self.0[vpn_segment] = PageTableEntry::indirect(virt_to_phys(indirect as *mut _));
                indirect
            }
            PageTableEntryUnpacked::Indirect { phys_ptr } => unsafe {
                &mut *phys_to_virt(phys_ptr)
            },
            PageTableEntryUnpacked::Leaf { .. } => unreachable!(),
        }
    }
}

impl TopPageTable {
    pub fn map_page(&mut self, virtual_addr: usize, phys: usize, virt: PageFlags) {
        assert!(virtual_addr.is_multiple_of(PAGE_SIZE));
        assert!(phys.is_multiple_of(PAGE_SIZE));
        assert!(phys < MAX_PHYSICAL_ADDR);

        let vpn2 = (virtual_addr >> 30) & ((1 << 9) - 1);
        let table1 = unsafe { self.get_or_create_indirect(vpn2) };
        let vpn1 = (virtual_addr >> 21) & ((1 << 9) - 1);
        let table0 = unsafe { table1.get_or_create_indirect(vpn1) };

        let vpn0 = (virtual_addr >> 12) & ((1 << 9) - 1);
        table0.map_entry(vpn0, PageTableEntry::leaf(phys, virt));
    }

    pub fn map_level_2_page(&mut self, virt: usize, phys: usize, flags: PageFlags) {
        assert!(virt.is_multiple_of(LEVEL_2_PAGE_SIZE));
        assert!(phys.is_multiple_of(LEVEL_2_PAGE_SIZE));
        assert!(phys < MAX_PHYSICAL_ADDR);

        let vpn2 = (virt >> 30) & ((1 << 9) - 1);
        self.map_entry(vpn2, PageTableEntry::leaf(phys, flags));
    }
}

impl<const LEVEL: usize> Default for PageTable<LEVEL> {
    fn default() -> PageTable<LEVEL> {
        PageTable([PageTableEntry::default(); _])
    }
}
