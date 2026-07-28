mod entry;
mod table;

use core::ops::Range;
pub use entry::PageFlags;
pub use table::{PageTable, TopPageTable};

use crate::util::address::Address;
use deravel_types::PAGE_SIZE;
use riscv::register::satp::{Mode, Satp};

#[derive(Clone)]
#[repr(C, align(4096))]
pub struct Page(pub [u8; 4096]);

#[repr(C, align(4096))]
pub struct PageAligned<T>(pub T);

const PAGE_TABLE_ENTRY_COUNT: usize = PAGE_SIZE / size_of::<usize>();

const LEVEL_0_PAGE_SIZE: usize = PAGE_SIZE;
const LEVEL_1_PAGE_SIZE: usize = PAGE_TABLE_ENTRY_COUNT * LEVEL_0_PAGE_SIZE;
const LEVEL_2_PAGE_SIZE: usize = PAGE_TABLE_ENTRY_COUNT * LEVEL_1_PAGE_SIZE;

const DIRECT_MAPPING_START: usize = MAX_VIRTUAL_ADDR / 2;
const DIRECT_MAPPING_END: usize = MAX_VIRTUAL_ADDR;
const DIRECT_MAPPING_SIZE: usize = DIRECT_MAPPING_END - DIRECT_MAPPING_START;

const MAX_PHYSICAL_ADDR: usize = DIRECT_MAPPING_SIZE;
const MAX_VIRTUAL_ADDR: usize = LEVEL_2_PAGE_SIZE * PAGE_TABLE_ENTRY_COUNT;

unsafe extern "C" {
    static image_start: u8;
    static image_end: u8;
}

static mut KERNEL_PAGE_TABLE: TopPageTable = PageTable::new();

pub fn initialize_memory_mapping() {
    let table = unsafe { &mut *&raw mut KERNEL_PAGE_TABLE };
    map_direct_mapping(table);
    map_kernel_image(table);

    // No need for SFENCE.VMA when changing from Bare mode (RISC-V Privileged 12.2.1).
    debug_assert_eq!(riscv::register::satp::read().mode(), Mode::Bare);

    unsafe { riscv::register::satp::write(satp(table)) }
}

pub fn map_direct_mapping(table: &mut TopPageTable) {
    let virt = DIRECT_MAPPING_START;
    let size = DIRECT_MAPPING_SIZE;
    table.map_pages(virt, 0, size, PageFlags::readwrite());
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
    table.map_pages(start, start, size, flags);
}

pub fn phys_to_virt<T: Address>(phys: T) -> T {
    phys.deep_map_addr(|phys| {
        assert!(phys < MAX_PHYSICAL_ADDR);
        // TODO: What about the exact end address in ranges? Also kind of, every pointer is a range.
        if phys >= &raw const image_start as usize && phys <= &raw const image_end as usize {
            phys
        } else {
            sign_extend(phys + DIRECT_MAPPING_START)
        }
    })
}

pub fn phys_to_drmp<T: Address>(phys: T) -> T {
    phys.deep_map_addr(|phys| {
        assert!(phys < MAX_PHYSICAL_ADDR);
        sign_extend(phys + DIRECT_MAPPING_START)
    })
}

pub fn virt_to_phys<T: Address>(virt: T) -> T {
    virt.deep_map_addr(|virt| {
        let virt = sign_unextend(virt);
        match virt {
            DIRECT_MAPPING_START..DIRECT_MAPPING_END => virt - DIRECT_MAPPING_START,
            _ if virt >= &raw const image_start as usize
                && virt < &raw const image_end as usize =>
            {
                virt
            }
            _ => panic!("{virt:#x} can't be translated to a physical address"),
        }
    })
}

fn sign_extend(addr: usize) -> usize {
    (!(((addr & (MAX_VIRTUAL_ADDR >> 1)) << 1) - 1)) | addr
}

fn sign_unextend(addr: usize) -> usize {
    addr & (MAX_VIRTUAL_ADDR - 1)
}

pub fn satp(table: &TopPageTable) -> Satp {
    let mut satp = Satp::from_bits(0);
    satp.set_mode(Mode::Sv39);
    satp.set_ppn(virt_to_phys(table as *const _) as usize / PAGE_SIZE);
    satp
}
