use crate::align::CACHE_LINE_SIZE;
use crate::{CacheLineAligned, PAGE_SIZE, PageAligned};
use alloc::alloc::{alloc_zeroed, handle_alloc_error};
use alloc::boxed::Box;
use alloc::sync::Arc;
use core::alloc::Layout;
use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicUsize, Ordering};

#[repr(C, align(4096))]
pub struct RingBuffer<T, D: ?Sized = [UnsafeCell<MaybeUninit<T>>]> {
    _phantom: PhantomData<[T]>,
    read: CacheLineAligned<AtomicUsize>,
    written: CacheLineAligned<AtomicUsize>,
    data: CacheLineAligned<D>,
}

// TODO: This is pretty broken with multiple readers.
impl<T: Copy + Default> RingBuffer<T> {
    pub fn new(element_count: usize) -> Box<RingBuffer<T>> {
        assert!(element_count > 0);
        assert!(align_of::<T>() <= 2 * CACHE_LINE_SIZE);
        let total_size = 2 * CACHE_LINE_SIZE + element_count * size_of::<T>();
        let layout = Layout::from_size_align(total_size, PAGE_SIZE).unwrap();
        let thin = unsafe { alloc_zeroed(layout) };
        if thin.is_null() {
            handle_alloc_error(layout);
        }
        unsafe { Box::from_raw(RingBuffer::new_in(element_count, thin)) }
    }

    fn new_in(element_count: usize, thin: *mut u8) -> *mut RingBuffer<T> {
        assert!(element_count > 0);
        let fat = core::ptr::from_raw_parts_mut::<RingBuffer<T>>(thin, element_count);
        let ring_buffer = unsafe { &mut *fat };
        for element in &mut ring_buffer.data.0 {
            *element.get_mut() = MaybeUninit::new(T::default());
        }
        ring_buffer
    }

    pub fn new_single_page() -> Box<RingBuffer<T>> {
        RingBuffer::new((PAGE_SIZE - 2 * CACHE_LINE_SIZE) / size_of::<T>())
    }

    pub fn new_arc<const CAPACITY: usize>() -> Arc<RingBuffer<T>> {
        Arc::new(RingBuffer {
            _phantom: PhantomData,
            read: CacheLineAligned(AtomicUsize::new(0)),
            written: CacheLineAligned(AtomicUsize::new(0)),
            data: CacheLineAligned::<[UnsafeCell<MaybeUninit<T>>; CAPACITY]>(
                [const { UnsafeCell::new(MaybeUninit::<T>::uninit()) }; _],
            ),
        })
    }

    /// # Safety
    ///
    /// Allocation must be valid for 'static and at least page-sized.
    pub unsafe fn new_in_single_page(
        page_pointer: *mut PageAligned<[u8]>,
    ) -> &'static RingBuffer<T> {
        let page_pointer: *mut [u8] = PageAligned::cast_mut(page_pointer);
        assert_eq!(page_pointer.len(), PAGE_SIZE);
        let element_count = (PAGE_SIZE - 2 * CACHE_LINE_SIZE) / size_of::<T>();
        unsafe { &*RingBuffer::new_in(element_count, page_pointer as *mut u8) }
    }

    pub fn capacity(&self) -> usize {
        self.data.0.len()
    }
}

impl<T: Copy> RingBuffer<T> {
    pub fn push(&self, value: T) {
        let written = self.written.0.load(Ordering::Relaxed);
        let read = self.read.0.load(Ordering::Acquire);
        assert!(written < read + self.data.0.len());
        let element_ptr = self.data.0[written % self.data.0.len()].get();
        unsafe { element_ptr.write(MaybeUninit::new(value)) }
        self.written.0.store(written + 1, Ordering::Release);
    }

    pub fn poll(&self) -> Option<T> {
        let read = self.read.0.load(Ordering::Relaxed);
        let written = self.written.0.load(Ordering::Acquire);
        if written <= read {
            return None;
        }
        let element_ptr = self.data.0[read % self.data.0.len()].get();
        let element = unsafe { element_ptr.read().assume_init() };
        self.read.0.store(read + 1, Ordering::Release);
        Some(element)
    }
}

unsafe impl<T: Send> Send for RingBuffer<T> {}

// Elements are moved out of the ring before accessing, so Send is enough.
unsafe impl<T: Send> Sync for RingBuffer<T> {}
