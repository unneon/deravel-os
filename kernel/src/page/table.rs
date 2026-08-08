use crate::page::entry::{PageTableEntry, PageTableEntryUnpacked};
use crate::page::{PageFlags, phys_to_virt, virt_to_phys};
use alloc::boxed::Box;
use core::assert_matches;
use core::ops::Range;
use deravel_types::memory::{PHYSICAL_ADDRESSES, VIRTUAL_ADDRESSES};
use deravel_types::{LEVEL_1_PAGE_SIZE, LEVEL_2_PAGE_SIZE, PAGE_SIZE, PAGE_TABLE_ENTRY_COUNT};
use log::*;

#[repr(align(4096))]
pub struct PageTable([PageTableEntry; PAGE_TABLE_ENTRY_COUNT]);

const LEVEL_PAGE_SIZES: [usize; 3] = [PAGE_SIZE, LEVEL_1_PAGE_SIZE, LEVEL_2_PAGE_SIZE];

impl PageTable {
    pub const fn new() -> PageTable {
        PageTable([PageTableEntry(0); _])
    }

    #[track_caller]
    pub fn map(&mut self, virt: usize, phys: usize, size: usize, flags: PageFlags) {
        assert!(virt.is_multiple_of(PAGE_SIZE));
        assert!(virt < VIRTUAL_ADDRESSES.end);
        assert!(virt + size <= VIRTUAL_ADDRESSES.end);
        assert!(phys.is_multiple_of(PAGE_SIZE));
        assert!(phys < PHYSICAL_ADDRESSES.end);
        assert!(phys + size <= PHYSICAL_ADDRESSES.end);
        assert!(size.is_multiple_of(PAGE_SIZE));
        self.map_impl(virt, phys, size, flags, 2);
    }

    #[track_caller]
    pub fn unmap(&mut self, virt: usize, phys: usize, size: usize) {
        assert!(virt.is_multiple_of(PAGE_SIZE));
        assert!(virt < VIRTUAL_ADDRESSES.end);
        assert!(virt + size <= VIRTUAL_ADDRESSES.end);
        assert!(phys.is_multiple_of(PAGE_SIZE));
        assert!(phys < PHYSICAL_ADDRESSES.end);
        assert!(phys + size <= PHYSICAL_ADDRESSES.end);
        assert!(size.is_multiple_of(PAGE_SIZE));
        self.unmap_impl(virt, phys, size, 2);
    }

    pub fn is_mapped(&self, virt: usize) -> bool {
        self.is_mapped_impl(virt, 2)
    }

    fn map_impl(&mut self, virt: usize, phys: usize, size: usize, flags: PageFlags, level: usize) {
        let leaf_size = LEVEL_PAGE_SIZES[level];
        let (prefix, aligned, suffix) = align_by(virt..virt + size, leaf_size);

        if !prefix.is_empty() {
            self.map_indirect(prefix.start, level).map_impl(
                prefix.start,
                phys + (prefix.start - virt),
                prefix.end - prefix.start,
                flags,
                level - 1,
            );
        }

        if !aligned.is_empty() && !(phys + (aligned.start - virt)).is_multiple_of(leaf_size) {
            // Sv39 supports 2 MiB megapages and 1 GiB gigapages, each of which must be virtually
            // and physically aligned to a boundary equal to its size. (RISC-V Privileged 12.4.1).
            warn!("physical address {phys:#x} not superpage-aligned with {virt:#x}");

            for v in aligned.step_by(leaf_size) {
                self.map_indirect(v, level).map_impl(
                    v,
                    phys + (v - virt),
                    leaf_size,
                    flags,
                    level - 1,
                );
            }
        } else {
            for v in aligned.step_by(leaf_size) {
                self.map_leaf(v, phys + (v - virt), flags, level);
            }
        }

        if !suffix.is_empty() {
            self.map_indirect(suffix.start, level).map_impl(
                suffix.start,
                phys + (suffix.start - virt),
                suffix.end - suffix.start,
                flags,
                level - 1,
            );
        }
    }

    fn unmap_impl(&mut self, virt: usize, phys: usize, size: usize, level: usize) {
        let leaf_size = LEVEL_PAGE_SIZES[level];
        let (prefix, aligned, suffix) = align_by(virt..virt + size, leaf_size);

        if !prefix.is_empty() {
            self.unwrap_indirect(prefix.start, level).unmap_impl(
                prefix.start,
                phys + (prefix.start - virt),
                prefix.end - prefix.start,
                level - 1,
            );
            self.check_unmap_indirect(prefix.start, level);
        }

        if !aligned.is_empty() && !(phys + (aligned.start - virt)).is_multiple_of(leaf_size) {
            for v in aligned.step_by(leaf_size) {
                self.unwrap_indirect(v, level).unmap_impl(
                    v,
                    phys + (v - virt),
                    leaf_size,
                    level - 1,
                );
                self.check_unmap_indirect(v, level);
            }
        } else {
            for v in aligned.step_by(leaf_size) {
                self.unmap_leaf(v, phys + (v - virt), level);
            }
        }

        if !suffix.is_empty() {
            self.unwrap_indirect(suffix.start, level).unmap_impl(
                suffix.start,
                phys + (suffix.start - virt),
                suffix.end - suffix.start,
                level - 1,
            );
            self.check_unmap_indirect(suffix.start, level);
        }
    }

    fn is_mapped_impl(&self, virt: usize, level: usize) -> bool {
        let vpn_segment = vpn_segment(virt, level);
        match self.0[vpn_segment].unpack() {
            PageTableEntryUnpacked::Invalid => false,
            PageTableEntryUnpacked::Indirect { phys_ptr } => unsafe {
                (*phys_to_virt(phys_ptr)).is_mapped_impl(virt, level - 1)
            },
            PageTableEntryUnpacked::Leaf { .. } => true,
        }
    }

    fn map_leaf(&mut self, virt: usize, phys: usize, flags: PageFlags, level: usize) {
        let vpn_segment = vpn_segment(virt, level);
        assert!(
            !self.0[vpn_segment].is_valid(),
            "leaf {virt:#x} (level {level}) already contains {:?}",
            self.0[vpn_segment].unpack()
        );
        self.0[vpn_segment] = PageTableEntry::leaf(phys, flags);
    }

    fn unmap_leaf(&mut self, virt: usize, phys: usize, level: usize) {
        let vpn_segment = vpn_segment(virt, level);
        assert_matches!(self.0[vpn_segment].unpack(), PageTableEntryUnpacked::Leaf { phys_ptr, .. } if phys_ptr as usize == phys);
        self.0[vpn_segment] = PageTableEntry::invalid();
    }

    fn map_indirect(&mut self, virt: usize, level: usize) -> &'static mut PageTable {
        let vpn_segment = vpn_segment(virt, level);
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

    fn unwrap_indirect(&mut self, virt: usize, level: usize) -> &'static mut PageTable {
        let vpn_segment = vpn_segment(virt, level);
        let PageTableEntryUnpacked::Indirect { phys_ptr } = self.0[vpn_segment].unpack() else {
            unreachable!()
        };
        unsafe { &mut *phys_to_virt(phys_ptr) }
    }

    fn check_unmap_indirect(&mut self, virt: usize, level: usize) {
        if self.unwrap_indirect(virt, level).is_completely_unmapped() {
            self.unmap_indirect(virt, level);
        }
    }

    fn unmap_indirect(&mut self, virt: usize, level: usize) {
        let vpn_segment = vpn_segment(virt, level);
        let PageTableEntryUnpacked::Indirect { phys_ptr } = self.0[vpn_segment].unpack() else {
            unreachable!()
        };
        let _ = unsafe { Box::from_raw(phys_to_virt(phys_ptr)) };
        self.0[vpn_segment] = PageTableEntry::invalid();
    }

    fn is_completely_unmapped(&self) -> bool {
        !self.0.iter().any(PageTableEntry::is_valid)
    }
}

impl Drop for PageTable {
    fn drop(&mut self) {
        for entry in &mut self.0 {
            if let PageTableEntryUnpacked::Indirect { phys_ptr } = entry.unpack() {
                let ptr = phys_to_virt(phys_ptr);
                drop(unsafe { Box::from_raw(ptr) });
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

fn vpn_segment(virt: usize, level: usize) -> usize {
    virt >> (12 + 9 * level) & ((1 << 9) - 1)
}
