use crate::page::entry::{PageTableEntry, PageTableEntryUnpacked};
use crate::page::{
    LEVEL_2_PAGE_SIZE, MAX_PHYSICAL_ADDR, MAX_VIRTUAL_ADDR, PAGE_TABLE_ENTRY_COUNT, PageFlags,
    phys_to_virt, virt_to_phys,
};
use alloc::boxed::Box;
use core::ops::Range;
use deravel_types::PAGE_SIZE;
use log::*;

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
    #[track_caller]
    pub fn map_pages(&mut self, virt: usize, phys: usize, size: usize, flags: PageFlags) {
        assert!(virt.is_multiple_of(PAGE_SIZE));
        assert!(virt < MAX_VIRTUAL_ADDR);
        assert!(virt + size <= MAX_VIRTUAL_ADDR);
        assert!(phys.is_multiple_of(PAGE_SIZE));
        assert!(phys < MAX_PHYSICAL_ADDR);
        assert!(phys + size <= MAX_PHYSICAL_ADDR);
        assert!(size.is_multiple_of(PAGE_SIZE));
        if self
            .try_map_with_leaf_pages(virt, phys, size, flags)
            .is_err()
        {
            self.map_without_huge_pages(virt, phys, size, flags);
        }
    }

    fn try_map_with_leaf_pages(
        &mut self,
        virt: usize,
        phys: usize,
        size: usize,
        flags: PageFlags,
    ) -> Result<(), ()> {
        let (prefix, l2p_aligned, suffix) = align_by(virt..virt + size, LEVEL_2_PAGE_SIZE);
        if !l2p_aligned.is_empty()
            && !(phys + (l2p_aligned.start - virt)).is_multiple_of(LEVEL_2_PAGE_SIZE)
        {
            // Sv39 supports 2 MiB megapages and 1 GiB gigapages, each of which must be virtually
            // and physically aligned to a boundary equal to its size. (RISC-V Privileged 12.4.1).
            warn!("physical address {phys:#x} not gigapage-aligned with {virt:#x}");
            return Err(());
        }
        for v in prefix.step_by(PAGE_SIZE) {
            self.map_page(v, phys + (v - virt), flags);
        }
        for v in l2p_aligned.step_by(LEVEL_2_PAGE_SIZE) {
            self.map_level_2_page(v, phys + (v - virt), flags);
        }
        for v in suffix.step_by(PAGE_SIZE) {
            self.map_page(v, phys + (v - virt), flags);
        }
        Ok(())
    }

    fn map_without_huge_pages(&mut self, virt: usize, phys: usize, size: usize, flags: PageFlags) {
        for v in (virt..virt + size).step_by(PAGE_SIZE) {
            self.map_page(v, phys + (v - virt), flags);
        }
    }

    fn map_page(&mut self, virt: usize, phys: usize, flags: PageFlags) {
        assert!(virt.is_multiple_of(PAGE_SIZE));
        assert!(phys.is_multiple_of(PAGE_SIZE));
        assert!(phys < MAX_PHYSICAL_ADDR);

        let vpn2 = (virt >> 30) & ((1 << 9) - 1);
        let table1 = unsafe { self.get_or_create_indirect(vpn2) };
        let vpn1 = (virt >> 21) & ((1 << 9) - 1);
        let table0 = unsafe { table1.get_or_create_indirect(vpn1) };

        let vpn0 = (virt >> 12) & ((1 << 9) - 1);
        table0.map_entry(vpn0, PageTableEntry::leaf(phys, flags));
    }

    fn map_level_2_page(&mut self, virt: usize, phys: usize, flags: PageFlags) {
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

fn align_by(range: Range<usize>, align: usize) -> (Range<usize>, Range<usize>, Range<usize>) {
    let aligned_start = range.start.next_multiple_of(align);
    if aligned_start >= range.end {
        return (range, 0..0, 0..0);
    }
    let unaligned_prefix = range.start..aligned_start;
    let mut aligned_end = range.end.next_multiple_of(align);
    if aligned_end > range.end {
        aligned_end -= align;
    }
    let aligned = aligned_start..aligned_end;
    let unaligned_suffix = aligned_end..range.end;
    (unaligned_prefix, aligned, unaligned_suffix)
}
