#[repr(align(64))]
pub struct CacheLineAligned<T: ?Sized>(pub T);

#[repr(C, align(4096))]
pub struct PageAligned<T: ?Sized>(pub T);

pub const CACHE_LINE_SIZE: usize = 64;

pub const PAGE_SIZE: usize = 4096;
pub const PAGE_TABLE_ENTRY_COUNT: usize = PAGE_SIZE / size_of::<usize>();
pub const LEVEL_1_PAGE_SIZE: usize = PAGE_TABLE_ENTRY_COUNT * PAGE_SIZE;
pub const LEVEL_2_PAGE_SIZE: usize = PAGE_TABLE_ENTRY_COUNT * LEVEL_1_PAGE_SIZE;

impl PageAligned<[u8]> {
    pub fn cast<T>(ptr: *const PageAligned<[u8]>) -> *const [T] {
        assert!(PAGE_SIZE.is_multiple_of(size_of::<T>()));
        assert!(align_of::<T>() <= PAGE_SIZE);
        let (ptr, size) = ptr.to_raw_parts();
        core::ptr::from_raw_parts(ptr, size / size_of::<T>())
    }

    pub fn cast_mut<T>(ptr: *mut PageAligned<[u8]>) -> *mut [T] {
        assert!(PAGE_SIZE.is_multiple_of(size_of::<T>()));
        assert!(align_of::<T>() <= PAGE_SIZE);
        let (ptr, size) = ptr.to_raw_parts();
        core::ptr::from_raw_parts_mut(ptr, size / size_of::<T>())
    }
}
