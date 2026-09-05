use crate::capability::Handler;
use crate::heap::granularity::PageGranular;
use crate::page::{PageFlags, PageTable, virt_to_phys};
use crate::virtual_memory::VirtualMemoryRawMapping;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::alloc::{Allocator, Layout};
use core::ops::Range;
use core::ptr::NonNull;
use deravel_types::{ProcessId, UntypedRingBuffer};

pub struct UntypedBox<A: Allocator> {
    ptr: NonNull<()>,
    layout: Layout,
    allocator: A,
}

impl<A: Allocator> UntypedBox<A> {
    pub fn new<T: ?Sized + Send + Sync>(typed: Box<T, A>) -> UntypedBox<A> {
        let layout = Layout::for_value::<T>(&typed);
        let (ptr, allocator) = Box::into_non_null_with_allocator(typed);
        let ptr = ptr.cast();
        UntypedBox {
            ptr,
            layout,
            allocator,
        }
    }

    pub fn as_untyped_ptr(&self) -> *mut () {
        self.ptr.as_ptr()
    }

    pub fn layout(&self) -> Layout {
        self.layout
    }

    pub fn byte_size(&self) -> usize {
        self.layout.size()
    }
}

impl<A: Allocator> Drop for UntypedBox<A> {
    fn drop(&mut self) {
        unsafe { self.allocator.deallocate(self.ptr.cast(), self.layout) }
    }
}

unsafe impl<A: Allocator + Send> Send for UntypedBox<A> {}

unsafe impl<A: Allocator + Sync> Sync for UntypedBox<A> {}

impl Handler<deravel_types::SharedMemory> for UntypedBox<PageGranular> {
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
        _: &mut Vec<(Range<usize>, &'static (dyn VirtualMemoryRawMapping + Sync))>,
    ) {
        let phys = virt_to_phys(self.as_untyped_ptr().addr());
        page_table.map(virt, phys, self.byte_size(), PageFlags::read_write().user());
    }

    fn shared_memory_size(&self) -> usize {
        self.byte_size()
    }
}
