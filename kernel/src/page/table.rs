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

    fn map_leaf(&mut self, virt: usize, phys: usize, flags: PageFlags) {
        let vpn_segment = vpn_segment::<LEVEL>(virt);
        assert!(!self.0[vpn_segment].is_valid());
        self.0[vpn_segment] = PageTableEntry::leaf(phys, flags);
    }

    fn indirect(&mut self, virt: usize) -> &'static mut PageTable<{ LEVEL - 1 }> {
        let vpn_segment = vpn_segment::<LEVEL>(virt);
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
        if self.map_with_gigapages(virt, phys, size, flags).is_err() {
            self.map_without_gigapages(virt, phys, size, flags);
        }
    }

    fn map_with_gigapages(
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
            self.map_leaf(v, phys + (v - virt), flags);
        }
        for v in suffix.step_by(PAGE_SIZE) {
            self.map_page(v, phys + (v - virt), flags);
        }
        Ok(())
    }

    fn map_without_gigapages(&mut self, virt: usize, phys: usize, size: usize, flags: PageFlags) {
        for v in (virt..virt + size).step_by(PAGE_SIZE) {
            self.map_page(v, phys + (v - virt), flags);
        }
    }

    fn map_page(&mut self, virt: usize, phys: usize, flags: PageFlags) {
        assert!(virt.is_multiple_of(PAGE_SIZE));
        assert!(phys.is_multiple_of(PAGE_SIZE));
        assert!(phys < MAX_PHYSICAL_ADDR);

        let table1 = self.indirect(virt);
        let table0 = table1.indirect(virt);
        table0.map_leaf(virt, phys, flags);
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

fn vpn_segment<const LEVEL: usize>(virt: usize) -> usize {
    virt >> (12 + 9 * LEVEL) & ((1 << 9) - 1)
}
