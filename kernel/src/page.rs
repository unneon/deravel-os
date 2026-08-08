mod entry;
mod table;

pub use entry::PageFlags;
pub use table::PageTable;

use crate::util::address::Address;
use core::ops::Range;
use deravel_types::PAGE_SIZE;
use deravel_types::memory::{DIRECT_MAPPING, PHYSICAL_ADDRESSES, VIRTUAL_ADDRESSES};
use riscv::register::satp::{Mode, Satp};

#[derive(Clone)]
#[repr(C, align(4096))]
pub struct Page(pub [u8; 4096]);

unsafe extern "C" {
    static image_start: u8;
    static image_end: u8;
}

static mut KERNEL_PAGE_TABLE: PageTable = PageTable::new();

pub extern "C" fn initialize_early_memory_mapping() {
    let table = unsafe { &mut *&raw mut KERNEL_PAGE_TABLE };
    map_lh_direct_mapping(table);
    map_hh_direct_mapping(table);

    unsafe { riscv::register::satp::write(satp(table)) }
}

fn map_lh_direct_mapping(table: &mut PageTable) {
    let virt = 0;
    let size = DIRECT_MAPPING.end - DIRECT_MAPPING.start;
    table.map(virt, 0, size, PageFlags::read_write_execute());
}

pub fn map_hh_direct_mapping(table: &mut PageTable) {
    let virt = DIRECT_MAPPING.start;
    let size = DIRECT_MAPPING.end - DIRECT_MAPPING.start;
    table.map(virt, 0, size, PageFlags::read_write_execute());
}

pub fn initialize_late_memory_mapping() {
    let table = unsafe { &mut *&raw mut KERNEL_PAGE_TABLE };
    unmap_lh_direct_mapping(table);
    riscv::asm::sfence_vma_all();
}

fn unmap_lh_direct_mapping(table: &mut PageTable) {
    table.unmap(0, 0, DIRECT_MAPPING.end - DIRECT_MAPPING.start);
}

pub fn map_kernel_image(table: &mut PageTable) {
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
    map_kernel_image_section(table, readwrite, PageFlags::read_write());
}

fn map_kernel_image_section(table: &mut PageTable, range: Range<*const u8>, flags: PageFlags) {
    let start = range.start as usize;
    let size = (range.end as usize - start).next_multiple_of(PAGE_SIZE);
    assert!(start.is_multiple_of(PAGE_SIZE));
    table.map(start, start, size, flags);
}

pub fn phys_to_virt<T: Address>(phys: T) -> T {
    phys.deep_map_addr(|phys| {
        assert!(phys < PHYSICAL_ADDRESSES.end);
        // TODO: What about the exact end address in ranges? Also kind of, every pointer is a range.
        if phys >= &raw const image_start as usize && phys <= &raw const image_end as usize {
            phys
        } else {
            sign_extend(phys + DIRECT_MAPPING.start)
        }
    })
}

pub fn phys_to_drmp<T: Address>(phys: T) -> T {
    phys.deep_map_addr(|phys| {
        assert!(phys < PHYSICAL_ADDRESSES.end);
        sign_extend(phys + DIRECT_MAPPING.start)
    })
}

pub fn virt_to_phys<T: Address>(virt: T) -> T {
    virt.deep_map_addr(|virt| {
        let virt = sign_unextend(virt);
        if DIRECT_MAPPING.contains(&virt) {
            virt - DIRECT_MAPPING.start
        } else if (&raw const image_start..&raw const image_end).contains(&(virt as *const u8)) {
            virt
        } else {
            panic!("{virt:#x} can't be translated to a physical address")
        }
    })
}

pub const fn sign_extend(addr: usize) -> usize {
    (!(((addr & (VIRTUAL_ADDRESSES.end >> 1)) << 1) - 1)) | addr
}

fn sign_unextend(addr: usize) -> usize {
    addr & (VIRTUAL_ADDRESSES.end - 1)
}

pub fn satp(table: &PageTable) -> Satp {
    let mut satp = Satp::from_bits(0);
    satp.set_mode(Mode::Sv39);
    satp.set_ppn(virt_to_phys(table as *const _) as usize / PAGE_SIZE);
    satp
}
