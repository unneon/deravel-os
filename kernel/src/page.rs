mod entry;
mod table;

use core::ops::Range;
pub use entry::PageFlags;
pub use table::{PageTable, TopPageTable};

use crate::page::entry::PageTableEntry;
use deravel_types::{LEVEL_2_PAGE_SIZE, PAGE_SIZE};
use riscv::register::satp::{Mode, Satp};

#[repr(C, align(4096))]
pub struct Page(pub [u8; 4096]);

#[repr(C, align(4096))]
pub struct PageAligned<T>(pub T);

const IDENTITY_MAPPING_START: usize = VIRTUAL_ADDRESS_SPACE_SIZE / 2;
const IDENTITY_MAPPING_END: usize = VIRTUAL_ADDRESS_SPACE_SIZE;
const IDENTITY_MAPPING_SIZE: usize = IDENTITY_MAPPING_END - IDENTITY_MAPPING_START;

const VIRTUAL_ADDRESS_SPACE_SIZE: usize = LEVEL_2_PAGE_SIZE * (PAGE_SIZE / size_of::<usize>());

static mut KERNEL_PAGE_TABLE: TopPageTable = PageTable::new();

pub fn initialize_memory_mapping() {
    let table = unsafe { &mut *&raw mut KERNEL_PAGE_TABLE };
    map_identity_mapping(table);
    map_kernel_image(table);

    // No need for SFENCE.VMA when changing from Bare mode (RISC-V Privileged 12.2.1).
    debug_assert_eq!(riscv::register::satp::read().mode(), Mode::Bare);

    unsafe { riscv::register::satp::write(satp(table)) }
}

pub fn map_identity_mapping(table: &mut TopPageTable) {
    let pages_per_level = table.0.len();
    let total_pages = pages_per_level.pow(3);
    let total_identity_mapped = total_pages / 2;
    let virtual_addr = total_identity_mapped * PAGE_SIZE;
    let size = total_identity_mapped * PAGE_SIZE;
    map_pages(table, virtual_addr, 0, PageFlags::readwrite(), size);
}

pub fn map_kernel_image(table: &mut TopPageTable) {
    unsafe extern "C" {
        static text_start: u8;
        static text_end: u8;
        static rodata_start: u8;
        static rodata_end: u8;
        static readwrite_start: u8;
        static readwrite_end: u8;
    }
    let text = &raw const text_start..&raw const text_end;
    let rodata = &raw const rodata_start..&raw const rodata_end;
    let readwrite = &raw const readwrite_start..&raw const readwrite_end;
    map_kernel_image_section(table, text, PageFlags::executable());
    map_kernel_image_section(table, rodata, PageFlags::readonly());
    map_kernel_image_section(table, readwrite, PageFlags::readwrite());
}

fn map_kernel_image_section(table: &mut TopPageTable, range: Range<*const u8>, flags: PageFlags) {
    let start = range.start as usize;
    let size = (range.end as usize - start).next_multiple_of(PAGE_SIZE);
    assert!(start.is_multiple_of(PAGE_SIZE));
    map_pages(table, start, start, flags, size);
}

pub fn map_pages(
    table: &mut TopPageTable,
    virtual_start: usize,
    physical_start: usize,
    flags: PageFlags,
    size: usize,
) {
    assert!(virtual_start.is_multiple_of(PAGE_SIZE));
    assert!(physical_start.is_multiple_of(PAGE_SIZE));
    assert!(size.is_multiple_of(PAGE_SIZE));
    let virtual_end = virtual_start + size;
    let (prefix, l2p_aligned, suffix) = align_by(virtual_start..virtual_end, LEVEL_2_PAGE_SIZE);
    for v in prefix.step_by(PAGE_SIZE) {
        table.map_page(v, physical_start + (v - virtual_start), flags);
    }
    for v in l2p_aligned.step_by(LEVEL_2_PAGE_SIZE) {
        table.0[v / LEVEL_2_PAGE_SIZE] =
            PageTableEntry::leaf(physical_start + (v - virtual_start), flags);
    }
    for v in suffix.step_by(PAGE_SIZE) {
        table.map_page(v, physical_start + (v - virtual_start), flags);
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

pub fn physical_to_identity_mapped<T>(physical: *mut T) -> *mut T {
    physical.map_addr(|physical| {
        assert!(physical < IDENTITY_MAPPING_SIZE);
        (!(VIRTUAL_ADDRESS_SPACE_SIZE - 1)) | (physical + IDENTITY_MAPPING_START)
    })
}

pub fn satp(table: *mut TopPageTable) -> Satp {
    let mut satp = Satp::from_bits(0);
    satp.set_mode(Mode::Sv39);
    satp.set_ppn(table as usize / PAGE_SIZE);
    satp
}
