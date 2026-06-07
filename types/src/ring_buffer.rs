use crate::align::CACHE_LINE_SIZE;
use crate::{CacheLineAligned, PAGE_SIZE};
use alloc::alloc::{alloc_zeroed, handle_alloc_error};
use alloc::boxed::Box;
use core::alloc::Layout;
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicUsize, Ordering};

pub struct RingBuffer<T> {
    pub read: CacheLineAligned<AtomicUsize>,
    pub written: CacheLineAligned<AtomicUsize>,
    pub data: CacheLineAligned<[UnsafeCell<T>]>,
}

#[repr(transparent)]
pub struct UntypedRingBuffer(pub RingBuffer<u8>);

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
            *element.get_mut() = T::default();
        }
        ring_buffer
    }

    pub fn new_single_page() -> Box<RingBuffer<T>> {
        RingBuffer::new((PAGE_SIZE - 2 * CACHE_LINE_SIZE) / size_of::<T>())
    }

    /// # Safety
    ///
    /// Allocation must be valid for 'static and at least page-sized.
    pub unsafe fn new_in_single_page(page_pointer: *mut [u8]) -> &'static RingBuffer<T> {
        assert_eq!(page_pointer.len(), PAGE_SIZE);
        let element_count = (PAGE_SIZE - 2 * CACHE_LINE_SIZE) / size_of::<T>();
        unsafe { &*RingBuffer::new_in(element_count, page_pointer as *mut u8) }
    }
}

impl<T: Copy> RingBuffer<T> {
    pub fn push(&self, value: T) {
        let written = self.written.0.load(Ordering::Relaxed);
        let read = self.read.0.load(Ordering::Acquire);
        assert!(written < read + self.data.0.len());
        let element_ptr = self.data.0[written % self.data.0.len()].get();
        unsafe { element_ptr.write(value) }
        self.written.0.store(written + 1, Ordering::Release);
    }

    pub fn poll(&self) -> Option<T> {
        let read = self.read.0.load(Ordering::Relaxed);
        let written = self.written.0.load(Ordering::Acquire);
        if written <= read {
            return None;
        }
        let element_ptr = self.data.0[read % self.data.0.len()].get();
        let element = unsafe { element_ptr.read() };
        self.read.0.store(read + 1, Ordering::Release);
        Some(element)
    }
}

impl<T> RingBuffer<T> {
    pub fn untype(&self) -> &UntypedRingBuffer {
        let (thin_pointer, element_count) = (self as *const RingBuffer<T>).to_raw_parts();
        let byte_count = element_count * size_of::<T>();
        unsafe { &*core::ptr::from_raw_parts(thin_pointer, byte_count) }
    }
}

impl UntypedRingBuffer {
    /// # Safety
    ///
    /// The UntypedRingBuffer reference must have been created by previously calling RingBuffer<T>::untype.
    pub unsafe fn cast<T>(&self) -> &RingBuffer<T> {
        let (thin_pointer, byte_count) = (self as *const UntypedRingBuffer).to_raw_parts();
        let element_count = byte_count / size_of::<T>();
        unsafe { &*core::ptr::from_raw_parts(thin_pointer, element_count) }
    }
}

unsafe impl<T: Send> Send for RingBuffer<T> {}

// Elements are moved out of the ring before accessing, so Send is enough.
unsafe impl<T: Send> Sync for RingBuffer<T> {}
