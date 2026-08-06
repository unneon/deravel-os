use alloc::boxed::Box;
use core::alloc::{Allocator, Layout};
use core::ptr::NonNull;

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
