mod entry;
mod table;

pub use entry::PageFlags;
pub use table::{PageTable, TopPageTable};

use crate::page::entry::PageTableEntry;
use deravel_types::{LEVEL_2_PAGE_SIZE, PAGE_SIZE};
use riscv::register::satp::{Mode, Satp};

#[repr(C, align(4096))]
pub struct Page(pub [u8; 4096]);

#[repr(C, align(4096))]
pub struct PageAligned<T>(pub T);

static mut INITIAL_PAGE_TABLE: TopPageTable = PageTable::new();

unsafe extern "C" {
    static text_start: u8;
    static text_end: u8;
    static rodata_start: u8;
    static rodata_end: u8;
    static readwrite_start: u8;
    static readwrite_end: u8;
}

pub fn initialize_memory_mapping() {
    let table = unsafe { &mut *&raw mut INITIAL_PAGE_TABLE };
    map_identity_mapping(table);
    map_kernel_image(table);
    let _ = table;

    // No need for SFENCE.VMA when changing from Bare mode (RISC-V Privileged 12.2.1).
    debug_assert_eq!(riscv::register::satp::read().mode(), Mode::Bare);

    unsafe { riscv::register::satp::write(satp(&raw mut INITIAL_PAGE_TABLE)) }
}

pub fn map_identity_mapping(page_table: &mut TopPageTable) {
    let pages_per_level = page_table.0.len();
    let total_pages = pages_per_level.pow(3);
    let total_identity_mapped = total_pages / 2;
    let virtual_addr = total_identity_mapped * PAGE_SIZE;
    map_pages(
        page_table,
        virtual_addr,
        0,
        PageFlags::readwrite(),
        total_identity_mapped * PAGE_SIZE,
    );
}

pub fn map_kernel_image(page_table: &mut TopPageTable) {
    map_kernel_image_section(
        page_table,
        &raw const text_start,
        &raw const text_end,
        PageFlags::executable(),
    );
    map_kernel_image_section(
        page_table,
        &raw const rodata_start,
        &raw const rodata_end,
        PageFlags::readonly(),
    );
    map_kernel_image_section(
        page_table,
        &raw const readwrite_start,
        &raw const readwrite_end,
        PageFlags::readwrite(),
    )
}

fn map_kernel_image_section(
    page_table: &mut TopPageTable,
    start: *const u8,
    end: *const u8,
    flags: PageFlags,
) {
    let start = start as usize;
    assert!(start.is_multiple_of(PAGE_SIZE));
    map_pages(
        page_table,
        start,
        start,
        flags,
        (end as usize - start).next_multiple_of(PAGE_SIZE),
    );
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
    let vl2_start = virtual_start.next_multiple_of(LEVEL_2_PAGE_SIZE);
    let vl2_end = {
        let nmo = virtual_end.next_multiple_of(LEVEL_2_PAGE_SIZE);
        let pmo = if nmo > virtual_end {
            nmo - LEVEL_2_PAGE_SIZE
        } else {
            nmo
        };
        pmo.max(vl2_start)
    };
    assert!(vl2_start.is_multiple_of(LEVEL_2_PAGE_SIZE));
    assert!(vl2_end.is_multiple_of(LEVEL_2_PAGE_SIZE));
    assert!(vl2_start / LEVEL_2_PAGE_SIZE < PAGE_SIZE / size_of::<usize>());
    assert!(vl2_end / LEVEL_2_PAGE_SIZE <= PAGE_SIZE / size_of::<usize>());
    let prefix_end = vl2_start.min(virtual_end);
    let suffix_start = vl2_end.min(virtual_end);
    for v in (virtual_start..prefix_end).step_by(PAGE_SIZE) {
        table.map_page(v, physical_start + (v - virtual_start), flags);
    }
    for v in (vl2_start..vl2_end).step_by(LEVEL_2_PAGE_SIZE) {
        table.0[v / LEVEL_2_PAGE_SIZE] =
            PageTableEntry::leaf(physical_start + (v - virtual_start), flags);
    }
    for v in (suffix_start..virtual_end).step_by(PAGE_SIZE) {
        table.map_page(v, physical_start + (v - virtual_start), flags);
    }
}

pub fn physical_to_identity_mapped<T>(physical: *mut T) -> *mut T {
    physical.map_addr(|physical| {
        let identity_mapped_bytes = LEVEL_2_PAGE_SIZE * (PAGE_SIZE / size_of::<usize>()) / 2;
        assert_eq!(identity_mapped_bytes.count_ones(), 1);
        assert!(physical < identity_mapped_bytes);
        (!(identity_mapped_bytes - 1)) | physical
    })
}

pub fn satp(table: *mut TopPageTable) -> Satp {
    let mut satp = Satp::from_bits(0);
    satp.set_mode(Mode::Sv39);
    satp.set_ppn(table as usize / PAGE_SIZE);
    satp
}
