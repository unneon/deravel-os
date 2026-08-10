mod entry;
mod table;

pub use entry::{PageFlags, PageTableEntry};
pub use table::PageTable;

use crate::util::address::Address;
use deravel_types::PAGE_SIZE;
use deravel_types::memory::{DIRECT_MAPPING, PHYSICAL_ADDRESSES, VIRTUAL_ADDRESSES};
use riscv::register::satp::{Mode, Satp};

#[derive(Clone)]
#[repr(C, align(4096))]
pub struct Page(pub [u8; 4096]);

pub fn map_hh_direct_mapping(table: &mut PageTable) {
    let virt = DIRECT_MAPPING.start;
    let size = DIRECT_MAPPING.end - DIRECT_MAPPING.start;
    table.map(virt, 0, size, PageFlags::read_write_execute());
}

// TODO: What about the exact end address in ranges? Also kind of, every pointer is a range.

pub fn phys_to_virt<T: Address>(phys: T) -> T {
    phys.deep_map_addr(|phys| {
        assert!(phys < PHYSICAL_ADDRESSES.end);
        sign_extend(phys + DIRECT_MAPPING.start)
    })
}

pub fn virt_to_phys<T: Address>(virt: T) -> T {
    virt.deep_map_addr(|virt| {
        assert!(virt >= DIRECT_MAPPING.start);
        let virt = sign_unextend(virt);
        assert!(DIRECT_MAPPING.contains(&virt));
        virt - DIRECT_MAPPING.start
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
