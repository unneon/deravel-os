use crate::page::entry::{PageTableEntry, PageTableEntryUnpacked};
use crate::page::{PageFlags, phys_to_virt, virt_to_phys};
use alloc::boxed::Box;
use core::assert_matches;
use core::ops::Range;
use deravel_types::memory::{PHYSICAL_ADDRESSES, VIRTUAL_ADDRESSES};
use deravel_types::{PAGE_SIZE, PAGE_TABLE_ENTRY_COUNT};
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

    fn unmap_range(
        &mut self,
        virt: usize,
        phys: usize,
        size: usize,
        recurse: impl Fn(&'static mut PageTable<{ LEVEL - 1 }>, usize, usize, usize) + 'static,
    ) {
        let (prefix, aligned, suffix) = align_by(virt..virt + size, Self::LEVEL_PAGE_SIZE);

        if !prefix.is_empty() {
            recurse(
                self.unwrap_indirect(prefix.start),
                prefix.start,
                phys + (prefix.start - virt),
                prefix.end - prefix.start,
            );
            self.check_unmap_indirect(prefix.start);
        }

        if !aligned.is_empty()
            && !(phys + (aligned.start - virt)).is_multiple_of(Self::LEVEL_PAGE_SIZE)
        {
            // Sv39 supports 2 MiB megapages and 1 GiB gigapages, each of which must be virtually
            // and physically aligned to a boundary equal to its size. (RISC-V Privileged 12.4.1).
            warn!("physical address {phys:#x} not superpage-aligned with {virt:#x}");

            for v in aligned.step_by(Self::LEVEL_PAGE_SIZE) {
                recurse(
                    self.unwrap_indirect(v),
                    v,
                    phys + (v - virt),
                    Self::LEVEL_PAGE_SIZE,
                );
                self.check_unmap_indirect(v);
            }
        } else {
            for v in aligned.step_by(Self::LEVEL_PAGE_SIZE) {
                self.unmap_leaf(v, phys + (v - virt));
            }
        }

        if !suffix.is_empty() {
            recurse(
                self.unwrap_indirect(suffix.start),
                suffix.start,
                phys + (suffix.start - virt),
                suffix.end - suffix.start,
            );
            self.check_unmap_indirect(suffix.start);
        }
    }

    fn is_mapped_impl(
        &self,
        virt: usize,
        recurse: impl Fn(&PageTable<{ LEVEL - 1 }>, usize) -> bool,
    ) -> bool {
        let vpn_segment = vpn_segment::<LEVEL>(virt);
        match self.0[vpn_segment].unpack() {
            PageTableEntryUnpacked::Invalid => false,
            PageTableEntryUnpacked::Indirect { phys_ptr } => recurse(
                unsafe { &mut *phys_to_virt(phys_ptr as *mut PageTable<{ LEVEL - 1 }>) },
                virt,
            ),
            PageTableEntryUnpacked::Leaf { .. } => true,
        }
    }

    fn map_leaf(&mut self, virt: usize, phys: usize, flags: PageFlags) {
        let vpn_segment = vpn_segment::<LEVEL>(virt);
        assert!(
            !self.0[vpn_segment].is_valid(),
            "leaf {virt:#x} (level {LEVEL}) already contains {:?}",
            self.0[vpn_segment].unpack()
        );
        self.0[vpn_segment] = PageTableEntry::leaf(phys, flags);
    }

    fn unmap_leaf(&mut self, virt: usize, phys: usize) {
        let vpn_segment = vpn_segment::<LEVEL>(virt);
        assert_matches!(self.0[vpn_segment].unpack(), PageTableEntryUnpacked::Leaf { phys_ptr, .. } if phys_ptr as usize == phys);
        self.0[vpn_segment] = PageTableEntry::invalid();
    }

    fn is_mapped_leaf(&self, virt: usize) -> bool {
        let vpn_segment = vpn_segment::<LEVEL>(virt);
        match self.0[vpn_segment].unpack() {
            PageTableEntryUnpacked::Invalid => false,
            PageTableEntryUnpacked::Indirect { .. } => unreachable!(),
            PageTableEntryUnpacked::Leaf { .. } => true,
        }
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

    fn unwrap_indirect(&mut self, virt: usize) -> &'static mut PageTable<{ LEVEL - 1 }> {
        let vpn_segment = vpn_segment::<LEVEL>(virt);
        let PageTableEntryUnpacked::Indirect { phys_ptr } = self.0[vpn_segment].unpack() else {
            unreachable!()
        };
        unsafe { &mut *phys_to_virt(phys_ptr as *mut PageTable<{ LEVEL - 1 }>) }
    }

    fn check_unmap_indirect(&mut self, virt: usize)
    where
        PageTable<{ LEVEL - 1 }>:,
    {
        if self.unwrap_indirect(virt).is_completely_unmapped() {
            self.unmap_indirect(virt);
        }
    }

    fn unmap_indirect(&mut self, virt: usize) {
        let vpn_segment = vpn_segment::<LEVEL>(virt);
        let PageTableEntryUnpacked::Indirect { phys_ptr } = self.0[vpn_segment].unpack() else {
            unreachable!()
        };
        let _ = unsafe { Box::from_raw(phys_to_virt(phys_ptr as *mut PageTable<0>)) };
        self.0[vpn_segment] = PageTableEntry::invalid();
    }

    fn is_completely_unmapped(&self) -> bool {
        !self.0.iter().any(PageTableEntry::is_valid)
    }
}

impl TopPageTable {
    #[track_caller]
    pub fn map_pages(&mut self, virt: usize, phys: usize, size: usize, flags: PageFlags) {
        assert!(virt.is_multiple_of(PAGE_SIZE));
        assert!(virt < VIRTUAL_ADDRESSES.end);
        assert!(virt + size <= VIRTUAL_ADDRESSES.end);
        assert!(phys.is_multiple_of(PAGE_SIZE));
        assert!(phys < PHYSICAL_ADDRESSES.end);
        assert!(phys + size <= PHYSICAL_ADDRESSES.end);
        assert!(size.is_multiple_of(PAGE_SIZE));
        self.map_range(virt, phys, size, flags, PageTable::<1>::map_pages);
    }

    #[track_caller]
    pub fn unmap_pages(&mut self, virt: usize, phys: usize, size: usize) {
        assert!(virt.is_multiple_of(PAGE_SIZE));
        assert!(virt < VIRTUAL_ADDRESSES.end);
        assert!(virt + size <= VIRTUAL_ADDRESSES.end);
        assert!(phys.is_multiple_of(PAGE_SIZE));
        assert!(phys < PHYSICAL_ADDRESSES.end);
        assert!(phys + size <= PHYSICAL_ADDRESSES.end);
        assert!(size.is_multiple_of(PAGE_SIZE));
        self.unmap_range(virt, phys, size, PageTable::<1>::unmap_pages);
    }

    pub fn is_mapped(&self, virt: usize) -> bool {
        self.is_mapped_impl(virt, PageTable::<1>::is_mapped)
    }
}

impl PageTable<1> {
    fn map_pages(&mut self, virt: usize, phys: usize, size: usize, flags: PageFlags) {
        self.map_range(virt, phys, size, flags, PageTable::<0>::map_pages);
    }

    fn unmap_pages(&mut self, virt: usize, phys: usize, size: usize) {
        self.unmap_range(virt, phys, size, PageTable::<0>::unmap_pages);
    }

    pub fn is_mapped(&self, virt: usize) -> bool {
        self.is_mapped_impl(virt, PageTable::<0>::is_mapped)
    }
}

impl PageTable<0> {
    fn map_pages(&mut self, virt: usize, phys: usize, size: usize, flags: PageFlags) {
        for v in (virt..virt + size).step_by(PAGE_SIZE) {
            self.map_leaf(v, phys + (v - virt), flags);
        }
    }

    fn unmap_pages(&mut self, virt: usize, phys: usize, size: usize) {
        for v in (virt..virt + size).step_by(PAGE_SIZE) {
            self.unmap_leaf(v, phys + (v - virt));
        }
    }

    pub fn is_mapped(&self, virt: usize) -> bool {
        self.is_mapped_leaf(virt)
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
