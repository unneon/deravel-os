use core::alloc::{AllocError, Allocator, Layout};
use core::ptr::NonNull;

pub macro singleton_allocator(struct $name:ident for $instance:path;) {
    #[derive(Clone, Copy)]
    struct $name;

    unsafe impl Allocator for $name {
        fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
            $instance.allocate(layout)
        }

        unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
            unsafe { $instance.deallocate(ptr, layout) }
        }
    }
}
