use crate::page::entry::{PageTableEntry, PageTableEntryUnpacked};
use crate::page::{
    MAX_PHYSICAL_ADDR, MAX_VIRTUAL_ADDR, PAGE_TABLE_ENTRY_COUNT, PageFlags, phys_to_virt,
    virt_to_phys,
};
use alloc::boxed::Box;
use core::ops::Range;
use deravel_types::PAGE_SIZE;
use log::*;

#[repr(align(4096))]
pub struct PageTable<const LEVEL: usize>([PageTableEntry<LEVEL>; PAGE_TABLE_ENTRY_COUNT]);

pub type TopPageTable = PageTable<2>;

impl<const LEVEL: usize> PageTable<LEVEL> {
    const LEVEL_PAGE_SIZE: usize = PAGE_SIZE * PAGE_TABLE_ENTRY_COUNT.pow(LEVEL as u32);

    pub const fn new() -> PageTable<LEVEL> {
        PageTable([PageTableEntry(0); _])
    }

    fn map_range(
        &mut self,
        virt: usize,
        phys: usize,
        size: usize,
        flags: PageFlags,
        recurse: impl Fn(&'static mut PageTable<{ LEVEL - 1 }>, usize, usize, usize, PageFlags)
        + 'static,
    ) {
        let (prefix, aligned, suffix) = align_by(virt..virt + size, Self::LEVEL_PAGE_SIZE);

        if !prefix.is_empty() {
            recurse(
                self.map_indirect(prefix.start),
                prefix.start,
                phys + (prefix.start - virt),
                prefix.end - prefix.start,
                flags,
            );
        }

        if !aligned.is_empty()
            && !(phys + (aligned.start - virt)).is_multiple_of(Self::LEVEL_PAGE_SIZE)
        {
            // Sv39 supports 2 MiB megapages and 1 GiB gigapages, each of which must be virtually
            // and physically aligned to a boundary equal to its size. (RISC-V Privileged 12.4.1).
            warn!("physical address {phys:#x} not superpage-aligned with {virt:#x}");

            for v in aligned.step_by(Self::LEVEL_PAGE_SIZE) {
                recurse(
                    self.map_indirect(v),
                    v,
                    phys + (v - virt),
                    Self::LEVEL_PAGE_SIZE,
                    flags,
                );
            }
        } else {
            for v in aligned.step_by(Self::LEVEL_PAGE_SIZE) {
                self.map_leaf(v, phys + (v - virt), flags);
            }
        }

        if !suffix.is_empty() {
            recurse(
                self.map_indirect(suffix.start),
                suffix.start,
                phys + (suffix.start - virt),
                suffix.end - suffix.start,
                flags,
            );
        }
    }

    fn map_leaf(&mut self, virt: usize, phys: usize, flags: PageFlags) {
        let vpn_segment = vpn_segment::<LEVEL>(virt);
        assert!(!self.0[vpn_segment].is_valid());
        self.0[vpn_segment] = PageTableEntry::leaf(phys, flags);
    }

    fn map_indirect(&mut self, virt: usize) -> &'static mut PageTable<{ LEVEL - 1 }> {
        let vpn_segment = vpn_segment::<LEVEL>(virt);
        match self.0[vpn_segment].unpack() {
            PageTableEntryUnpacked::Invalid => {
                let indirect = Box::leak(Box::new(PageTable::new()));
                self.0[vpn_segment] = PageTableEntry::indirect(virt_to_phys(indirect as *mut _));
                indirect
            }
            PageTableEntryUnpacked::Indirect { phys_ptr } => unsafe {
                &mut *phys_to_virt(phys_ptr as *mut PageTable<{ LEVEL - 1 }>)
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
        self.map_range(virt, phys, size, flags, PageTable::<1>::map_pages);
    }
}

impl PageTable<1> {
    fn map_pages(&mut self, virt: usize, phys: usize, size: usize, flags: PageFlags) {
        self.map_range(virt, phys, size, flags, PageTable::<0>::map_pages);
    }
}

impl PageTable<0> {
    fn map_pages(&mut self, virt: usize, phys: usize, size: usize, flags: PageFlags) {
        for v in (virt..virt + size).step_by(PAGE_SIZE) {
            self.map_leaf(v, phys + (v - virt), flags);
        }
    }
}

impl<const LEVEL: usize> Drop for PageTable<LEVEL> {
    fn drop(&mut self) {
        for entry in &mut self.0 {
            if let PageTableEntryUnpacked::Indirect { phys_ptr } = entry.unpack() {
                let ptr = phys_to_virt(phys_ptr);
                drop(unsafe { Box::from_raw(ptr as *mut PageTable<0>) });
                *entry = PageTableEntry::invalid();
            }
        }
    }
}

fn align_by(range: Range<usize>, align: usize) -> (Range<usize>, Range<usize>, Range<usize>) {
    let aligned_start = range.start.next_multiple_of(align);
    if aligned_start >= range.end {
        return (range.start..range.end, 0..0, range.end..range.end);
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
