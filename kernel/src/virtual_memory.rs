use crate::capability::Handler;
use crate::page::{Page, PageFlags, PageTable, virt_to_phys};
use crate::sync::Mutex;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::ops::Range;
use deravel_types::{PAGE_SIZE, ProcessId, SharedMemory, UntypedRingBuffer};

pub trait VirtualMemoryLoader {
    fn load_page(&self, page_index: usize) -> Box<Page>;
}

pub trait VirtualMemoryRawMapping {
    fn load_page(&self, virt_base: usize, page_index: usize, page_table: &mut PageTable);
}

pub struct VirtualMemoryMapping<T> {
    loader: T,
    size: usize,
    backed_pages: Mutex<Vec<(usize, Box<Page>)>>,
}

impl<T: VirtualMemoryLoader> VirtualMemoryMapping<T> {
    pub fn new(loader: T, size: usize) -> VirtualMemoryMapping<T> {
        assert!(size.is_multiple_of(PAGE_SIZE));
        VirtualMemoryMapping {
            backed_pages: Mutex::new(Vec::new()),
            size,
            loader,
        }
    }
}

impl<T: VirtualMemoryLoader + Sync + 'static> Handler<SharedMemory> for VirtualMemoryMapping<T> {
    fn call_method(&self, _: usize, _: &[u8], _: ProcessId) -> Vec<u8> {
        unreachable!()
    }

    fn map_stream(&self, _: usize) -> &'static UntypedRingBuffer {
        unreachable!()
    }

    fn shared_memory_map(
        &self,
        virt: usize,
        page_table: &mut PageTable,
        vmms: &mut Vec<(Range<usize>, &'static (dyn VirtualMemoryRawMapping + Sync))>,
    ) {
        let backed_pages = self.backed_pages.lock();
        for (page_index, page) in &*backed_pages {
            let virt = virt + PAGE_SIZE * page_index;
            let phys = virt_to_phys(page.as_ref() as *const _) as usize;
            page_table.map(virt, phys, PAGE_SIZE, PageFlags::readonly().user());
        }
        vmms.push((virt..virt + self.size, unsafe { &*(self as *const _) }));
    }

    fn shared_memory_size(&self) -> usize {
        self.size
    }
}

impl<T: VirtualMemoryLoader + Sync + 'static> VirtualMemoryRawMapping for VirtualMemoryMapping<T> {
    fn load_page(&self, virt_base: usize, page_index: usize, page_table: &mut PageTable) {
        let virt = virt_base + PAGE_SIZE * page_index;
        let page = self.loader.load_page(page_index);
        let phys = virt_to_phys(page.as_ref() as *const _) as usize;
        page_table.map(virt, phys, PAGE_SIZE, PageFlags::readonly().user());
        self.backed_pages.lock().push((page_index, page));
    }
}
