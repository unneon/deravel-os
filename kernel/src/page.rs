use alloc::boxed::Box;
use deravel_types::{LEVEL_2_PAGE_SIZE, PAGE_SIZE};
use riscv::register::satp::{Mode, Satp};

#[repr(C, align(4096))]
pub struct PageAligned<T>(pub T);

#[derive(Clone, Copy)]
pub struct PageFlags(usize);

pub struct PageTable<const LEVEL: usize>(
    pub [PageTableEntry<LEVEL>; PAGE_SIZE / size_of::<usize>()],
);

#[derive(Clone, Copy, Default)]
pub struct PageTableEntry<const LEVEL: usize>(pub usize);

const PAGE_V: usize = 1 << 0;
const PAGE_R: usize = 1 << 1;
const PAGE_W: usize = 1 << 2;
const PAGE_X: usize = 1 << 3;
const PAGE_U: usize = 1 << 4;

static mut INITIAL_PAGE_TABLE: PageTable<2> = PageTable::new();

unsafe extern "C" {
    static text_start: u8;
    static text_end: u8;
    static rodata_start: u8;
    static rodata_end: u8;
    static readwrite_start: u8;
    static readwrite_end: u8;
}

impl PageFlags {
    pub fn readonly() -> PageFlags {
        PageFlags(PAGE_R)
    }

    pub fn readwrite() -> PageFlags {
        PageFlags(PAGE_R | PAGE_W)
    }

    pub fn executable() -> PageFlags {
        PageFlags(PAGE_R | PAGE_X)
    }

    pub fn user(self) -> PageFlags {
        PageFlags(self.0 | PAGE_U)
    }

    pub fn is_writable(&self) -> bool {
        self.0 & PAGE_W != 0
    }
}

impl<const LEVEL: usize> PageTable<LEVEL> {
    pub const fn new() -> PageTable<LEVEL> {
        PageTable([PageTableEntry(0); _])
    }

    unsafe fn get_or_create_indirect(
        &mut self,
        vpn_segment: usize,
    ) -> &'static mut PageTable<{ LEVEL - 1 }> {
        if !self.0[vpn_segment].is_valid() {
            let indirect = Box::leak(Box::new(PageTable::new()));
            self.0[vpn_segment] =
                PageTableEntry::indirect(indirect as *mut PageTable<{ LEVEL - 1 }>);
            indirect
        } else {
            unsafe { &mut *self.0[vpn_segment].unwrap_indirect() }
        }
    }
}

impl<const LEVEL: usize> PageTableEntry<LEVEL> {
    fn indirect(table: *mut PageTable<{ LEVEL - 1 }>) -> PageTableEntry<LEVEL> {
        PageTableEntry(((table as usize / PAGE_SIZE) << 10) | PAGE_V)
    }

    fn leaf(physical_addr: usize, flags: PageFlags) -> PageTableEntry<LEVEL> {
        PageTableEntry(((physical_addr / PAGE_SIZE) << 10) | PAGE_V | flags.0)
    }

    fn is_indirect(&self) -> bool {
        self.is_valid() && self.0 & (PAGE_R | PAGE_W | PAGE_X) == 0
    }

    fn is_valid(&self) -> bool {
        self.0 & PAGE_V != 0
    }

    fn unwrap_indirect(&mut self) -> *mut PageTable<{ LEVEL - 1 }> {
        assert!(self.is_indirect());
        ((self.0 >> 10) * PAGE_SIZE) as *mut PageTable<{ LEVEL - 1 }>
    }
}

impl<const LEVEL: usize> Default for PageTable<LEVEL> {
    fn default() -> PageTable<LEVEL> {
        PageTable([PageTableEntry::default(); _])
    }
}

pub fn initialize_memory_mapping() {
    #[allow(clippy::deref_addrof)]
    let table = unsafe { &mut *&raw mut INITIAL_PAGE_TABLE };
    map_kernel_identity_mapping(table);
    map_kernel_memory(table);
    let _ = table;
    unsafe { riscv::register::satp::write(satp(&raw mut INITIAL_PAGE_TABLE)) }
}

pub fn map_kernel_identity_mapping(page_table: &mut PageTable<2>) {
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

pub fn map_kernel_memory(page_table: &mut PageTable<2>) {
    map_kernel_memory_section(
        page_table,
        &raw const text_start,
        &raw const text_end,
        PageFlags::executable(),
    );
    map_kernel_memory_section(
        page_table,
        &raw const rodata_start,
        &raw const rodata_end,
        PageFlags::readonly(),
    );
    map_kernel_memory_section(
        page_table,
        &raw const readwrite_start,
        &raw const readwrite_end,
        PageFlags::readwrite(),
    )
}

fn map_kernel_memory_section(
    page_table: &mut PageTable<2>,
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
    table: &mut PageTable<2>,
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
        map_page(table, v, physical_start + (v - virtual_start), flags);
    }
    for v in (vl2_start..vl2_end).step_by(LEVEL_2_PAGE_SIZE) {
        table.0[v / LEVEL_2_PAGE_SIZE] =
            PageTableEntry::leaf(physical_start + (v - virtual_start), flags);
    }
    for v in (suffix_start..virtual_end).step_by(PAGE_SIZE) {
        map_page(table, v, physical_start + (v - virtual_start), flags);
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

pub fn satp(table: *mut PageTable<2>) -> Satp {
    let mut satp = Satp::from_bits(0);
    satp.set_mode(Mode::Sv39);
    satp.set_ppn(table as usize / PAGE_SIZE);
    satp
}

fn map_page(
    table2: &mut PageTable<2>,
    virtual_addr: usize,
    physical_addr: usize,
    flags: PageFlags,
) {
    assert!(virtual_addr.is_multiple_of(PAGE_SIZE));
    assert!(physical_addr.is_multiple_of(PAGE_SIZE));

    let vpn2 = (virtual_addr >> 30) & ((1 << 9) - 1);
    let table1 = unsafe { table2.get_or_create_indirect(vpn2) };
    let vpn1 = (virtual_addr >> 21) & ((1 << 9) - 1);
    let table0 = unsafe { table1.get_or_create_indirect(vpn1) };

    let vpn0 = (virtual_addr >> 12) & ((1 << 9) - 1);
    assert!(!table0.0[vpn0].is_valid());
    table0.0[vpn0] = PageTableEntry::leaf(physical_addr, flags);
}
