use alloc::boxed::Box;

#[repr(C, align(4096))]
pub struct PageAligned<T>(pub T);

#[derive(Clone)]
pub struct PageFlags(usize);

pub struct PageTable([PageTableEntry; PAGE_SIZE / size_of::<PageTableEntry>()]);

#[derive(Clone, Copy, Default)]
struct PageTableEntry(usize);

pub const PAGE_SIZE: usize = 4096;

const PAGE_V: usize = 1 << 0;
const PAGE_R: usize = 1 << 1;
const PAGE_W: usize = 1 << 2;
const PAGE_X: usize = 1 << 3;
const PAGE_U: usize = 1 << 4;

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

    pub fn unsafe_readwriteexecute() -> PageFlags {
        PageFlags(PAGE_R | PAGE_W | PAGE_X)
    }

    pub fn user(self) -> PageFlags {
        PageFlags(self.0 | PAGE_U)
    }

    pub fn is_writable(&self) -> bool {
        self.0 & PAGE_W != 0
    }
}

impl PageTable {
    unsafe fn get_or_create_indirect(&mut self, vpn_segment: usize) -> &'static mut PageTable {
        if !self.0[vpn_segment].is_valid() {
            let indirect = Box::leak(Box::new(PageTable::default()));
            self.0[vpn_segment] = PageTableEntry::indirect(indirect as *mut PageTable);
            indirect
        } else {
            unsafe { &mut *self.0[vpn_segment].unwrap_indirect() }
        }
    }
}

impl PageTableEntry {
    fn indirect(table: *mut PageTable) -> PageTableEntry {
        PageTableEntry(((table as usize / PAGE_SIZE) << 10) | PAGE_V)
    }

    fn leaf(physical_addr: usize, flags: PageFlags) -> PageTableEntry {
        PageTableEntry(((physical_addr / PAGE_SIZE) << 10) | PAGE_V | flags.0)
    }

    fn is_valid(&self) -> bool {
        self.0 & PAGE_V != 0
    }

    fn unwrap_indirect(&mut self) -> *mut PageTable {
        ((self.0 >> 10) * PAGE_SIZE) as *mut PageTable
    }
}

impl Default for PageTable {
    fn default() -> PageTable {
        PageTable([PageTableEntry::default(); PAGE_SIZE / size_of::<PageTableEntry>()])
    }
}

pub fn map_pages(
    table2: &mut PageTable,
    virtual_addr: usize,
    physical_addr: usize,
    flags: PageFlags,
    count: usize,
) {
    // TODO: Optimize using huge pages.
    for i in 0..count {
        map_page(
            table2,
            virtual_addr + PAGE_SIZE * i,
            physical_addr + PAGE_SIZE * i,
            flags.clone(),
        );
    }
}

fn map_page(table2: &mut PageTable, virtual_addr: usize, physical_addr: usize, flags: PageFlags) {
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
