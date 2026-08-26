use crate::heap::MutAllocator;
use crate::sync::{Mutex, MutexGuard};
use core::alloc::{AllocError, Allocator, GlobalAlloc, Layout};
use core::ptr::NonNull;

pub struct SyncAllocator<T>(Mutex<Option<T>>);

impl<T: MutAllocator> SyncAllocator<T> {
    pub const fn new() -> SyncAllocator<T> {
        SyncAllocator(Mutex::new(None))
    }

    pub unsafe fn set(&self, heap: T) {
        *self.0.lock() = Some(heap);
    }

    pub unsafe fn lock_inner(&self) -> MutexGuard<'_, Option<T>> {
        self.0.lock()
    }
}

unsafe impl<T: MutAllocator> Allocator for SyncAllocator<T> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let data = self.0.lock().as_mut().unwrap().alloc(layout)? as *mut _;
        Ok(NonNull::new(core::ptr::slice_from_raw_parts_mut(data, layout.size())).unwrap())
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        self.0
            .lock()
            .as_mut()
            .unwrap()
            .dealloc(ptr.as_ptr() as usize, layout)
    }
}

unsafe impl<T: MutAllocator> GlobalAlloc for SyncAllocator<T> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        match self.0.lock().as_mut().unwrap().alloc(layout) {
            Ok(addr) => addr as *mut u8,
            Err(AllocError) => core::ptr::null_mut(),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.0
            .lock()
            .as_mut()
            .unwrap()
            .dealloc(ptr as usize, layout);
    }
}
